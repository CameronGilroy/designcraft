//! Pencil tool (N): draw freehand; the stroke becomes a smooth path. Alt while releasing closes
//! it.

use designcraft_doc::SpreadRef;
use designcraft_geom::{BezPath, Point};
use serde_json::json;

use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct PencilTool {
    /// Spread, canvas offset of that spread, and the canvas points so far.
    stroke: Option<(SpreadRef, designcraft_geom::Vec2, Vec<Point>)>,
}

impl Tool for PencilTool {
    fn id(&self) -> &'static str {
        "pencil"
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        match ev.kind {
            PointerKind::Down => {
                let Some((sr, _)) = cx.layout.spread_at(ev.pos) else { return vec![] };
                self.stroke = Some((sr, cx.layout.offset(sr), vec![ev.pos]));
                vec![]
            }
            PointerKind::Drag => {
                if let Some((_, _, pts)) = &mut self.stroke
                    && pts.last().is_none_or(|q| (*q - ev.pos).hypot() >= cx.tol(1.0))
                {
                    pts.push(ev.pos);
                }
                vec![]
            }
            PointerKind::Up => {
                let Some((sr, off, mut pts)) = self.stroke.take() else { return vec![] };
                pts.push(ev.pos);
                let spread: Vec<Point> = pts.iter().map(|p| *p - off).collect();
                // Simplify to about two screen pixels.
                let anchors = designcraft_geom::freehand::fit(&spread, cx.tol(2.0), ev.mods.alt);
                if anchors.len() < 2 {
                    return vec![];
                }
                let a: Vec<_> =
                    anchors.iter().map(|a| json!({"p": [a.p.x, a.p.y], "in": [a.h_in.x, a.h_in.y], "out": [a.h_out.x, a.h_out.y]})).collect();
                vec![Action::Exec("path.create".into(), json!({"spread": sr, "anchors": a, "closed": ev.mods.alt}))]
            }
            _ => vec![],
        }
    }

    fn overlays(&self, _cx: &ToolContext) -> Vec<Overlay> {
        let Some((_, _, pts)) = &self.stroke else { return vec![] };
        let mut path = BezPath::new();
        for (i, p) in pts.iter().enumerate() {
            if i == 0 {
                path.move_to(*p);
            } else {
                path.line_to(*p);
            }
        }
        vec![Overlay::Path { path, color: [0, 0, 0], dashed: false }]
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::Pen
    }

    fn busy(&self) -> bool {
        self.stroke.is_some()
    }
}
