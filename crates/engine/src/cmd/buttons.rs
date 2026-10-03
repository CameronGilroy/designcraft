//! Window › Interactive › Buttons and Forms: objects that act when clicked in an interactive PDF
//! (go to a page, the next / previous / first / last page, or a URL).

use designcraft_doc::ButtonAction;
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_selection, targets};
use crate::Result;

fn parse(p: &Value) -> Result<Option<ButtonAction>> {
    let a = p.get("action").and_then(Value::as_str).unwrap_or("nextPage");
    Ok(Some(match a {
        "none" => return Ok(None),
        "page" => ButtonAction::GoToPage {
            page: p.get("page").and_then(Value::as_u64).ok_or_else(|| bad("button.set", "`page` (0-based) required"))? as usize,
        },
        "firstPage" => ButtonAction::GoToFirstPage,
        "lastPage" => ButtonAction::GoToLastPage,
        "nextPage" => ButtonAction::GoToNextPage,
        "previousPage" => ButtonAction::GoToPreviousPage,
        "url" => ButtonAction::GoToUrl { url: p.get("url").and_then(Value::as_str).ok_or_else(|| bad("button.set", "`url` required"))?.to_string() },
        o => return Err(bad("button.set", format!("unknown action `{o}`"))),
    }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "button.set",
            "Convert to Button",
            ["Object", "Interactive"],
            None,
            "{ids?, action: page|firstPage|lastPage|nextPage|previousPage|url|none, page?, url?} — the selected objects act on release in interactive PDF",
            has_selection,
            |s, p| {
                let ids = targets(s, p)?;
                let action = parse(p)?;
                s.edit(|d, _| {
                    for id in &ids {
                        if let Some(it) = d.item_mut(*id) {
                            it.button = action.clone();
                        }
                    }
                    Ok(json!({"buttons": ids.len()}))
                })
            }
        ),
        cmd!("button.clear", "Convert to Object", ["Object", "Interactive"], None, "{ids?}", has_selection, |s, p| {
            let mut q = p.clone();
            q["action"] = json!("none");
            s.execute("button.set", &q)
        }),
        cmd!(query "button.list", "Buttons", [], None, "{} → [{id, name, action}]", super::has_doc, |s, _| {
            let d = &s.doc()?.doc;
            let mut out = Vec::new();
            for it in d.all_items().into_iter().filter_map(|id| d.item(id)) {
                if let Some(b) = &it.button {
                    out.push(json!({"id": it.id.0, "name": it.name, "action": b}));
                }
            }
            Ok(Value::Array(out))
        }),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn buttons_become_pdf_links() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 3})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [72, 72, 200, 120]})).unwrap()["id"].clone();
        s.execute("button.set", &json!({"ids": [id], "action": "lastPage"})).unwrap();
        assert_eq!(s.execute("button.list", &json!({})).unwrap()[0]["action"]["kind"], "goToLastPage");
        let links = |s: &mut Session| {
            let r = s.execute("file.exportPdf", &json!({})).unwrap();
            let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
            String::from_utf8_lossy(&bytes).matches("/Link").count()
        };
        let with = links(&mut s);
        s.execute("button.clear", &json!({"ids": [id]})).unwrap();
        assert!(with > links(&mut s), "the button exports a link annotation");
        assert!(s.execute("button.list", &json!({})).unwrap().as_array().unwrap().is_empty());
    }
}
