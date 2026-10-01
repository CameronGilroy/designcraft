//! Hand (H) and Zoom (Z) tools.

use designcraft_geom::Point;
use serde_json::json;

use crate::{Action, Cursor, Mods, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct HandTool {
    last: Option<Point>,
}

impl Tool for HandTool {
    fn id(&self) -> &'static str {
        "hand"
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        match ev.kind {
            PointerKind::Down => {
                self.last = Some(ev.pos);
                vec![]
            }
            PointerKind::Drag => {
                // Canvas positions shift as we pan, so convert the delta to screen pixels once.
                let Some(l) = self.last else { return vec![] };
                let d = (ev.pos - l) * cx.zoom;
                vec![Action::View(json!({"pan": [d.x, d.y]}))]
            }
            PointerKind::Up => {
                self.last = None;
                vec![]
            }
            _ => vec![],
        }
    }
    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        if self.last.is_some() { Cursor::HandGrab } else { Cursor::Hand }
    }
    fn busy(&self) -> bool {
        self.last.is_some()
    }
}

#[derive(Default)]
pub struct ZoomTool;

impl Tool for ZoomTool {
    fn id(&self) -> &'static str {
        "zoom"
    }
    fn pointer(&mut self, _cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        if ev.kind == PointerKind::Up {
            let f = if ev.mods.alt { 0.5 } else { 2.0 };
            return vec![Action::View(json!({"zoomAt": [ev.pos.x, ev.pos.y], "factor": f}))];
        }
        vec![]
    }
    fn cursor(&self, _cx: &ToolContext, _p: Point, m: Mods) -> Cursor {
        if m.alt { Cursor::ZoomOut } else { Cursor::ZoomIn }
    }
}

/// The loaded place cursor: click places at actual size, drag draws the frame, click on an empty
/// frame places into it.
#[derive(Default)]
pub struct PlaceGun {
    start: Option<Point>,
    cur: Point,
}

impl Tool for PlaceGun {
    fn id(&self) -> &'static str {
        "placeGun"
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        match ev.kind {
            PointerKind::Down => {
                self.start = Some(ev.pos);
                self.cur = ev.pos;
                vec![]
            }
            PointerKind::Drag => {
                self.cur = ev.pos;
                vec![]
            }
            PointerKind::Up => {
                let Some(a) = self.start.take() else { return vec![] };
                let Some((sr, sa)) = cx.layout.spread_at(a) else { return vec![] };
                let off = cx.layout.offset(sr);
                let b = ev.pos - off;
                if (ev.pos - a).hypot() < cx.tol(3.0)
                    && let Some((_, id)) = cx.hit(a)
                    && cx
                        .doc
                        .item(id)
                        .is_some_and(|i| matches!(i.content, designcraft_doc::Content::Unassigned | designcraft_doc::Content::Graphic(_)))
                {
                    return vec![Action::Exec("place.drop".into(), json!({"frame": id.0}))];
                }
                let r = designcraft_geom::Rect::from_points(sa, b);
                vec![Action::Exec(
                    "place.drop".into(),
                    json!({"spread": crate::spread_json(sr), "x": sa.x, "y": sa.y, "rect": [r.x0, r.y0, r.x1, r.y1]}),
                )]
            }
            _ => vec![],
        }
    }
    fn overlays(&self, _cx: &ToolContext) -> Vec<crate::Overlay> {
        match self.start {
            Some(a) => vec![crate::Overlay::Marquee(designcraft_geom::Rect::from_points(a, self.cur))],
            None => vec![],
        }
    }
    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::LoadedGraphic
    }
}
