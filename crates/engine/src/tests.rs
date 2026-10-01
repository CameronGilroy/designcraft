use serde_json::json;

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"pages": 2})).unwrap();
    s
}

#[test]
fn create_frame_type_and_undo() {
    let mut s = session();
    let r = s.execute("frame.create", &json!({"rect": [36, 36, 300, 200], "content": "text"})).unwrap();
    let sid = r["story"].as_u64().unwrap();
    s.execute("text.insert", &json!({"text": "Hello, world"})).unwrap();
    let st = s.execute("story.get", &json!({"story": sid})).unwrap();
    assert_eq!(st["text"], "Hello, world");
    s.execute("text.delete", &json!({})).unwrap();
    assert_eq!(s.execute("story.get", &json!({"story": sid})).unwrap()["text"], "Hello, worl");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("story.get", &json!({"story": sid})).unwrap()["text"], "Hello, world");
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.doc().unwrap().doc.stories.is_empty());
}

#[test]
fn smart_quotes_and_formatting() {
    let mut s = session();
    s.execute("frame.create", &json!({"rect": [36, 36, 300, 200], "content": "text"})).unwrap();
    s.execute("text.insert", &json!({"text": "\"hi\" it's"})).unwrap();
    let t = s.execute("document.inspect", &json!({})).unwrap();
    let sid = t["stories"][0]["id"].as_u64().unwrap();
    assert_eq!(s.execute("story.get", &json!({"story": sid})).unwrap()["text"], "\u{201C}hi\u{201D} it\u{2019}s");
    s.execute("text.select", &json!({"story": sid, "anchor": 0, "focus": 4})).unwrap();
    s.execute("type.char", &json!({"attrs": {"size": 24, "fontStyle": "Bold"}})).unwrap();
    let a = s.execute("type.selectionAttrs", &json!({})).unwrap();
    assert_eq!(a["chars"]["size"], 24.0);
    s.execute("type.para", &json!({"attrs": {"align": "center"}})).unwrap();
    assert_eq!(s.execute("type.selectionAttrs", &json!({})).unwrap()["para"]["align"], "center");
}

#[test]
fn tool_gesture_creates_one_undo_step() {
    let mut s = session();
    s.set_tool("rectangle");
    let v = ViewInfo { zoom: 1.0 };
    use designcraft_tools::{PointerEvent, PointerKind};
    s.pointer(&PointerEvent::new(PointerKind::Down, 10.0, 10.0), v).unwrap();
    for i in 1..10 {
        s.pointer(&PointerEvent::new(PointerKind::Drag, 10.0 + i as f64 * 10.0, 10.0 + i as f64 * 5.0), v).unwrap();
    }
    s.pointer(&PointerEvent::new(PointerKind::Up, 100.0, 55.0), v).unwrap();
    let st = s.doc().unwrap();
    assert_eq!(st.history.undo.len(), 1);
    assert_eq!(st.doc.spreads[0].items.len(), 1);
    assert_eq!(st.doc.spreads[0].items[0].bounds(), designcraft_geom::Rect::new(10.0, 10.0, 100.0, 55.0));
    // Move with the selection tool.
    s.set_tool("selection");
    s.pointer(&PointerEvent::new(PointerKind::Down, 50.0, 30.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Drag, 70.0, 30.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Drag, 80.0, 40.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Up, 80.0, 40.0), v).unwrap();
    let st = s.doc().unwrap();
    assert_eq!(st.history.undo.len(), 2);
    assert_eq!(st.doc.spreads[0].items[0].bounds().x0, 40.0);
}

#[test]
fn pages_and_layers() {
    let mut s = session();
    s.execute("layout.pages.insert", &json!({"count": 3})).unwrap();
    assert_eq!(s.doc().unwrap().doc.page_count(), 5);
    s.execute("layout.pages.delete", &json!({"pages": [0]})).unwrap();
    assert_eq!(s.doc().unwrap().doc.page_count(), 4);
    s.execute("layer.new", &json!({"name": "Text"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.layers[0].name, "Text");
}

#[test]
fn sample_document_is_valid_and_composes() {
    let mut s = Session::new();
    s.execute("file.newSample", &json!({})).unwrap();
    let st = s.doc().unwrap();
    st.doc.check().unwrap();
    assert_eq!(st.doc.page_count(), 4);
    let i = s.execute("document.inspect", &json!({})).unwrap();
    assert!(i["stories"].as_array().unwrap().len() >= 8);
    // Save/load round trip.
    let bytes = cmd::file_bytes(&s.doc().unwrap().doc);
    let back = cmd::file_from(&bytes).unwrap();
    assert_eq!(back.page_count(), 4);
    assert_eq!(back.assets.len(), s.doc().unwrap().doc.assets.len());
    assert!(back.assets.values().all(|a| !a.data.is_empty()));
}

#[test]
fn every_command_has_metadata() {
    let mut ids = std::collections::HashSet::new();
    for c in command_specs() {
        assert!(ids.insert(c.id), "duplicate command {}", c.id);
        assert!(!c.label.is_empty() && !c.params.is_empty(), "{}", c.id);
    }
    assert!(command_specs().len() > 60);
}

#[test]
fn align_and_distribute() {
    let mut s = session();
    let mut ids = vec![];
    for (x, w) in [(10.0, 20.0), (100.0, 40.0), (300.0, 10.0)] {
        let r = s.execute("frame.create", &json!({"rect": [x, 50.0 + x / 10.0, x + w, 100.0], "content": "unassigned"})).unwrap();
        ids.push(r["id"].as_u64().unwrap());
    }
    s.execute("object.align", &json!({"ids": ids, "edge": "top"})).unwrap();
    let st = s.doc().unwrap();
    let tops: Vec<f64> = ids.iter().map(|i| st.doc.item(designcraft_doc::ItemId(*i)).unwrap().bounds().y0).collect();
    assert!(tops.iter().all(|t| (*t - 51.0).abs() < 1e-9), "{tops:?}");
    s.execute("object.distribute", &json!({"ids": ids, "by": "spacing"})).unwrap();
    let st = s.doc().unwrap();
    let b: Vec<designcraft_geom::Rect> = ids.iter().map(|i| st.doc.item(designcraft_doc::ItemId(*i)).unwrap().bounds()).collect();
    assert!(((b[1].x0 - b[0].x1) - (b[2].x0 - b[1].x1)).abs() < 1e-9);
    s.execute("object.align", &json!({"ids": [ids[0]], "edge": "left", "to": "margins"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.item(designcraft_doc::ItemId(ids[0])).unwrap().bounds().x0, 36.0);
}

#[test]
fn pen_draws_and_direct_selection_edits() {
    use designcraft_tools::{PointerEvent, PointerKind};
    let mut s = session();
    let v = ViewInfo { zoom: 1.0 };
    s.set_tool("pen");
    let click = |s: &mut Session, x: f64, y: f64| {
        s.pointer(&PointerEvent::new(PointerKind::Down, x, y), v).unwrap();
        s.pointer(&PointerEvent::new(PointerKind::Up, x, y), v).unwrap();
    };
    click(&mut s, 100.0, 100.0);
    click(&mut s, 200.0, 100.0);
    // A smooth point (drag).
    s.pointer(&PointerEvent::new(PointerKind::Down, 200.0, 200.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Drag, 240.0, 200.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Up, 240.0, 200.0), v).unwrap();
    click(&mut s, 100.0, 100.0); // close
    let st = s.doc().unwrap();
    let it = &st.doc.spreads[0].items[0];
    assert_eq!(it.path.subpaths[0].anchors.len(), 3);
    assert!(it.path.subpaths[0].closed);
    assert!(it.path.subpaths[0].anchors[2].has_out());
    let id = it.id;
    // Direct Selection: drag the first anchor.
    s.set_tool("directSelection");
    s.execute("selection.set", &json!({"ids": [id.0]})).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Down, 100.0, 100.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Drag, 90.0, 80.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Up, 90.0, 80.0), v).unwrap();
    let a = s.doc().unwrap().doc.item(id).unwrap().path.subpaths[0].anchors[0].p;
    assert_eq!(a, designcraft_geom::Point::new(90.0, 80.0));
}
