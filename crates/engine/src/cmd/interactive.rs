//! Hyperlinks and bookmarks (Window → Interactive), exported to PDF as link annotations and the outline.

use designcraft_doc::{Bookmark, Hyperlink, HyperlinkDest, HyperlinkSource};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "hyperlink.create",
            "New Hyperlink…",
            ["Type", "Hyperlinks & Cross-References"],
            None,
            "{url? | email? | page? (1-based), name?, ids?} — from the selected text, or the selected frames",
            has_doc,
            create
        ),
        cmd!("hyperlink.delete", "Delete Hyperlink", [], None, "{id}", has_doc, |s, p| {
            let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad("hyperlink.delete", "missing id"))?;
            s.edit(|d, _| {
                d.hyperlinks.retain(|h| h.id != id);
                Ok(Value::Null)
            })
        }),
        cmd!(query "hyperlink.list", "Hyperlinks", [], None, "{}", has_doc, |s, _| Ok(serde_json::to_value(&s.doc()?.doc.hyperlinks).unwrap_or_default())),
        cmd!("bookmark.add", "New Bookmark", [], None, "{name?, page? (1-based; default: current selection's page or 1)}", has_doc, |s, p| {
            let page = p.get("page").and_then(Value::as_u64).map(|v| (v as usize).saturating_sub(1)).unwrap_or(0);
            let n = s.doc()?.doc.page_count();
            if page >= n {
                return Err(bad("bookmark.add", format!("no page {}", page + 1)));
            }
            let name = str_param(p, "name").map(str::to_string);
            s.edit(|d, _| {
                let name = name.clone().unwrap_or_else(|| format!("Bookmark {}", d.bookmarks.len() + 1));
                d.bookmarks.push(Bookmark { name, page, children: vec![] });
                Ok(json!({"index": d.bookmarks.len() - 1}))
            })
        }),
        cmd!("bookmark.delete", "Delete Bookmark", [], None, "{index}", has_doc, |s, p| {
            let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("bookmark.delete", "missing index"))? as usize;
            s.edit(|d, _| {
                if i < d.bookmarks.len() {
                    d.bookmarks.remove(i);
                }
                Ok(Value::Null)
            })
        }),
        cmd!(query "bookmark.list", "Bookmarks", [], None, "{}", has_doc, |s, _| Ok(serde_json::to_value(&s.doc()?.doc.bookmarks).unwrap_or_default())),
    ]
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let dest = if let Some(u) = str_param(p, "url") {
        HyperlinkDest::Url(u.to_string())
    } else if let Some(e) = str_param(p, "email") {
        HyperlinkDest::Email(e.to_string())
    } else if let Some(pg) = p.get("page").and_then(Value::as_u64) {
        HyperlinkDest::Page((pg as usize).saturating_sub(1))
    } else {
        return Err(bad("hyperlink.create", "give `url`, `email` or `page`"));
    };
    let st = s.doc()?;
    let sources: Vec<HyperlinkSource> = if let Some(ids) = super::ids_param(p, "ids") {
        ids.into_iter().map(|id| HyperlinkSource::Item { id }).collect()
    } else if let Some(t) = st.selection.text.filter(|t| !t.is_caret()) {
        vec![HyperlinkSource::Text { story: t.story, start: t.range().start, end: t.range().end }]
    } else {
        st.selection.items.iter().map(|id| HyperlinkSource::Item { id: *id }).collect()
    };
    if sources.is_empty() {
        return Err(bad("hyperlink.create", "select text or frames first"));
    }
    let name = str_param(p, "name").map(str::to_string);
    s.edit(|d, _| {
        let mut ids = vec![];
        for src in sources {
            let id = d.alloc();
            let name = name.clone().unwrap_or_else(|| match &dest {
                HyperlinkDest::Url(u) => u.clone(),
                HyperlinkDest::Email(e) => e.clone(),
                HyperlinkDest::Page(p) => format!("Page {}", p + 1),
            });
            d.hyperlinks.push(Hyperlink { id, name, source: src, dest: dest.clone() });
            ids.push(id);
        }
        Ok(json!({"ids": ids}))
    })
}
