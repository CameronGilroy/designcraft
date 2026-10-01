//! Where spreads sit on the canvas (the pasteboard): stacked vertically, spines aligned at x = 0,
//! parents shown on their own canvas when a parent spread is being edited.

use designcraft_doc::{Document, SpreadRef};
use designcraft_geom::{Point, Rect, Vec2};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SpreadSlot {
    pub spread: SpreadRef,
    /// canvas = spread + offset
    pub offset: Vec2,
    /// Spread page bounds in canvas coordinates.
    pub bounds: Rect,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CanvasLayout {
    pub slots: Vec<SpreadSlot>,
    /// Pasteboard extent (canvas coordinates).
    pub pasteboard: Rect,
}

/// Vertical gap between spreads (points).
pub const SPREAD_GAP: f64 = 72.0;

impl CanvasLayout {
    /// Layout of the document spreads (or the parent spreads when `parents` is true).
    pub fn new(doc: &Document, parents: bool) -> Self {
        let list: Vec<SpreadRef> = if parents { (0..doc.parents.len()).map(SpreadRef::Parent).collect() } else { (0..doc.spreads.len()).map(SpreadRef::Doc).collect() };
        let mut slots = Vec::with_capacity(list.len());
        let mut y = 0.0;
        let mut pb: Option<Rect> = None;
        let (px, py) = doc.settings.pasteboard;
        for r in list {
            let Some(sp) = doc.spread(r) else { continue };
            let b = sp.bounds();
            let offset = Vec2::new(-sp.spine_x(), y - b.y0);
            let cb = b + offset;
            let pr = Rect::new(cb.x0 - px.max(b.width()), cb.y0 - py * 0.5, cb.x1 + px.max(b.width()), cb.y1 + py * 0.5);
            pb = Some(pb.map_or(pr, |p| p.union(pr)));
            slots.push(SpreadSlot { spread: r, offset, bounds: cb });
            y += b.height() + SPREAD_GAP;
        }
        let pasteboard = pb.unwrap_or(Rect::new(-500.0, -500.0, 500.0, 500.0)).inflate(0.0, py * 0.5);
        CanvasLayout { slots, pasteboard }
    }

    /// The spread under (or nearest to) a canvas point, and the point in spread coordinates.
    pub fn spread_at(&self, p: Point) -> Option<(SpreadRef, Point)> {
        let s = self.slots.iter().min_by(|a, b| dist_y(a.bounds, p.y).total_cmp(&dist_y(b.bounds, p.y)))?;
        Some((s.spread, p - s.offset))
    }

    pub fn slot(&self, r: SpreadRef) -> Option<&SpreadSlot> {
        self.slots.iter().find(|s| s.spread == r)
    }

    pub fn offset(&self, r: SpreadRef) -> Vec2 {
        self.slot(r).map(|s| s.offset).unwrap_or(Vec2::ZERO)
    }

    /// Canvas rect of a document page.
    pub fn page_rect(&self, doc: &Document, abs: usize) -> Option<Rect> {
        let (si, pi) = doc.page_loc(abs)?;
        let off = self.offset(SpreadRef::Doc(si));
        Some(doc.spreads[si].pages[pi].bounds() + off)
    }
}

fn dist_y(r: Rect, y: f64) -> f64 {
    if y < r.y0 - SPREAD_GAP / 2.0 {
        r.y0 - y
    } else if y > r.y1 + SPREAD_GAP / 2.0 {
        y - r.y1
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_doc::build::NewDocument;

    #[test]
    fn spreads_stack_on_the_spine() {
        let d = Document::new(&NewDocument { pages: 3, ..Default::default() });
        let l = CanvasLayout::new(&d, false);
        assert_eq!(l.slots.len(), 2);
        // Page 1 is a right page: it starts at the spine (x = 0).
        assert_eq!(l.slots[0].bounds.x0, 0.0);
        // Spread 2 (pages 2–3) straddles the spine.
        assert_eq!(l.slots[1].bounds.x0, -612.0);
        assert_eq!(l.slots[1].bounds.y0, 792.0 + SPREAD_GAP);
        let (r, p) = l.spread_at(Point::new(-100.0, 900.0)).unwrap();
        assert_eq!(r, SpreadRef::Doc(1));
        assert_eq!(p, Point::new(512.0, 900.0 - 792.0 - SPREAD_GAP));
        assert_eq!(l.page_rect(&d, 2).unwrap().x0, 0.0);
    }
}
