//! DesignCraft renderer: spreads → premultiplied RGBA pixels with `vello_cpu`.
//!
//! The renderer draws *document content only* (paper, page items, parent items, composed text,
//! placed images). Non-printing chrome — guides, frame edges, ports, selection — is drawn by the
//! UI as vector overlays so it stays crisp at any zoom.
//!
//! Callers pass a list of spreads with their placement on a shared canvas and a canvas → pixel
//! view transform, so the UI can render exactly its viewport with every visible spread.
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use designcraft_color::BlendMode as DcBlend;
use designcraft_compose::{Cache, ComposedStory, FrameText};
use designcraft_doc::{AssetId, Content, Document, Item, PageSide, SpreadRef, StrokeAlign, StrokeType};
use designcraft_fonts::FontDb;
use designcraft_geom::corners;
use designcraft_geom::{Affine, BezPath, Rect, Shape, Vec2};
use vello_cpu::kurbo;
use vello_cpu::peniko::{self, BlendMode, Compose, Mix};
use vello_cpu::{Pixmap, RenderContext, Resources};

pub use vello_cpu;

/// A rendered image (premultiplied RGBA8, row-major).
#[derive(Clone)]
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rendered {
    pub fn to_straight(&self) -> Vec<u8> {
        let mut out = self.pixels.clone();
        for px in out.chunks_exact_mut(4) {
            let a = px[3] as u32;
            if a != 0 && a != 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        out
    }
    pub fn to_png(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let img = image::RgbaImage::from_raw(self.width, self.height, self.to_straight()).expect("size");
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).expect("png encode");
        buf
    }
    pub fn to_jpeg(&self, quality: u8) -> Vec<u8> {
        let rgba = self.to_straight();
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|p| {
                let a = p[3] as u32;
                let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                [mix(p[0]), mix(p[1]), mix(p[2])]
            })
            .collect();
        let mut buf = Vec::new();
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100));
        let _ = image::ImageEncoder::write_image(enc, &rgb, self.width, self.height, image::ExtendedColorType::Rgb8);
        buf
    }
    /// Straight-alpha RGBA at (x, y).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        let p = &self.pixels[i..i + 4];
        let a = p[3] as u32;
        if a == 0 {
            return [0, 0, 0, 0];
        }
        let un = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
        [un(p[0]), un(p[1]), un(p[2]), p[3]]
    }
}

/// What to draw.
#[derive(Clone, Debug)]
pub struct RenderOptions {
    /// Canvas background (premultiplied RGBA8); `None` = transparent.
    pub background: Option<[u8; 4]>,
    /// Fill pages with paper colour.
    pub paper: bool,
    /// Skip non-printing items and layers (Preview mode, export).
    pub printing_only: bool,
    /// Items not to draw (live drag previews).
    pub hidden: Vec<designcraft_doc::ItemId>,
    /// Draw text smaller than this many pixels as grey bars ("greeking"); 0 = never.
    pub greek_below_px: f64,
    /// Drop shadow on pages (screen view).
    pub page_shadow: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { background: None, paper: true, printing_only: false, hidden: vec![], greek_below_px: 0.0, page_shadow: false }
    }
}

/// A spread placed on the canvas: canvas = spread + offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub spread: SpreadRef,
    pub offset: Vec2,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    pub items: usize,
    pub glyphs: usize,
    pub micros: u64,
}

/// Reusable renderer (keeps the render context and decoded images).
pub struct Renderer {
    ctx: Option<RenderContext>,
    resources: Resources,
    images: HashMap<(AssetId, usize), Arc<Pixmap>>,
    pub threads: u16,
    pub stats: FrameStats,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

struct Frame<'a> {
    doc: &'a Document,
    cache: &'a Cache,
    view: Affine,
    visible: Rect,
    px: f64,
    opts: &'a RenderOptions,
}

pub fn default_threads() -> u16 {
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(n) = std::env::var("DESIGNCRAFT_RENDER_THREADS").ok().and_then(|v| v.parse().ok()) {
            return n;
        }
        std::thread::available_parallelism().map(|n| (n.get().saturating_sub(1)).clamp(0, 8) as u16).unwrap_or(0)
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self { ctx: None, resources: Resources::new(), images: HashMap::new(), threads: default_threads(), stats: FrameStats::default() }
    }

    /// Render the given spreads into a `width`×`height` image with `view` (canvas → pixels).
    pub fn render(
        &mut self,
        doc: &Document,
        cache: &Cache,
        spreads: &[Placed],
        width: u32,
        height: u32,
        view: Affine,
        opts: &RenderOptions,
    ) -> Rendered {
        let start = now();
        let w = width.clamp(1, u16::MAX as u32) as u16;
        let h = height.clamp(1, u16::MAX as u32) as u16;
        let mut ctx = match self.ctx.take() {
            Some(mut c) if c.width() == w && c.height() == h => {
                c.reset();
                c
            }
            _ => RenderContext::new_with(w, h, vello_cpu::RenderSettings { num_threads: self.threads, ..Default::default() }),
        };
        self.stats = FrameStats::default();
        if let Some(bg) = opts.background {
            ctx.set_transform(Affine::IDENTITY);
            ctx.set_paint(peniko::Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
            ctx.fill_rect(&kurbo::Rect::new(0.0, 0.0, w as f64, h as f64));
        }
        let px = 1.0 / view.determinant().abs().sqrt().max(1e-12);
        let canvas_visible = view.inverse().transform_rect_bbox(Rect::new(0.0, 0.0, w as f64, h as f64));
        for pl in spreads {
            let Some(sp) = doc.spread(pl.spread) else { continue };
            let sview = view * Affine::translate(pl.offset);
            let visible = canvas_visible - pl.offset;
            let bounds = sp.bounds();
            // Cull spreads far outside the viewport (pasteboard items may extend; allow a margin).
            if !rect_overlaps(bounds.inflate(2000.0, 2000.0), visible) {
                continue;
            }
            let f = Frame { doc, cache, view: sview, visible, px, opts };
            self.draw_spread(&mut ctx, &f, pl.spread);
        }
        ctx.flush();
        let mut pm = Pixmap::new(w, h);
        ctx.render(&mut pm, &mut self.resources);
        self.ctx = Some(ctx);
        self.stats.micros = now().saturating_sub(start);
        Rendered { width: w as u32, height: h as u32, pixels: pm.data_as_u8_slice().to_vec() }
    }

    /// Render one document page (absolute index) at `scale` px/pt, optionally including bleed.
    pub fn render_page(&mut self, doc: &Document, cache: &Cache, abs: usize, scale: f64, bleed: bool, opts: &RenderOptions) -> Option<Rendered> {
        let (si, pi) = doc.page_loc(abs)?;
        let page = &doc.spreads[si].pages[pi];
        let mut r = page.bounds();
        if bleed {
            let b = doc.settings.bleed;
            let (l, rr) = if page.side == PageSide::Left { (b[3], b[2]) } else { (b[2], b[3]) };
            r = Rect::new(r.x0 - l, r.y0 - b[0], r.x1 + rr, r.y1 + b[1]);
        }
        let w = (r.width() * scale).round().max(1.0) as u32;
        let h = (r.height() * scale).round().max(1.0) as u32;
        let view = Affine::scale(scale) * Affine::translate(-r.origin().to_vec2());
        let mut o = opts.clone();
        o.background = Some([255, 255, 255, 255]);
        Some(self.render(doc, cache, &[Placed { spread: SpreadRef::Doc(si), offset: Vec2::ZERO }], w, h, view, &o))
    }

    fn draw_spread(&mut self, ctx: &mut RenderContext, f: &Frame, sr: SpreadRef) {
        let doc = f.doc;
        let Some(sp) = doc.spread(sr) else { return };
        // Paper.
        if f.opts.paper {
            let paper = doc.resolve_color(designcraft_color::swatch::PAPER, 1.0).unwrap_or(designcraft_color::Color::WHITE);
            if f.opts.page_shadow {
                ctx.set_transform(f.view);
                ctx.set_paint(peniko::Color::from_rgba8(0, 0, 0, 90));
                for p in &sp.pages {
                    let s = 3.0 * f.px;
                    ctx.fill_rect(&(p.bounds() + Vec2::new(s, s)));
                }
            }
            ctx.set_transform(f.view);
            ctx.set_paint(color_of(&paper, 1.0));
            // One rect for equal-height spreads avoids an anti-aliasing seam at the spine.
            if sp.pages.windows(2).all(|w| w[0].height == w[1].height) {
                ctx.fill_rect(&sp.bounds());
            } else {
                for p in &sp.pages {
                    ctx.fill_rect(&p.bounds());
                }
            }
        }
        // Layers back to front; within each layer parent items first (document spreads only).
        for layer in doc.layers.iter().rev() {
            if !layer.visible || f.opts.printing_only && !layer.printable {
                continue;
            }
            if let SpreadRef::Doc(si) = sr {
                let first = doc.first_page_of_spread(si);
                for (pi, page) in sp.pages.iter().enumerate() {
                    if !page.show_parent_items {
                        continue;
                    }
                    let Some((ppi, ppage)) = doc.parent_page_for(first + pi) else { continue };
                    let parent = &doc.parents[ppi];
                    let dx = page.x - parent.pages[ppage].x;
                    let page_name = doc.page_name(first + pi);
                    let pr = parent.pages[ppage].bounds();
                    for it in &parent.items {
                        if it.layer != layer.id || page.overridden.contains(&it.id) {
                            continue;
                        }
                        // Parent items belong to the parent page they sit on.
                        if parent.page_at_x(it.bounds().center().x) != Some(ppage) && parent.pages.len() > 1 {
                            continue;
                        }
                        let _ = pr;
                        self.draw_item(ctx, f, it, Affine::translate((dx, 0.0)), Some(&page_name));
                    }
                }
            }
            for it in &sp.items {
                if it.layer == layer.id {
                    self.draw_item(ctx, f, it, Affine::IDENTITY, None);
                }
            }
        }
    }

    fn draw_item(&mut self, ctx: &mut RenderContext, f: &Frame, it: &Item, parent: Affine, page_name: Option<&str>) {
        if it.hidden || f.opts.hidden.contains(&it.id) || f.opts.printing_only && it.nonprinting {
            return;
        }
        let xf = parent * it.xf;
        let vb = xf.transform_rect_bbox(it.inner_bounds()).inflate(it.stroke.weight + 2.0, it.stroke.weight + 2.0);
        if !it.children().is_empty() {
            // Groups: children carry their own transforms relative to the group.
            let layered = it.opacity < 0.999 || it.blend != DcBlend::Normal;
            if layered {
                ctx.set_transform(Affine::IDENTITY);
                ctx.push_layer(None, Some(blend_mode(it.blend)), Some(it.opacity), None, None);
            }
            for c in it.children() {
                self.draw_item(ctx, f, c, xf, page_name);
            }
            if layered {
                ctx.pop_layer();
            }
            return;
        }
        if !rect_overlaps(vb, f.visible) && !matches!(it.content, Content::Text(_)) {
            return;
        }
        self.stats.items += 1;
        let doc = f.doc;
        let bp = if it.corners.is_none() { it.path.to_bezpath() } else { corners::apply(&it.path, &it.corners) };
        let layered = it.opacity < 0.999 || it.blend != DcBlend::Normal;
        if layered {
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, Some(blend_mode(it.blend)), Some(it.opacity), None, None);
        }
        // Drop shadow (simple offset silhouette).
        if it.effects.drop_shadow.on {
            let ds = &it.effects.drop_shadow;
            let a = ds.angle.to_radians();
            let off = Vec2::new(-a.cos() * ds.distance, a.sin() * ds.distance);
            if let Some(c) = doc.resolve_color(&ds.color, 1.0) {
                ctx.set_transform(f.view * Affine::translate(off) * xf);
                ctx.set_paint(color_of(&c, ds.opacity));
                ctx.fill_path(&bp);
            }
        }
        // Fill.
        if !it.fill.is_none() {
            ctx.set_transform(f.view * xf);
            if set_fill_paint(ctx, doc, &it.fill.swatch, it.fill.tint, bp.bounding_box(), it.fill.gradient_angle) && it.path.is_closed() {
                ctx.fill_path(&bp);
            }
            ctx.reset_paint_transform();
        }
        // Content.
        match &it.content {
            Content::Graphic(g) => {
                ctx.set_transform(f.view * xf);
                ctx.push_clip_layer(&bp);
                self.draw_graphic(ctx, f, g, xf);
                ctx.pop_layer();
            }
            Content::Text(tf) => {
                let cs = f.cache.get(doc, tf.story, page_name);
                if let Some(ft) = cs.frame(it.id) {
                    self.draw_text(ctx, f, &cs, ft, xf);
                }
            }
            _ => {}
        }
        // Stroke.
        if !it.stroke.is_none() {
            self.draw_stroke(ctx, f, it, &bp, xf);
        }
        if layered {
            ctx.pop_layer();
        }
    }

    fn draw_stroke(&mut self, ctx: &mut RenderContext, f: &Frame, it: &Item, bp: &BezPath, xf: Affine) {
        let st = &it.stroke;
        let Some(c) = f.doc.resolve_color(&st.swatch, st.tint) else { return };
        let mut stroke = kurbo::Stroke::new(st.weight)
            .with_caps(match st.cap {
                designcraft_doc::Cap::Butt => kurbo::Cap::Butt,
                designcraft_doc::Cap::Round => kurbo::Cap::Round,
                designcraft_doc::Cap::Projecting => kurbo::Cap::Square,
            })
            .with_join(match st.join {
                designcraft_doc::Join::Miter => kurbo::Join::Miter,
                designcraft_doc::Join::Round => kurbo::Join::Round,
                designcraft_doc::Join::Bevel => kurbo::Join::Bevel,
            })
            .with_miter_limit(st.miter_limit);
        match &st.kind {
            StrokeType::Dashed { pattern } if !pattern.is_empty() => stroke = stroke.with_dashes(0.0, pattern.iter().copied()),
            StrokeType::Dotted => stroke = stroke.with_dashes(0.0, [0.0, st.weight * 2.0]).with_caps(kurbo::Cap::Round),
            _ => {}
        }
        ctx.set_transform(f.view * xf);
        ctx.set_paint(color_of(&c, 1.0));
        let closed = it.path.is_closed();
        match st.align {
            StrokeAlign::Inside if closed => {
                // Clip a double-width stroke to the path.
                ctx.push_clip_layer(bp);
                ctx.set_stroke(kurbo::Stroke { width: st.weight * 2.0, ..stroke });
                ctx.stroke_path(bp);
                ctx.pop_layer();
            }
            StrokeAlign::Outside if closed => {
                let outline =
                    kurbo::stroke(bp.iter(), &kurbo::Stroke { width: st.weight * 2.0, ..stroke }, &kurbo::StrokeOpts::default(), 0.05 * f.px);
                let mut p = outline;
                // Remove the interior: even-odd with the path itself.
                for el in bp.elements() {
                    p.push(*el);
                }
                ctx.set_fill_rule(peniko::Fill::EvenOdd);
                ctx.fill_path(&p);
                ctx.set_fill_rule(peniko::Fill::NonZero);
            }
            _ => {
                ctx.set_stroke(stroke);
                ctx.stroke_path(bp);
            }
        }
    }

    fn draw_graphic(&mut self, ctx: &mut RenderContext, f: &Frame, g: &designcraft_doc::Graphic, xf: Affine) {
        let Some(asset) = f.doc.assets.get(&g.asset) else { return };
        // Pick a mip level close to the on-screen size.
        let on_screen = (f.view * xf * g.xf).determinant().abs().sqrt();
        let full = match self.images.get(&(g.asset, 0)) {
            Some(p) => p.clone(),
            None => {
                let Some(pm) = decode_pixmap(&asset.data) else { return };
                let pm = Arc::new(pm);
                self.images.insert((g.asset, 0), pm.clone());
                pm
            }
        };
        let ppt = full.width() as f64 / g.size.0.max(1e-6); // pixels per point
        let need = on_screen * 1.0; // pixels per point on screen
        let mut level = 0usize;
        let mut pm = full.clone();
        while ppt / (1 << (level + 1)) as f64 >= need * 1.2 && pm.width() > 64 && pm.height() > 64 && level < 8 {
            level += 1;
            pm = match self.images.get(&(g.asset, level)) {
                Some(p) => p.clone(),
                None => {
                    let half = Arc::new(halve(&pm));
                    self.images.insert((g.asset, level), half.clone());
                    half
                }
            };
        }
        let rect = Rect::new(0.0, 0.0, g.size.0, g.size.1);
        ctx.set_transform(f.view * xf * g.xf);
        let sx = g.size.0 / pm.width().max(1) as f64;
        let sy = g.size.1 / pm.height().max(1) as f64;
        ctx.set_paint(vello_cpu::Image {
            image: vello_cpu::ImageSource::Pixmap(pm),
            sampler: peniko::ImageSampler::default().with_quality(peniko::ImageQuality::Medium),
        });
        ctx.set_paint_transform(Affine::scale_non_uniform(sx, sy));
        ctx.fill_rect(&rect);
        ctx.reset_paint_transform();
    }

    fn draw_text(&mut self, ctx: &mut RenderContext, f: &Frame, cs: &ComposedStory, ft: &FrameText, xf: Affine) {
        let db = FontDb::global();
        let doc = f.doc;
        // Decorations under text (shading) and rules.
        for d in &ft.decos {
            if let Some(c) = doc.resolve_color(&d.color, d.tint) {
                ctx.set_transform(f.view * xf);
                ctx.set_paint(color_of(&c, 1.0));
                ctx.fill_rect(&d.rect);
            }
        }
        let scale = (f.view * xf).determinant().abs().sqrt();
        // Batch glyph outlines per run style into one path (in frame inner space).
        let mut batches: HashMap<u32, BezPath> = HashMap::new();
        let mut lines_deco: Vec<(u32, kurbo::Rect)> = Vec::new();
        for l in &ft.lines {
            let greek = f.opts.greek_below_px > 0.0 && l.ascent * scale < f.opts.greek_below_px;
            if greek {
                if let (Some(a), Some(_)) = (l.glyphs.first(), l.glyphs.last()) {
                    let r = kurbo::Rect::new(a.x, l.baseline - l.ascent * 0.5, l.end_x, l.baseline);
                    lines_deco.push((u32::MAX, r));
                }
                continue;
            }
            for g in &l.glyphs {
                if !g.visible {
                    continue;
                }
                let style = &cs.styles[g.style as usize];
                if style.underline || style.strikethrough {
                    let w = style.size / 14.0;
                    if style.underline {
                        lines_deco
                            .push((g.style, kurbo::Rect::new(g.x, l.baseline + style.size * 0.12, g.x + g.adv, l.baseline + style.size * 0.12 + w)));
                    }
                    if style.strikethrough {
                        lines_deco
                            .push((g.style, kurbo::Rect::new(g.x, l.baseline - style.size * 0.3, g.x + g.adv, l.baseline - style.size * 0.3 + w)));
                    }
                }
                let outline = db.outline(&g.face, g.gid);
                if outline.elements().is_empty() {
                    continue;
                }
                let skew = if style.skew != 0.0 { Affine::new([1.0, 0.0, -style.skew.to_radians().tan(), 1.0, 0.0, 0.0]) } else { Affine::IDENTITY };
                let a = Affine::translate((g.x, l.baseline + g.y)) * skew * Affine::scale_non_uniform(g.sx, g.sy);
                let b = batches.entry(g.style).or_default();
                for el in outline.elements() {
                    b.push(transform_el(a, *el));
                }
                self.stats.glyphs += 1;
            }
        }
        ctx.set_transform(f.view * xf);
        for (si, bp) in &batches {
            let st = &cs.styles[*si as usize];
            if let Some(c) = doc.resolve_color(&st.fill, st.fill_tint) {
                ctx.set_paint(color_of(&c, 1.0));
                ctx.fill_path(bp);
            }
            if st.stroke != designcraft_color::swatch::NONE
                && let Some(c) = doc.resolve_color(&st.stroke, st.stroke_tint)
            {
                ctx.set_paint(color_of(&c, 1.0));
                ctx.set_stroke(kurbo::Stroke::new(st.stroke_weight));
                ctx.stroke_path(bp);
            }
        }
        for (si, r) in lines_deco {
            let c = if si == u32::MAX {
                designcraft_color::Color::gray(0.35)
            } else {
                let st = &cs.styles[si as usize];
                doc.resolve_color(&st.fill, st.fill_tint).unwrap_or(designcraft_color::Color::BLACK)
            };
            ctx.set_paint(color_of(&c, if si == u32::MAX { 0.5 } else { 1.0 }));
            ctx.fill_rect(&r);
        }
    }
}

fn transform_el(a: Affine, el: kurbo::PathEl) -> kurbo::PathEl {
    use kurbo::PathEl::*;
    match el {
        MoveTo(p) => MoveTo(a * p),
        LineTo(p) => LineTo(a * p),
        QuadTo(p1, p2) => QuadTo(a * p1, a * p2),
        CurveTo(p1, p2, p3) => CurveTo(a * p1, a * p2, a * p3),
        ClosePath => ClosePath,
    }
}

fn rect_overlaps(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

pub fn color_of(c: &designcraft_color::Color, alpha: f32) -> peniko::Color {
    let [r, g, b, a] = c.to_rgba8(alpha);
    peniko::Color::from_rgba8(r, g, b, a)
}

/// Set a swatch fill (solid or gradient) as the paint. Returns false for [None]/unknown.
fn set_fill_paint(ctx: &mut RenderContext, doc: &Document, swatch: &str, tint: f32, bounds: Rect, angle: Option<f64>) -> bool {
    if let Some(g) = designcraft_color::swatch::resolve_gradient(&doc.swatches, swatch) {
        let stops: Vec<peniko::ColorStop> = g.expanded_stops().iter().map(|(o, c, a)| peniko::ColorStop::from((*o, color_of(c, *a)))).collect();
        if stops.is_empty() {
            return false;
        }
        let c = bounds.center();
        let grad = match g.kind {
            designcraft_color::GradientKind::Radial => {
                let r = bounds.width().max(bounds.height()) / 2.0;
                peniko::Gradient::new_radial(c, r as f32).with_stops(stops.as_slice())
            }
            _ => {
                let a = angle.unwrap_or(0.0).to_radians();
                let half = (bounds.width() * a.cos().abs() + bounds.height() * a.sin().abs()) / 2.0;
                let d = Vec2::new(a.cos(), -a.sin()) * half;
                peniko::Gradient::new_linear(c - d, c + d).with_stops(stops.as_slice())
            }
        };
        ctx.set_paint(grad);
        return true;
    }
    match doc.resolve_color(swatch, tint) {
        Some(c) => {
            ctx.set_paint(color_of(&c, 1.0));
            true
        }
        None => false,
    }
}

pub fn blend_mode(b: DcBlend) -> BlendMode {
    use DcBlend as B;
    let mix = match b {
        B::Normal => Mix::Normal,
        B::Darken => Mix::Darken,
        B::Multiply => Mix::Multiply,
        B::ColorBurn => Mix::ColorBurn,
        B::Lighten => Mix::Lighten,
        B::Screen => Mix::Screen,
        B::ColorDodge => Mix::ColorDodge,
        B::Overlay => Mix::Overlay,
        B::SoftLight => Mix::SoftLight,
        B::HardLight => Mix::HardLight,
        B::Difference => Mix::Difference,
        B::Exclusion => Mix::Exclusion,
        B::Hue => Mix::Hue,
        B::Saturation => Mix::Saturation,
        B::Color => Mix::Color,
        B::Luminosity => Mix::Luminosity,
    };
    BlendMode::new(mix, Compose::SrcOver)
}

/// Decode encoded image bytes into a premultiplied pixmap.
pub fn decode_pixmap(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || w > u16::MAX as u32 || h > u16::MAX as u32 {
        return None;
    }
    let data: Vec<vello_cpu::color::PremulRgba8> = img
        .pixels()
        .map(|p| {
            let a = p[3] as u16;
            let m = |c: u8| ((c as u16 * a + 127) / 255) as u8;
            vello_cpu::color::PremulRgba8 { r: m(p[0]), g: m(p[1]), b: m(p[2]), a: p[3] }
        })
        .collect();
    Some(Pixmap::from_parts(data, w as u16, h as u16))
}

/// Box-filter downsample by 2.
fn halve(pm: &Pixmap) -> Pixmap {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let src = pm.data();
    let mut out = Vec::with_capacity(nw * nh);
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0u32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let sx = (2 * x + dx).min(w - 1);
                let sy = (2 * y + dy).min(h - 1);
                let p = src[sy * w + sx];
                acc[0] += p.r as u32;
                acc[1] += p.g as u32;
                acc[2] += p.b as u32;
                acc[3] += p.a as u32;
            }
            out.push(vello_cpu::color::PremulRgba8 { r: (acc[0] / 4) as u8, g: (acc[1] / 4) as u8, b: (acc[2] / 4) as u8, a: (acc[3] / 4) as u8 });
        }
    }
    Pixmap::from_parts(out, nw as u16, nh as u16)
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_micros() as u64).unwrap_or(0)
}
#[cfg(target_arch = "wasm32")]
fn now() -> u64 {
    0
}

/// Pixel size of an encoded image without decoding it fully.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

/// MIME type guess for encoded image bytes.
pub fn image_mime(bytes: &[u8]) -> &'static str {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => "image/png",
        Ok(image::ImageFormat::Jpeg) => "image/jpeg",
        Ok(image::ImageFormat::Gif) => "image/gif",
        Ok(image::ImageFormat::WebP) => "image/webp",
        Ok(image::ImageFormat::Tiff) => "image/tiff",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_doc::build::NewDocument;
    use designcraft_doc::{Fill, ParaFormat};

    #[test]
    fn renders_page_with_text_and_fill() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        let (fid, _) =
            d.add_text_frame(SpreadRef::Doc(0), Rect::new(36.0, 36.0, 576.0, 300.0), lid, "Hello DesignCraft", ParaFormat::default()).unwrap();
        let id = designcraft_doc::ItemId(d.alloc());
        let mut box_ =
            Item::new(id, lid, designcraft_doc::Shape::Rectangle, designcraft_geom::shapes::rectangle(Rect::new(36.0, 400.0, 136.0, 500.0)));
        box_.fill = Fill::swatch("C=100 M=0 Y=0 K=0");
        d.insert_item(SpreadRef::Doc(0), box_, None).unwrap();
        let cache = Cache::new();
        let mut r = Renderer::new();
        r.threads = 0;
        let img = r.render_page(&d, &cache, 0, 1.0, false, &RenderOptions::default()).unwrap();
        assert_eq!((img.width, img.height), (612, 792));
        // Paper is white, the box is cyan-ish, text drew some dark pixels.
        assert_eq!(img.pixel(300, 700), [255, 255, 255, 255]);
        let c = img.pixel(80, 450);
        assert!(c[0] < 80 && c[2] > 150, "{c:?}");
        let dark = (36..300).flat_map(|x| (36..60).map(move |y| (x, y))).filter(|&(x, y)| img.pixel(x, y)[0] < 100).count();
        assert!(dark > 50, "text pixels: {dark}");
        let _ = fid;
        assert!(r.stats.glyphs >= 15);
    }
}
