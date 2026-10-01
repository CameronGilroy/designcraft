//! File menu: new, open, save, place, export, close.

use std::sync::Arc;

use designcraft_doc::build::{NewDocument, PRESETS};
use designcraft_doc::{Asset, AssetId, Content, Document, Graphic, Item, ItemId, Selection, Shape};
use designcraft_geom::{Affine, Rect, shapes};
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, f64_or, has_doc, ok, str_param};
use crate::{DocState, EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "file.new", "Document…", ["File", "New"], Some("Cmd+N"),
            "{preset?: \"Letter\"|\"A4\"|…, width?, height?, pages?, facingPages?, columns?, gutter?, margins?: number|{top,bottom,inside,outside}, bleed?, title?}",
            always, file_new),
        cmd!(noundo "file.newSample", "Sample Document", ["Help"], None, "{} — a multi-page magazine sample", always, file_sample),
        cmd!(query "file.presets", "Document Presets", [], None, "{}", always, |_, _| Ok(serde_json::to_value(PRESETS).unwrap_or_default())),
        cmd!(noundo "file.open", "Open…", ["File"], Some("Cmd+O"), "{path} — .designcraft (JSON)", always, file_open),
        cmd!(noundo "file.openBytes", "Open Bytes", [], None, "{name, base64}", always, file_open_bytes),
        cmd!(noundo "file.save", "Save", ["File"], Some("Cmd+S"), "{path?}", has_doc, file_save),
        cmd!(noundo "file.saveAs", "Save As…", ["File"], Some("Cmd+Shift+S"), "{path}", has_doc, file_save),
        cmd!(query "file.serialize", "Serialize", [], None, "{} → {json}", has_doc, |s, _| Ok(json!({"json": String::from_utf8_lossy(&to_bytes(&s.doc()?.doc)).to_string()}))),
        cmd!(noundo "file.close", "Close", ["File"], Some("Cmd+W"), "{index?}", has_doc, |s, p| {
            let i = p.get("index").and_then(Value::as_u64).map(|v| v as usize).or(s.active_index()).unwrap_or(0);
            s.close_document(i);
            ok()
        }),
        cmd!(noundo "file.activate", "Activate Document", [], None, "{index}", has_doc, |s, p| {
            s.set_active(p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize);
            ok()
        }),
        cmd!(
            "place.load",
            "Load Place Cursor",
            [],
            None,
            "{path | base64, name?} — load a graphic into the place cursor (then click/drag with the placeGun tool)",
            has_doc,
            place_load
        ),
        cmd!("place.drop", "Place Loaded Graphic", [], None, "{spread?, x, y, rect?: [x0,y0,x1,y1], frame?: id}", has_doc, place_drop),
        cmd!(
            "file.place",
            "Place…",
            ["File"],
            Some("Cmd+D"),
            "{path?|base64?, name?, frame?: id (place into), spread?, x?, y?, width?} — places an image; into the selected empty frame if any",
            has_doc,
            file_place
        ),
    ]
}

pub fn to_bytes(d: &Document) -> Vec<u8> {
    designcraft_format::save(d).unwrap_or_default()
}

pub fn from_bytes(b: &[u8]) -> Result<Document> {
    designcraft_format::load(b).map_err(|e| EngineError::Other(e.to_string()))
}

fn file_new(s: &mut Session, p: &Value) -> Result<Value> {
    let mut nd = match str_param(p, "preset") {
        Some(name) => NewDocument::from_preset(name).ok_or_else(|| bad("file.new", format!("unknown preset `{name}`")))?,
        None => NewDocument::default(),
    };
    nd.width = f64_or(p, "width", nd.width);
    nd.height = f64_or(p, "height", nd.height);
    nd.pages = p.get("pages").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(nd.pages).clamp(1, 9999);
    nd.facing_pages = p.get("facingPages").and_then(Value::as_bool).unwrap_or(nd.facing_pages);
    nd.columns = p.get("columns").and_then(Value::as_u64).map(|v| v as u32).unwrap_or(nd.columns);
    nd.gutter = f64_or(p, "gutter", nd.gutter);
    nd.primary_text_frame = p.get("primaryTextFrame").and_then(Value::as_bool).unwrap_or(false);
    match p.get("margins") {
        Some(Value::Number(n)) => nd.margins = designcraft_doc::Margins::uniform(n.as_f64().unwrap_or(36.0)),
        Some(v @ Value::Object(_)) => {
            if let Ok(m) = serde_json::from_value(v.clone()) {
                nd.margins = m;
            }
        }
        _ => {}
    }
    if let Some(b) = p.get("bleed").and_then(Value::as_f64) {
        nd.bleed = [b; 4];
    }
    s.untitled += 1;
    nd.title = str_param(p, "title").map(str::to_string).unwrap_or_else(|| format!("Untitled-{}", s.untitled));
    if nd.width <= 0.0 || nd.height <= 0.0 || nd.width > 15552.0 || nd.height > 15552.0 {
        return Err(bad("file.new", "page size out of range (0 < size ≤ 216 in)"));
    }
    let d = Document::new(&nd);
    let i = s.add_document(DocState::new(d, None));
    Ok(json!({"index": i}))
}

fn file_sample(s: &mut Session, _p: &Value) -> Result<Value> {
    let d = crate::sample::magazine();
    let i = s.add_document(DocState::new(d, None));
    Ok(json!({"index": i}))
}

fn file_open(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("file.open", "missing `path`"))?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bytes = std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
        let d = from_bytes(&bytes)?;
        let i = s.add_document(DocState::new(d, Some(path.to_string())));
        Ok(json!({"index": i}))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (s, path);
        Err(EngineError::Other("use file.openBytes on the web".into()))
    }
}

fn file_open_bytes(s: &mut Session, p: &Value) -> Result<Value> {
    let b = base64_decode(str_param(p, "base64").unwrap_or(""));
    let mut d = from_bytes(&b)?;
    if let Some(n) = str_param(p, "name") {
        d.title = n.to_string();
    }
    let i = s.add_document(DocState::new(d, None));
    Ok(json!({"index": i}))
}

fn file_save(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc_mut()?;
    let path = str_param(p, "path")
        .map(str::to_string)
        .or_else(|| st.path.clone())
        .ok_or_else(|| bad("file.save", "missing `path` (document has never been saved)"))?;
    let bytes = to_bytes(&st.doc);
    #[cfg(not(target_arch = "wasm32"))]
    std::fs::write(&path, &bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    st.path = Some(path.clone());
    st.saved_revision = st.revision;
    st.saved_doc = st.doc.clone();
    Ok(json!({"path": path, "bytes": bytes.len()}))
}

fn file_place(s: &mut Session, p: &Value) -> Result<Value> {
    let (bytes, name, link) = if let Some(b) = str_param(p, "base64") {
        (base64_decode(b), str_param(p, "name").unwrap_or("image").to_string(), None)
    } else if let Some(path) = str_param(p, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        let b = std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
        #[cfg(target_arch = "wasm32")]
        let b: Vec<u8> = vec![];
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.into());
        (b, name, Some(path.to_string()))
    } else {
        s.ui_requests.push(crate::UiRequest::Pick { purpose: "place".into(), params: json!({}) });
        return ok();
    };
    let (pw, ph) = designcraft_render::image_size(&bytes).ok_or_else(|| bad("file.place", "unsupported or corrupt image"))?;
    // 72 ppi by default unless the file says otherwise; scale so it fits the page when huge.
    let (nw, nh) = (pw as f64, ph as f64);
    let target_frame = super::id_param(p, "frame").or_else(|| {
        let st = s.active()?;
        st.selection.items.iter().copied().find(|i| st.doc.item(*i).is_some_and(|it| matches!(it.content, Content::Unassigned | Content::Graphic(_))))
    });
    let spread = super::spread_param(p, "spread");
    let lid = s.doc()?.active_layer;
    let px = p.get("x").and_then(Value::as_f64);
    let py = p.get("y").and_then(Value::as_f64);
    let want_w = p.get("width").and_then(Value::as_f64);
    s.edit(|d, sel| {
        let aid = AssetId(d.alloc());
        let mime = designcraft_render::image_mime(&bytes).to_string();
        d.assets.insert(aid, Arc::new(Asset { id: aid, name, mime, link, data: Arc::new(bytes), pixels: Some((pw, ph)) }));
        let (w, h) = match want_w {
            Some(w) => (w, w * nh / nw),
            None => {
                let page_w = d.settings.page_width * 0.6;
                if nw > page_w { (page_w, page_w * nh / nw) } else { (nw, nh) }
            }
        };
        let id = if let Some(fid) = target_frame {
            let it = d.item_mut(fid).ok_or(designcraft_doc::DocError::NoItem(fid))?;
            let r = it.inner_bounds();
            // Fill frame proportionally.
            let k = (r.width() / nw).max(r.height() / nh);
            let (gw, gh) = (nw * k, nh * k);
            it.content = Content::Graphic(Graphic {
                asset: aid,
                size: (nw, nh),
                xf: Affine::translate((r.x0 + (r.width() - gw) / 2.0, r.y0 + (r.height() - gh) / 2.0)) * Affine::scale(k),
                auto_fit: designcraft_doc::Fitting::FillProportionally,
            });
            fid
        } else {
            let sp = d.spread(spread).ok_or_else(|| bad("file.place", "no such spread"))?;
            let pr = sp.pages.first().map(|pg| pg.margin_rect()).unwrap_or(Rect::new(36.0, 36.0, 300.0, 300.0));
            let (x, y) = (px.unwrap_or(pr.x0), py.unwrap_or(pr.y0));
            let id = ItemId(d.alloc());
            let mut it = Item::new(id, lid, Shape::Rectangle, shapes::rectangle(Rect::new(x, y, x + w, y + h)));
            it.object_style = d.styles.default_graphic_frame.clone();
            it.content = Content::Graphic(Graphic {
                asset: aid,
                size: (nw, nh),
                xf: Affine::translate((x, y)) * Affine::scale(w / nw),
                auto_fit: Default::default(),
            });
            d.insert_item(spread, it, None)?;
            id
        };
        *sel = Selection::items(vec![id]);
        Ok(json!({"id": id.0, "asset": aid.0}))
    })
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn base64_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        } as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

fn read_source(p: &Value) -> Result<(Vec<u8>, String, Option<String>)> {
    if let Some(b) = str_param(p, "base64") {
        return Ok((base64_decode(b), str_param(p, "name").unwrap_or("image").to_string(), None));
    }
    let path = str_param(p, "path").ok_or_else(|| bad("place", "missing `path` or `base64`"))?;
    #[cfg(not(target_arch = "wasm32"))]
    let b = std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    #[cfg(target_arch = "wasm32")]
    let b: Vec<u8> = vec![];
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.into());
    Ok((b, name, Some(path.to_string())))
}

fn place_load(s: &mut Session, p: &Value) -> Result<Value> {
    let (bytes, name, link) = read_source(p)?;
    let (pw, ph) = designcraft_render::image_size(&bytes).ok_or_else(|| bad("place.load", "unsupported or corrupt image"))?;
    let aid = s.edit(|d, _| {
        let aid = AssetId(d.alloc());
        let mime = designcraft_render::image_mime(&bytes).to_string();
        d.assets
            .insert(aid, Arc::new(Asset { id: aid, name: name.clone(), mime, link: link.clone(), data: Arc::new(bytes), pixels: Some((pw, ph)) }));
        Ok(aid)
    })?;
    s.loaded = Some((aid, (pw as f64, ph as f64)));
    s.set_tool("placeGun");
    Ok(json!({"asset": aid.0, "name": name, "width": pw, "height": ph}))
}

fn place_drop(s: &mut Session, p: &Value) -> Result<Value> {
    let (aid, (nw, nh)) = s.loaded.ok_or_else(|| bad("place.drop", "nothing loaded in the place cursor"))?;
    let sr = super::spread_param(p, "spread");
    let lid = s.doc()?.active_layer;
    let frame = super::id_param(p, "frame");
    let rect = super::rect_param(p, "rect");
    let (x, y) = (super::f64_or(p, "x", 0.0), super::f64_or(p, "y", 0.0));
    let r = s.edit(|d, sel| {
        let id = match frame {
            Some(fid) => {
                let it = d.item_mut(fid).ok_or(designcraft_doc::DocError::NoItem(fid))?;
                let r = it.inner_bounds();
                let k = (r.width() / nw).max(r.height() / nh);
                it.content = Content::Graphic(Graphic {
                    asset: aid,
                    size: (nw, nh),
                    xf: Affine::translate((r.x0 + (r.width() - nw * k) / 2.0, r.y0 + (r.height() - nh * k) / 2.0)) * Affine::scale(k),
                    auto_fit: designcraft_doc::Fitting::FillProportionally,
                });
                fid
            }
            None => {
                // Drag: fit proportionally into the dragged rect; click: actual size at the point.
                let (frame_r, k) = match rect {
                    Some(r) if r.width() > 2.0 && r.height() > 2.0 => {
                        let k = (r.width() / nw).min(r.height() / nh);
                        (Rect::new(r.x0, r.y0, r.x0 + nw * k, r.y0 + nh * k), k)
                    }
                    _ => (Rect::new(x, y, x + nw, y + nh), 1.0),
                };
                let id = ItemId(d.alloc());
                let mut it = Item::new(id, lid, Shape::Rectangle, shapes::rectangle(frame_r));
                it.object_style = d.styles.default_graphic_frame.clone();
                it.content = Content::Graphic(Graphic {
                    asset: aid,
                    size: (nw, nh),
                    xf: Affine::translate((frame_r.x0, frame_r.y0)) * Affine::scale(k),
                    auto_fit: Default::default(),
                });
                d.insert_item(sr, it, None)?;
                id
            }
        };
        *sel = Selection::items(vec![id]);
        Ok(json!({"id": id.0}))
    })?;
    s.loaded = None;
    s.set_tool("selection");
    Ok(r)
}
