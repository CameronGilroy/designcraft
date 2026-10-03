//! File › Export: Adobe PDF (Print).

use designcraft_pdf::{Marks, PdfOptions, Standard};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "file.exportPdf", "Export PDF…", ["File"], None,
        "{path?, pages?: \"1-3,5\" | [1,3] (1-based positions; default all), spreads?: bool, bleed?: bool (document bleed), marks?: bool | {crop?, bleed?, pageInfo?, weight?, offset?}, standard?: \"none\"|\"x4\"|\"a2b\", compressImages?: bool, tagged?: bool (structure tree: stories as paragraphs, figures with alt text), title?, author?} → {path, bytes, pages, warnings} (no path: {base64, …})",
        has_doc, export_pdf),
        cmd!(noundo "file.exportEpub", "Export EPUB (Reflowable)…", ["File"], None,
        "{path?, title?, author?, language?: \"en\"} → {path, bytes} (no path: {base64, bytes})",
        has_doc, export_epub),
        cmd!(noundo "file.exportHtml", "Export HTML…", ["File"], None,
        "{path?, title?, language?} — one self-contained page (styles inline, images embedded), stories and graphics in reading order → {path, bytes} (no path: {text, bytes})",
        has_doc, export_html),
        cmd!(noundo "file.exportText", "Export Text…", ["File"], None,
        "{path?, format?: \"txt\"|\"rtf\" (default from the path, else txt), story?, frame?} — the story being edited or of the selected frame → {path, bytes} (no path: {text, bytes})",
        has_story_target, export_text),
    ]
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

fn export_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let opts = options(p, st.doc.page_count())?;
    let r = designcraft_pdf::export_pdf_with_report(&st.doc, &s.cache, &opts).map_err(|e| EngineError::Other(e.to_string()))?;
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
