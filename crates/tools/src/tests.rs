use designcraft_compose::Cache;
use designcraft_doc::build::NewDocument;
use designcraft_doc::{Document, ParaFormat, Selection, SpreadRef};
use designcraft_geom::Rect;

use super::*;

fn ctx<'a>(d: &'a Document, s: &'a Selection, c: &'a Cache, l: &'a CanvasLayout) -> ToolContext<'a> {
    ToolContext { doc: d, selection: s, cache: c, layout: l, zoom: 1.0, layer: d.default_layer(), snap: false }
}

#[test]
fn frame_tool_drag_emits_begin_preview_commit() {
    let d = Document::new(&NewDocument::default());
    let (s, c, l) = (Selection::default(), Cache::new(), CanvasLayout::new(&d, false));
    let cx = ctx(&d, &s, &c, &l);
    let mut t = create("rectangleFrame");
    assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 10.0, 10.0)).is_empty());
    let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 110.0, 60.0));
    assert!(matches!(&a[0], Action::Begin(_)));
    match &a[1] {
        Action::Preview(cmd, p) => {
            assert_eq!(cmd, "frame.create");
            assert_eq!(p["rect"], serde_json::json!([10.0, 10.0, 110.0, 60.0]));
            assert_eq!(p["content"], "graphic");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 110.0, 60.0)), vec![Action::Commit]);
}

#[test]
fn selection_click_and_marquee() {
    let mut d = Document::new(&NewDocument::default());
    let lid = d.default_layer();
    let (fid, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(100.0, 100.0, 200.0, 200.0), lid, "x", ParaFormat::default()).unwrap();
    let (s, c, l) = (Selection::default(), Cache::new(), CanvasLayout::new(&d, false));
    let cx = ctx(&d, &s, &c, &l);
    let mut t = create("selection");
    let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0));
    assert_eq!(a, vec![Action::Exec("selection.set".into(), serde_json::json!({"ids": [fid.0], "content": false}))]);
    t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 150.0, 150.0));
    // Marquee from empty space.
    t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 20.0, 20.0));
    t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 300.0, 300.0));
    let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 300.0, 300.0));
    assert_eq!(a, vec![Action::Exec("selection.set".into(), serde_json::json!({"ids": [fid.0], "add": false}))]);
}

#[test]
fn resize_rect_modifiers() {
    let r = Rect::new(0.0, 0.0, 100.0, 50.0);
    assert_eq!(select::resize_rect(r, 4, designcraft_geom::Point::new(200.0, 80.0), Mods::default()), Rect::new(0.0, 0.0, 200.0, 80.0));
    let p = select::resize_rect(r, 4, designcraft_geom::Point::new(200.0, 60.0), Mods { shift: true, ..Default::default() });
    assert_eq!(p, Rect::new(0.0, 0.0, 200.0, 100.0));
    let c = select::resize_rect(r, 3, designcraft_geom::Point::new(150.0, 25.0), Mods { alt: true, ..Default::default() });
    assert_eq!(c, Rect::new(-50.0, 0.0, 150.0, 50.0));
}
