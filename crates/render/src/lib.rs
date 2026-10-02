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
use designcraft_compose::{Cache, ComposedStory};
use designcraft_doc::{Content, Document, Item, PageSide, SpreadRef, StoryId, StrokeAlign, StrokeType};
use designcraft_geom::corners;
use designcraft_geom::{Affine, BezPath, Rect, Shape, Vec2};
use vello_cpu::kurbo;
use vello_cpu::peniko::{self, BlendMode, Compose, Mix};
use vello_cpu::{Pixmap, RenderContext, Resources};

pub use vello_cpu;

pub mod damage;
mod fx;
pub mod images;
mod text;

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
    /// Pink highlight behind text set in fonts that aren't installed (screen view).
    pub highlight_missing_fonts: bool,
    /// View › Display Performance.
    pub quality: DisplayQuality,
}

/// View › Display Performance: how placed graphics and effects are drawn on screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayQuality {
    /// Graphics as grey boxes, no transparency effects.
    Fast,
    /// Low-resolution proxies (72 ppi at 100%).
    Typical,
    /// Full resolution.
    #[default]
    High,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            background: None,
            paper: true,
            printing_only: false,
            hidden: vec![],
            greek_below_px: 0.0,
            page_shadow: false,
            highlight_missing_fonts: false,
            quality: DisplayQuality::High,
        }
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

/// Reusable renderer: keeps the render context, the frame-space glyph paths of composed text
/// and (process-wide) decoded images between renders.
pub struct Renderer {
    ctx: Option<RenderContext>,
    resources: Resources,
    glyphs: text::GlyphCache,
    /// Composed stories looked up during the current render.
    stories: HashMap<(StoryId, Option<String>), Arc<ComposedStory>>,
    pub threads: u16,
    pub stats: FrameStats,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Frame<'a> {
    doc: &'a Document,
    cache: &'a Cache,
    view: Affine,
    /// Visible area in spread space.
    visible: Rect,
    /// Size of a device pixel in points.
    px: f64,
    opts: &'a RenderOptions,
    /// Drawing into a multithreaded context (no filter layers).
    mt: bool,
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
        Self {
            ctx: None,
            resources: Resources::new(),
            glyphs: text::GlyphCache::default(),
            stories: HashMap::new(),
            threads: default_threads(),
            stats: FrameStats::default(),
        }
    }

    /// Drop cached glyph paths (they are rebuilt on demand).
    pub fn clear_caches(&mut self) {
        self.glyphs.clear();
        self.stories.clear();
    }

    /// Number of text frames whose glyph paths are cached.
    pub fn cached_frames(&self) -> usize {
        self.glyphs.len()
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
            Some(mut c) if c.width() == w && c.height() == h && c.render_settings().num_threads == self.threads => {
                c.reset();
                c
            }
            _ => RenderContext::new_with(w, h, vello_cpu::RenderSettings { num_threads: self.threads, ..Default::default() }),
        };
        let mt = ctx.render_settings().num_threads > 0;
        self.stats = FrameStats::default();
        self.stories.clear();
        self.glyphs.tick();
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
            let f = Frame { doc, cache, view: sview, visible, px, opts, mt };
            self.draw_spread(&mut ctx, &f, pl.spread);
        }
        ctx.flush();
        // Render straight into the returned buffer (no extra copy of the frame).
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        ctx.render(vello_cpu::PixmapMut::new(w, h, &mut pixels).expect("buffer size"), &mut self.resources);
        self.ctx = Some(ctx);
        self.stories.clear();
        self.stats.micros = now().saturating_sub(start);
        Rendered { width: w as u32, height: h as u32, pixels }
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
        let vb = xf.transform_rect_bbox(it.inner_bounds()).inflate(it.stroke.extent() + 2.0, it.stroke.extent() + 2.0);
        if it.has_nested_items() {
            // A frame with items pasted into it: fill, the items clipped to the frame, stroke.
            if !rect_overlaps(vb, f.visible) {
                return;
            }
            let bp = if it.corners.is_none() { it.path.to_bezpath() } else { corners::apply(&it.path, &it.corners) };
            let layered = it.opacity < 0.999 || it.blend != DcBlend::Normal;
            if layered {
                ctx.set_transform(Affine::IDENTITY);
                ctx.push_layer(None, Some(blend_mode(it.blend)), Some(it.opacity), None, None);
            }
            if !it.fill.is_none() {
                ctx.set_transform(f.view * xf);
                if set_fill_paint(ctx, f.doc, &it.fill, bp.bounding_box()) {
                    ctx.fill_path(&bp);
                }
                ctx.reset_paint_transform();
            }
            ctx.set_transform(f.view * xf);
            ctx.push_clip_layer(&bp);
            for c in it.children() {
                self.draw_item(ctx, f, c, xf, page_name);
            }
            ctx.pop_layer();
            if !it.stroke.is_none() {
                self.draw_stroke(ctx, f, it, &bp, xf);
            }
            if layered {
                ctx.pop_layer();
            }
            return;
        }
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
        // Composed text stays inside its frame apart from glyph overhang (italics, swashes, big
        // initials), so text frames get a margin rather than being exempt from culling.
        let margin = match it.content {
            Content::Text(_) => 36.0 + 0.25 * vb.width().max(vb.height()),
            _ => 0.0,
        };
        let effects = it.effects.any() && f.opts.quality != DisplayQuality::Fast;
        let fx_outset = if effects { fx::outset(it) } else { 0.0 };
        if !rect_overlaps(vb.inflate(margin + fx_outset, margin + fx_outset), f.visible) {
            return;
        }
        self.stats.items += 1;
        let bp = if it.corners.is_none() { it.path.to_bezpath() } else { corners::apply(&it.path, &it.corners) };
        let layered = it.opacity < 0.999 || it.blend != DcBlend::Normal;
        if layered {
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, Some(blend_mode(it.blend)), Some(it.opacity), None, None);
        }
        if effects {
            self.draw_item_fx(ctx, f, it, &bp, xf, page_name);
        } else {
            self.draw_body(ctx, f, it, &bp, xf, page_name);
        }
        if layered {
            ctx.pop_layer();
        }
    }

    /// The composed story for a frame (memoised for the current render: the cache lookup
    /// validates the whole thread each time).
    fn story(&mut self, f: &Frame, sid: StoryId, page_name: Option<&str>) -> Arc<ComposedStory> {
        let key = (sid, page_name.map(str::to_string));
        if let Some(cs) = self.stories.get(&key) {
            return cs.clone();
        }
        let cs = f.cache.get(f.doc, sid, page_name);
        self.stories.insert(key, cs.clone());
        cs
    }

    /// Fill, content and stroke of a leaf item (no opacity, blend or effects).
    pub(crate) fn draw_body(&mut self, ctx: &mut RenderContext, f: &Frame, it: &Item, bp: &BezPath, xf: Affine, page_name: Option<&str>) {
        let doc = f.doc;
        // Fill.
        if !it.fill.is_none() {
            ctx.set_transform(f.view * xf);
            if set_fill_paint(ctx, doc, &it.fill, bp.bounding_box()) && it.path.is_closed() {
                ctx.fill_path(bp);
            }
            ctx.reset_paint_transform();
        }
        // Content.
        match &it.content {
            Content::Graphic(g) => {
                ctx.set_transform(f.view * xf);
                ctx.push_clip_layer(bp);
                self.draw_graphic(ctx, f, g, xf);
                ctx.pop_layer();
            }
            Content::Text(tf) => {
                let cs = self.story(f, tf.story, page_name);
                if let Some(ft) = cs.frame(it.id) {
                    self.draw_text(ctx, f, &cs, ft, xf);
                }
            }
            _ => {}
        }
        // Stroke.
        if !it.stroke.is_none() {
            self.draw_stroke(ctx, f, it, bp, xf);
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
        // Arrowheads: the path is shortened under them; they're drawn after it.
        let arrows = designcraft_doc::arrow::apply(bp, st, closed);
        let bp = arrows.as_ref().map_or(bp, |a| &a.0);
        // Gap colour under dashes and dots: the whole path, undashed.
        let gap = if matches!(st.kind, StrokeType::Solid) { None } else { f.doc.resolve_color(&st.gap_swatch, st.gap_tint) };
        if let Some(g) = gap
            && !matches!(st.align, StrokeAlign::Inside | StrokeAlign::Outside if closed)
        {
            ctx.set_paint(color_of(&g, 1.0));
            ctx.set_stroke(kurbo::Stroke {
                dash_pattern: Default::default(),
                start_cap: kurbo::Cap::Butt,
                end_cap: kurbo::Cap::Butt,
                ..stroke.clone()
            });
            ctx.stroke_path(bp);
            ctx.set_paint(color_of(&c, 1.0));
        }
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
        for h in arrows.iter().flat_map(|a| &a.1) {
            match h.outline {
                Some(w) => {
                    ctx.set_stroke(kurbo::Stroke::new(w).with_join(kurbo::Join::Miter));
                    ctx.stroke_path(&h.path);
                }
                None => ctx.fill_path(&h.path),
            }
        }
    }

    fn draw_graphic(&mut self, ctx: &mut RenderContext, f: &Frame, g: &designcraft_doc::Graphic, xf: Affine) {
        let Some(asset) = f.doc.assets.get(&g.asset) else { return };
        if f.opts.quality == DisplayQuality::Fast {
            ctx.set_transform(f.view * xf * g.xf);
            ctx.set_paint(peniko::Color::from_rgba8(178, 178, 178, 255));
            ctx.fill_rect(&Rect::new(0.0, 0.0, g.size.0, g.size.1));
            return;
        }
        // Pick a mip level close to the on-screen size (Typical: a 72 ppi proxy).
        let on_screen = (f.view * xf * g.xf).determinant().abs().sqrt();
        let on_screen = if f.opts.quality == DisplayQuality::Typical { on_screen.min(1.0) } else { on_screen };
        let Some(pm) = images::mip(&asset.data, g.size.0, on_screen) else { return };
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
}

fn rect_overlaps(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

pub fn color_of(c: &designcraft_color::Color, alpha: f32) -> peniko::Color {
    let [r, g, b, a] = c.to_rgba8(alpha);
    peniko::Color::from_rgba8(r, g, b, a)
}

/// Set a swatch fill (solid or gradient) as the paint. Returns false for [None]/unknown.
fn set_fill_paint(ctx: &mut RenderContext, doc: &Document, fill: &designcraft_doc::Fill, bounds: Rect) -> bool {
    let (swatch, tint, angle) = (fill.swatch.as_str(), fill.tint, fill.gradient_angle);
    if let Some(g) = designcraft_color::swatch::resolve_gradient(&doc.swatches, swatch) {
        let stops: Vec<peniko::ColorStop> = g.expanded_stops().iter().map(|(o, c, a)| peniko::ColorStop::from((*o, color_of(c, *a)))).collect();
        if stops.is_empty() {
            return false;
        }
        let c = bounds.center();
        let grad = match (g.kind, fill.gradient_vector) {
            (designcraft_color::GradientKind::Radial, Some([x0, y0, x1, y1])) => {
                let r = Vec2::new(x1 - x0, y1 - y0).hypot().max(1e-3);
                peniko::Gradient::new_radial((x0, y0), r as f32).with_stops(stops.as_slice())
            }
            (_, Some([x0, y0, x1, y1])) => peniko::Gradient::new_linear((x0, y0), (x1, y1)).with_stops(stops.as_slice()),
            (designcraft_color::GradientKind::Radial, None) => {
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
    if is_pdf(bytes) {
        return render_pdf_page(bytes, 0, 3000);
    }
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
    if is_pdf(bytes) {
        // Placed PDFs are sized by their page (crop box) in points.
        return pdf_page_size(bytes, 0).map(|(w, h)| (w.round().max(1.0) as u32, h.round().max(1.0) as u32));
    }
    image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

/// MIME type guess for encoded image bytes.
/// Is this a PDF file (placed PDFs are graphics)?
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF")
}

/// Page count of a PDF.
pub fn pdf_page_count(bytes: &[u8]) -> Option<usize> {
    let pdf = hayro::hayro_syntax::Pdf::new(std::sync::Arc::new(bytes.to_vec())).ok()?;
    Some(pdf.pages().len())
}

/// Size of PDF page `page` (0-based), in points, as it shows (crop box, rotation applied).
pub fn pdf_page_size(bytes: &[u8], page: usize) -> Option<(f64, f64)> {
    let pdf = hayro::hayro_syntax::Pdf::new(std::sync::Arc::new(bytes.to_vec())).ok()?;
    let p = pdf.pages().get(page)?;
    let (w, h) = p.render_dimensions();
    Some((w as f64, h as f64))
}

/// Rasterize PDF page `page` so its longer side is about `max_side` pixels (screen display; PDF
/// export embeds the page as vectors).
pub fn render_pdf_page(bytes: &[u8], page: usize, max_side: u32) -> Option<Pixmap> {
    let pdf = hayro::hayro_syntax::Pdf::new(std::sync::Arc::new(bytes.to_vec())).ok()?;
    let p = pdf.pages().get(page)?;
    let (w, h) = p.render_dimensions();
    let scale = (max_side as f32 / w.max(h).max(1.0)).min(8.0);
    let rs = hayro::RenderSettings { x_scale: scale, y_scale: scale, ..Default::default() };
    let pm = hayro::render(p, &hayro::RenderCache::new(), &hayro::hayro_interpret::InterpreterSettings::default(), &rs);
    let (pw, ph) = (pm.width(), pm.height());
    let data: Vec<vello_cpu::color::PremulRgba8> =
        pm.data_as_u8_slice().chunks_exact(4).map(|c| vello_cpu::color::PremulRgba8 { r: c[0], g: c[1], b: c[2], a: c[3] }).collect();
    Some(Pixmap::from_parts(data, pw, ph))
}

pub fn image_mime(bytes: &[u8]) -> &'static str {
    if is_pdf(bytes) {
        return "application/pdf";
    }
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

    #[test]
    fn arrowheads_draw_at_line_ends() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        let id = designcraft_doc::ItemId(d.alloc());
        let path = designcraft_geom::shapes::line(designcraft_geom::Point::new(100.0, 400.0), designcraft_geom::Point::new(400.0, 400.0));
        let mut line = Item::new(id, lid, designcraft_doc::Shape::GraphicLine, path);
        line.stroke = designcraft_doc::Stroke {
            swatch: designcraft_color::swatch::BLACK.into(),
            weight: 2.0,
            end: designcraft_doc::Arrowhead::TriangleWide,
            ..Default::default()
        };
        d.insert_item(SpreadRef::Doc(0), line, None).unwrap();
        // A dashed line with a gap colour: the gaps are painted.
        let id = designcraft_doc::ItemId(d.alloc());
        let path = designcraft_geom::shapes::line(designcraft_geom::Point::new(100.0, 500.0), designcraft_geom::Point::new(400.0, 500.0));
        let mut dashed = Item::new(id, lid, designcraft_doc::Shape::GraphicLine, path);
        dashed.stroke = designcraft_doc::Stroke {
            swatch: designcraft_color::swatch::BLACK.into(),
            weight: 4.0,
            kind: designcraft_doc::StrokeType::Dashed { pattern: vec![12.0, 4.0] },
            gap_swatch: "C=100 M=0 Y=0 K=0".into(),
            ..Default::default()
        };
        d.insert_item(SpreadRef::Doc(0), dashed, None).unwrap();
        let cache = Cache::new();
        let mut r = Renderer::new();
        r.threads = 0;
        let img = r.render_page(&d, &cache, 0, 1.0, false, &RenderOptions::default()).unwrap();
        // The head is 6.4 × weight wide: ink 4 pt above the line near the tip, none at the start.
        assert!(img.pixel(393, 396)[0] < 128, "{:?}", img.pixel(393, 396));
        assert_eq!(img.pixel(105, 396), [255, 255, 255, 255]);
        // Nothing past the tip.
        assert_eq!(img.pixel(403, 400), [255, 255, 255, 255]);
        let (dash, gap) = (img.pixel(106, 500), img.pixel(114, 500));
        assert!(dash[0] < 80 && dash[2] < 80, "dash {dash:?}");
        assert!(gap[0] < 80 && gap[2] > 150, "gap {gap:?}");
    }

    #[test]
    fn display_performance_fast_draws_grey_boxes() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        // A 4×4 red PNG placed at 100,100 (100×100 pt).
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let aid = designcraft_doc::AssetId(d.alloc());
        d.assets.insert(
            aid,
            Arc::new(designcraft_doc::Asset {
                id: aid,
                name: "red.png".into(),
                mime: "image/png".into(),
                link: None,
                data: Arc::new(png),
                pixels: Some((4, 4)),
            }),
        );
        let id = designcraft_doc::ItemId(d.alloc());
        let mut it =
            Item::new(id, lid, designcraft_doc::Shape::Rectangle, designcraft_geom::shapes::rectangle(Rect::new(100.0, 100.0, 200.0, 200.0)));
        it.content = designcraft_doc::Content::Graphic(designcraft_doc::Graphic {
            asset: aid,
            size: (100.0, 100.0),
            xf: Affine::translate((100.0, 100.0)),
            auto_fit: Default::default(),
        });
        d.insert_item(SpreadRef::Doc(0), it, None).unwrap();
        let cache = Cache::new();
        let mut r = Renderer::new();
        r.threads = 0;
        let px = |q: DisplayQuality, r: &mut Renderer| {
            r.render_page(&d, &cache, 0, 1.0, false, &RenderOptions { quality: q, ..Default::default() }).unwrap().pixel(150, 150)
        };
        assert_eq!(px(DisplayQuality::High, &mut r)[..3], [255, 0, 0]);
        assert_eq!(px(DisplayQuality::Typical, &mut r)[..3], [255, 0, 0]);
        assert_eq!(px(DisplayQuality::Fast, &mut r)[..3], [178, 178, 178]);
    }

    #[test]
    fn missing_fonts_are_highlighted_on_screen_only() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        let (_, sid) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(36.0, 36.0, 336.0, 100.0), lid, "Missing font", ParaFormat::default()).unwrap();
        d.story_mut(sid).unwrap().format_chars(0..12, |f| f.over.font_family = Some("No Such Family".into()));
        let cache = Cache::new();
        let cs = cache.get(&d, sid, None);
        assert!(cs.styles.iter().any(|s| s.missing_font));
        let l = &cs.frames[0].lines[0];
        // A point inside the line box but between glyph strokes: the space after "Missing".
        let g = l.glyphs.iter().find(|g| g.byte == 7).unwrap();
        let (x, y) = ((g.x + g.adv / 2.0) as u32, (l.baseline - l.ascent * 0.5) as u32);
        let mut r = Renderer::new();
        r.threads = 0;
        let plain = r.render_page(&d, &cache, 0, 1.0, false, &RenderOptions::default()).unwrap();
        let shown = r.render_page(&d, &cache, 0, 1.0, false, &RenderOptions { highlight_missing_fonts: true, ..Default::default() }).unwrap();
        assert_eq!(plain.pixel(x, y)[1], 255, "no highlight in output");
        let p = shown.pixel(x, y);
        assert!(p[0] > 240 && p[1] < 200, "pink on screen: {p:?}");
    }

    #[test]
    fn renders_tables() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        let (_, sid) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(36.0, 36.0, 336.0, 400.0), lid, "", ParaFormat::default()).unwrap();
        let mut t = designcraft_doc::Table::new(5, 3, 3, 1, 0, 300.0);
        for c in 0..3 {
            let cell = t.cell_mut(0, c).unwrap();
            cell.fill = "C=100 M=0 Y=0 K=0".into();
            cell.text.insert(0, "Head");
            t.cell_mut(2, c).unwrap().text.insert(0, "WWWWWWWW");
        }
        d.story_mut(sid).unwrap().insert_table(0, t);
        let cache = Cache::new();
        let cs = cache.get(&d, sid, None);
        let tf = &cs.frames[0].tables[0];
        let head = tf.cell(0, 1).unwrap().rect;
        let body = tf.cell(2, 0).unwrap().rect;
        let mut r = Renderer::new();
        r.threads = 0;
        let img = r.render_page(&d, &cache, 0, 1.0, false, &RenderOptions::default()).unwrap();
        // Header fill (cyan) near the right edge of the middle header cell.
        let c = img.pixel((head.x1 - 3.0) as u32, (head.y0 + 3.0) as u32);
        assert!(c[0] < 80 && c[2] > 150, "{c:?}");
        // The table border (black) on the left edge.
        let e = img.pixel(body.x0.round() as u32, body.center().y as u32);
        assert!(e[0] < 200, "{e:?}");
        // Cell text drew dark pixels inside the body cell.
        let dark = ((body.x0 + 5.0) as u32..(body.x1 - 2.0) as u32)
            .flat_map(|x| ((body.y0 + 2.0) as u32..(body.y1 - 2.0) as u32).map(move |y| (x, y)))
            .filter(|&(x, y)| img.pixel(x, y)[0] < 100)
            .count();
        assert!(dark > 30, "cell text pixels: {dark}");
    }
}
