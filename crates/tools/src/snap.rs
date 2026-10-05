//! Snapping and smart guides.
//!
//! [`snap`] tries each pass on a free axis and keeps the first hit inside the snap zone.
//! Ruler guides, margins, and columns draw nothing. Alignment draws [`Overlay::Guide`] in
//! canvas coordinates. [`snap_rect`] is the older closest-target helper the selection tool
//! still calls.

use designcraft_doc::{ItemId, Orientation, SpreadRef};
use designcraft_geom::{Point, Rect};

pub use crate::{Gesture, Snap, SnapRequest, SnapView};

use crate::{Overlay, ToolContext};

#[derive(Clone, Copy)]
struct Target {
    v: f64,
    /// Extent of the target along the other axis (for drawing the guide).
    lo: f64,
    hi: f64,
    smart: bool,
}

fn targets(cx: &ToolContext, sr: SpreadRef, exclude: &[ItemId]) -> (Vec<Target>, Vec<Target>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let Some(sp) = cx.doc.spread(sr) else { return (xs, ys) };
    let push = |v: &mut Vec<Target>, val: f64, lo: f64, hi: f64, smart: bool| v.push(Target { v: val, lo, hi, smart });
    for p in &sp.pages {
        let b = p.bounds();
        let m = p.margin_rect();
        for x in [b.x0, b.center().x, b.x1, m.x0, m.x1] {
            push(&mut xs, x, b.y0, b.y1, false);
        }
        for y in [b.y0, b.center().y, b.y1, m.y0, m.y1] {
            push(&mut ys, y, b.x0, b.x1, false);
        }
        for c in p.column_rects() {
            push(&mut xs, c.x0, b.y0, b.y1, false);
            push(&mut xs, c.x1, b.y0, b.y1, false);
        }
        for g in &p.guides {
            match g.orientation {
                Orientation::Vertical => push(&mut xs, g.position, b.y0, b.y1, false),
                Orientation::Horizontal => push(&mut ys, g.position, b.x0, b.x1, false),
            }
        }
    }
    for it in &sp.items {
        if exclude.contains(&it.id) || it.hidden {
            continue;
        }
        let b = it.bounds();
        for x in [b.x0, b.center().x, b.x1] {
            push(&mut xs, x, b.y0, b.y1, true);
        }
        for y in [b.y0, b.center().y, b.y1] {
            push(&mut ys, y, b.x0, b.x1, true);
        }
    }
    (xs, ys)
}

fn best(cands: &[f64], targets: &[Target], tol: f64) -> Option<(f64, Target)> {
    let mut out: Option<(f64, Target)> = None;
    for &c in cands {
        for t in targets {
            let d = t.v - c;
            if d.abs() <= tol && out.is_none_or(|(od, _)| d.abs() < od.abs()) {
                out = Some((d, *t));
            }
        }
    }
    out
}

/// Closest target on each free axis. Tolerance is 5 screen pixels, same as [`snap_rect`].
fn snap_axes(cx: &ToolContext, sr: SpreadRef, r: Rect, exclude: &[ItemId], x_free: bool, y_free: bool) -> Snap {
    let tol = cx.tol(5.0);
    let (xs, ys) = targets(cx, sr, exclude);
    let mut s = Snap::default();
    let xf = cx.layout.xf(sr);
    if x_free && let Some((d, t)) = best(&[r.x0, r.center().x, r.x1], &xs, tol) {
        s.delta.x = d;
        let lo = t.lo.min(r.y0) - 6.0;
        let hi = t.hi.max(r.y1) + 6.0;
        s.guides.push(guide(xf * Point::new(t.v, lo), xf * Point::new(t.v, hi), t.smart));
    }
    if y_free && let Some((d, t)) = best(&[r.y0, r.center().y, r.y1], &ys, tol) {
        s.delta.y = d;
        let lo = t.lo.min(r.x0) - 6.0;
        let hi = t.hi.max(r.x1) + 6.0;
        s.guides.push(guide(xf * Point::new(lo, t.v), xf * Point::new(hi, t.v), t.smart));
    }
    s
}

/// Snap a moving rect (spread coords). Edges and centre snap independently per axis.
pub fn snap_rect(cx: &ToolContext, sr: SpreadRef, r: Rect, exclude: &[ItemId]) -> Snap {
    snap_axes(cx, sr, r, exclude, true, true)
}

/// Snap a single point (drawing tools).
///
/// Zero-size rect, and every edge flag on, so the point can meet a guide or an alignment target.
pub fn snap_point(cx: &ToolContext, sr: SpreadRef, p: Point) -> (Point, Vec<Overlay>) {
    let s = snap(
        cx,
        SnapRequest {
            spread: sr,
            gesture: Gesture::Point,
            rect: Rect::from_points(p, p),
            x_edges: [true, true, true],
            y_edges: [true, true, true],
            exclude: &[],
            copying: false,
            lengths: [None, None],
            angle: None,
            radius: 0.0,
            pointer: p,
        },
    );
    (p + s.delta, s.guides)
}

fn rect_finite(r: Rect) -> bool {
    r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()
}

/// First hit on each free axis.
///
/// A non-finite rect, or no snap category, returns no correction. An axis whose three edge
/// flags are all false stays at 0 and draws nothing. The zone is [`SnapView::zone_px`] screen
/// pixels, converted with [`ToolContext::tol`]. A zone of 0 or less is no hit.
pub fn snap(cx: &ToolContext, req: SnapRequest<'_>) -> Snap {
    if !rect_finite(req.rect) || !cx.snap.any() {
        return Snap::default();
    }
    let Some(tol) = zone_tol(cx) else {
        return Snap::default();
    };
    // Three false flags lock the axis: do not move it and do not draw a guide for it.
    let x_free = req.x_edges != [false; 3];
    let y_free = req.y_edges != [false; 3];
    let x_hit = x_free.then(|| first_on_axis(cx, &req, Axis::X, tol)).flatten();
    let y_hit = y_free.then(|| first_on_axis(cx, &req, Axis::Y, tol)).flatten();
    let mut out = Snap::default();
    if let Some(hit) = &x_hit {
        out.delta.x = hit.delta;
    }
    if let Some(hit) = &y_hit {
        out.delta.y = hit.delta;
    }
    let placed = Rect::new(req.rect.x0 + out.delta.x, req.rect.y0 + out.delta.y, req.rect.x1 + out.delta.x, req.rect.y1 + out.delta.y);
    if let Some(hit) = &x_hit {
        push_alignment_guide(&mut out.guides, cx, req.spread, Axis::X, hit, placed);
    }
    if let Some(hit) = &y_hit {
        push_alignment_guide(&mut out.guides, cx, req.spread, Axis::Y, hit, placed);
    }
    out
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy)]
struct AxisHit {
    delta: f64,
    /// Position of the winning line on this axis.
    at: f64,
    /// Target box for an alignment guide. None when the hit draws nothing.
    span: Option<Rect>,
}

struct Best {
    dist: f64,
    hit: AxisHit,
}

/// Screen pixels to spread points. A zone of 0 or less, or a non-finite zone, is no hit.
fn zone_tol(cx: &ToolContext) -> Option<f64> {
    let zone = cx.snap.zone_px;
    if !zone.is_finite() || zone <= 0.0 {
        return None;
    }
    let tol = cx.tol(zone);
    if tol.is_finite() && tol > 0.0 { Some(tol) } else { None }
}

fn first_on_axis(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis, tol: f64) -> Option<AxisHit> {
    pass_grid(cx, req, axis, tol)
        .or_else(|| pass_baseline(cx, req, axis, tol))
        .or_else(|| pass_guides(cx, req, axis, tol))
        .or_else(|| pass_align(cx, req, axis, tol))
        .or_else(|| pass_spacing(cx, req, axis, tol))
        .or_else(|| pass_dimensions(cx, req, axis, tol))
}

fn pass_grid(_cx: &ToolContext, _req: &SnapRequest<'_>, _axis: Axis, _tol: f64) -> Option<AxisHit> {
    None
}

fn pass_baseline(_cx: &ToolContext, _req: &SnapRequest<'_>, _axis: Axis, _tol: f64) -> Option<AxisHit> {
    None
}

fn pass_spacing(_cx: &ToolContext, _req: &SnapRequest<'_>, _axis: Axis, _tol: f64) -> Option<AxisHit> {
    None
}

fn pass_dimensions(_cx: &ToolContext, _req: &SnapRequest<'_>, _axis: Axis, _tol: f64) -> Option<AxisHit> {
    None
}

/// Ruler guides, margins, and column sides. No overlay.
///
/// A page guide attracts when the moving rect overlaps that page on the other axis. A spread
/// guide uses the spread bounds. Locked guides are included. A guide is a target only when
/// `Guide::visible_in` accepts it.
fn pass_guides(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis, tol: f64) -> Option<AxisHit> {
    if !cx.snap.snap_to_guides || !cx.snap.show_guides {
        return None;
    }
    let sp = cx.doc.spread(req.spread)?;
    let spread_bounds = sp.bounds();
    let flags = axis_flags(req, axis);
    let (moving_lo, moving_hi) = perp_ends(req.rect, axis);
    let mut best: Option<Best> = None;
    for page in &sp.pages {
        let page_bounds = page.bounds();
        for guide in &page.guides {
            if !guide.visible_in(cx.doc) || !guide.position.is_finite() || guide.orientation != axis_orientation(axis) {
                continue;
            }
            let range = if guide.spread { spread_bounds } else { page_bounds };
            let (range_lo, range_hi) = perp_ends(range, axis);
            if !ranges_overlap(moving_lo, moving_hi, range_lo, range_hi) {
                continue;
            }
            offer_line(&mut best, req.rect, flags, axis, guide.position, tol);
        }
        let margin = page.margin_rect();
        let (a, b) = match axis {
            Axis::X => (margin.x0, margin.x1),
            Axis::Y => (margin.y0, margin.y1),
        };
        offer_line(&mut best, req.rect, flags, axis, a, tol);
        offer_line(&mut best, req.rect, flags, axis, b, tol);
        if matches!(axis, Axis::X) {
            for column in page.column_rects() {
                offer_line(&mut best, req.rect, flags, axis, column.x0, tol);
                offer_line(&mut best, req.rect, flags, axis, column.x1, tol);
            }
        }
    }
    best.map(|b| b.hit)
}

/// Page and top-level item edges and centers. Draws one [`Overlay::Guide`] per winning axis.
///
/// Edges match edges only while `align_edges` is on. Centers match centers only while
/// `align_centers` is on, and only when the request's middle flag is set. Hidden and excluded
/// items are skipped. Parent items and group children are not targets.
fn pass_align(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis, tol: f64) -> Option<AxisHit> {
    if !cx.snap.smart_guides || (!cx.snap.align_edges && !cx.snap.align_centers) {
        return None;
    }
    let sp = cx.doc.spread(req.spread)?;
    let flags = axis_flags(req, axis);
    let mut best: Option<Best> = None;
    for page in &sp.pages {
        offer_box(&mut best, req.rect, flags, axis, page.bounds(), tol, cx.snap.align_edges, cx.snap.align_centers);
    }
    for item in &sp.items {
        if req.exclude.contains(&item.id) || item.hidden {
            continue;
        }
        offer_box(&mut best, req.rect, flags, axis, item.visible_bounds(), tol, cx.snap.align_edges, cx.snap.align_centers);
    }
    best.map(|b| b.hit)
}

fn axis_flags(req: &SnapRequest<'_>, axis: Axis) -> [bool; 3] {
    match axis {
        Axis::X => req.x_edges,
        Axis::Y => req.y_edges,
    }
}

fn axis_orientation(axis: Axis) -> Orientation {
    match axis {
        Axis::X => Orientation::Vertical,
        Axis::Y => Orientation::Horizontal,
    }
}

/// Left/center/right, or top/center/bottom.
fn axis_triple(r: Rect, axis: Axis) -> (f64, f64, f64) {
    match axis {
        Axis::X => (r.x0, r.center().x, r.x1),
        Axis::Y => (r.y0, r.center().y, r.y1),
    }
}

/// The rect's extent on the axis perpendicular to `axis`.
fn perp_ends(r: Rect, axis: Axis) -> (f64, f64) {
    match axis {
        Axis::X => (r.y0, r.y1),
        Axis::Y => (r.x0, r.x1),
    }
}

fn ranges_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> bool {
    if !a0.is_finite() || !a1.is_finite() || !b0.is_finite() || !b1.is_finite() {
        return false;
    }
    let (a_lo, a_hi) = (a0.min(a1), a0.max(a1));
    let (b_lo, b_hi) = (b0.min(b1), b0.max(b1));
    a_lo <= b_hi && a_hi >= b_lo
}

/// Any requested edge, including the center, against one guide line. No span, so nothing is drawn.
fn offer_line(best: &mut Option<Best>, moving: Rect, flags: [bool; 3], axis: Axis, target: f64, tol: f64) {
    let (left, mid, right) = axis_triple(moving, axis);
    let [use_left, use_mid, use_right] = flags;
    if use_left {
        offer(best, left, target, tol, None);
    }
    if use_mid {
        offer(best, mid, target, tol, None);
    }
    if use_right {
        offer(best, right, target, tol, None);
    }
}

fn offer_box(best: &mut Option<Best>, moving: Rect, flags: [bool; 3], axis: Axis, target: Rect, tol: f64, edges: bool, centers: bool) {
    let (m0, m_mid, m1) = axis_triple(moving, axis);
    let (t0, t_mid, t1) = axis_triple(target, axis);
    let [use_left, use_mid, use_right] = flags;
    if edges {
        if use_left {
            offer(best, m0, t0, tol, Some(target));
            offer(best, m0, t1, tol, Some(target));
        }
        if use_right {
            offer(best, m1, t0, tol, Some(target));
            offer(best, m1, t1, tol, Some(target));
        }
    }
    if centers && use_mid {
        offer(best, m_mid, t_mid, tol, Some(target));
    }
}

fn offer(best: &mut Option<Best>, moving: f64, target: f64, tol: f64, span: Option<Rect>) {
    if !moving.is_finite() || !target.is_finite() {
        return;
    }
    let delta = target - moving;
    let dist = delta.abs();
    if dist <= tol && best.as_ref().is_none_or(|b| dist < b.dist) {
        *best = Some(Best { dist, hit: AxisHit { delta, at: target, span } });
    }
}

fn push_alignment_guide(out: &mut Vec<Overlay>, cx: &ToolContext, spread: SpreadRef, axis: Axis, hit: &AxisHit, moving: Rect) {
    let Some(target) = hit.span else { return };
    let xf = cx.layout.xf(spread);
    let (a, b) = match axis {
        Axis::X => {
            let lo = moving.y0.min(moving.y1).min(target.y0.min(target.y1)) - 6.0;
            let hi = moving.y0.max(moving.y1).max(target.y0.max(target.y1)) + 6.0;
            (xf * Point::new(hit.at, lo), xf * Point::new(hit.at, hi))
        }
        Axis::Y => {
            let lo = moving.x0.min(moving.x1).min(target.x0.min(target.x1)) - 6.0;
            let hi = moving.x0.max(moving.x1).max(target.x0.max(target.x1)) + 6.0;
            (xf * Point::new(lo, hit.at), xf * Point::new(hi, hit.at))
        }
    };
    out.push(Overlay::Guide { a, b });
}

fn guide(a: Point, b: Point, smart: bool) -> Overlay {
    if smart { Overlay::Guide { a, b } } else { Overlay::Line { a, b, color: [255, 0, 255], dashed: false } }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use designcraft_compose::Cache;
    use designcraft_doc::build::NewDocument;
    use designcraft_doc::{Document, Guide, Item, Selection, Shape};
    use designcraft_geom::Unit;
    use designcraft_geom::shapes;

    use crate::layout::CanvasLayout;

    use super::*;

    // The factory flags are const. Pin them anyway.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn factory_zone_is_four_pixels() {
        assert!(SnapView::FACTORY.snap_to_guides);
        assert!(!SnapView::FACTORY.snap_to_document_grid);
        assert!(SnapView::FACTORY.smart_guides);
        assert_eq!(SnapView::FACTORY.zone_px, 4.0);
        assert!(!SnapView::OFF.any());
        assert!(SnapView::FACTORY.any());
    }

    fn doc_with_items(moving: Rect, other: Rect, guide_x: f64) -> (Document, ItemId, ItemId) {
        let mut doc = Document::new(&NewDocument::default());
        let layer = doc.default_layer();
        let make = |doc: &mut Document, rect: Rect| {
            let id = ItemId(doc.alloc());
            let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(rect));
            doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
            id
        };
        let moving_id = make(&mut doc, moving);
        let other_id = make(&mut doc, other);
        let page = &mut Arc::make_mut(&mut doc.spreads[0]).pages[0];
        page.guides.push(Guide { orientation: Orientation::Vertical, position: guide_x, spread: false, locked: false, layer: None, liquid: false });
        (doc, moving_id, other_id)
    }

    fn ctx_on<'a>(doc: &'a Document, sel: &'a Selection, cache: &'a Cache, layout: &'a CanvasLayout) -> ToolContext<'a> {
        ToolContext { doc, selection: sel, cache, layout, zoom: 1.0, layer: doc.default_layer(), snap: SnapView::FACTORY, unit: Unit::Points }
    }

    #[test]
    fn guide_beats_a_closer_object_edge() {
        // Right edge at 100.5. Object edge at 100 (0.5 away). Guide at 104 (3.5 away). Zone is 4.
        let (doc, moving, _other) = doc_with_items(Rect::new(80.0, 100.0, 100.5, 140.0), Rect::new(90.0, 100.0, 100.0, 160.0), 104.0);
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::default();
        let mut cx = ctx_on(&doc, &sel, &cache, &layout);
        cx.snap.zone_px = 4.0;
        let hit = snap(
            &cx,
            SnapRequest {
                spread: SpreadRef::Doc(0),
                gesture: Gesture::Move,
                rect: Rect::new(80.0, 100.0, 100.5, 140.0),
                x_edges: [true, true, true],
                y_edges: [false, false, false],
                exclude: &[moving],
                copying: false,
                lengths: [None, None],
                angle: None,
                radius: 0.0,
                pointer: Point::new(0.0, 0.0),
            },
        );
        assert!((hit.delta.x - 3.5).abs() < 1e-6);
        assert!(hit.guides.is_empty());
    }

    #[test]
    fn object_edge_snaps_when_guide_snap_is_off() {
        let (doc, moving, _other) = doc_with_items(Rect::new(80.0, 100.0, 100.5, 140.0), Rect::new(90.0, 100.0, 100.0, 160.0), 104.0);
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::default();
        let mut cx = ctx_on(&doc, &sel, &cache, &layout);
        cx.snap.snap_to_guides = false;
        let hit = snap(
            &cx,
            SnapRequest {
                spread: SpreadRef::Doc(0),
                gesture: Gesture::Move,
                rect: Rect::new(80.0, 100.0, 100.5, 140.0),
                x_edges: [true, true, true],
                y_edges: [false, false, false],
                exclude: &[moving],
                copying: false,
                lengths: [None, None],
                angle: None,
                radius: 0.0,
                pointer: Point::new(0.0, 0.0),
            },
        );
        assert!((hit.delta.x - -0.5).abs() < 1e-6);
        assert!(matches!(hit.guides.first(), Some(Overlay::Guide { .. })));
    }

    #[test]
    fn hidden_layer_guide_is_not_a_target() {
        let (mut doc, moving, _) = doc_with_items(Rect::new(80.0, 100.0, 102.0, 140.0), Rect::new(300.0, 100.0, 340.0, 140.0), 104.0);
        let layer = doc.default_layer();
        doc.layers.iter_mut().find(|l| l.id == layer).unwrap().visible = false;
        Arc::make_mut(&mut doc.spreads[0]).pages[0].guides[0].layer = Some(layer);
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::default();
        let cx = ctx_on(&doc, &sel, &cache, &layout);
        let hit = snap(
            &cx,
            SnapRequest {
                spread: SpreadRef::Doc(0),
                gesture: Gesture::Move,
                rect: Rect::new(80.0, 100.0, 102.0, 140.0),
                x_edges: [false, false, true],
                y_edges: [false, false, false],
                exclude: &[moving],
                copying: false,
                lengths: [None, None],
                angle: None,
                radius: 0.0,
                pointer: Point::new(0.0, 0.0),
            },
        );
        assert_eq!(hit.delta.x, 0.0);
    }
}
