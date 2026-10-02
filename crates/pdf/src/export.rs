//! Document → PDF. The item walk mirrors `designcraft-render`'s `draw_spread` / `draw_item`.

use std::collections::HashMap;
use std::sync::Arc;

use designcraft_color::swatch::{ColorType, SwatchValue};
use designcraft_color::{BlendMode as DcBlend, Color, GradientKind};
use designcraft_compose::Cache;
use designcraft_doc::{AssetId, Content, Document, Item, PageSide, Spread, StrokeAlign, StrokeType};
use designcraft_geom::kurbo::{PathEl, Shape as _};
use designcraft_geom::{Affine, BezPath, Rect, Vec2, corners};
use krilla::color::{cmyk, luma, rgb};
use krilla::configure::{Archival, ConfigurationBuilder, PdfVersion};
use krilla::geom::{Path, PathBuilder, Size, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, LinearGradient, RadialGradient, SpreadMethod, Stop, Stroke, StrokeDash};
use krilla::surface::Surface;

use crate::{ExportReport, PdfError, PdfOptions, Result, Standard};

/// Export `doc` as PDF bytes.
pub fn export_pdf(doc: &Document, cache: &Cache, opts: &PdfOptions) -> Result<Vec<u8>> {
    export_pdf_with_report(doc, cache, opts).map(|r| r.bytes)
}

/// One output page: a document page or a spread, in spread coordinates.
pub(crate) struct Sheet {
    pub spread: usize,
    pub trim: Rect,
    pub bleed: Rect,
    pub media: Rect,
    /// Page name(s) for the page-information mark.
    pub label: String,
}

pub(crate) const MARK_LEN: f64 = 18.0;

fn bleed_lr(side: PageSide, b: [f64; 4]) -> (f64, f64) {
    // Bleed is top, bottom, inside, outside; single-sided pages read inside as left.
    if side == PageSide::Left { (b[3], b[2]) } else { (b[2], b[3]) }
}

fn sheets(doc: &Document, opts: &PdfOptions) -> Result<Vec<Sheet>> {
    let count = doc.page_count();
    let pages: Vec<usize> = match &opts.pages {
        Some(v) => v.clone(),
        None => (0..count).collect(),
    };
    if pages.is_empty() {
        return Err(PdfError::NoPages);
    }
    if let Some(bad) = pages.iter().find(|p| **p >= count) {
        return Err(PdfError::BadPage(bad + 1));
    }
    let b = doc.settings.bleed.map(|v| if opts.bleed { v.max(0.0) } else { 0.0 });
    let mut out: Vec<Sheet> = Vec::new();
    let mut seen_spreads: Vec<usize> = Vec::new();
    for abs in pages {
        let (si, pi) = doc.page_loc(abs).ok_or(PdfError::BadPage(abs + 1))?;
        let sp: &Spread = &doc.spreads[si];
        let (trim, l, r, label) = if opts.spreads {
            if seen_spreads.contains(&si) {
                continue;
            }
            seen_spreads.push(si);
            let first = doc.first_page_of_spread(si);
            let (l, _) = sp.pages.first().map(|p| bleed_lr(p.side, b)).unwrap_or((b[2], b[3]));
            let (_, r) = sp.pages.last().map(|p| bleed_lr(p.side, b)).unwrap_or((b[2], b[3]));
            let names: Vec<String> = (0..sp.pages.len()).map(|i| doc.page_name(first + i)).collect();
            (sp.bounds(), l, r, names.join("–"))
        } else {
            let p = &sp.pages[pi];
            let (l, r) = bleed_lr(p.side, b);
            (p.bounds(), l, r, doc.page_name(abs))
        };
        let bleed = Rect::new(trim.x0 - l, trim.y0 - b[0], trim.x1 + r, trim.y1 + b[1]);
        let media = if opts.marks.any() {
            let m = mark_start(opts, b) + MARK_LEN + 6.0;
            trim.inflate(m, m)
        } else {
            bleed
        };
        out.push(Sheet { spread: si, trim, bleed, media, label });
    }
    Ok(out)
}

/// Distance from the trim edge where marks start: the offset, but never inside the bleed.
pub(crate) fn mark_start(opts: &PdfOptions, bleed: [f64; 4]) -> f64 {
    bleed.iter().copied().fold(opts.marks.offset.max(0.0), f64::max)
}

/// Like [`export_pdf`], also returning warnings about approximated or dropped features.
pub fn export_pdf_with_report(doc: &Document, cache: &Cache, opts: &PdfOptions) -> Result<ExportReport> {
    let sheets = sheets(doc, opts)?;
    let mut warnings = Vec::new();
    let (version, archival) = match opts.standard {
        Standard::None => (PdfVersion::Pdf17, None),
        Standard::PdfX4 => {
            warnings.push(
                "PDF/X-4: the output intent and PDF/X identification are not written yet; the file is PDF 1.6 with trim and bleed boxes".to_string(),
            );
            (PdfVersion::Pdf16, None)
        }
        Standard::PdfA2b => (PdfVersion::Pdf17, Some(Archival::A2_B)),
    };
    let mut cb = ConfigurationBuilder::new().with_version(version);
    if let Some(a) = archival {
        cb = cb.with_archival_validator(a);
    }
    let configuration = cb.finish().map_err(|e| PdfError::Write(format!("{e:?}")))?;
    let settings = krilla::SerializeSettings { compress_content_streams: opts.compress, configuration, ..Default::default() };
    let mut pdf = krilla::Document::new_with(settings);

    let title = opts.title.clone().unwrap_or_else(|| doc.title.clone());
    let mut meta = Metadata::new().creator("DesignCraft".into()).producer("DesignCraft".into());
    if !title.is_empty() {
        meta = meta.title(title.clone());
    }
    if let Some(a) = &opts.author {
        meta = meta.authors(vec![a.clone()]);
    }
    let created = opts.created.or_else(now_unix);
    if let Some(t) = created {
        let c = civil(t);
        meta = meta.creation_date(
            krilla::metadata::DateTime::new(c.0.clamp(0, 9999) as u16)
                .month(c.1)
                .day(c.2)
                .hour(c.3)
                .minute(c.4)
                .second(c.5)
                .utc_offset_hour(0)
                .utc_offset_minute(0),
        );
    }
    pdf.set_metadata(meta);

    let mut ex = Exporter {
        doc,
        cache,
        opts,
        clip: Rect::ZERO,
        warnings,
        images: HashMap::new(),
        pdfs: HashMap::new(),
        fonts: HashMap::new(),
        reverse_cmaps: HashMap::new(),
        rgb_only: archival.is_some(),
    };
    if ex.rgb_only {
        ex.warn("PDF/A: CMYK colours were converted to RGB (no CMYK output intent profile is available yet)");
    }
    if let Some(o) = crate::links::outline(doc, &sheets) {
        pdf.set_outline(o);
    }
    for (sheet_idx, sh) in sheets.iter().enumerate() {
        let size = Size::from_wh(sh.media.width().max(1.0) as f32, sh.media.height().max(1.0) as f32).ok_or(PdfError::NoPages)?;
        let local = |r: Rect| {
            krilla::geom::Rect::from_ltrb(
                (r.x0 - sh.media.x0) as f32,
                (r.y0 - sh.media.y0) as f32,
                (r.x1 - sh.media.x0) as f32,
                (r.y1 - sh.media.y0) as f32,
            )
        };
        let settings = PageSettings::new(size).with_trim_box(local(sh.trim)).with_bleed_box(local(sh.bleed));
        let mut page = pdf.start_page_with(settings);
        let mut s = page.surface();
        s.push_transform(&tf(Affine::translate(-sh.media.origin().to_vec2())));
        ex.clip = sh.bleed;
        if let Some(clip) = to_path(&sh.bleed.to_path(0.1)) {
            s.push_clip_path(&clip, &FillRule::NonZero);
            ex.spread(&mut s, sh.spread);
            s.pop();
        }
        ex.marks(&mut s, sh, &title, created);
        s.pop();
        s.finish();
        for a in crate::links::annotations(doc, cache, &sheets, sheet_idx) {
            page.add_annotation(a);
        }
        page.finish();
    }
    let bytes = pdf.finish().map_err(|e| PdfError::Write(format!("{e:?}")))?;
    let mut warnings = ex.warnings;
    warnings.dedup();
    Ok(ExportReport { bytes, pages: sheets.len(), warnings })
}

#[cfg(not(target_arch = "wasm32"))]
fn now_unix() -> Option<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}
#[cfg(target_arch = "wasm32")]
fn now_unix() -> Option<i64> {
    None
}

/// Unix seconds (UTC) → (year, month, day, hour, minute, second), proleptic Gregorian.
pub(crate) fn civil(t: i64) -> (i64, u8, u8, u8, u8, u8) {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month as u8, day as u8, (secs / 3600) as u8, (secs / 60 % 60) as u8, (secs % 60) as u8)
}

pub(crate) struct Exporter<'a> {
    pub doc: &'a Document,
    pub cache: &'a Cache,
    pub opts: &'a PdfOptions,
    /// Current sheet's visible area (spread space) for culling.
    pub clip: Rect,
    pub warnings: Vec<String>,
    images: HashMap<AssetId, Option<Image>>,
    /// Placed PDFs (embedded as vector pages).
    pdfs: HashMap<AssetId, Option<krilla::pdf::PdfDocument>>,
    pub fonts: HashMap<u32, Option<krilla::text::Font>>,
    pub reverse_cmaps: HashMap<u32, Arc<HashMap<u32, char>>>,
    /// Convert CMYK to RGB (PDF/A: krilla needs a CMYK output profile we don't ship yet).
    pub rgb_only: bool,
}

pub(crate) fn tf(a: Affine) -> Transform {
    let c = a.as_coeffs();
    Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

pub(crate) fn to_path(bp: &BezPath) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(a, p) => pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
            PathEl::CurveTo(a, b, p) => pb.cubic_to(a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32),
            PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

pub(crate) fn norm(v: f32) -> NormalizedF32 {
    NormalizedF32::new(if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 }).unwrap_or(NormalizedF32::ONE)
}

fn q(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn regular(c: &Color, rgb_only: bool) -> krilla::color::RegularColor {
    use krilla::color::RegularColor as R;
    if rgb_only && matches!(c, Color::Cmyk { .. }) {
        let [r, g, b] = c.to_rgb();
        return R::Rgb(rgb::Color::new(q(r), q(g), q(b)));
    }
    match *c {
        Color::Rgb { r, g, b } => R::Rgb(rgb::Color::new(q(r), q(g), q(b))),
        Color::Cmyk { c, m, y, k } => R::Cmyk(cmyk::Color::new(q(c), q(m), q(y), q(k))),
        // DesignCraft grey is ink coverage (0 = white); PDF DeviceGray is lightness.
        Color::Gray { k } => R::Luma(luma::Color::new(q(1.0 - k))),
    }
}

/// A process colour in its own device space (CMYK → DeviceCMYK, RGB → DeviceRGB, Gray → DeviceGray).
/// `rgb_only` converts CMYK to RGB (PDF/A without a CMYK output profile).
pub(crate) fn device(c: &Color, rgb_only: bool) -> krilla::color::Color {
    match regular(c, rgb_only) {
        krilla::color::RegularColor::Rgb(v) => v.into(),
        krilla::color::RegularColor::Cmyk(v) => v.into(),
        krilla::color::RegularColor::Luma(v) => v.into(),
    }
}

/// `[Registration]` prints on every plate: `/Separation /All`.
pub(crate) fn registration(tint: f32, rgb_only: bool) -> krilla::color::Color {
    use krilla::color::separation::{Color as SepColor, SeparationColorant, SeparationSpace};
    let alt = regular(&Color::cmyk(1.0, 1.0, 1.0, 1.0), rgb_only);
    SepColor::new(q(tint), SeparationSpace::new(SeparationColorant::AllColorants, alt)).into()
}

fn blend(b: DcBlend) -> krilla::blend::BlendMode {
    use krilla::blend::BlendMode as K;
    match b {
        DcBlend::Normal => K::Normal,
        DcBlend::Darken => K::Darken,
        DcBlend::Multiply => K::Multiply,
        DcBlend::ColorBurn => K::ColorBurn,
        DcBlend::Lighten => K::Lighten,
        DcBlend::Screen => K::Screen,
        DcBlend::ColorDodge => K::ColorDodge,
        DcBlend::Overlay => K::Overlay,
        DcBlend::SoftLight => K::SoftLight,
        DcBlend::HardLight => K::HardLight,
        DcBlend::Difference => K::Difference,
        DcBlend::Exclusion => K::Exclusion,
        DcBlend::Hue => K::Hue,
        DcBlend::Saturation => K::Saturation,
        DcBlend::Color => K::Color,
        DcBlend::Luminosity => K::Luminosity,
    }
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

pub(crate) fn solid_fill(c: krilla::color::Color, opacity: f32) -> Fill {
    Fill { paint: c.into(), opacity: norm(opacity), rule: FillRule::NonZero }
}

impl Exporter<'_> {
    pub(crate) fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    /// A swatch + tint as a PDF colour: spot swatches (and tints of them) become `/Separation`
    /// with the swatch's values as the alternate space; process colours stay in their device space.
    pub(crate) fn swatch_color(&self, name: &str, tint: f32) -> Option<krilla::color::Color> {
        use krilla::color::separation::{Color as SepColor, SeparationColorant, SeparationSpace};
        let mut n = name;
        let mut t = tint;
        for _ in 0..8 {
            let sw = self.doc.swatch(n)?;
            match &sw.value {
                SwatchValue::None => return None,
                SwatchValue::Tint { base, tint: bt } => {
                    t *= bt;
                    n = base;
                }
                SwatchValue::Registration => return Some(registration(t, self.rgb_only)),
                SwatchValue::Color { color, color_type: ColorType::Spot } => {
                    let space = SeparationSpace::new(SeparationColorant::Custom(sw.name.clone()), regular(color, self.rgb_only));
                    return Some(SepColor::new(q(t), space).into());
                }
                _ => break,
            }
        }
        self.doc.resolve_color(name, tint).map(|c| device(&c, self.rgb_only))
    }

    fn spread(&mut self, s: &mut Surface, si: usize) {
        let doc = self.doc;
        let Some(sp) = doc.spreads.get(si) else { return };
        let first = doc.first_page_of_spread(si);
        for layer in doc.layers.iter().rev() {
            if !layer.visible || !layer.printable {
                continue;
            }
            for (pi, page) in sp.pages.iter().enumerate() {
                if !page.show_parent_items {
                    continue;
                }
                let Some((ppi, ppage)) = doc.parent_page_for(first + pi) else { continue };
                let parent = &doc.parents[ppi];
                let dx = page.x - parent.pages[ppage].x;
                let page_name = doc.page_name(first + pi);
                for it in &parent.items {
                    if it.layer != layer.id || page.overridden.contains(&it.id) {
                        continue;
                    }
                    // Parent items belong to the parent page they sit on.
                    if parent.page_at_x(it.bounds().center().x) != Some(ppage) && parent.pages.len() > 1 {
                        continue;
                    }
                    self.item(s, it, Affine::translate((dx, 0.0)), Some(&page_name));
                }
            }
            for it in &sp.items {
                if it.layer == layer.id {
                    self.item(s, it, Affine::IDENTITY, None);
                }
            }
        }
    }

    /// Push blend/opacity for an item drawn as a group. Returns the number of pushes.
    fn push_group(s: &mut Surface, opacity: f32, mode: DcBlend) -> usize {
        let mut n = 0;
        if mode != DcBlend::Normal {
            s.push_blend_mode(blend(mode));
            n += 1;
        }
        if opacity < 0.999 {
            s.push_opacity(norm(opacity));
            n += 1;
        } else if mode != DcBlend::Normal {
            s.push_isolated();
            n += 1;
        }
        n
    }

    fn item(&mut self, s: &mut Surface, it: &Item, parent: Affine, page_name: Option<&str>) {
        if it.hidden || it.nonprinting {
            return;
        }
        let ds = &it.effects.drop_shadow;
        let grow = it.stroke.extent() + 2.0 + if ds.on { ds.distance.abs() + ds.size.abs() } else { 0.0 };
        let vb = parent.transform_rect_bbox(it.bounds()).inflate(grow, grow);
        if !rects_overlap(vb, self.clip) {
            return;
        }
        let xf = parent * it.xf;
        if it.has_nested_items() {
            // A frame with items pasted into it: fill, the items clipped to the frame, stroke.
            let bp = if it.corners.is_none() { it.path.to_bezpath() } else { corners::apply(&it.path, &it.corners) };
            let path = to_path(&bp);
            let pushes = Self::push_group(s, it.opacity, it.blend);
            s.push_transform(&tf(xf));
            if !it.fill.is_none()
                && let Some(p) = &path
                && let Some(paint) = self.fill_paint(&it.fill.swatch, it.fill.tint, bp.bounding_box(), it.fill.gradient_angle)
            {
                s.set_stroke(None);
                s.set_fill(Some(Fill { paint, opacity: NormalizedF32::ONE, rule: FillRule::NonZero }));
                s.draw_path(p);
                s.set_fill(None);
            }
            s.pop();
            // The clip in page space, so the children keep their absolute transforms.
            let mut page_bp = bp.clone();
            page_bp.apply_affine(xf);
            let clip = to_path(&page_bp);
            if let Some(c) = &clip {
                s.push_clip_path(c, &FillRule::NonZero);
            }
            for c in it.children() {
                self.item(s, c, xf, page_name);
            }
            if clip.is_some() {
                s.pop();
            }
            if !it.stroke.is_none()
                && let Some(p) = &path
            {
                s.push_transform(&tf(xf));
                self.stroke(s, it, &bp, p);
                s.pop();
            }
            for _ in 0..pushes {
                s.pop();
            }
            return;
        }
        if !it.children().is_empty() {
            let pushes = Self::push_group(s, it.opacity, it.blend);
            for c in it.children() {
                self.item(s, c, xf, page_name);
            }
            for _ in 0..pushes {
                s.pop();
            }
            return;
        }
        let doc = self.doc;
        let bp = if it.corners.is_none() { it.path.to_bezpath() } else { corners::apply(&it.path, &it.corners) };
        let path = to_path(&bp);
        let pushes = Self::push_group(s, it.opacity, it.blend);
        // Drop shadow (simple offset silhouette, like the renderer).
        if ds.on
            && let Some(p) = &path
            && let Some(c) = self.swatch_color(&ds.color, 1.0)
        {
            let a = ds.angle.to_radians();
            let off = Vec2::new(-a.cos() * ds.distance, a.sin() * ds.distance);
            s.push_transform(&tf(Affine::translate(off) * xf));
            s.set_stroke(None);
            s.set_fill(Some(solid_fill(c, ds.opacity)));
            s.draw_path(p);
            s.set_fill(None);
            s.pop();
        }
        s.push_transform(&tf(xf));
        // Fill.
        if !it.fill.is_none()
            && it.path.is_closed()
            && let Some(p) = &path
            && let Some(paint) = self.fill_paint(&it.fill.swatch, it.fill.tint, bp.bounding_box(), it.fill.gradient_angle)
        {
            if it.fill.overprint {
                self.warn("overprint is not exported yet");
            }
            s.set_stroke(None);
            s.set_fill(Some(Fill { paint, opacity: NormalizedF32::ONE, rule: FillRule::NonZero }));
            s.draw_path(p);
            s.set_fill(None);
        }
        // Content.
        match &it.content {
            Content::Graphic(g) => {
                if let Some(p) = &path {
                    s.push_clip_path(p, &FillRule::NonZero);
                    self.graphic(s, g);
                    s.pop();
                }
            }
            Content::Text(tfr) => {
                let cs = self.cache.get(doc, tfr.story, page_name);
                if let Some(ft) = cs.frame(it.id) {
                    let text = doc.story(tfr.story).map(|st| st.text.as_str()).unwrap_or("");
                    self.frame_text(s, &cs, ft, text);
                }
            }
            _ => {}
        }
        // Stroke.
        if !it.stroke.is_none()
            && let Some(p) = &path
        {
            self.stroke(s, it, &bp, p);
        }
        s.pop();
        for _ in 0..pushes {
            s.pop();
        }
        // Anchored objects in the frame's text.
        if let Content::Text(tfr) = &it.content
            && let Some(st) = doc.story(tfr.story).filter(|st| !st.objects.is_empty())
        {
            let cs = self.cache.get(doc, tfr.story, page_name);
            if let Some(ft) = cs.frame(it.id) {
                for o in &ft.objects {
                    if let Some(obj) = st.objects.get(o.index) {
                        self.item(s, &obj.item, xf * Affine::translate(o.origin.to_vec2()), page_name);
                    }
                }
            }
        }
    }

    /// A swatch fill (solid or gradient) in the item's inner space. `None` for [None]/unknown.
    fn fill_paint(&self, swatch: &str, tint: f32, bounds: Rect, angle: Option<f64>) -> Option<krilla::paint::Paint> {
        if let Some(g) = designcraft_color::swatch::resolve_gradient(&self.doc.swatches, swatch) {
            let mut stops: Vec<Stop> = Vec::new();
            let mut last = 0.0f32;
            for (o, c, a) in g.expanded_stops() {
                let o = o.clamp(last, 1.0);
                last = o;
                stops.push(Stop { offset: norm(o), color: device(&c, self.rgb_only), opacity: norm(a) });
            }
            if stops.is_empty() {
                return None;
            }
            let c = bounds.center();
            return Some(match g.kind {
                GradientKind::Radial => {
                    let r = (bounds.width().max(bounds.height()) / 2.0).max(1e-3) as f32;
                    let (cx, cy) = (c.x as f32, c.y as f32);
                    RadialGradient {
                        fx: cx,
                        fy: cy,
                        fr: 0.0,
                        cx,
                        cy,
                        cr: r,
                        transform: Transform::identity(),
                        spread_method: SpreadMethod::Pad,
                        stops,
                        anti_alias: false,
                    }
                    .into()
                }
                _ => {
                    let a = angle.unwrap_or(0.0).to_radians();
                    let half = ((bounds.width() * a.cos().abs() + bounds.height() * a.sin().abs()) / 2.0).max(1e-3);
                    let d = Vec2::new(a.cos(), -a.sin()) * half;
                    let (p0, p1) = (c - d, c + d);
                    LinearGradient {
                        x1: p0.x as f32,
                        y1: p0.y as f32,
                        x2: p1.x as f32,
                        y2: p1.y as f32,
                        transform: Transform::identity(),
                        spread_method: SpreadMethod::Pad,
                        stops,
                        anti_alias: false,
                    }
                    .into()
                }
            });
        }
        self.swatch_color(swatch, tint).map(Into::into)
    }

    fn stroke(&mut self, s: &mut Surface, it: &Item, bp: &BezPath, path: &Path) {
        let st = &it.stroke;
        let Some(c) = self.swatch_color(&st.swatch, st.tint) else { return };
        let closed = it.path.is_closed();
        let mut cap = match st.cap {
            designcraft_doc::Cap::Butt => krilla::paint::LineCap::Butt,
            designcraft_doc::Cap::Round => krilla::paint::LineCap::Round,
            designcraft_doc::Cap::Projecting => krilla::paint::LineCap::Square,
        };
        let dash = match &st.kind {
            StrokeType::Dashed { pattern } if pattern.iter().any(|v| *v > 0.0) => {
                let mut pat: Vec<f32> = pattern.iter().map(|v| v.max(0.0) as f32).collect();
                if pat.len() % 2 == 1 {
                    pat.extend(pat.clone());
                }
                Some(StrokeDash { array: pat, offset: 0.0 })
            }
            StrokeType::Dotted => {
                cap = krilla::paint::LineCap::Round;
                Some(StrokeDash { array: vec![0.0, (st.weight * 2.0) as f32], offset: 0.0 })
            }
            StrokeType::Solid | StrokeType::Dashed { .. } => None,
            _ => {
                self.warn("striped stroke types are exported as solid strokes");
                None
            }
        };
        let arrows = designcraft_doc::arrow::apply(bp, st, closed);
        let trimmed = arrows.as_ref().and_then(|a| to_path(&a.0));
        let path = trimmed.as_ref().unwrap_or(path);
        let aligned = closed && st.align != StrokeAlign::Center;
        let width = if aligned { st.weight * 2.0 } else { st.weight };
        let mut pushes = 0;
        match st.align {
            StrokeAlign::Inside if closed => {
                s.push_clip_path(path, &FillRule::NonZero);
                pushes += 1;
            }
            StrokeAlign::Outside if closed => {
                // Clip to everything outside the path: a big frame plus the path, even-odd.
                let b = bp.bounding_box().inflate(width * 2.0 + 10.0, width * 2.0 + 10.0);
                let mut outside = b.to_path(0.1);
                outside.extend(bp.iter());
                if let Some(p) = to_path(&outside) {
                    s.push_clip_path(&p, &FillRule::EvenOdd);
                    pushes += 1;
                }
            }
            _ => {}
        }
        s.set_fill(None);
        if dash.is_some()
            && pushes == 0
            && let Some(g) = self.swatch_color(&st.gap_swatch, st.gap_tint)
        {
            // Gap colour under dashes and dots.
            s.set_stroke(Some(Stroke { paint: g.into(), width: width as f32, opacity: NormalizedF32::ONE, ..Default::default() }));
            s.draw_path(path);
        }
        s.set_stroke(Some(Stroke {
            paint: c.clone().into(),
            width: width as f32,
            miter_limit: st.miter_limit.max(1.0) as f32,
            line_cap: cap,
            line_join: match st.join {
                designcraft_doc::Join::Miter => krilla::paint::LineJoin::Miter,
                designcraft_doc::Join::Round => krilla::paint::LineJoin::Round,
                designcraft_doc::Join::Bevel => krilla::paint::LineJoin::Bevel,
            },
            opacity: NormalizedF32::ONE,
            dash,
        }));
        s.draw_path(path);
        s.set_stroke(None);
        for _ in 0..pushes {
            s.pop();
        }
        for h in arrows.iter().flat_map(|a| &a.1) {
            let Some(p) = to_path(&h.path) else { continue };
            match h.outline {
                Some(w) => {
                    s.set_stroke(Some(Stroke { paint: c.clone().into(), width: w as f32, opacity: NormalizedF32::ONE, ..Default::default() }));
                    s.draw_path(&p);
                    s.set_stroke(None);
                }
                None => {
                    s.set_fill(Some(krilla::paint::Fill { paint: c.clone().into(), opacity: NormalizedF32::ONE, rule: FillRule::NonZero }));
                    s.draw_path(&p);
                    s.set_fill(None);
                }
            }
        }
    }

    fn load_image(&mut self, id: AssetId) -> Option<Image> {
        if let Some(i) = self.images.get(&id) {
            return i.clone();
        }
        let asset = self.doc.assets.get(&id)?.clone();
        let data = asset.data.clone();
        let fmt = image::guess_format(&data).ok();
        let img = match fmt {
            Some(image::ImageFormat::Jpeg) => Image::from_jpeg(data.clone().into(), true).ok(),
            _ if self.opts.compress_images => recompress(&data).or_else(|| lossless(&data, fmt)),
            _ => lossless(&data, fmt),
        };
        if img.is_none() {
            self.warn(format!("image `{}` could not be decoded and was skipped", asset.name));
        }
        self.images.insert(id, img.clone());
        img
    }

    fn graphic(&mut self, s: &mut Surface, g: &designcraft_doc::Graphic) {
        // Placed PDFs go in as vectors (the page as a form XObject).
        if let Some(asset) = self.doc.assets.get(&g.asset)
            && asset.data.starts_with(b"%PDF")
        {
            let Some(size) = Size::from_wh(g.size.0.max(1e-3) as f32, g.size.1.max(1e-3) as f32) else { return };
            let doc = self
                .pdfs
                .entry(g.asset)
                .or_insert_with(|| krilla::pdf::Pdf::new(asset.data.clone()).ok().map(|p| krilla::pdf::PdfDocument::new(Arc::new(p))))
                .clone();
            match doc {
                Some(doc) => {
                    s.push_transform(&tf(g.xf));
                    s.draw_pdf_page(&doc, size, 0);
                    s.pop();
                }
                None => self.warn(format!("{}: can't read the placed PDF", asset.name)),
            }
            return;
        }
        let Some(img) = self.load_image(g.asset) else { return };
        let Some(size) = Size::from_wh(g.size.0.max(1e-3) as f32, g.size.1.max(1e-3) as f32) else { return };
        s.push_transform(&tf(g.xf));
        s.draw_image(img, size);
        s.pop();
    }
}

fn lossless(data: &Arc<Vec<u8>>, fmt: Option<image::ImageFormat>) -> Option<Image> {
    let direct = match fmt {
        Some(image::ImageFormat::Png) => Image::from_png(data.clone().into(), true).ok(),
        Some(image::ImageFormat::Gif) => Image::from_gif(data.clone().into(), true).ok(),
        Some(image::ImageFormat::WebP) => Image::from_webp(data.clone().into(), true).ok(),
        _ => None,
    };
    direct.or_else(|| {
        let rgba = image::load_from_memory(data).ok()?.to_rgba8();
        let (w, h) = rgba.dimensions();
        Some(Image::from_rgba8(rgba.into_raw(), w, h))
    })
}

/// Opaque raster → JPEG (quality 90). `None` when the image has transparency or can't be decoded.
fn recompress(data: &[u8]) -> Option<Image> {
    let img = image::load_from_memory(data).ok()?;
    if img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p[3] < 255) {
        return None;
    }
    let rgb = img.to_rgb8();
    let mut buf = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
    image::ImageEncoder::write_image(enc, rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8).ok()?;
    Image::from_jpeg(buf.into(), true).ok()
}
