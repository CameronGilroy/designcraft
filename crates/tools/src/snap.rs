//! Snapping and smart guides: page edges and centres, margins, column guides, ruler guides and
//! other objects' edges/centres on the same spread. Returns the snap correction and the guide
//! lines to draw (canvas coordinates).

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
pub fn snap_point(cx: &ToolContext, sr: SpreadRef, p: Point) -> (Point, Vec<Overlay>) {
    let s = snap_rect(cx, sr, Rect::from_points(p, p), &[]);
    (p + s.delta, s.guides)
}

fn rect_finite(r: Rect) -> bool {
    r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()
}

/// Closest-target snap for one gesture.
///
/// Same targets and 5 px tolerance as [`snap_rect`], and only when a snap category is on.
/// A non-finite rect returns no correction. An axis whose three edge flags are all false stays at 0.
pub fn snap(cx: &ToolContext, req: SnapRequest<'_>) -> Snap {
    if !rect_finite(req.rect) || !cx.snap.any() {
        return Snap::default();
    }
    let x_free = req.x_edges != [false; 3];
    let y_free = req.y_edges != [false; 3];
    if x_free && y_free {
        return snap_rect(cx, req.spread, req.rect, req.exclude);
    }
    snap_axes(cx, req.spread, req.rect, req.exclude, x_free, y_free)
}

fn guide(a: Point, b: Point, smart: bool) -> Overlay {
    if smart { Overlay::Guide { a, b } } else { Overlay::Line { a, b, color: [255, 0, 255], dashed: false } }
}

#[cfg(test)]
mod tests {
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
}
