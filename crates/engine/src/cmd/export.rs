//! File › Export: Adobe PDF (Print).

use designcraft_pdf::{Marks, PdfOptions, Standard};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "file.exportPdf", "Export PDF…", ["File"], None,
        "{path?, pages?: \"1-3,5\" | [1,3] (1-based positions; default all), spreads?: bool, bleed?: bool (document bleed), marks?: bool | {crop?, bleed?, pageInfo?, weight?, offset?}, standard?: \"none\"|\"x4\"|\"a2b\", compressImages?: bool, title?, author?} → {path, bytes, pages, warnings} (no path: {base64, …})",
        has_doc, export_pdf),
        cmd!(noundo "file.exportEpub", "Export EPUB (Reflowable)…", ["File"], None,
        "{path?, title?, author?, language?: \"en\"} → {path, bytes} (no path: {base64, bytes})",
        has_doc, export_epub),
    ]
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
