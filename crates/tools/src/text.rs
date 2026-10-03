//! Type tool (T): click in a frame to place the caret, drag in a frame to select text, drag on
//! empty canvas to draw a new text frame. Keys move the caret / delete; typing goes through
//! `text.insert` (sent by the UI as text input).

use designcraft_geom::Point;
use serde_json::json;

use crate::{Action, Cursor, Mods, PointerEvent, PointerKind, Tool, ToolContext, ToolKey, frame::drag_rect, rect_json, spread_json};

#[derive(Default)]
pub struct TypeTool {
    start: Option<Point>,
    selecting: Option<u64>,
    drawing: bool,
}

impl Tool for TypeTool {
    fn id(&self) -> &'static str {
        "type"
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        match ev.kind {
            PointerKind::Down | PointerKind::DoubleClick => {
                self.start = Some(ev.pos);
                self.drawing = false;
                self.selecting = None;
                if let Some((_, id)) = cx.hit(ev.pos)
                    && let Some(it) = cx.doc.item(id)
                    && !matches!(it.content, designcraft_doc::Content::Graphic(_) | designcraft_doc::Content::Group { .. })
                {
                    let sp = cx.layout.spread_at(ev.pos).map(|(_, p)| p).unwrap_or(ev.pos);
                    self.selecting = Some(id.0);
                    let cmd = if ev.kind == PointerKind::DoubleClick {
                        "text.selectWord"
                    } else if ev.mods.shift {
                        "text.extendTo"
                    } else {
                        "text.placeCaret"
                    };
                    return vec![Action::Exec(cmd.into(), json!({"frame": id.0, "point": [sp.x, sp.y]}))];
                }
                vec![]
            }
            PointerKind::Drag => {
                let Some(a) = self.start else { return vec![] };
                if let Some(fid) = self.selecting {
                    let sp = cx.layout.spread_at(ev.pos).map(|(_, p)| p).unwrap_or(ev.pos);
                    return vec![Action::Exec("text.extendTo".into(), json!({"frame": fid, "point": [sp.x, sp.y]}))];
                }
                if (ev.pos - a).hypot() < cx.tol(3.0) && !self.drawing {
                    return vec![];
                }
                let Some((sr, sa)) = cx.layout.spread_at(a) else { return vec![] };
                let off = cx.layout.offset(sr);
                let r = drag_rect(sa, ev.pos - off, ev.mods);
                let mut out = vec![];
                if !self.drawing {
                    self.drawing = true;
                    out.push(Action::Begin("Create Text Frame".into()));
                }
                out.push(Action::Preview(
                    "frame.create".into(),
                    json!({"spread": spread_json(sr), "shape": "rectangle", "content": "text", "rect": rect_json(r), "caret": true}),
                ));
                out
            }
            PointerKind::Up => {
                let start = self.start.take();
                if std::mem::take(&mut self.drawing) {
                    self.selecting = None;
                    return vec![Action::Commit];
                }
                // Ends a press in text: drops dragged text, or is ignored by the engine.
                if let Some(fid) = self.selecting.take() {
                    let sp = cx.layout.spread_at(ev.pos).map(|(_, p)| p).unwrap_or(ev.pos);
                    let moved = start.is_some_and(|a| (ev.pos - a).hypot() >= cx.tol(3.0));
                    return vec![Action::Exec(
                        "text.release".into(),
                        json!({"frame": fid, "point": [sp.x, sp.y], "moved": moved, "copy": ev.mods.alt}),
                    )];
                }
                vec![]
            }
            PointerKind::Move => vec![],
        }
    }

    fn key(&mut self, cx: &ToolContext, key: ToolKey, mods: Mods) -> Vec<Action> {
        if cx.selection.text.is_none() {
            return vec![];
        }
        let mv = |dir: &str| {
            vec![Action::Exec("text.move".into(), json!({"dir": dir, "extend": mods.shift, "word": mods.alt || mods.ctrl, "far": mods.cmd}))]
        };
        match key {
            ToolKey::Left => mv("left"),
            ToolKey::Right => mv("right"),
            ToolKey::Up => mv("up"),
            ToolKey::Down => mv("down"),
            ToolKey::Home => mv("lineStart"),
            ToolKey::End => mv("lineEnd"),
            ToolKey::Backspace => vec![Action::Exec("text.delete".into(), json!({"forward": false, "word": mods.alt}))],
            ToolKey::Delete => vec![Action::Exec("text.delete".into(), json!({"forward": true, "word": mods.alt}))],
            ToolKey::Enter => vec![Action::Exec("text.insert".into(), json!({"text": if mods.shift { "\u{2028}" } else { "\n" }}))],
            ToolKey::Tab => vec![Action::Exec("text.insert".into(), json!({"text": "\t"}))],
            ToolKey::Escape => vec![Action::SwitchTool("selection".into()), Action::Exec("text.exitToFrame".into(), json!({}))],
            _ => vec![],
        }
    }

    fn cursor(&self, cx: &ToolContext, p: Point, _m: Mods) -> Cursor {
        match cx.hit(p) {
            Some((_, id)) if cx.doc.item(id).is_some_and(|i| i.is_text_frame()) => Cursor::Text,
            _ => Cursor::Text,
        }
    }

    fn busy(&self) -> bool {
        self.drawing
    }

    fn wants_text(&self, cx: &ToolContext) -> bool {
        cx.selection.text.is_some()
    }
}
