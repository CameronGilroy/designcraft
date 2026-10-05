//! Snapping and smart guides.
//!
//! [`snap`] tries each pass on a free axis and keeps the first hit inside the snap zone.
//! Ruler guides, margins, and columns draw nothing. Alignment draws [`Overlay::Guide`] in
//! canvas coordinates. [`snap_rect`] is the older closest-target helper.

use std::collections::HashSet;

use designcraft_doc::{Item, ItemId, Orientation, PageSide, Selection, SpreadId, SpreadRef};
use designcraft_geom::snap::snap_to_grid;
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

/// Document grid. No overlay.
///
/// The interval is the major spacing divided by the subdivision count. A count below 1 counts
/// as 1. `grid.vertical` spaces the vertical lines (x). `grid.horizontal` spaces the horizontal
/// lines (y). A spacing of 0 or less skips that axis. The origin is the page-bounds origin of
/// the page under the rect center. Whether the grid is shown is not read.
fn pass_grid(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis, tol: f64) -> Option<AxisHit> {
    if !cx.snap.snap_to_document_grid {
        return None;
    }
    let spacing = grid_interval(cx, axis)?;
    let origin = grid_origin(cx, req, axis)?;
    let mut best: Option<Best> = None;
    offer_enabled(&mut best, req.rect, axis_flags(req, axis), axis, tol, |v| Some(snap_to_grid(v, spacing, origin)));
    best.map(|b| b.hit)
}

/// Major spacing divided by the subdivision count. Non-finite or non-positive spacing is no hit.
fn grid_interval(cx: &ToolContext, axis: Axis) -> Option<f64> {
    let grid = &cx.doc.settings.grid;
    let major = match axis {
        Axis::X => grid.vertical,
        Axis::Y => grid.horizontal,
    };
    if !major.is_finite() || major <= 0.0 {
        return None;
    }
    let interval = major / f64::from(grid.subdivisions.max(1));
    (interval.is_finite() && interval > 0.0).then_some(interval)
}

/// Page-bounds origin on `axis` for the page under the rect center.
fn grid_origin(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis) -> Option<f64> {
    let spread = cx.doc.spread(req.spread)?;
    let bounds = spread.pages.get(spread.page_at_x(req.rect.center().x)?)?.bounds();
    let origin = match axis {
        Axis::X => bounds.x0,
        Axis::Y => bounds.y0,
    };
    origin.is_finite().then_some(origin)
}

/// Baseline grid. No overlay.
///
/// Lines are `page.y0 + start + n * increment` for n >= 0 while the line is inside the page
/// (the bottom edge is not a line, matching the canvas). An increment of 0 or less skips the
/// pass. `relative_to` and `view_threshold` are not read, and hiding the baseline grid does not
/// turn this off. Only a page the rect crosses on x contributes.
fn pass_baseline(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis, tol: f64) -> Option<AxisHit> {
    if !matches!(axis, Axis::Y) || !cx.snap.snap_to_guides {
        return None;
    }
    let baseline = &cx.doc.settings.baseline_grid;
    let increment = baseline.increment;
    if !increment.is_finite() || increment <= 0.0 || !baseline.start.is_finite() {
        return None;
    }
    let spread = cx.doc.spread(req.spread)?;
    let flags = axis_flags(req, axis);
    let (moving_lo, moving_hi) = perp_ends(req.rect, axis);
    let mut best: Option<Best> = None;
    for page in &spread.pages {
        let bounds = page.bounds();
        let (lo, hi) = perp_ends(bounds, axis);
        if !ranges_overlap(moving_lo, moving_hi, lo, hi) {
            continue;
        }
        let origin = bounds.y0 + baseline.start;
        offer_enabled(&mut best, req.rect, flags, axis, tol, |v| nearest_baseline(v, origin, increment, bounds.y0, bounds.y1));
    }
    best.map(|b| b.hit)
}

/// Nearest baseline at or after `origin`, strictly inside `[y0, y1)`.
fn nearest_baseline(v: f64, origin: f64, increment: f64, y0: f64, y1: f64) -> Option<f64> {
    let (n_min, n_max) = baseline_n_range(origin, increment, y0, y1)?;
    let snapped = snap_to_grid(v, increment, origin);
    if baseline_line_ok(snapped, origin, increment, y0, y1) {
        return Some(snapped);
    }
    let n = ((snapped - origin) / increment).round();
    if !n.is_finite() {
        return None;
    }
    let n = if n < n_min {
        n_min
    } else if n > n_max {
        n_max
    } else {
        n
    };
    let line = origin + n * increment;
    baseline_line_ok(line, origin, increment, y0, y1).then_some(line)
}

fn baseline_n_range(origin: f64, increment: f64, y0: f64, y1: f64) -> Option<(f64, f64)> {
    if !origin.is_finite() || !increment.is_finite() || increment <= 0.0 || !y0.is_finite() || !y1.is_finite() || y0 >= y1 {
        return None;
    }
    let mut n_min = if origin < y0 { ((y0 - origin) / increment).ceil() } else { 0.0 };
    if !n_min.is_finite() || n_min < 0.0 {
        return None;
    }
    if origin + n_min * increment < y0 {
        n_min += 1.0;
    }
    let mut n_max = ((y1 - origin) / increment).floor();
    if !n_max.is_finite() {
        return None;
    }
    if origin + n_max * increment >= y1 {
        n_max -= 1.0;
    }
    (n_max >= n_min).then_some((n_min, n_max))
}

fn baseline_line_ok(line: f64, origin: f64, increment: f64, y0: f64, y1: f64) -> bool {
    if !line.is_finite() || !increment.is_finite() || increment <= 0.0 || line < y0 || line >= y1 {
        return false;
    }
    let n = ((line - origin) / increment).round();
    n.is_finite() && n >= 0.0
}

/// Each edge whose flag is set, against its own target. Nothing is drawn.
fn offer_enabled(best: &mut Option<Best>, moving: Rect, flags: [bool; 3], axis: Axis, tol: f64, mut target_at: impl FnMut(f64) -> Option<f64>) {
    let (left, mid, right) = axis_triple(moving, axis);
    let [use_left, use_mid, use_right] = flags;
    for (on, v) in [(use_left, left), (use_mid, mid), (use_right, right)] {
        if !on {
            continue;
        }
        let Some(target) = target_at(v) else { continue };
        offer(best, v, target, tol, None);
    }
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

/// How deep a group may nest before alignment stops walking it.
const ALIGN_GROUP_DEPTH: usize = 32;

/// Page and item edges and centers. Draws one [`Overlay::Guide`] per winning axis.
///
/// Edges match edges only while `align_edges` is on. Centers match centers only while
/// `align_centers` is on, and only when the request's middle flag is set. Hidden items are
/// skipped. Without copying, excluded items are skipped. With copying, that list is the
/// drag-start set and stays a target. Live selection items outside that set are preview
/// copies and are skipped. A group with children uses the union of those children's visible
/// bounds in group space, then the group's transform. It does not use the group's own stroke.
/// Parent items are added for a document page that shows them, in document spread space.
/// A layout built for parent editing already has those items on the spread, so they are not
/// added again.
fn pass_align(cx: &ToolContext, req: &SnapRequest<'_>, axis: Axis, tol: f64) -> Option<AxisHit> {
    if !cx.snap.smart_guides || (!cx.snap.align_edges && !cx.snap.align_centers) {
        return None;
    }
    let flags = axis_flags(req, axis);
    let mut best: Option<Best> = None;
    if let Some(sp) = cx.doc.spread(req.spread) {
        for page in &sp.pages {
            offer_box(&mut best, req.rect, flags, axis, page.bounds(), tol, cx.snap.align_edges, cx.snap.align_centers);
        }
        for item in &sp.items {
            let Some(rect) = item_target_rect(cx.selection, req, item) else { continue };
            offer_box(&mut best, req.rect, flags, axis, rect, tol, cx.snap.align_edges, cx.snap.align_centers);
        }
    }
    if !layout_edits_parents(cx) {
        offer_parent_items(&mut best, cx, req, flags, axis, tol);
    }
    best.map(|b| b.hit)
}

/// Drag-start ids stay targets while copying. Preview copies are the live selection minus that set.
fn align_skips_item(selection: &Selection, req: &SnapRequest<'_>, id: ItemId) -> bool {
    if req.copying { selection.items.contains(&id) && !req.exclude.contains(&id) } else { req.exclude.contains(&id) }
}

/// Parent spreads are the canvas while a parent is being edited. Their items are ordinary targets.
fn layout_edits_parents(cx: &ToolContext) -> bool {
    cx.layout.slots.iter().any(|slot| matches!(slot.spread, SpreadRef::Parent(_)))
}

/// Visible box used for alignment. Groups with children contribute the union of the children.
fn item_target_rect(selection: &Selection, req: &SnapRequest<'_>, item: &Item) -> Option<Rect> {
    if item.hidden || align_skips_item(selection, req, item.id) {
        return None;
    }
    if item.is_group() && !item.children().is_empty() {
        let mut seen = HashSet::new();
        return group_target_rect(selection, req, item, &mut seen, 0);
    }
    let bounds = item.visible_bounds();
    rect_finite(bounds).then_some(bounds)
}

/// Union of children's visible bounds in this group's space, then `item.xf`.
fn group_target_rect(selection: &Selection, req: &SnapRequest<'_>, item: &Item, seen: &mut HashSet<ItemId>, depth: usize) -> Option<Rect> {
    if depth >= ALIGN_GROUP_DEPTH || !seen.insert(item.id) {
        return None;
    }
    let mut acc: Option<Rect> = None;
    for child in item.children() {
        if child.hidden || align_skips_item(selection, req, child.id) {
            continue;
        }
        let local = if child.is_group() && !child.children().is_empty() {
            group_target_rect(selection, req, child, seen, depth + 1)
        } else {
            let bounds = child.visible_bounds();
            rect_finite(bounds).then_some(bounds)
        };
        let Some(local) = local else { continue };
        if !rect_finite(local) {
            continue;
        }
        acc = Some(acc.map_or(local, |have| have.union(local)));
    }
    acc.map(|r| item.xf.transform_rect_bbox(r)).filter(|r| rect_finite(*r))
}

/// Parent items shown on the document spread, shifted into that spread's space.
fn offer_parent_items(best: &mut Option<Best>, cx: &ToolContext, req: &SnapRequest<'_>, flags: [bool; 3], axis: Axis, tol: f64) {
    let SpreadRef::Doc(si) = req.spread else { return };
    let page_count = cx.doc.spreads.get(si).map(|sp| sp.pages.len()).unwrap_or(0);
    let first = cx.doc.first_page_of_spread(si);
    for pi in 0..page_count {
        let Some(abs) = first.checked_add(pi) else { continue };
        let Some(page) = cx.doc.page(abs) else { continue };
        if !page.show_parent_items {
            continue;
        }
        let doc_x = page.x;
        let doc_side = page.side;
        let overridden = page.overridden.clone();
        let Some((ppi, _)) = cx.doc.parent_page_for(abs) else { continue };
        let mut seen = HashSet::new();
        offer_parent_chain(best, cx, req, flags, axis, tol, doc_x, doc_side, &overridden, ppi, &mut seen);
    }
}

/// Walk `based_on`. A repeated spread id ends the walk.
fn offer_parent_chain(
    best: &mut Option<Best>,
    cx: &ToolContext,
    req: &SnapRequest<'_>,
    flags: [bool; 3],
    axis: Axis,
    tol: f64,
    doc_x: f64,
    doc_side: PageSide,
    overridden: &[ItemId],
    index: usize,
    seen: &mut HashSet<SpreadId>,
) {
    let Some(parent) = cx.doc.parents.get(index) else { return };
    if !seen.insert(parent.id) {
        return;
    }
    let next = parent.parent.as_ref().and_then(|info| info.based_on);
    if let Some(page_idx) = shown_parent_page_index(parent.pages.len(), doc_side)
        && let Some(parent_page) = parent.pages.get(page_idx)
    {
        let dx = doc_x - parent_page.x;
        if dx.is_finite() {
            for item in &parent.items {
                if overridden.contains(&item.id) {
                    continue;
                }
                // Same other-page skip as the renderer: a facing parent only shows items
                // whose center sits on the parent page this document page uses.
                if parent.pages.len() > 1 && parent.page_at_x(item.bounds().center().x) != Some(page_idx) {
                    continue;
                }
                let Some(rect) = item_target_rect(cx.selection, req, item) else { continue };
                offer_box(best, req.rect, flags, axis, shift_x(rect, dx), tol, cx.snap.align_edges, cx.snap.align_centers);
            }
        }
    }
    let Some(next_id) = next else { return };
    let Some(next_index) = cx.doc.parent_index(next_id) else { return };
    offer_parent_chain(best, cx, req, flags, axis, tol, doc_x, doc_side, overridden, next_index, seen);
}

/// Left page of a facing parent, otherwise the last page. Same choice as `Document::parent_page_for`.
fn shown_parent_page_index(page_count: usize, side: PageSide) -> Option<usize> {
    if page_count >= 2 && side == PageSide::Left { Some(0) } else { page_count.checked_sub(1) }
}

fn shift_x(r: Rect, dx: f64) -> Rect {
    Rect::new(r.x0 + dx, r.y0, r.x1 + dx, r.y1)
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
    use designcraft_doc::{Document, Guide, Item, Margins, Selection, Shape, Stroke};
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

    fn y_req(rect: Rect) -> SnapRequest<'static> {
        SnapRequest {
            spread: SpreadRef::Doc(0),
            gesture: Gesture::Move,
            rect,
            x_edges: [false, false, false],
            y_edges: [true, true, true],
            exclude: &[],
            copying: false,
            lengths: [None, None],
            angle: None,
            radius: 0.0,
            pointer: Point::new(0.0, 0.0),
        }
    }

    fn move_req<'a>(rect: Rect, exclude: &'a [ItemId]) -> SnapRequest<'a> {
        SnapRequest {
            spread: SpreadRef::Doc(0),
            gesture: Gesture::Move,
            rect,
            x_edges: [true, true, true],
            y_edges: [false, false, false],
            exclude,
            copying: false,
            lengths: [None, None],
            angle: None,
            radius: 0.0,
            pointer: Point::new(0.0, 0.0),
        }
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
    fn parent_centered_stroke_attracts_at_the_outer_edge() {
        let mut doc = Document::new(&NewDocument::default());
        let pid = doc.add_parent("A", "A-Parent", 1, 12.0, Margins::uniform(36.0));
        doc.apply_parent(&[0], Some(pid)).unwrap();
        let layer = doc.default_layer();
        let id = ItemId(doc.alloc());
        let (ppi, ppg) = doc.parent_page_for(0).unwrap();
        let parent_x = doc.parents[ppi].pages[ppg].x;
        let mut item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(parent_x + 72.0, 140.0, parent_x + 252.0, 300.0)));
        item.stroke = Stroke::default(); // 1 pt, center
        doc.insert_item(SpreadRef::Parent(ppi), item, None).unwrap();
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::default();
        let mut cx = ctx_on(&doc, &sel, &cache, &layout);
        cx.snap.snap_to_guides = false;
        // Moving left edge at 253. Visible parent right edge is 252.5.
        let hit = snap(
            &cx,
            SnapRequest {
                spread: SpreadRef::Doc(0),
                gesture: Gesture::Move,
                rect: Rect::new(253.0, 140.0, 353.0, 240.0),
                x_edges: [true, false, false],
                y_edges: [false, false, false],
                exclude: &[],
                copying: false,
                lengths: [None, None],
                angle: None,
                radius: 0.0,
                pointer: Point::new(0.0, 0.0),
            },
        );
        assert!((hit.delta.x - -0.5).abs() < 1e-6, "delta {}", hit.delta.x);
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

    #[test]
    fn alt_copy_snaps_to_the_original() {
        let mut doc = Document::new(&NewDocument::default());
        let layer = doc.default_layer();
        let id = ItemId(doc.alloc());
        let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(100.0, 40.0, 160.0, 80.0)));
        doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
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
                rect: Rect::new(162.0, 40.0, 222.0, 80.0),
                x_edges: [true, false, false],
                y_edges: [false, false, false],
                exclude: &[id],
                copying: true,
                lengths: [None, None],
                angle: None,
                radius: 0.0,
                pointer: Point::new(0.0, 0.0),
            },
        );
        assert!((hit.delta.x - -2.0).abs() < 1e-6);
    }

    #[test]
    fn alt_copy_does_not_stick_to_the_preview() {
        let mut doc = Document::new(&NewDocument::default());
        let layer = doc.default_layer();
        let make = |doc: &mut Document, rect: Rect| {
            let id = ItemId(doc.alloc());
            let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(rect));
            doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
            id
        };
        let original = make(&mut doc, Rect::new(100.0, 40.0, 160.0, 80.0));
        let ghost = make(&mut doc, Rect::new(161.0, 40.0, 221.0, 80.0));
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::items(vec![ghost]);
        let mut cx = ctx_on(&doc, &sel, &cache, &layout);
        cx.snap.snap_to_guides = false;
        let hit = snap(
            &cx,
            SnapRequest {
                spread: SpreadRef::Doc(0),
                gesture: Gesture::Move,
                rect: Rect::new(161.0, 40.0, 221.0, 80.0),
                x_edges: [true, false, false],
                y_edges: [false, false, false],
                exclude: &[original],
                copying: true,
                lengths: [None, None],
                angle: None,
                radius: 0.0,
                pointer: Point::new(0.0, 0.0),
            },
        );
        assert!((hit.delta.x - -1.0).abs() < 1e-6, "delta.x = {}", hit.delta.x);
    }

    #[test]
    fn grid_beats_a_closer_guide() {
        let mut doc = Document::new(&NewDocument::default());
        doc.settings.grid.horizontal = 72.0;
        doc.settings.grid.vertical = 72.0;
        doc.settings.grid.subdivisions = 8; // 9 pt
        let layer = doc.default_layer();
        let id = ItemId(doc.alloc());
        let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(10.0, 10.0, 40.0, 40.0)));
        doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
        // Guide 1 pt away at x = 11. Subdivision line at 9 is 1 pt the other way. Grid must win.
        Arc::make_mut(&mut doc.spreads[0]).pages[0].guides.push(Guide {
            orientation: Orientation::Vertical,
            position: 11.0,
            spread: true,
            locked: false,
            layer: None,
            liquid: false,
        });
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::default();
        let mut cx = ctx_on(&doc, &sel, &cache, &layout);
        cx.snap.snap_to_document_grid = true;
        let hit = snap(&cx, move_req(Rect::new(10.0, 10.0, 40.0, 40.0), &[id]));
        assert!((hit.delta.x - -1.0).abs() < 1e-6);
        assert!(hit.guides.is_empty());
    }

    #[test]
    fn hidden_baseline_still_snaps_and_zero_increment_does_not() {
        let mut doc = Document::new(&NewDocument::default());
        doc.settings.baseline_grid.start = 36.0;
        doc.settings.baseline_grid.increment = 12.0;
        let cache = Cache::new();
        let layout = CanvasLayout::new(&doc, false);
        let sel = Selection::default();
        let cx = ctx_on(&doc, &sel, &cache, &layout);
        // Top at 49 wants the line at 48 (start 36, increment 12). The margin at 36 is 13 pt away.
        let hit = snap(&cx, y_req(Rect::new(100.0, 49.0, 140.0, 80.0)));
        assert!((hit.delta.y - -1.0).abs() < 1e-6);
        let mut dead = doc.clone();
        dead.settings.baseline_grid.increment = 0.0;
        let layout = CanvasLayout::new(&dead, false);
        let cx = ctx_on(&dead, &sel, &cache, &layout);
        let hit = snap(&cx, y_req(Rect::new(100.0, 49.0, 140.0, 80.0)));
        assert_eq!(hit.delta.y, 0.0);
    }
}
