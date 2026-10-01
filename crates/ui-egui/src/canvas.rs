//! The document canvas: pasteboard, spreads (rendered by `designcraft-render`), guides, frame edges,
//! selection, text ports and threads, the text caret, rulers, and pointer/keyboard input.

use designcraft_compose as compose;
use designcraft_doc::{Content, Document, Item, PageSide, SpreadRef};
use designcraft_engine::doc::Selection;
use designcraft_geom::{Affine, Point, Rect as DRect, Vec2 as DVec2};
use designcraft_tools::{CanvasLayout, Cursor, Mods, Overlay, PointerEvent, PointerKind, ToolKey};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Value, json};

use crate::{DesignApp, ScreenMode, View, theme::Tokens};

pub const RULER: f32 = 15.0;

/// Canvas ↔ screen transform.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub min: Pos2,
    pub origin: Point,
    pub zoom: f64,
}

impl Xf {
    pub fn new(rect: Rect, v: &View) -> Self {
        Xf { min: rect.min, origin: v.origin, zoom: v.zoom }
    }
    pub fn to_screen(&self, p: Point) -> Pos2 {
        pos2(self.min.x + ((p.x - self.origin.x) * self.zoom) as f32, self.min.y + ((p.y - self.origin.y) * self.zoom) as f32)
    }
    pub fn to_canvas(&self, p: Pos2) -> Point {
        Point::new(self.origin.x + (p.x - self.min.x) as f64 / self.zoom, self.origin.y + (p.y - self.min.y) as f64 / self.zoom)
    }
    pub fn rect(&self, r: DRect) -> Rect {
        Rect::from_two_pos(self.to_screen(Point::new(r.x0, r.y0)), self.to_screen(Point::new(r.x1, r.y1)))
    }
    /// Canvas → screen affine (for vello rendering: pixels = points × ppp).
    pub fn affine(&self, ppp: f64, area_min: Pos2) -> Affine {
        let dx = (self.min.x - area_min.x) as f64;
        let dy = (self.min.y - area_min.y) as f64;
        Affine::scale(ppp) * Affine::translate((dx, dy)) * Affine::scale(self.zoom) * Affine::translate((-self.origin.x, -self.origin.y))
    }
}

fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Fit the current spread (or page) in the canvas.
pub fn fit(app: &mut DesignApp, rect: Rect, what: &str) {
    let Some(st) = app.session.active() else { return };
    let layout = CanvasLayout::new(&st.doc, st.editing_parents);
    let cur = current_slot(app, &layout).unwrap_or(0);
    let Some(slot) = layout.slots.get(cur) else { return };
    let mut b = slot.bounds;
    if what == "page"
        && let Some(sp) = st.doc.spread(slot.spread)
    {
        let center = app.view().map(|v| v.origin.x + rect.width() as f64 / 2.0 / v.zoom).unwrap_or(0.0);
        let pi = sp.page_at_x(center - slot.offset.x).unwrap_or(0);
        b = sp.pages[pi].bounds() + slot.offset;
    }
    if what == "all" {
        b = layout.slots.iter().map(|s| s.bounds).reduce(|a, b| a.union(b)).unwrap_or(b);
    }
    let pad = 40.0;
    let zoom = ((rect.width() as f64 - 2.0 * pad) / b.width()).min((rect.height() as f64 - 2.0 * pad) / b.height()).clamp(0.05, 40.0);
    let origin = Point::new(b.center().x - rect.width() as f64 / 2.0 / zoom, b.center().y - rect.height() as f64 / 2.0 / zoom);
    if let Some(v) = app.view_mut() {
        *v = View { zoom, origin, fitted: true };
    }
}

/// Index of the slot nearest the view centre.
pub fn current_slot(app: &DesignApp, layout: &CanvasLayout) -> Option<usize> {
    let v = app.view()?;
    let r = app.canvas_rect?;
    let c = Point::new(v.origin.x + r.width() as f64 / 2.0 / v.zoom, v.origin.y + r.height() as f64 / 2.0 / v.zoom);
    layout
        .slots
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let da = (a.1.bounds.center().y - c.y).abs();
            let db = (b.1.bounds.center().y - c.y).abs();
            da.total_cmp(&db)
        })
        .map(|(i, _)| i)
}

/// Absolute page index under the view centre.
pub fn current_page(app: &DesignApp) -> Option<usize> {
    let st = app.session.active()?;
    let layout = CanvasLayout::new(&st.doc, st.editing_parents);
    let i = current_slot(app, &layout)?;
    let slot = layout.slots.get(i)?;
    let SpreadRef::Doc(si) = slot.spread else { return Some(0) };
    let v = app.view()?;
    let r = app.canvas_rect?;
    let cx = v.origin.x + r.width() as f64 / 2.0 / v.zoom - slot.offset.x;
    let pi = st.doc.spreads[si].page_at_x(cx).unwrap_or(0);
    Some(st.doc.first_page_of_spread(si) + pi)
}

/// Scroll so absolute page `abs` is centred (keeps zoom).
pub fn go_to_page(app: &mut DesignApp, abs: usize) {
    let Some(st) = app.session.active() else { return };
    let layout = CanvasLayout::new(&st.doc, false);
    let Some(pr) = layout.page_rect(&st.doc, abs) else { return };
    let Some(rect) = app.canvas_rect else { return };
    if let Some(v) = app.view_mut() {
        v.origin = Point::new(pr.center().x - rect.width() as f64 / 2.0 / v.zoom, pr.center().y - rect.height() as f64 / 2.0 / v.zoom);
    }
}

pub fn zoom_at(app: &mut DesignApp, screen: Pos2, factor: f64) {
    let Some(rect) = app.canvas_rect else { return };
    let Some(v) = app.view_mut() else { return };
    let xf = Xf::new(rect, v);
    let c = xf.to_canvas(screen);
    let nz = (v.zoom * factor).clamp(0.05, 40.0);
    v.zoom = nz;
    v.origin = Point::new(c.x - (screen.x - rect.min.x) as f64 / nz, c.y - (screen.y - rect.min.y) as f64 / nz);
}

pub fn set_zoom(app: &mut DesignApp, z: f64) {
    let Some(rect) = app.canvas_rect else { return };
    let cur = app.view().map(|v| v.zoom).unwrap_or(1.0);
    zoom_at(app, rect.center(), z / cur);
}

pub fn apply_view_request(app: &mut DesignApp, p: &Value) {
    if let Some(d) = p.get("pan").and_then(Value::as_array) {
        let (dx, dy) = (d.first().and_then(Value::as_f64).unwrap_or(0.0), d.get(1).and_then(Value::as_f64).unwrap_or(0.0));
        if let Some(v) = app.view_mut() {
            v.origin = Point::new(v.origin.x - dx / v.zoom, v.origin.y - dy / v.zoom);
        }
    }
    if let (Some(a), Some(rect)) = (p.get("zoomAt").and_then(Value::as_array), app.canvas_rect)
        && let Some(v) = app.view()
    {
        let c = Point::new(a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0));
        let xf = Xf::new(rect, v);
        let s = xf.to_screen(c);
        zoom_at(app, s, p.get("factor").and_then(Value::as_f64).unwrap_or(2.0));
    }
}

fn mods(i: &egui::InputState, space: bool) -> Mods {
    Mods { shift: i.modifiers.shift, alt: i.modifiers.alt, cmd: i.modifiers.command, ctrl: i.modifiers.ctrl, space }
}

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let full = ui.available_rect_before_wrap();
    let rulers = app.ui.rulers && app.ui.screen_mode != ScreenMode::Presentation;
    let rect = if rulers { Rect::from_min_max(full.min + vec2(RULER, RULER), full.max) } else { full };
    app.canvas_rect = Some(rect);
    let resp = ui.allocate_rect(full, Sense::click_and_drag());
    if !app.view().is_some_and(|v| v.fitted) {
        fit(app, rect, "spread");
    }
    handle_input(app, ui, &resp, rect);
    let Some(st) = app.session.active() else { return };
    let v = *app.view().expect("view");
    let xf = Xf::new(rect, &v);
    let doc = st.doc.clone();
    let layout = CanvasLayout::new(&doc, st.editing_parents);
    let painter = ui.painter_at(rect);
    let preview = matches!(app.ui.screen_mode, ScreenMode::Preview | ScreenMode::Presentation);
    let bg = if app.ui.screen_mode == ScreenMode::Presentation { Color32::BLACK } else { t.pasteboard };
    painter.rect_filled(rect, 0.0, bg);
    // Page shadow: a hard 1.5 pt black offset on the right and bottom (InDesign 2026).
    for slot in &layout.slots {
        let r = xf.rect(slot.bounds);
        painter.rect_filled(r.translate(vec2(1.5, 1.5)), 0.0, Color32::BLACK);
    }
    // Rendered content.
    render_texture(app, ui.ctx(), rect, &xf, &layout, preview);
    if let Some(tex) = &app.canvas.texture {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        if preview {
            // Preview: only page areas (trim) show content.
            for slot in &layout.slots {
                let r = xf.rect(slot.bounds);
                let ruv = Rect::from_min_max(
                    pos2((r.min.x - rect.min.x) / rect.width(), (r.min.y - rect.min.y) / rect.height()),
                    pos2((r.max.x - rect.min.x) / rect.width(), (r.max.y - rect.min.y) / rect.height()),
                );
                painter.with_clip_rect(r.intersect(rect)).image(tex.id(), r, ruv, Color32::WHITE);
            }
        } else {
            painter.image(tex.id(), rect, uv, Color32::WHITE);
        }
    }
    let hair = 1.0 / ui.ctx().pixels_per_point();
    for slot in &layout.slots {
        painter.rect_stroke(xf.rect(slot.bounds), 0.0, Stroke::new(hair, Color32::from_rgb(0x4a, 0x4a, 0x4a)), StrokeKind::Outside);
    }
    if !preview {
        draw_guides(app, &painter, &xf, &doc, &layout, &t);
        draw_frames(app, &painter, &xf, &doc, &layout);
        if app.ui.hidden_characters {
            draw_hidden_characters(app, &painter, &xf, &doc, &layout);
        }
    }
    draw_selection(app, &painter, &xf, &doc, &layout);
    draw_tool_overlays(app, &painter, &xf);
    if rulers {
        draw_rulers(app, ui, full, rect, &xf, &doc, &layout, &t);
    }
    // Cursor.
    if let Some(p) = resp.hover_pos().filter(|p| rect.contains(*p)) {
        let space = ui.input(|i| i.key_down(egui::Key::Space)) && !app.session.wants_text();
        let c = if space { Cursor::Hand } else { app.session.cursor(xf.to_canvas(p), ui.input(|i| mods(i, false)), app.view_info()) };
        ui.ctx().set_cursor_icon(cursor_icon(c));
    }
}

fn cursor_icon(c: Cursor) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match c {
        Cursor::Arrow | Cursor::ArrowHollow => C::Default,
        Cursor::Move => C::Move,
        Cursor::Crosshair => C::Crosshair,
        Cursor::ResizeH => C::ResizeHorizontal,
        Cursor::ResizeV => C::ResizeVertical,
        Cursor::ResizeNwSe => C::ResizeNwSe,
        Cursor::ResizeNeSw => C::ResizeNeSw,
        Cursor::Rotate => C::Alias,
        Cursor::Pen => C::Crosshair,
        Cursor::Text => C::Text,
        Cursor::Hand => C::Grab,
        Cursor::HandGrab => C::Grabbing,
        Cursor::ZoomIn => C::ZoomIn,
        Cursor::ZoomOut => C::ZoomOut,
        Cursor::Eyedropper => C::Crosshair,
        Cursor::LoadedText | Cursor::LoadedGraphic => C::Copy,
        Cursor::NotAllowed => C::NotAllowed,
    }
}

fn render_texture(app: &mut DesignApp, ctx: &egui::Context, rect: Rect, xf: &Xf, layout: &CanvasLayout, preview: bool) {
    let Some(st) = app.session.active() else { return };
    let ppp = ctx.pixels_per_point() as f64;
    let w = (rect.width() as f64 * ppp).round().max(1.0) as u32;
    let h = (rect.height() as f64 * ppp).round().max(1.0) as u32;
    let doc_ptr = std::sync::Arc::as_ptr(&st.doc) as u64;
    let key = (st.uid, doc_ptr, xf.zoom.to_bits(), xf.origin.x.to_bits() ^ xf.origin.y.to_bits().rotate_left(17), st.revision, w, h, preview as u8);
    if app.canvas.key == Some(key) && app.canvas.texture.is_some() {
        return;
    }
    let t0 = crate::now_ms();
    let placed: Vec<designcraft_render::Placed> =
        layout.slots.iter().map(|s| designcraft_render::Placed { spread: s.spread, offset: s.offset }).collect();
    let view = xf.affine(ppp, rect.min);
    let opts = designcraft_render::RenderOptions { printing_only: preview, greek_below_px: 3.0 * ppp, ..Default::default() };
    let img = app.canvas.renderer.render(&st.doc, &app.session.cache, &placed, w, h, view, &opts);
    let ci = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &img.pixels);
    match &mut app.canvas.texture {
        Some(tex) => tex.set(ci, egui::TextureOptions::LINEAR),
        None => app.canvas.texture = Some(ctx.load_texture("canvas", ci, egui::TextureOptions::LINEAR)),
    }
    app.canvas.key = Some(key);
    app.perf.render_ms = crate::now_ms() - t0;
}

fn dashed(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke, dash: f32, gap: f32) {
    painter.extend(egui::Shape::dashed_line(&[a, b], stroke, dash, gap));
}

fn draw_guides(app: &DesignApp, painter: &egui::Painter, xf: &Xf, doc: &Document, layout: &CanvasLayout, t: &Tokens) {
    let s = &doc.settings;
    for slot in &layout.slots {
        let Some(sp) = doc.spread(slot.spread) else { continue };
        let off = slot.offset;
        // Bleed: one rectangle around the spread (inside bleed only applies at the spread's outer edges).
        let b = s.bleed;
        if app.ui.guides && b.iter().any(|v| *v > 0.0) && !sp.pages.is_empty() {
            let first = &sp.pages[0];
            let last = &sp.pages[sp.pages.len() - 1];
            let l = if first.side == PageSide::Left { b[3] } else { b[2] };
            let r = if last.side == PageSide::Right || last.side == PageSide::Single { b[3] } else { b[2] };
            let sb = sp.bounds() + off;
            let br = DRect::new(sb.x0 - l, sb.y0 - b[0], sb.x1 + r, sb.y1 + b[1]);
            painter.rect_stroke(xf.rect(br), 0.0, Stroke::new(hair(painter), c32(s.bleed_color)), StrokeKind::Middle);
        }
        for p in &sp.pages {
            let pr = p.bounds() + off;
            if !app.ui.guides {
                continue;
            }
            // Baseline grid.
            if app.ui.baseline_grid && xf.zoom >= s.baseline_grid.view_threshold {
                let g = &s.baseline_grid;
                let mut y = g.start;
                while y < p.height {
                    let a = xf.to_screen(Point::new(pr.x0, pr.y0 + y));
                    let bb = xf.to_screen(Point::new(pr.x1, pr.y0 + y));
                    painter.line_segment([a, bb], Stroke::new(hair(painter), c32(g.color).gamma_multiply(0.8)));
                    y += g.increment.max(1.0);
                }
            }
            // Margins (magenta) and columns (violet).
            let m = p.margin_rect() + off;
            painter.rect_stroke(xf.rect(m), 0.0, Stroke::new(hair(painter), c32(s.margin_color)), StrokeKind::Middle);
            let cols = p.column_rects();
            if cols.len() > 1 {
                for (i, c) in cols.iter().enumerate() {
                    let c = *c + off;
                    let col = Stroke::new(hair(painter), c32(s.column_color));
                    if i > 0 {
                        painter.line_segment([xf.to_screen(Point::new(c.x0, c.y0)), xf.to_screen(Point::new(c.x0, c.y1))], col);
                    }
                    if i + 1 < cols.len() {
                        painter.line_segment([xf.to_screen(Point::new(c.x1, c.y0)), xf.to_screen(Point::new(c.x1, c.y1))], col);
                    }
                }
            }
            for g in &p.guides {
                let (a, bb) = match g.orientation {
                    designcraft_doc::Orientation::Horizontal => (Point::new(pr.x0, g.position + off.y), Point::new(pr.x1, g.position + off.y)),
                    designcraft_doc::Orientation::Vertical => (Point::new(g.position + off.x, pr.y0), Point::new(g.position + off.x, pr.y1)),
                };
                painter.line_segment([xf.to_screen(a), xf.to_screen(bb)], Stroke::new(hair(painter), Color32::from_rgb(74, 227, 255)));
            }
        }
    }
    let _ = t;
}

/// Spread-space → canvas transform for an item at `loc`.
fn item_canvas_xf(doc: &Document, layout: &CanvasLayout, id: designcraft_doc::ItemId) -> Option<(Affine, &'static str)> {
    let loc = doc.find(id)?;
    let off = layout.offset(loc.spread);
    Some((Affine::translate(off) * doc.parent_xf(&loc), ""))
}

fn path_screen(it: &Item, xf: &Xf, a: Affine) -> Vec<Vec<Pos2>> {
    let bp = it.path.to_bezpath();
    let full = a * it.xf;
    let mut out = Vec::new();
    let mut cur: Vec<Pos2> = Vec::new();
    designcraft_geom::kurbo::flatten(&bp, 0.25 / xf.zoom.max(0.01), |el| {
        use designcraft_geom::PathEl::*;
        match el {
            MoveTo(p) => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                cur.push(xf.to_screen(full * p));
            }
            LineTo(p) => cur.push(xf.to_screen(full * p)),
            ClosePath => {
                if let Some(f) = cur.first().copied() {
                    cur.push(f);
                }
            }
            _ => {}
        }
    });
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

fn draw_frames(app: &DesignApp, painter: &egui::Painter, xf: &Xf, doc: &Document, layout: &CanvasLayout) {
    if !app.ui.frame_edges {
        return;
    }
    for slot in &layout.slots {
        let Some(sp) = doc.spread(slot.spread) else { continue };
        let a = Affine::translate(slot.offset);
        // Parent items on document pages: dotted edges.
        if let SpreadRef::Doc(si) = slot.spread {
            let first = doc.first_page_of_spread(si);
            for (pi, page) in sp.pages.iter().enumerate() {
                let Some((ppi, ppg)) = doc.parent_page_for(first + pi) else { continue };
                let parent = &doc.parents[ppi];
                let dx = page.x - parent.pages[ppg].x;
                for it in &parent.items {
                    if parent.pages.len() > 1 && parent.page_at_x(it.bounds().center().x) != Some(ppg) {
                        continue;
                    }
                    let col = doc.layer(it.layer).map(|l| c32(l.color)).unwrap_or(Color32::LIGHT_BLUE);
                    for poly in path_screen(it, xf, a * Affine::translate((dx, 0.0))) {
                        for w in poly.windows(2) {
                            dashed(painter, w[0], w[1], Stroke::new(hair(painter), col.gamma_multiply(0.8)), 2.0, 2.0);
                        }
                    }
                }
            }
        }
        for it in &sp.items {
            draw_item_edges(painter, xf, doc, it, a);
        }
    }
}

fn draw_item_edges(painter: &egui::Painter, xf: &Xf, doc: &Document, it: &Item, a: Affine) {
    if it.hidden || doc.layer(it.layer).is_some_and(|l| !l.visible) {
        return;
    }
    let col = doc.layer(it.layer).map(|l| c32(l.color)).unwrap_or(Color32::LIGHT_BLUE);
    if let Content::Group { items } = &it.content {
        for c in items {
            draw_item_edges(painter, xf, doc, c, a * it.xf);
        }
        return;
    }
    // Frames without a stroke show their edge; shapes with strokes are visible already.
    let show = it.stroke.is_none() || matches!(it.content, Content::Text(_) | Content::Graphic(_));
    if show {
        for poly in path_screen(it, xf, a) {
            painter.add(egui::Shape::line(poly, Stroke::new(hair(painter), col.gamma_multiply(0.75))));
        }
    }
    // Empty graphic frame: the X.
    if it.object_style == designcraft_doc::BASIC_GRAPHICS_FRAME && matches!(it.content, Content::Unassigned) {
        let r = it.inner_bounds();
        let m = a * it.xf;
        let p = |x: f64, y: f64| xf.to_screen(m * Point::new(x, y));
        painter.line_segment([p(r.x0, r.y0), p(r.x1, r.y1)], Stroke::new(hair(painter), col.gamma_multiply(0.75)));
        painter.line_segment([p(r.x1, r.y0), p(r.x0, r.y1)], Stroke::new(hair(painter), col.gamma_multiply(0.75)));
    }
}

fn layer_color(doc: &Document, it: &Item) -> Color32 {
    doc.layer(it.layer).map(|l| c32(l.color)).unwrap_or(Color32::from_rgb(79, 153, 255))
}

fn draw_selection(app: &DesignApp, painter: &egui::Painter, xf: &Xf, doc: &Document, layout: &CanvasLayout) {
    let Some(st) = app.session.active() else { return };
    let sel: &Selection = &st.selection;
    // Text selection / caret.
    if let Some(ts) = sel.text {
        draw_text_selection(app, painter, xf, doc, layout, ts);
    }
    let mut union: Option<Rect> = None;
    let mut color = Color32::from_rgb(79, 153, 255);
    for id in &sel.items {
        let (Some(it), Some((a, _))) = (doc.item(*id), item_canvas_xf(doc, layout, *id)) else { continue };
        color = layer_color(doc, it);
        for poly in path_screen(it, xf, a) {
            painter.add(egui::Shape::line(poly, Stroke::new(1.0, color)));
        }
        if sel.items.len() == 1 && sel.text.is_none() {
            // InDesign draws a 1 pt bounding box around the selection.
            painter.rect_stroke(xf.rect(a.transform_rect_bbox(it.bounds())), 0.0, Stroke::new(1.0, color), StrokeKind::Middle);
        }
        let b = xf.rect(a.transform_rect_bbox(it.bounds()));
        union = Some(union.map_or(b, |u| u.union(b)));
        // Text frame ports.
        if let Content::Text(tf) = &it.content {
            draw_ports(app, painter, xf, doc, layout, it, tf.story, a, color);
        }
        if let Content::Graphic(g) = &it.content
            && sel.content
        {
            let gb = xf.rect((a * it.xf).transform_rect_bbox(g.xf.transform_rect_bbox(DRect::new(0.0, 0.0, g.size.0, g.size.1))));
            painter.rect_stroke(gb, 0.0, Stroke::new(1.0, Color32::from_rgb(196, 111, 43)), StrokeKind::Middle);
        }
    }
    let direct = matches!(app.session.tool_id(), "directSelection" | "pen");
    if direct {
        for id in &sel.items {
            let (Some(it), Some((a, _))) = (doc.item(*id), item_canvas_xf(doc, layout, *id)) else { continue };
            let m = a * it.xf;
            let col = layer_color(doc, it);
            for sp in &it.path.subpaths {
                for an in &sp.anchors {
                    let p = xf.to_screen(m * an.p);
                    for (h, has) in [(an.h_in, an.has_in()), (an.h_out, an.has_out())] {
                        if has {
                            let hp = xf.to_screen(m * h);
                            painter.line_segment([p, hp], Stroke::new(1.0, col));
                            painter.circle_filled(hp, 2.5, col);
                        }
                    }
                    let r = Rect::from_center_size(p, vec2(5.0, 5.0));
                    painter.rect_filled(r, 0.0, Color32::WHITE);
                    painter.rect_stroke(r, 0.0, Stroke::new(1.0, col), StrokeKind::Inside);
                }
            }
        }
    }
    if let Some(u) = union
        && sel.text.is_none()
        && !direct
    {
        if sel.items.len() > 1 {
            painter.rect_stroke(u, 0.0, Stroke::new(1.0, color), StrokeKind::Middle);
        }
        let hs = 6.5;
        // Centre point and the live-corner widget.
        painter.rect_filled(Rect::from_center_size(u.center(), vec2(3.5, 3.5)), 0.0, color);
        if sel.items.len() == 1
            && doc.item(sel.items[0]).is_some_and(|i| matches!(i.shape, designcraft_doc::Shape::Rectangle) && i.children().is_empty())
        {
            let lc = Rect::from_center_size(pos2(u.max.x, u.min.y + 11.5), vec2(6.0, 6.0));
            painter.rect_filled(lc, 0.0, Color32::from_rgb(0xff, 0xe5, 0x00));
            painter.rect_stroke(lc, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        for h in designcraft_tools::select::handles(DRect::new(u.min.x as f64, u.min.y as f64, u.max.x as f64, u.max.y as f64)) {
            let r = Rect::from_center_size(pos2(h.x as f32, h.y as f32), vec2(hs, hs));
            painter.rect_filled(r, 0.0, Color32::WHITE);
            painter.rect_stroke(r, 0.0, Stroke::new(hair(painter), color), StrokeKind::Inside);
        }
        // Content grabber on graphic frames.
        if sel.items.len() == 1 && doc.item(sel.items[0]).is_some_and(|i| matches!(i.content, Content::Graphic(_))) {
            painter.circle_stroke(u.center(), 8.0, Stroke::new(1.5, Color32::from_white_alpha(170)));
            painter.circle_stroke(u.center(), 4.0, Stroke::new(1.5, Color32::from_white_alpha(170)));
        }
    }
    // Threads.
    if app.ui.text_threads || sel.items.iter().any(|i| doc.item(*i).is_some_and(|x| x.is_text_frame())) {
        for id in &sel.items {
            let Some(sid) = doc.item(*id).and_then(|i| i.text_frame()).map(|t| t.story) else { continue };
            let Some(story) = doc.story(sid) else { continue };
            for w in story.frames.windows(2) {
                let (Some(a), Some(b)) = (port_pos(doc, layout, w[0], true), port_pos(doc, layout, w[1], false)) else { continue };
                let col = doc.item(w[0]).map(|i| layer_color(doc, i)).unwrap_or(color);
                painter.line_segment([xf.to_screen(a), xf.to_screen(b)], Stroke::new(1.0, col));
            }
        }
    }
}

/// Port location (canvas): out port at bottom-right, in port at top-left.
fn port_pos(doc: &Document, layout: &CanvasLayout, id: designcraft_doc::ItemId, out: bool) -> Option<Point> {
    let it = doc.item(id)?;
    let (a, _) = item_canvas_xf(doc, layout, id)?;
    let r = it.inner_bounds();
    let p = if out { Point::new(r.x1 - 10.0, r.y1) } else { Point::new(r.x0 + 10.0, r.y0) };
    Some(a * it.xf * p)
}

#[allow(clippy::too_many_arguments)]
fn draw_ports(
    app: &DesignApp,
    painter: &egui::Painter,
    xf: &Xf,
    doc: &Document,
    layout: &CanvasLayout,
    it: &Item,
    sid: designcraft_doc::StoryId,
    a: Affine,
    color: Color32,
) {
    let Some(story) = doc.story(sid) else { return };
    let pos = story.frames.iter().position(|f| *f == it.id).unwrap_or(0);
    let is_last = pos + 1 == story.frames.len();
    let cs = app.session.cache.get(doc, sid, None);
    let overset = is_last && cs.is_overset();
    let s = 8.5;
    let _ = layout;
    let r = it.inner_bounds();
    let m = a * it.xf;
    // In port on the left edge below the top-left corner; out port on the right edge above the
    // bottom-right corner (screen-space offsets).
    let inp = xf.to_screen(m * Point::new(r.x0, r.y0)) + vec2(0.0, 14.5);
    let outp = xf.to_screen(m * Point::new(r.x1, r.y1)) - vec2(0.0, 12.0);
    for (c, has_link, is_out) in [(inp, pos > 0, false), (outp, !is_last, true)] {
        let pr = Rect::from_center_size(c, vec2(s, s));
        painter.rect_filled(pr, 0.0, Color32::WHITE);
        painter.rect_stroke(pr, 0.0, Stroke::new(hair(painter), color), StrokeKind::Inside);
        if is_out && overset {
            let red = Color32::from_rgb(230, 20, 20);
            painter.line_segment([c - vec2(2.5, 0.0), c + vec2(2.5, 0.0)], Stroke::new(1.5, red));
            painter.line_segment([c - vec2(0.0, 2.5), c + vec2(0.0, 2.5)], Stroke::new(1.5, red));
        } else if has_link {
            painter.add(egui::Shape::convex_polygon(vec![c + vec2(-2.0, -2.5), c + vec2(2.5, 0.0), c + vec2(-2.0, 2.5)], color, Stroke::NONE));
        }
    }
}

fn draw_text_selection(app: &DesignApp, painter: &egui::Painter, xf: &Xf, doc: &Document, layout: &CanvasLayout, ts: designcraft_doc::TextSel) {
    let cs = app.session.cache.get(doc, ts.story, None);
    let range = ts.range();
    for ft in &cs.frames {
        let Some((a, _)) = item_canvas_xf(doc, layout, ft.frame) else { continue };
        let Some(it) = doc.item(ft.frame) else { continue };
        let m = a * it.xf;
        // Show the frame edge while typing.
        for poly in path_screen(it, xf, a) {
            painter.add(egui::Shape::line(poly, Stroke::new(1.0, layer_color(doc, it).gamma_multiply(0.6))));
        }
        if !range.is_empty() {
            for l in &ft.lines {
                let s = range.start.max(l.range.start);
                let e = range.end.min(l.range.end);
                if s > e || (s == e && !(l.range.end < range.end && e == l.range.end)) {
                    continue;
                }
                let x0 = caret_x(l, s);
                let x1 = if e == l.range.end && range.end > l.range.end { l.end_x.max(x0 + 3.0) } else { caret_x(l, e) };
                let q = [
                    xf.to_screen(m * Point::new(x0, l.baseline - l.ascent)),
                    xf.to_screen(m * Point::new(x1, l.baseline - l.ascent)),
                    xf.to_screen(m * Point::new(x1, l.baseline + l.descent)),
                    xf.to_screen(m * Point::new(x0, l.baseline + l.descent)),
                ];
                painter.add(egui::Shape::convex_polygon(q.to_vec(), Color32::from_rgba_unmultiplied(80, 140, 255, 110), Stroke::NONE));
            }
        }
    }
    if ts.is_caret()
        && let Some((fi, x, bl, asc, desc)) = compose::caret(&cs, ts.focus)
        && let Some(ft) = cs.frames.get(fi)
        && let (Some((a, _)), Some(it)) = (item_canvas_xf(doc, layout, ft.frame), doc.item(ft.frame))
    {
        let m = a * it.xf;
        let blink = (painter.ctx().input(|i| i.time) * 1.6) as i64 % 2 == 0;
        if blink {
            let p0 = xf.to_screen(m * Point::new(x, bl - asc));
            let p1 = xf.to_screen(m * Point::new(x, bl + desc));
            painter.line_segment([p0, p1], Stroke::new(1.0, Color32::BLACK));
        }
        painter.ctx().request_repaint_after(std::time::Duration::from_millis(330));
    }
}

fn caret_x(l: &compose::Line, pos: usize) -> f64 {
    for g in &l.glyphs {
        if g.len > 0 && pos >= g.byte && pos < g.byte + g.len {
            return g.x + g.adv * (pos - g.byte) as f64 / g.len as f64;
        }
        if g.len > 0 && g.byte >= pos {
            return g.x;
        }
    }
    l.end_x
}

fn draw_tool_overlays(app: &mut DesignApp, painter: &egui::Painter, xf: &Xf) {
    let ov = app.session.overlays(app.view_info());
    for o in ov {
        match o {
            Overlay::Marquee(r) => {
                let r = xf.rect(r);
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(80, 140, 255, 30));
                for (a, b) in [
                    (r.left_top(), r.right_top()),
                    (r.right_top(), r.right_bottom()),
                    (r.right_bottom(), r.left_bottom()),
                    (r.left_bottom(), r.left_top()),
                ] {
                    dashed(painter, a, b, Stroke::new(1.0, Color32::from_gray(200)), 3.0, 3.0);
                }
            }
            Overlay::Measure { p, text } => {
                let s = xf.to_screen(p) + vec2(14.0, 14.0);
                let g = painter.layout_no_wrap(text, egui::FontId::proportional(11.0), Color32::WHITE);
                let r = Rect::from_min_size(s, g.size() + vec2(10.0, 6.0));
                painter.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(70, 70, 70, 230));
                painter.galley(r.min + vec2(5.0, 3.0), g, Color32::WHITE);
            }
            Overlay::Line { a, b, color, dashed: d } => {
                let st = Stroke::new(1.0, c32(color));
                if d {
                    dashed(painter, xf.to_screen(a), xf.to_screen(b), st, 3.0, 3.0);
                } else {
                    painter.line_segment([xf.to_screen(a), xf.to_screen(b)], st);
                }
            }
            Overlay::Guide { a, b } => {
                painter.line_segment([xf.to_screen(a), xf.to_screen(b)], Stroke::new(1.0, Color32::from_rgb(0, 200, 83)));
            }
            Overlay::Path { .. } => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rulers(app: &DesignApp, ui: &egui::Ui, full: Rect, rect: Rect, xf: &Xf, doc: &Document, layout: &CanvasLayout, t: &Tokens) {
    let painter = ui.painter_at(full);
    let top = Rect::from_min_max(pos2(rect.min.x, full.min.y), pos2(full.max.x, rect.min.y));
    let left = Rect::from_min_max(pos2(full.min.x, rect.min.y), pos2(rect.min.x, full.max.y));
    painter.rect_filled(top, 0.0, t.ruler);
    painter.rect_filled(left, 0.0, t.ruler);
    painter.rect_filled(Rect::from_min_max(full.min, rect.min), 0.0, t.ruler);
    painter.line_segment([pos2(rect.min.x, rect.min.y - 0.25), pos2(full.max.x, rect.min.y - 0.25)], Stroke::new(0.5, t.ruler_tick));
    painter.line_segment([pos2(rect.min.x - 0.25, rect.min.y), pos2(rect.min.x - 0.25, full.max.y)], Stroke::new(0.5, t.ruler_tick));
    // Zero-point crosshair box.
    let zc = Rect::from_min_max(full.min, rect.min).center();
    painter.line_segment([zc - vec2(4.0, 0.0), zc + vec2(4.0, 0.0)], Stroke::new(1.0, t.ruler_tick));
    painter.line_segment([zc - vec2(0.0, 4.0), zc + vec2(0.0, 4.0)], Stroke::new(1.0, t.ruler_tick));
    // Origin: top-left of the current spread's first page.
    let Some(i) = current_slot(app, layout) else { return };
    let slot = &layout.slots[i];
    let origin = Point::new(slot.bounds.x0, slot.bounds.y0);
    let unit = doc.settings.horizontal_units;
    let (major, sub) = unit.ruler_ticks(xf.zoom);
    let font = egui::FontId::proportional(9.0);
    let tick = Stroke::new(1.0, t.ruler_tick);
    // Horizontal.
    let c0 = xf.to_canvas(top.min).x - origin.x;
    let c1 = xf.to_canvas(pos2(top.max.x, 0.0)).x - origin.x;
    let mut k = (c0 / major).floor() as i64;
    while (k as f64) * major <= c1 {
        let x0 = k as f64 * major;
        for s in 0..sub {
            let x = x0 + major * s as f64 / sub as f64;
            let sx = xf.to_screen(Point::new(origin.x + x, 0.0)).x;
            let len = if s == 0 {
                RULER
            } else if sub % 2 == 0 && s == sub / 2 {
                7.0
            } else {
                4.0
            };
            painter.line_segment([pos2(sx, top.max.y - len), pos2(sx, top.max.y)], tick);
            if s == 0 {
                let v = unit.from_pt(x);
                painter.text(pos2(sx + 2.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, fmt_tick(v), font.clone(), t.ruler_text);
            }
        }
        k += 1;
    }
    // Vertical.
    let c0 = xf.to_canvas(pos2(0.0, left.min.y)).y - origin.y;
    let c1 = xf.to_canvas(pos2(0.0, left.max.y)).y - origin.y;
    let mut k = (c0 / major).floor() as i64;
    while (k as f64) * major <= c1 {
        let y0 = k as f64 * major;
        for s in 0..sub {
            let y = y0 + major * s as f64 / sub as f64;
            let sy = xf.to_screen(Point::new(0.0, origin.y + y)).y;
            let len = if s == 0 {
                RULER
            } else if sub % 2 == 0 && s == sub / 2 {
                7.0
            } else {
                4.0
            };
            painter.line_segment([pos2(left.max.x - len, sy), pos2(left.max.x, sy)], tick);
            if s == 0 {
                // Stacked digits like InDesign's vertical ruler.
                let label = fmt_tick(unit.from_pt(y));
                for (j, ch) in label.chars().enumerate() {
                    painter.text(pos2(left.min.x + 3.0, sy + 2.0 + j as f32 * 8.5), egui::Align2::LEFT_TOP, ch, font.clone(), t.ruler_text);
                }
            }
        }
        k += 1;
    }
    // Pointer position markers.
    if let Some(p) = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p)) {
        let m = Stroke::new(1.0, t.text_dim);
        dashed(&painter, pos2(p.x, top.min.y), pos2(p.x, top.max.y), m, 1.5, 1.5);
        dashed(&painter, pos2(left.min.x, p.y), pos2(left.max.x, p.y), m, 1.5, 1.5);
    }
}

fn fmt_tick(v: f64) -> String {
    let r = v.round();
    if (v - r).abs() < 1e-6 { format!("{}", r as i64) } else { format!("{v:.1}") }
}

fn handle_input(app: &mut DesignApp, ui: &mut egui::Ui, resp: &egui::Response, rect: Rect) {
    let Some(v) = app.view().copied() else { return };
    let xf = Xf::new(rect, &v);
    let wants_text = app.session.wants_text();
    let space = ui.input(|i| i.key_down(egui::Key::Space)) && !wants_text && !ui.ctx().egui_wants_keyboard_input();
    // Scroll / zoom.
    if resp.hovered() {
        let (scroll, zoom_delta, m, hover) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers, i.pointer.hover_pos()));
        if let Some(hp) = hover {
            if zoom_delta != 1.0 {
                zoom_at(app, hp, zoom_delta as f64);
            } else if m.command && scroll.y != 0.0 {
                zoom_at(app, hp, (1.0 + scroll.y as f64 * 0.003).clamp(0.5, 2.0));
            } else if scroll != egui::Vec2::ZERO
                && let Some(v) = app.view_mut()
            {
                v.origin = Point::new(v.origin.x - scroll.x as f64 / v.zoom, v.origin.y - scroll.y as f64 / v.zoom);
            }
        }
    }
    // Space-drag = hand.
    if space && resp.dragged() {
        let d = resp.drag_delta();
        if let Some(v) = app.view_mut() {
            v.origin = Point::new(v.origin.x - d.x as f64 / v.zoom, v.origin.y - d.y as f64 / v.zoom);
        }
        return;
    }
    let m = ui.input(|i| mods(i, space));
    let vi = app.view_info();
    let pos = |p: Pos2| xf.to_canvas(p);
    let mut events: Vec<PointerEvent> = Vec::new();
    let down_id = egui::Id::new("canvas_pointer_down");
    let mut down: bool = ui.data(|d| d.get_temp(down_id)).unwrap_or(false);
    let (pressed, released, origin, latest, dbl) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.pointer.press_origin(),
            i.pointer.latest_pos(),
            i.pointer.button_double_clicked(egui::PointerButton::Primary),
        )
    });
    if pressed
        && !space
        && let Some(o) = origin.filter(|o| rect.contains(*o) && resp.hovered())
    {
        events.push(PointerEvent { kind: if dbl { PointerKind::DoubleClick } else { PointerKind::Down }, pos: pos(o), mods: m });
        down = true;
    }
    if down
        && resp.dragged()
        && resp.drag_delta() != egui::Vec2::ZERO
        && let Some(p) = latest
    {
        events.push(PointerEvent { kind: PointerKind::Drag, pos: pos(p), mods: m });
    }
    if down && released {
        let p = latest.unwrap_or(rect.center());
        events.push(PointerEvent { kind: PointerKind::Up, pos: pos(p), mods: m });
        down = false;
    } else if !down && let Some(p) = resp.hover_pos() {
        events.push(PointerEvent { kind: PointerKind::Move, pos: pos(p), mods: m });
    }
    ui.data_mut(|d| d.insert_temp(down_id, down));
    for e in events {
        if let Err(err) = app.session.pointer(&e, vi) {
            app.status(err.to_string());
        }
    }
    app.after_engine();
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    // Keyboard for the active tool.
    if ui.ctx().egui_wants_keyboard_input() && !resp.has_focus() {
        return;
    }
    let evs = ui.input(|i| i.events.clone());
    for e in evs {
        match e {
            egui::Event::Text(t) if wants_text => {
                let _ = app.run("text.insert", json!({"text": t}));
            }
            egui::Event::Paste(t) if wants_text => {
                let _ = app.run("text.insert", json!({"text": t.replace("\r\n", "\n").replace('\r', "\n")}));
            }
            egui::Event::Copy | egui::Event::Cut if wants_text => {
                if let Some(t) = selected_text(app) {
                    ui.ctx().copy_text(t);
                    if matches!(e, egui::Event::Cut) {
                        let _ = app.run("text.delete", json!({}));
                    }
                }
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                let k = match key {
                    egui::Key::ArrowLeft => Some(ToolKey::Left),
                    egui::Key::ArrowRight => Some(ToolKey::Right),
                    egui::Key::ArrowUp => Some(ToolKey::Up),
                    egui::Key::ArrowDown => Some(ToolKey::Down),
                    egui::Key::Enter => Some(ToolKey::Enter),
                    egui::Key::Escape => Some(ToolKey::Escape),
                    egui::Key::Backspace => Some(ToolKey::Backspace),
                    egui::Key::Delete => Some(ToolKey::Delete),
                    egui::Key::Tab => Some(ToolKey::Tab),
                    egui::Key::Home => Some(ToolKey::Home),
                    egui::Key::End => Some(ToolKey::End),
                    _ => None,
                };
                if let Some(k) = k {
                    let m = Mods { shift: modifiers.shift, alt: modifiers.alt, cmd: modifiers.command, ctrl: modifiers.ctrl, space: false };
                    let handled = app.session.tool_key(k, m, vi).unwrap_or(false);
                    if !handled && k == ToolKey::Escape {
                        let _ = app.run("edit.deselectAll", json!({}));
                    }
                    app.after_engine();
                }
            }
            _ => {}
        }
    }
    let _ = DVec2::ZERO;
}

/// View → Show Hidden Characters: ¶ paragraph ends, » tabs, · spaces, ¬ forced line breaks, # end of story.
fn draw_hidden_characters(app: &DesignApp, painter: &egui::Painter, xf: &Xf, doc: &Document, layout: &CanvasLayout) {
    for story in doc.stories.values() {
        let cs = app.session.cache.get(doc, story.id, None);
        for ft in &cs.frames {
            let (Some((a, _)), Some(it)) = (item_canvas_xf(doc, layout, ft.frame), doc.item(ft.frame)) else { continue };
            let m = a * it.xf;
            let col = layer_color(doc, it);
            for l in &ft.lines {
                let size = (l.ascent * 0.75 * xf.zoom).clamp(6.0, 40.0) as f32;
                let font = egui::FontId::proportional(size);
                for g in &l.glyphs {
                    if g.len == 0 {
                        continue;
                    }
                    let ch = story.text[g.byte.min(story.text.len())..].chars().next().unwrap_or(' ');
                    let mark = match ch {
                        ' ' => "·",
                        '\t' => "»",
                        '\u{2028}' => "¬",
                        '\u{A0}' => "°",
                        _ => continue,
                    };
                    let p = xf.to_screen(m * Point::new(g.x + if ch == ' ' { g.adv / 2.0 } else { 0.0 }, l.baseline));
                    let align = if ch == ' ' { egui::Align2::CENTER_BOTTOM } else { egui::Align2::LEFT_BOTTOM };
                    painter.text(p, align, mark, font.clone(), col);
                }
                if l.last_in_para {
                    let end = l.range.end;
                    let mark = if end >= story.text.len() { "#" } else { "¶" };
                    let p = xf.to_screen(m * Point::new(l.end_x + 1.0, l.baseline));
                    painter.text(p, egui::Align2::LEFT_BOTTOM, mark, font.clone(), col);
                }
            }
        }
    }
}

/// Plain text of the current text selection (story markers resolved to readable characters).
pub fn selected_text(app: &DesignApp) -> Option<String> {
    let st = app.session.active()?;
    let t = st.selection.text?;
    let story = st.doc.story(t.story)?;
    let s = story.slice(t.range());
    if s.is_empty() {
        return None;
    }
    Some(s.replace(designcraft_doc::story::FORCED_LINE_BREAK, "\n"))
}

/// One device pixel, the width InDesign uses for guides and frame edges.
fn hair(painter: &egui::Painter) -> f32 {
    1.0 / painter.ctx().pixels_per_point()
}
