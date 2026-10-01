//! Edit menu: undo/redo, selection, clear, clipboard.

use std::sync::Arc;

use designcraft_doc::{ItemId, Selection, SpreadRef};
use serde_json::{Value, json};

use super::{CommandSpec, bool_or, cmd, has_doc, has_selection, ids_param, ok};
use crate::{HistoryEntry, Result, Session};

fn can_undo(s: &Session) -> std::result::Result<(), String> {
    s.active().filter(|d| !d.history.undo.is_empty()).map(|_| ()).ok_or_else(|| "nothing to undo".into())
}
fn can_redo(s: &Session) -> std::result::Result<(), String> {
    s.active().filter(|d| !d.history.redo.is_empty()).map(|_| ()).ok_or_else(|| "nothing to redo".into())
}
fn has_clip(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)?;
    if s.clipboard.is_some() { Ok(()) } else { Err("clipboard is empty".into()) }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "edit.undo", "Undo", ["Edit"], Some("Cmd+Z"), "{}", can_undo, |s, _| {
            let st = s.doc_mut()?;
            let e = st.history.undo.pop().expect("checked");
            st.history.redo.push(HistoryEntry { label: e.label.clone(), doc: st.doc.clone(), selection: st.selection.clone() });
            st.doc = e.doc;
            st.selection = e.selection;
            st.revision += 1;
            Ok(json!({"undone": e.label}))
        }),
        cmd!(noundo "edit.redo", "Redo", ["Edit"], Some("Cmd+Shift+Z"), "{}", can_redo, |s, _| {
            let st = s.doc_mut()?;
            let e = st.history.redo.pop().expect("checked");
            st.history.undo.push(HistoryEntry { label: e.label.clone(), doc: st.doc.clone(), selection: st.selection.clone() });
            st.doc = e.doc;
            st.selection = e.selection;
            st.revision += 1;
            Ok(json!({"redone": e.label}))
        }),
        cmd!(noundo "selection.set", "Select", [], None, "{ids: [id], add?: bool, content?: bool}", has_doc, |s, p| {
            let ids = ids_param(p, "ids").unwrap_or_default();
            let add = bool_or(p, "add", false);
            let content = bool_or(p, "content", false);
            let st = s.doc_mut()?;
            let ids: Vec<ItemId> = ids.into_iter().filter(|i| st.doc.item(*i).is_some()).collect();
            if add {
                for i in ids {
                    if !st.selection.items.contains(&i) {
                        st.selection.items.push(i);
                    }
                }
            } else {
                st.selection = Selection { items: ids, content, ..Default::default() };
            }
            st.selection.text = None;
            st.revision += 1;
            ok()
        }),
        cmd!(noundo "selection.toggle", "Toggle Selection", [], None, "{id}", has_doc, |s, p| {
            let id = super::id_param(p, "id").ok_or_else(|| super::bad("selection.toggle", "missing id"))?;
            let st = s.doc_mut()?;
            st.selection.text = None;
            if let Some(i) = st.selection.items.iter().position(|x| *x == id) {
                st.selection.items.remove(i);
            } else {
                st.selection.items.push(id);
            }
            st.revision += 1;
            ok()
        }),
        cmd!(noundo "edit.selectAll", "Select All", ["Edit"], Some("Cmd+A"), "{} — all items on the active spreads, or all text in the story", has_doc, |s, _| {
            let st = s.doc_mut()?;
            if let Some(t) = st.selection.text {
                let len = st.doc.story(t.story).map(|x| x.len()).unwrap_or(0);
                st.selection.text = Some(designcraft_doc::TextSel { anchor: 0, focus: len, ..t });
            } else {
                let mut ids = Vec::new();
                let parents = st.editing_parents;
                let refs: Vec<SpreadRef> = st.doc.spread_refs().filter(|r| matches!(r, SpreadRef::Parent(_)) == parents).collect();
                for r in refs {
                    if let Some(sp) = st.doc.spread(r) {
                        for it in &sp.items {
                            let locked = it.locked || st.doc.layer(it.layer).is_some_and(|l| l.locked || !l.visible);
                            if !locked && !it.hidden {
                                ids.push(it.id);
                            }
                        }
                    }
                }
                st.selection = Selection::items(ids);
            }
            st.revision += 1;
            ok()
        }),
        cmd!(noundo "edit.deselectAll", "Deselect All", ["Edit"], Some("Cmd+Shift+A"), "{}", has_doc, |s, _| {
            let st = s.doc_mut()?;
            st.selection = Selection::default();
            st.revision += 1;
            ok()
        }),
        cmd!("edit.clear", "Clear", ["Edit"], Some("Delete"), "{ids?} — delete selected items (or the selected text)", has_doc, |s, p| {
            if s.doc()?.selection.text.is_some() && p.get("ids").is_none() {
                return super::text::delete_selection(s);
            }
            let ids = super::targets(s, p)?;
            s.edit(|d, sel| {
                for id in &ids {
                    let _ = d.remove_item(*id);
                }
                *sel = Selection::default();
                Ok(json!({"deleted": ids.len()}))
            })
        }),
        cmd!(noundo "edit.copy", "Copy", ["Edit"], Some("Cmd+C"), "{}", has_selection, |s, _| {
            s.clipboard = Some(Arc::new(clip_doc(s)?));
            ok()
        }),
        cmd!("edit.cut", "Cut", ["Edit"], Some("Cmd+X"), "{}", has_selection, |s, p| {
            s.clipboard = Some(Arc::new(clip_doc(s)?));
            s.execute("edit.clear", p)
        }),
        cmd!("edit.paste", "Paste", ["Edit"], Some("Cmd+V"), "{inPlace?: bool}", has_clip, |s, p| {
            let clip = s.clipboard.clone().expect("checked");
            let off = if bool_or(p, "inPlace", false) { 0.0 } else { 12.0 };
            let ids: Vec<ItemId> = clip.spreads.first().map(|sp| sp.items.iter().map(|i| i.id).collect()).unwrap_or_default();
            s.edit(|d, sel| {
                let new = super::object::duplicate_from(d, &clip, &ids, SpreadRef::Doc(0), designcraft_geom::Vec2::new(off, off))?;
                *sel = Selection::items(new.clone());
                Ok(json!({"ids": new.iter().map(|i| i.0).collect::<Vec<_>>()}))
            })
        }),
        cmd!("edit.pasteInPlace", "Paste in Place", ["Edit"], Some("Cmd+Alt+Shift+V"), "{}", has_clip, |s, _| s
            .execute("edit.paste", &json!({"inPlace": true}))),
        cmd!("edit.duplicate", "Duplicate", ["Edit"], Some("Cmd+Alt+Shift+D"), "{}", has_selection, |s, _| s
            .execute("transform.move", &json!({"dx": 12.0, "dy": 12.0, "copy": true}))),
    ]
}

/// A document holding copies of the selected items (and their stories) on spread 0.
fn clip_doc(s: &Session) -> Result<designcraft_doc::Document> {
    let st = s.doc()?;
    let mut d = (*st.doc).clone();
    let keep: Vec<ItemId> = st.selection.items.clone();
    // Gather the selected items (keep their spread coordinates) into spread 0.
    let mut items = Vec::new();
    for id in &keep {
        if let Some(it) = st.doc.item(*id) {
            items.push(Arc::new(it.clone()));
        }
    }
    for sp in d.spreads.iter_mut() {
        Arc::make_mut(sp).items.clear();
    }
    if let Some(sp) = d.spreads.first_mut() {
        Arc::make_mut(sp).items = items;
    }
    Ok(d)
}

#[allow(dead_code)]
fn _unused(_: Value) {}
