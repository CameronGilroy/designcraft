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
