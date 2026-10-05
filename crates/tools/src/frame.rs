//! Frame and shape tools (Rectangle/Ellipse/Polygon Frame, Rectangle/Ellipse/Polygon, Line).
//! Drag to draw (Shift = square/45°, Alt = from centre); click opens the size dialog.

use designcraft_geom::{Point, Rect};
use serde_json::json;

use crate::{Action, Cursor, Gesture, Mods, Overlay, PointerEvent, PointerKind, SnapRequest, Tool, ToolContext, rect_json, spread_json};

pub struct FrameTool {
    id: &'static str,
    start: Option<Point>,
    cur: Point,
    active: bool,
    guides: Vec<crate::Overlay>,
    /// Gridify: columns and rows (arrow keys while dragging), and the last drawn rect.
    grid: (u32, u32),
    last: Option<(serde_json::Value, Rect)>,
}

impl FrameTool {
    pub fn new(id: &str) -> Self {
        let id = crate::tool_info(id).map(|t| t.id).unwrap_or("rectangleFrame");
        Self { id, start: None, cur: Point::ZERO, active: false, guides: vec![], grid: (1, 1), last: None }
    }

    /// The preview for `r`: one frame, or a grid of them.
    fn preview(&self, spread: serde_json::Value, r: Rect) -> Action {
        let (shape, content) = self.kind();
        let (c, rw) = self.grid;
        if c * rw > 1 {
            Action::Preview(
                "frame.grid".into(),
                json!({"spread": spread, "shape": shape, "content": content, "rect": rect_json(r), "cols": c, "rows": rw}),
            )
        } else {
            Action::Preview("frame.create".into(), json!({"spread": spread, "shape": shape, "content": content, "rect": rect_json(r)}))
        }
    }
    fn kind(&self) -> (&'static str, &'static str) {
        match self.id {
            "rectangleFrame" => ("rectangle", "graphic"),
            "ellipseFrame" => ("ellipse", "graphic"),
            "polygonFrame" => ("polygon", "graphic"),
            "rectangle" => ("rectangle", "unassigned"),
            "ellipse" => ("ellipse", "unassigned"),
            "polygon" => ("polygon", "unassigned"),
            _ => ("line", "unassigned"),
        }
    }
}

pub fn drag_rect(a: Point, b: Point, m: Mods) -> Rect {
    let (mut dx, mut dy) = (b.x - a.x, b.y - a.y);
    if m.shift {
        let s = dx.abs().max(dy.abs());
        dx = s * dx.signum();
        dy = s * dy.signum();
    }
    if m.alt {
        Rect::new(a.x - dx.abs(), a.y - dy.abs(), a.x + dx.abs(), a.y + dy.abs())
    } else {
        Rect::from_points(a, Point::new(a.x + dx, a.y + dy))
    }
}

/// Edges the drag is setting. Alt (drawn from the centre) snaps the pointer side.
fn moving_edges(anchor: Point, end: Point, rect: Rect, from_center: bool) -> ([bool; 3], [bool; 3]) {
    if from_center {
        let x = if (end.x - anchor.x).abs() <= 1e-9 {
            [false, false, false]
        } else if end.x >= anchor.x {
            [false, false, true]
        } else {
            [true, false, false]
        };
        let y = if (end.y - anchor.y).abs() <= 1e-9 {
            [false, false, false]
        } else if end.y >= anchor.y {
            [false, false, true]
        } else {
            [true, false, false]
        };
        return (x, y);
    }
    (anchored_edge(anchor.x, rect.x0, rect.x1), anchored_edge(anchor.y, rect.y0, rect.y1))
}

fn anchored_edge(anchor: f64, a0: f64, a1: f64) -> [bool; 3] {
    let on0 = (a0 - anchor).abs() <= 1e-4;
    let on1 = (a1 - anchor).abs() <= 1e-4;
    if on0 && !on1 {
        [false, false, true]
    } else if on1 && !on0 {
        [true, false, false]
    } else if on0 && on1 {
        [false, false, false]
    } else if (a0 - anchor).abs() <= (a1 - anchor).abs() {
        [false, false, true]
    } else {
        [true, false, false]
    }
}

fn length_on(edges: [bool; 3], size: f64) -> Option<f64> {
    if edges == [false, false, false] {
        return None;
    }
    let len = size.abs();
    len.is_finite().then_some(len)
}

impl Tool for FrameTool {
    fn id(&self) -> &'static str {
        self.id
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let (shape, content) = self.kind();
        match ev.kind {
            PointerKind::Down => {
                let pos = match cx.layout.spread_at(ev.pos) {
                    Some((sr, sp)) if cx.snap.any() => cx.layout.to_canvas(sr, crate::snap::snap_point(cx, sr, sp).0),
                    _ => ev.pos,
                };
                self.start = Some(pos);
                self.cur = ev.pos;
                self.active = false;
                self.grid = (1, 1);
                self.last = None;
                vec![]
            }
            PointerKind::Drag => {
                let Some(a) = self.start else { return vec![] };
                self.cur = ev.pos;
                if !self.active && (ev.pos - a).hypot() < cx.tol(3.0) {
                    return vec![];
                }
                let Some((sr, sa)) = cx.layout.spread_at(a) else { return vec![] };
                self.guides.clear();
                let mut out = vec![];
                if !self.active {
                    self.active = true;
                    out.push(Action::Begin(format!(
                        "Create {}",
                        crate::tool_info(self.id).map(|t| t.label.trim_end_matches(" Tool")).unwrap_or("Frame")
                    )));
                }
                if shape == "line" {
                    // Shift constrains the angle first. The free endpoint still snaps.
                    let mut b = cx.layout.to_spread(sr, ev.pos);
                    if ev.mods.shift {
                        let v = b - sa;
                        b = sa + designcraft_geom::constrain_angle(v, 45.0);
                    }
                    if cx.snap.any() {
                        let len = (b - sa).hypot();
                        let hit = crate::snap::snap(
                            cx,
                            SnapRequest {
                                spread: sr,
                                gesture: Gesture::Create,
                                rect: Rect::from_points(b, b),
                                x_edges: [true, true, true],
                                y_edges: [true, true, true],
                                exclude: &[],
                                copying: false,
                                lengths: [len.is_finite().then_some(len), None],
                                angle: None,
                                radius: 0.0,
                                pointer: b,
                            },
                        );
                        // Position and length both land on the endpoint. Rescaling the segment would leave the other axis.
                        if hit.delta.x.is_finite() {
                            b.x += hit.delta.x;
                        }
                        if hit.delta.y.is_finite() {
                            b.y += hit.delta.y;
                        }
                        self.guides = hit.guides;
                    }
                    out.push(Action::Preview("line.create".into(), json!({"spread": spread_json(sr), "a": [sa.x, sa.y], "b": [b.x, b.y]})));
                } else {
                    // Shift constrains the shape first. The free edges still snap.
                    let end = cx.layout.to_spread(sr, ev.pos);
                    let mut r = drag_rect(sa, end, ev.mods);
                    if cx.snap.any() {
                        let (x_edges, y_edges) = moving_edges(sa, end, r, ev.mods.alt);
                        let hit = crate::snap::snap(
                            cx,
                            SnapRequest {
                                spread: sr,
                                gesture: Gesture::Create,
                                rect: r,
                                x_edges,
                                y_edges,
                                exclude: &[],
                                copying: false,
                                lengths: [length_on(x_edges, r.width()), length_on(y_edges, r.height())],
                                angle: None,
                                radius: 0.0,
                                pointer: end,
                            },
                        );
                        r = crate::snap::nudge_edges(r, x_edges, y_edges, hit.delta);
                        self.guides = hit.guides;
                    }
                    self.last = Some((spread_json(sr), r));
                    out.push(self.preview(spread_json(sr), r));
                }
                out
            }
            PointerKind::Up => {
                self.guides.clear();
                let was = self.active;
                self.active = false;
                let start = self.start.take();
                if was {
                    return vec![Action::Commit];
                }
                // Click: size dialog.
                if let Some(a) = start
                    && let Some((sr, sa)) = cx.layout.spread_at(a)
                {
                    return vec![Action::Dialog(
                        "frameSize".into(),
                        json!({"spread": spread_json(sr), "shape": shape, "content": content, "x": sa.x, "y": sa.y, "width": 72.0, "height": 72.0}),
                    )];
                }
                vec![]
            }
            _ => vec![],
        }
    }

    fn key(&mut self, _cx: &ToolContext, key: crate::ToolKey, _mods: Mods) -> Vec<Action> {
        // Gridify while dragging: ←/→ columns, ↑/↓ rows.
        if !self.active || self.kind().0 == "line" {
            return vec![];
        }
        let (c, r) = &mut self.grid;
        match key {
            crate::ToolKey::Right => *c += 1,
            crate::ToolKey::Left => *c = (*c).saturating_sub(1).max(1),
            crate::ToolKey::Up => *r += 1,
            crate::ToolKey::Down => *r = (*r).saturating_sub(1).max(1),
            _ => return vec![],
        }
        match self.last.clone() {
            Some((sp, rect)) => vec![self.preview(sp, rect)],
            None => vec![],
        }
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        if let (true, Some(a)) = (self.active, self.start) {
            let r = Rect::from_points(a, self.cur);
            let _ = cx;
            let mut v = self.guides.clone();
            v.push(Overlay::Measure { p: self.cur, text: format!("W: {:.0} pt  H: {:.0} pt", r.width(), r.height()) });
            return v;
        }
        vec![]
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::Crosshair
    }

    fn busy(&self) -> bool {
        self.active
    }
}
