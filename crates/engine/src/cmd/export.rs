//! File › Export: Adobe PDF (Print).

use designcraft_pdf::{Marks, PdfOptions, Standard};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "file.exportPdf", "Export PDF…", ["File"], None,
        "{path?, pages?: \"1-3,5\" | [1,3] (1-based positions; default all), flatten?: high|medium|low|ppi (Transparency Flattener: spreads with transparency are rasterised), spreads?: bool, bleed?: bool (document bleed), marks?: bool | {crop?, bleed?, pageInfo?, weight?, offset?}, standard?: \"none\"|\"x4\"|\"a2b\", compressImages?: bool, tagged?: bool (structure tree: stories as paragraphs, figures with alt text), title?, author?} → {path, bytes, pages, warnings} (no path: {base64, …})",
        has_doc, export_pdf),
        cmd!(noundo "file.exportEpub", "Export EPUB (Reflowable)…", ["File"], None,
        "{path?, title?, author?, language?: \"en\"} → {path, bytes} (no path: {base64, bytes})",
        has_doc, export_epub),
        cmd!(noundo "file.printBooklet", "Print Booklet…", ["File"], None,
        "{path?, type?: saddleStitch|twoUpConsecutive, spaceBetween? (pt)} — printer spreads as PDF (pages imposed in booklet order) → {path, bytes, sheets} (no path: {base64, …})",
        has_doc, print_booklet),
        cmd!(noundo "file.exportHtml", "Export HTML…", ["File"], None,
        "{path?, title?, language?} — one self-contained page (styles inline, images embedded), stories and graphics in reading order → {path, bytes} (no path: {text, bytes})",
        has_doc, export_html),
        cmd!(noundo "file.exportText", "Export Text…", ["File"], None,
        "{path?, format?: \"txt\"|\"rtf\" (default from the path, else txt), story?, frame?} — the story being edited or of the selected frame → {path, bytes} (no path: {text, bytes})",
        has_story_target, export_text),
    ]
}

fn print_booklet(s: &mut Session, p: &Value) -> Result<Value> {
    const ID: &str = "file.printBooklet";
    let kind = match str_param(p, "type").unwrap_or("saddleStitch") {
        "saddleStitch" => designcraft_pdf::BookletKind::SaddleStitch,
        "twoUpConsecutive" | "twoUp" => designcraft_pdf::BookletKind::TwoUpConsecutive,
        t => return Err(bad(ID, format!("unknown type `{t}` (saddleStitch, twoUpConsecutive)"))),
    };
    let opts = designcraft_pdf::BookletOptions { kind, space_between: p.get("spaceBetween").and_then(Value::as_f64).unwrap_or(0.0), title: None };
    let st = s.doc()?;
    let r = designcraft_pdf::export_booklet(&st.doc, &s.cache, &opts).map_err(|e| EngineError::Other(e.to_string()))?;
    match str_param(p, "path") {
        Some(path) => {
            #[cfg(not(target_arch = "wasm32"))]
            std::fs::write(path, &r.bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            Ok(json!({"path": path, "bytes": r.bytes.len(), "sheets": r.pages, "warnings": r.warnings}))
        }
        None => Ok(json!({"base64": super::file::base64_encode(&r.bytes), "bytes": r.bytes.len(), "sheets": r.pages, "warnings": r.warnings})),
    }
}

fn export_html(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let opts =
        designcraft_epub::HtmlOptions { title: str_param(p, "title").map(str::to_string), language: str_param(p, "language").map(str::to_string) };
    let text = designcraft_epub::export_html(&st.doc, &opts);
    match str_param(p, "path") {
        Some(path) => {
            #[cfg(not(target_arch = "wasm32"))]
            std::fs::write(path, text.as_bytes()).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            Ok(json!({"path": path, "bytes": text.len()}))
        }
        None => Ok(json!({"bytes": text.len(), "text": text})),
    }
}

fn has_story_target(s: &Session) -> std::result::Result<(), String> {
    super::text::story_of(s, &Value::Null).map(|_| ()).ok_or_else(|| "edit text or select a text frame".into())
}

fn export_text(s: &mut Session, p: &Value) -> Result<Value> {
    const ID: &str = "file.exportText";
    let sid = super::text::story_of(s, p).ok_or_else(|| bad(ID, "edit text or select a text frame"))?;
    let doc = &s.doc()?.doc;
    let story = doc.stories.get(&sid).ok_or_else(|| bad(ID, format!("no story {}", sid.0)))?;
    let path = str_param(p, "path");
    let rtf = match str_param(p, "format") {
        Some("rtf") => true,
        Some("txt") | Some("text") => false,
        Some(f) => return Err(bad(ID, format!("unknown format `{f}` (txt, rtf)"))),
        None => path.is_some_and(|x| x.to_ascii_lowercase().ends_with(".rtf")),
    };
    let text = if rtf { designcraft_textimport::export::rtf(doc, story) } else { designcraft_textimport::export::plain_text(story) };
    match path {
        Some(path) => {
            #[cfg(not(target_arch = "wasm32"))]
            std::fs::write(path, text.as_bytes()).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            Ok(json!({"path": path, "bytes": text.len()}))
        }
        None => Ok(json!({"bytes": text.len(), "text": text})),
    }
}

fn export_epub(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let opts = designcraft_epub::EpubOptions {
        title: p.get("title").and_then(Value::as_str).map(str::to_string),
        author: p.get("author").and_then(Value::as_str).map(str::to_string),
        language: p.get("language").and_then(Value::as_str).unwrap_or("en").to_string(),
        identifier: None,
    };
    let bytes = designcraft_epub::export_epub(&st.doc, &opts).map_err(|e| crate::EngineError::Other(e.to_string()))?;
    match p.get("path").and_then(Value::as_str) {
        Some(path) => {
            #[cfg(not(target_arch = "wasm32"))]
            std::fs::write(path, &bytes).map_err(|e| crate::EngineError::Other(format!("{path}: {e}")))?;
            Ok(serde_json::json!({"path": path, "bytes": bytes.len()}))
        }
        None => Ok(serde_json::json!({"base64": super::file::base64_encode(&bytes), "bytes": bytes.len()})),
    }
}

pub(crate) fn options(p: &Value, page_count: usize) -> Result<PdfOptions> {
    const ID: &str = "file.exportPdf";

    let pages = match p.get("pages") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(designcraft_pdf::parse_page_range(s, page_count).map_err(|e| bad(ID, e.to_string()))?),
        Some(Value::Array(a)) => {
            let mut v = Vec::new();
            for x in a {
                let n = x.as_u64().filter(|n| *n >= 1 && (*n as usize) <= page_count).ok_or_else(|| bad(ID, format!("bad page {x}")))?;
                v.push(n as usize - 1);
            }
            Some(v)
        }
        Some(Value::Number(n)) => {
            let n = n.as_u64().filter(|n| *n >= 1 && (*n as usize) <= page_count).ok_or_else(|| bad(ID, format!("bad page {n}")))?;
            Some(vec![n as usize - 1])
        }
        Some(v) => return Err(bad(ID, format!("bad `pages`: {v}"))),
    };
    let spreads = p.get("spreads").and_then(Value::as_bool).unwrap_or(false);
    let bleed = p.get("bleed").and_then(Value::as_bool).unwrap_or(false);
    let marks = match p.get("marks") {
        Some(Value::Bool(true)) => Marks::ALL,
        Some(Value::Object(m)) => {
            let b = |k: &str| m.get(k).and_then(Value::as_bool).unwrap_or(false);
            Marks {
                crop: b("crop"),
                bleed: b("bleed"),
                page_info: b("pageInfo"),
                weight: m.get("weight").and_then(Value::as_f64).unwrap_or(0.25),
                offset: m.get("offset").and_then(Value::as_f64).unwrap_or(6.0),
            }
        }
        _ => Marks::NONE,
    };
    let standard = match str_param(p, "standard") {
        Some(s) => Standard::parse(s).ok_or_else(|| bad(ID, format!("unknown standard `{s}` (none, x4, a2b)")))?,
        None => Standard::None,
    };
    Ok(PdfOptions {
        pages,
        spreads,
        bleed,
        marks,
        standard,
        compress_images: p.get("compressImages").and_then(Value::as_bool).unwrap_or(false),
        title: str_param(p, "title").map(str::to_string),
        author: str_param(p, "author").map(str::to_string),
        tagged: p.get("tagged").and_then(Value::as_bool).unwrap_or(false),
        ..PdfOptions::default()
    })
}

/// Does this object (or anything in it) need the transparency flattener?
fn transparent(it: &designcraft_doc::Item) -> bool {
    it.opacity < 1.0 || it.blend != Default::default() || it.effects.any() || it.knockout || it.children().iter().any(|c| transparent(c))
}

/// Transparency Flattener: spreads with transparency become one raster per page at `ppi`
/// (in a copy of the document made for export). Returns the copy and the flattened page count.
fn flatten(d: &designcraft_doc::Document, cache: &designcraft_compose::Cache, ppi: f64) -> (designcraft_doc::Document, usize) {
    use designcraft_doc::{Asset, AssetId, Content, Graphic, Item, ItemId, Shape, SpreadRef};
    let mut out = d.clone();
    let mut n = 0;
    let mut rr = designcraft_render::Renderer::new();
    rr.threads = designcraft_render::default_threads();
    let k = ppi / 72.0;
    let parent_transparent = |pid: Option<designcraft_doc::SpreadId>| {
        pid.and_then(|id| d.parents.iter().find(|p| p.id == id)).is_some_and(|p| p.items.iter().any(|it| transparent(it)))
    };
    for (si, sp) in d.spreads.iter().enumerate() {
        if !sp.items.iter().any(|it| transparent(it)) && !sp.pages.iter().any(|pg| pg.show_parent_items && parent_transparent(pg.parent)) {
            continue;
        }
        let first = d.first_page_of_spread(si);
        let mut images = Vec::new();
        for (pi, pg) in sp.pages.iter().enumerate() {
            if let Some(img) = rr.render_page(d, cache, first + pi, k, false, &Default::default()) {
                images.push((pg.bounds(), img));
            }
        }
        let lid = out.default_layer();
        let osp = std::sync::Arc::make_mut(&mut out.spreads[si]);
        osp.items.clear();
        for pg in &mut osp.pages {
            pg.show_parent_items = false;
        }
        for (b, img) in images {
            let (w, h) = (img.width, img.height);
            let aid = AssetId(out.alloc());
            out.assets.insert(
                aid,
                std::sync::Arc::new(Asset {
                    page: 0,
                    id: aid,
                    name: format!("flattened-{}.png", aid.0),
                    mime: "image/png".into(),
                    link: None,
                    data: std::sync::Arc::new(img.to_png()),
                    pixels: Some((w, h)),
                }),
            );
            let id = ItemId(out.alloc());
            let mut it = Item::new(id, lid, Shape::Rectangle, designcraft_geom::shapes::rectangle(b));
            it.content = Content::Graphic(Graphic {
                asset: aid,
                size: (w as f64, h as f64),
                xf: designcraft_geom::Affine::translate((b.x0, b.y0))
                    * designcraft_geom::Affine::scale_non_uniform(b.width() / w as f64, b.height() / h as f64),
                auto_fit: Default::default(),
                fit_align: 4,
                crop: [0.0; 4],
            });
            it.stroke.weight = 0.0;
            let _ = out.insert_item(SpreadRef::Doc(si), it, None);
            n += 1;
        }
    }
    (out, n)
}

/// A copy of `d` where placed PDFs with Object Layer Options hidden layers are 300 ppi images.
fn rasterize_layered(d: &designcraft_doc::Document) -> (Option<designcraft_doc::Document>, usize) {
    let ids: Vec<designcraft_doc::ItemId> =
        d.all_items().into_iter().filter(|id| d.item(*id).is_some_and(|it| !it.pdf_hidden_layers.is_empty() && it.graphic().is_some())).collect();
    if ids.is_empty() {
        return (None, 0);
    }
    let mut out = d.clone();
    let mut n = 0;
    for id in ids {
        let Some((g, hidden)) = out.item(id).and_then(|it| Some((it.graphic()?.clone(), it.pdf_hidden_layers.clone()))) else { continue };
        let Some(a) = out.assets.get(&g.asset).cloned() else { continue };
        if !designcraft_render::is_pdf(&a.data) {
            continue;
        }
        let side = (g.size.0.max(g.size.1) * 300.0 / 72.0).clamp(64.0, 8000.0) as u32;
        let Some(png) = designcraft_render::pdf_page_png(&a.data, a.page as usize, side, &hidden) else { continue };
        let px = designcraft_render::image_size(&png);
        let aid = designcraft_doc::AssetId(out.alloc());
        out.assets.insert(
            aid,
            std::sync::Arc::new(designcraft_doc::Asset {
                page: 0,
                id: aid,
                name: format!("{}.png", a.name),
                mime: "image/png".into(),
                link: None,
                data: std::sync::Arc::new(png),
                pixels: px,
            }),
        );
        if let Some(designcraft_doc::Content::Graphic(gg)) = out.item_mut(id).map(|it| &mut it.content) {
            gg.asset = aid;
        }
        n += 1;
    }
    (Some(out), n)
}

fn export_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let opts = options(p, st.doc.page_count())?;
    // Transparency Flattener presets: High / Medium / Low Resolution, or a ppi.
    let ppi = match p.get("flatten") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::String(s)) if s == "high" => Some(300.0),
        Some(Value::String(s)) if s == "medium" => Some(150.0),
        Some(Value::String(s)) if s == "low" => Some(72.0),
        Some(Value::Bool(true)) => Some(300.0),
        Some(v) => Some(
            v.as_f64().filter(|v| (36.0..=1200.0).contains(v)).ok_or_else(|| bad("file.exportPdf", "`flatten`: high|medium|low or 36–1200 ppi"))?,
        ),
    };
    let flattened = ppi.map(|ppi| flatten(&st.doc, &s.cache, ppi));
    // Placed PDFs with hidden layers go out as images showing just their visible layers.
    let base: &designcraft_doc::Document = flattened.as_ref().map_or(&st.doc, |f| &f.0);
    let (layered, layer_count) = rasterize_layered(base);
    let doc: &designcraft_doc::Document = layered.as_ref().unwrap_or(base);
    let r = designcraft_pdf::export_pdf_with_report(doc, &s.cache, &opts).map_err(|e| EngineError::Other(e.to_string()))?;
    let mut r = r;
    // Transparency Blend Space: the page group of pages with transparency.
    let spreads = designcraft_pdf::sheet_spreads(doc, &opts);
    let blended: Vec<bool> = spreads.iter().map(|si| doc.spreads.get(*si).is_some_and(|sp| sp.items.iter().any(|it| transparent(it)))).collect();
    if blended.iter().any(|b| *b) {
        match designcraft_pdf::add_blend_space(&r.bytes, &blended, doc.settings.blend_space == designcraft_doc::BlendSpace::Cmyk) {
            Some(b) => r.bytes = b,
            None => r.warnings.push("the transparency blend space couldn't be set".into()),
        }
    }
    // Page transitions (each spread's first page holds them).
    let trans: Vec<Option<designcraft_doc::PageTransition>> =
        spreads.into_iter().map(|si| doc.spreads.get(si).and_then(|sp| sp.pages.first()).and_then(|pg| pg.transition)).collect();
    if trans.iter().any(Option::is_some) {
        match designcraft_pdf::add_transitions(&r.bytes, &trans) {
            Some(b) => r.bytes = b,
            None => r.warnings.push("page transitions couldn't be added to this PDF".into()),
        }
    }
    if layer_count > 0 {
        r.warnings.push(format!("{layer_count} placed PDF(s) with hidden layers exported as images"));
    }
    if let Some((_, n)) = &flattened
        && *n > 0
    {
        r.warnings.push(format!("transparency flattened: {n} page(s) rasterised"));
    }
    match str_param(p, "path") {
        Some(path) => {
            write_file(path, &r.bytes)?;
            Ok(json!({"path": path, "bytes": r.bytes.len(), "pages": r.pages, "warnings": r.warnings}))
        }
        None => Ok(json!({"base64": super::base64_encode(&r.bytes), "bytes": r.bytes.len(), "pages": r.pages, "warnings": r.warnings})),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_file(path: &str, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))
}
#[cfg(target_arch = "wasm32")]
fn write_file(path: &str, _bytes: &[u8]) -> Result<()> {
    Err(EngineError::Other(format!("{path}: no file system on the web; omit `path` to get the bytes")))
}

#[cfg(test)]
mod text_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn flattener_rasterises_spreads_with_transparency() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 3})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [100, 100, 300, 300]})).unwrap()["id"].clone();
        s.execute("object.fill", &json!({"swatch": "[Black]", "ids": [id]})).unwrap();
        // Opaque: nothing to flatten.
        let r = s.execute("file.exportPdf", &json!({"flatten": "low"})).unwrap();
        assert!(r["warnings"].as_array().unwrap().iter().all(|w| !w.as_str().unwrap().contains("flattened")));
        s.execute("object.opacity", &json!({"ids": [id], "opacity": 0.5})).unwrap();
        let r = s.execute("file.exportPdf", &json!({"flatten": "low"})).unwrap();
        assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("flattened: 1 page")), "{r}");
        assert_eq!(r["pages"], 3);
        let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
        assert_eq!(designcraft_render::pdf_page_count(&bytes), Some(3));
        // The document itself is untouched.
        assert!(s.doc().unwrap().doc.item(designcraft_doc::ItemId(id.as_u64().unwrap())).is_some());
        assert!(s.execute("file.exportPdf", &json!({"flatten": 5})).is_err());
        // Unflattened, the page blends in the document's blend space (CMYK for print).
        let r = s.execute("file.exportPdf", &json!({})).unwrap();
        let text = String::from_utf8_lossy(&super::super::file::base64_decode(r["base64"].as_str().unwrap())).to_string();
        assert_eq!(text.matches("/S/Transparency/CS/DeviceCMYK").count(), 1, "only the page with transparency");
    }

    #[test]
    fn export_story_as_text_and_rtf() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": "One\nTwo"})).unwrap();
        assert!(s.execute("file.exportText", &json!({"frame": r["id"]})).unwrap()["text"] == "One\r\nTwo");
        let html = s.execute("file.exportHtml", &json!({})).unwrap();
        assert!(html["text"].as_str().unwrap().contains("One</p>"));
        let rtf = s.execute("file.exportText", &json!({"frame": r["id"], "format": "rtf"})).unwrap();
        assert!(rtf["text"].as_str().unwrap().starts_with("{\\rtf1"));
        let dir = std::env::temp_dir().join(format!("dc-export-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("story.rtf");
        s.execute("file.exportText", &json!({"frame": r["id"], "path": path.to_string_lossy()})).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("{\\rtf1"));
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod tagged_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn tagged_pdf_has_structure_and_alt_text() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": "Hello"})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 320, 200, 420]})).unwrap();
        s.execute("object.altText", &json!({"text": "A plain box", "ids": [r["id"]]})).unwrap();
        let pdf = |s: &mut Session, tagged: bool| {
            let b64 = s.execute("file.exportPdf", &json!({"tagged": tagged})).unwrap()["base64"].as_str().unwrap().to_string();
            String::from_utf8_lossy(&super::super::file::base64_decode(&b64)).into_owned()
        };
        let t = pdf(&mut s, true);
        assert!(t.contains("/StructTreeRoot") && t.contains("/Figure") && t.contains("A plain box"), "tagged");
        assert!(t.contains("/P") && t.contains("/MarkInfo"));
        assert!(!pdf(&mut s, false).contains("/StructTreeRoot"));
    }
}

#[cfg(test)]
mod variable_font_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn variable_font_instance_exports_to_pdf() {
        let Ok(data) = std::fs::read("/System/Library/Fonts/Supplemental/Skia.ttf") else { return };
        designcraft_fonts::FontDb::global().add_font(data);
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": "Variable"})).unwrap();
        s.execute("text.select", &json!({"story": r["story"], "anchor": 0, "focus": 8})).unwrap();
        s.execute("type.char", &json!({"fontFamily": "Skia", "fontStyle": "Black"})).unwrap();
        let out = s.execute("file.exportPdf", &json!({})).unwrap();
        assert!(out["bytes"].as_u64().unwrap() > 1000);
        let fonts = s.execute("font.list", &json!({})).unwrap();
        assert!(fonts.to_string().contains("Black"), "{fonts}");
    }
}

#[cfg(test)]
mod booklet_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn booklet_imposes_pages_in_pairs() {
        use designcraft_pdf::{BookletKind, booklet_pairs};
        assert_eq!(booklet_pairs(8, BookletKind::SaddleStitch), [(Some(7), Some(0)), (Some(1), Some(6)), (Some(5), Some(2)), (Some(3), Some(4))]);
        assert_eq!(booklet_pairs(6, BookletKind::SaddleStitch)[0], (None, Some(0)), "padded with blanks");
        assert_eq!(booklet_pairs(3, BookletKind::TwoUpConsecutive), [(Some(0), Some(1)), (Some(2), None)]);
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 4})).unwrap();
        let r = s.execute("file.printBooklet", &json!({"spaceBetween": 18})).unwrap();
        assert_eq!(r["sheets"], 2);
        let pdf = String::from_utf8_lossy(&super::super::file::base64_decode(r["base64"].as_str().unwrap())).into_owned();
        let mb = pdf.find("/MediaBox").map(|i| pdf[i..i + 40].to_string()).unwrap_or_default();
        assert!(mb.contains("1242"), "two 612 pt pages and an 18 pt gap: {mb}");
        assert!(s.execute("file.printBooklet", &json!({"type": "perfectBound"})).is_err());
        // Page 1's black box lands on the right half of the first sheet (page 4 is on the left).
        let id = s.execute("frame.create", &json!({"rect": [100, 100, 300, 300]})).unwrap()["id"].clone();
        s.execute("object.fill", &json!({"swatch": "[Black]", "ids": [id]})).unwrap();
        let r = s.execute("file.printBooklet", &json!({})).unwrap();
        let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
        let px = designcraft_render::decode_pixmap_page(&bytes, 0).expect("rasterised");
        let (w, h) = (px.width() as f64, px.height() as f64);
        let at = |x: f64, y: f64| px.sample((x / 1224.0 * w) as u16, (y / 792.0 * h) as u16);
        let (ink, blank) = (at(612.0 + 200.0, 200.0), at(200.0, 200.0));
        assert!(ink.a > 200 && ink.r < 60, "right half has page 1: {ink:?}");
        assert!(blank.a < 30 || blank.r > 200, "left half (page 4) is empty: {blank:?}");
    }
}
