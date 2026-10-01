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

/// Unicode text per page, through hayro's interpreter (ToUnicode / ActualText).
fn pdf_text(bytes: &[u8]) -> Vec<String> {
    use hayro_interpret::font::Glyph;
    use hayro_interpret::{
        BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask,
        interpret_page,
    };
    struct Ex(String);
    impl Device<'_> for Ex {
        fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
        fn set_blend_mode(&mut self, _: BlendMode) {}
        fn draw_path(&mut self, _: &kurbo::BezPath, _: kurbo::Affine, _: &Paint<'_>, _: &PathDrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
        fn draw_glyph(&mut self, g: &Glyph<'_>, _: kurbo::Affine, _: kurbo::Affine, _: &Paint<'_>, _: &GlyphDrawMode) {
            match g.as_unicode() {
                Some(hayro_cmap::BfString::Char(c)) => self.0.push(c),
                Some(hayro_cmap::BfString::String(s)) => self.0.push_str(&s),
                None => self.0.push('\u{FFFD}'),
            }
        }
        fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
        fn pop_clip_path(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }
    let pdf = hayro_syntax::Pdf::new(bytes.to_vec()).expect("valid PDF");
    let cache = InterpreterCache::new();
    pdf.pages()
        .iter()
        .map(|page| {
            let mut ctx =
                Context::new(kurbo::Affine::IDENTITY, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), &cache, pdf.xref(), InterpreterSettings::default());
            let mut ex = Ex(String::new());
            interpret_page(page, &mut ctx, &mut ex);
            ex.0
        })
        .collect()
}

#[test]
fn export_pdf_of_the_sample() {
    let mut s = Session::new();
    s.execute("file.newSample", &json!({})).unwrap();
    let r = s.execute("file.exportPdf", &json!({"bleed": true, "marks": true})).unwrap();
    assert_eq!(r["pages"], 4);
    let bytes = cmd::base64_decode(r["base64"].as_str().unwrap());
    assert!(bytes.starts_with(b"%PDF-"));
    let pdf = hayro_syntax::Pdf::new(bytes.clone()).expect("parse");
    assert_eq!(pdf.pages().len(), 4);
    let raw = String::from_utf8_lossy(&bytes);
    assert!(raw.contains("/FontFile2") || raw.contains("/FontFile3"), "fonts embedded");
    let text = pdf_text(&bytes);
    let all: String = text.join("\n");
    assert!(all.contains("Notes on the Grid"), "{all}");
    // Page range + spreads + file output.
    let dir = std::env::temp_dir().join(format!("dc-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.pdf");
    let r = s.execute("file.exportPdf", &json!({"path": path.to_string_lossy(), "pages": "2-3", "spreads": true})).unwrap();
    assert_eq!(r["pages"], 1, "pages 2–3 are one spread");
    assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF-"));
    std::fs::remove_dir_all(&dir).ok();
    assert!(s.execute("file.exportPdf", &json!({"pages": "9"})).is_err());
    assert!(s.execute("file.exportPdf", &json!({"standard": "bogus"})).is_err());
}
