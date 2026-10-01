//! Frame and shape tools (Rectangle/Ellipse/Polygon Frame, Rectangle/Ellipse/Polygon, Line).
//! Drag to draw (Shift = square/45°, Alt = from centre); click opens the size dialog.

use designcraft_geom::{Point, Rect};
use serde_json::json;

use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, rect_json, spread_json};

pub struct FrameTool {
    id: &'static str,
    start: Option<Point>,
    cur: Point,
    active: bool,
}

impl FrameTool {
    pub fn new(id: &str) -> Self {
        let id = crate::tool_info(id).map(|t| t.id).unwrap_or("rectangleFrame");
        Self { id, start: None, cur: Point::ZERO, active: false }
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

impl Tool for FrameTool {
    fn id(&self) -> &'static str {
        self.id
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let (shape, content) = self.kind();
        match ev.kind {
            PointerKind::Down => {
                self.start = Some(ev.pos);
                self.cur = ev.pos;
                self.active = false;
                vec![]
            }
            PointerKind::Drag => {
                let Some(a) = self.start else { return vec![] };
                self.cur = ev.pos;
                if !self.active && (ev.pos - a).hypot() < cx.tol(3.0) {
                    return vec![];
                }
                let Some((sr, sa)) = cx.layout.spread_at(a) else { return vec![] };
                let off = cx.layout.offset(sr);
                let mut out = vec![];
                if !self.active {
                    self.active = true;
                    out.push(Action::Begin(format!(
                        "Create {}",
                        crate::tool_info(self.id).map(|t| t.label.trim_end_matches(" Tool")).unwrap_or("Frame")
                    )));
                }
                if shape == "line" {
                    let mut b = ev.pos - off;
                    if ev.mods.shift {
                        let v = b - sa;
                        let v = designcraft_geom::constrain_angle(v, 45.0);
                        b = sa + v;
                    }
                    out.push(Action::Preview("line.create".into(), json!({"spread": spread_json(sr), "a": [sa.x, sa.y], "b": [b.x, b.y]})));
                } else {
                    let r = drag_rect(sa, ev.pos - off, ev.mods);
                    out.push(Action::Preview(
                        "frame.create".into(),
                        json!({"spread": spread_json(sr), "shape": shape, "content": content, "rect": rect_json(r)}),
                    ));
                }
                out
            }
            PointerKind::Up => {
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

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        if let (true, Some(a)) = (self.active, self.start) {
            let r = Rect::from_points(a, self.cur);
            let _ = cx;
            return vec![Overlay::Measure { p: self.cur, text: format!("W: {:.0} pt  H: {:.0} pt", r.width(), r.height()) }];
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
