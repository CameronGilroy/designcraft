//! IDML round-trip tests (the sample magazine) and the interchange commands.

use designcraft_doc::{Document, ItemId};
use serde_json::json;

use crate::Session;

/// Every item of a spread list in order, flattened (pre-order).
fn items(spreads: &[std::sync::Arc<designcraft_doc::Spread>]) -> Vec<(ItemId, designcraft_geom::Rect)> {
    let mut v = Vec::new();
    for sp in spreads {
        for it in &sp.items {
            it.walk(&mut |i| v.push((i.id, i.bounds())));
        }
    }
    v
}

/// Thread order of every story as positions in the flattened item list.
fn positions(d: &Document) -> Vec<Vec<usize>> {
    let all: Vec<ItemId> = items(&d.spreads).into_iter().chain(items(&d.parents)).map(|(id, _)| id).collect();
    d.stories.values().map(|s| s.frames.iter().map(|f| all.iter().position(|i| i == f).expect("frame")).collect()).collect()
}

#[test]
fn magazine_round_trips_through_idml() {
    let d = crate::sample::magazine();
    let bytes = designcraft_idml::export_idml(&d);
    let back = designcraft_idml::import_idml(&bytes).unwrap();
    assert_eq!(back.page_count(), d.page_count());
    assert_eq!(back.spreads.len(), d.spreads.len());
    assert_eq!(back.parents.len(), d.parents.len());
    // Stories: same text in the same order, same styles applied.
    let ta: Vec<&str> = d.stories.values().map(|s| s.text.as_str()).collect();
    let tb: Vec<&str> = back.stories.values().map(|s| s.text.as_str()).collect();
    assert_eq!(ta, tb);
    for (a, b) in d.stories.values().zip(back.stories.values()) {
        let sa: Vec<&str> = a.paras.iter().map(|p| p.style.as_str()).collect();
        let sb: Vec<&str> = b.paras.iter().map(|p| p.style.as_str()).collect();
        assert_eq!(sa, sb);
        let ca: Vec<(usize, &str)> = a.chars.iter().map(|r| (r.len, r.format.style.as_str())).collect();
        let cb: Vec<(usize, &str)> = b.chars.iter().map(|r| (r.len, r.format.style.as_str())).collect();
        assert_eq!(ca, cb);
        for (ra, rb) in a.chars.iter().zip(&b.chars) {
            assert_eq!(ra.format.over, rb.format.over);
        }
        for (pa, pb) in a.paras.iter().zip(&b.paras) {
            assert_eq!(pa.para, pb.para);
        }
    }
    // Styles and swatches by name.
    for s in &d.styles.paragraph {
        let b = back.styles.para(&s.name).unwrap_or_else(|| panic!("paragraph style {}", s.name));
        if s.name != designcraft_doc::NO_PARA_STYLE {
            assert_eq!(b.chars, s.chars, "{}", s.name);
            assert_eq!(b.para, s.para, "{}", s.name);
        }
    }
    for s in &d.styles.character {
        let b = back.styles.char_style(&s.name).unwrap_or_else(|| panic!("character style {}", s.name));
        assert_eq!(b.chars, s.chars);
    }
    for s in &d.swatches {
        let b = back.swatch(&s.name).unwrap_or_else(|| panic!("swatch {}", s.name));
        assert_eq!(b.value, s.value, "{}", s.name);
    }
    // Frame geometry (document and parent spreads), pages.
    for (sa, sb) in [(&d.spreads, &back.spreads), (&d.parents, &back.parents)] {
        let (ia, ib) = (items(sa), items(sb));
        assert_eq!(ia.len(), ib.len());
        for ((_, a), (_, b)) in ia.iter().zip(&ib) {
            let close = (a.x0 - b.x0).abs() < 0.01 && (a.y0 - b.y0).abs() < 0.01 && (a.x1 - b.x1).abs() < 0.01 && (a.y1 - b.y1).abs() < 0.01;
            assert!(close, "{a:?} vs {b:?}");
        }
        for (pa, pb) in sa.iter().flat_map(|s| s.pages.iter()).zip(sb.iter().flat_map(|s| s.pages.iter())) {
            assert!((pa.x - pb.x).abs() < 0.01 && pa.side == pb.side && pa.margins == pb.margins && pa.columns == pb.columns);
            assert_eq!(pa.parent.is_some(), pb.parent.is_some());
        }
    }
    // Threads: the same frames in the same order.
    assert_eq!(positions(&d), positions(&back));
    assert!(back.stories.values().any(|s| s.frames.len() == 2));
    // Images are embedded.
    assert_eq!(back.assets.len(), d.assets.len());
    for (a, b) in d.assets.values().zip(back.assets.values()) {
        assert_eq!(a.data, b.data);
        assert_eq!(a.pixels, b.pixels);
    }
    // A second round trip is stable.
    let again = designcraft_idml::import_idml(&designcraft_idml::export_idml(&back)).unwrap();
    assert_eq!(items(&again.spreads).len(), items(&back.spreads).len());
}

#[test]
fn idml_commands() {
    let mut s = Session::new();
    s.execute("file.newSample", &json!({})).unwrap();
    let r = s.execute("file.exportIdml", &json!({})).unwrap();
    let b64 = r["base64"].as_str().unwrap().to_string();
    let r = s.execute("file.openIdml", &json!({"base64": b64, "name": "copy.idml"})).unwrap();
    let i = r["index"].as_u64().unwrap() as usize;
    assert_eq!(s.doc().unwrap().doc.title, "copy");
    assert_eq!(s.doc().unwrap().doc.page_count(), 4);
    assert!(s.doc().unwrap().path.is_none());
    // openBytes recognises the package too.
    let r = s.execute("file.openBytes", &json!({"base64": b64, "name": "again.idml"})).unwrap();
    assert_eq!(r["index"].as_u64().unwrap() as usize, i + 1);
    assert!(s.execute("file.openIdml", &json!({})).is_err());
    let dir = std::env::temp_dir().join(format!("dc-idml-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("mag.idml").to_string_lossy().to_string();
    let r = s.execute("file.exportIdml", &json!({"path": p})).unwrap();
    assert!(r["bytes"].as_u64().unwrap() > 1000);
    s.execute("file.open", &json!({"path": p})).unwrap();
    assert_eq!(s.doc().unwrap().doc.title, "mag");
    let _ = std::fs::remove_dir_all(&dir);
}
