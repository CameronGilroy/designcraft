//! History panel: every undo state, as a list the panel can jump, trim, and branch from.
//!
//! `document.history` stays `{undo, redo}` label arrays. The panel reads `document.history.states`.

use std::sync::Arc;

use designcraft_doc::{Document, Selection};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc};
use crate::{DocState, HistoryEntry, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            noundo "edit.history.go",
            "Go to History State",
            [],
            None,
            "{index: number}",
            has_doc,
            |s, p| go(s, p)
        ),
        cmd!(
            noundo "edit.history.delete",
            "Delete History State",
            [],
            None,
            "{index?: number}",
            has_doc,
            |s, p| delete_state(s, p)
        ),
        cmd!(noundo "edit.history.clear", "Clear History", [], None, "{}", has_doc, |s, _| clear(s)),
        cmd!(
            noundo "edit.history.newDocument",
            "New Document",
            [],
            None,
            "{index?: number}",
            has_doc,
            |s, p| new_document(s, p)
        ),
        cmd!(query "document.history.states", "History States", [], None, "{}", has_doc, |s, _| states(s)),
    ]
}

/// One undo step, shared with Edit › Undo. Does not leave data-merge preview.
pub(crate) fn undo_once(s: &mut Session) -> Result<String> {
    let st = s.doc_mut()?;
    let e = st.history.undo.pop().ok_or_else(|| bad("edit.undo", "nothing to undo"))?;
    let label = e.label.clone();
    st.history.redo.push(HistoryEntry { label: label.clone(), doc: st.doc.clone(), selection: st.selection.clone() });
    let mut doc = e.doc;
    super::rulers::keep_zero_point(&st.doc, &mut doc);
    st.doc = doc;
    st.selection = e.selection;
    st.bump_revision();
    // Pending typing overrides belong to the current edit state, not a restored snapshot.
    s.typing_format = None;
    Ok(label)
}

/// One redo step, shared with Edit › Redo.
pub(crate) fn redo_once(s: &mut Session) -> Result<String> {
    let st = s.doc_mut()?;
    let e = st.history.redo.pop().ok_or_else(|| bad("edit.redo", "nothing to redo"))?;
    let label = e.label.clone();
    st.history.undo.push(HistoryEntry { label: label.clone(), doc: st.doc.clone(), selection: st.selection.clone() });
    let mut doc = e.doc;
    super::rulers::keep_zero_point(&st.doc, &mut doc);
    st.doc = doc;
    st.selection = e.selection;
    st.bump_revision();
    s.typing_format = None;
    Ok(label)
}

fn go(s: &mut Session, p: &Value) -> Result<Value> {
    let (current, max) = lengths(s)?;
    let index = required_index("edit.history.go", p)?;
    if index > max {
        return Err(bad("edit.history.go", "history state is out of range"));
    }
    if index < current {
        for _ in index..current {
            undo_once(s)?;
        }
    } else if index > current {
        for _ in current..index {
            redo_once(s)?;
        }
    }
    Ok(json!({"index": index}))
}

fn delete_state(s: &mut Session, p: &Value) -> Result<Value> {
    let (current, max) = lengths(s)?;
    let index = optional_index("edit.history.delete", p, current)?;
    if index == 0 {
        return Err(bad("edit.history.delete", "the first history state stays"));
    }
    if index > max {
        return Err(bad("edit.history.delete", "history state is out of range"));
    }
    let target = index - 1;
    if current > target {
        for _ in target..current {
            undo_once(s)?;
        }
        s.doc_mut()?.history.redo.clear();
    } else if current == target {
        s.doc_mut()?.history.redo.clear();
    } else {
        let keep = target - current;
        let redo = &mut s.doc_mut()?.history.redo;
        let drop_n = redo.len().saturating_sub(keep);
        redo.drain(0..drop_n);
    }
    let index = s.doc()?.history.undo.len();
    Ok(json!({"index": index}))
}

fn clear(s: &mut Session) -> Result<Value> {
    s.stop_data_preview();
    let st = s.doc_mut()?;
    if st.history.undo.is_empty() && st.history.redo.is_empty() {
        return Ok(json!({"cleared": true}));
    }
    if st.history.undo.is_empty() && st.history.origin.is_none() {
        st.history.redo.clear();
        return Ok(json!({"cleared": true}));
    }
    let label = st.history.undo.last().map(|e| e.label.clone()).or_else(|| st.history.origin.clone()).unwrap_or_else(|| "New".to_string());
    st.history.origin = Some(label);
    st.history.undo.clear();
    st.history.redo.clear();
    Ok(json!({"cleared": true}))
}

fn new_document(s: &mut Session, p: &Value) -> Result<Value> {
    let (current, _) = lengths(s)?;
    let index = optional_index("edit.history.newDocument", p, current)?;
    let (doc, selection) = {
        let st = s.doc()?;
        snapshot(st, index).ok_or_else(|| bad("edit.history.newDocument", "history state is out of range"))?
    };
    s.untitled += 1;
    let mut inner = (*doc).clone();
    inner.title = format!("Untitled-{}", s.untitled);
    let mut state = DocState::new(inner, None);
    state.selection = selection;
    state.saved_doc = Arc::new((*state.doc).clone());
    let i = s.add_document(state);
    Ok(json!({"index": i}))
}

fn states(s: &Session) -> Result<Value> {
    let st = s.doc()?;
    let current = st.history.undo.len();
    let (base_label, base_kind) = baseline(st);
    let mut rows = Vec::with_capacity(current.saturating_add(st.history.redo.len()).saturating_add(1));
    rows.push(json!({
        "index": 0,
        "label": base_label,
        "kind": base_kind,
        "current": current == 0,
        "future": false,
    }));
    for (i, entry) in st.history.undo.iter().enumerate() {
        let index = i + 1;
        rows.push(json!({
            "index": index,
            "label": entry.label,
            "kind": "action",
            "current": index == current,
            "future": false,
        }));
    }
    let redo_len = st.history.redo.len();
    for step in 1..=redo_len {
        let Some(entry) = st.history.redo.get(redo_len - step) else { continue };
        let index = current + step;
        rows.push(json!({
            "index": index,
            "label": entry.label,
            "kind": "action",
            "current": false,
            "future": true,
        }));
    }
    Ok(json!({"current": current, "states": rows}))
}

fn lengths(s: &mut Session) -> Result<(usize, usize)> {
    s.stop_data_preview();
    let st = s.doc()?;
    Ok((st.history.undo.len(), st.history.undo.len().saturating_add(st.history.redo.len())))
}

fn baseline(st: &DocState) -> (String, &'static str) {
    if let Some(label) = &st.history.origin {
        return (label.clone(), "action");
    }
    if let Some(name) = st.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()) {
        return (name.to_string_lossy().into_owned(), "file");
    }
    ("New".to_string(), "new")
}

/// Document and selection at `index` without moving the current state.
fn snapshot(st: &DocState, index: usize) -> Option<(Arc<Document>, Selection)> {
    let current = st.history.undo.len();
    let max = current.saturating_add(st.history.redo.len());
    if index > max {
        return None;
    }
    if index == current {
        return Some((st.doc.clone(), st.selection.clone()));
    }
    if index < current {
        let entry = st.history.undo.get(index)?;
        return Some((entry.doc.clone(), entry.selection.clone()));
    }
    let step = index - current;
    let entry = st.history.redo.get(st.history.redo.len().checked_sub(step)?)?;
    Some((entry.doc.clone(), entry.selection.clone()))
}

fn required_index(cmd: &str, p: &Value) -> Result<usize> {
    match p.get("index") {
        Some(n) => parse_index(cmd, n),
        _ => Err(bad(cmd, "index must be a state number")),
    }
}

fn optional_index(cmd: &str, p: &Value, current: usize) -> Result<usize> {
    match p.get("index") {
        None | Some(Value::Null) => Ok(current),
        Some(n) => parse_index(cmd, n),
    }
}

fn parse_index(cmd: &str, n: &Value) -> Result<usize> {
    let Some(u) = n.as_u64() else {
        return Err(bad(cmd, "index must be a state number"));
    };
    usize::try_from(u).map_err(|_| bad(cmd, "index is out of range"))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::Session;

    fn items(s: &Session) -> usize {
        s.doc().unwrap().doc.spreads[0].items.len()
    }

    fn states(s: &mut Session) -> Value {
        s.execute("document.history.states", &json!({})).unwrap()
    }

    fn labels(v: &Value) -> Vec<String> {
        v["states"].as_array().unwrap().iter().map(|row| row["label"].as_str().unwrap().to_string()).collect()
    }

    fn frame(s: &mut Session) {
        s.execute("frame.create", &json!({"rect": [36.0, 36.0, 136.0, 136.0]})).unwrap();
    }

    #[test]
    fn a_new_document_has_one_state_named_new() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let v = states(&mut s);
        assert_eq!(v["current"], 0);
        assert_eq!(labels(&v), ["New"]);
        assert_eq!(v["states"][0]["kind"], "new");
        assert_eq!(v["states"][0]["current"], true);
        assert_eq!(v["states"][0]["future"], false);
        assert_eq!(s.execute("document.history", &json!({})).unwrap(), json!({"undo": [], "redo": []}));
    }

    #[test]
    fn two_frames_list_new_then_each_action_and_undo_stays_one_step() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        frame(&mut s);
        let v = states(&mut s);
        assert_eq!(labels(&v), ["New", "Create Frame", "Create Frame"]);
        assert_eq!(v["current"], 2);
        assert_eq!(v["states"][2]["current"], true);
        assert_eq!(items(&s), 2);
        assert_eq!(s.execute("document.history", &json!({})).unwrap(), json!({"undo": ["Create Frame", "Create Frame"], "redo": []}));
        assert_eq!(s.execute("edit.undo", &json!({})).unwrap(), json!({"undone": "Create Frame"}));
        assert_eq!(s.doc().unwrap().history.undo.len(), 1);
        assert_eq!(s.doc().unwrap().history.redo.len(), 1);
        assert_eq!(items(&s), 1);
    }

    #[test]
    fn go_moves_between_states_and_rejects_a_missing_one() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        frame(&mut s);
        s.execute("edit.history.go", &json!({"index": 1})).unwrap();
        let v = states(&mut s);
        assert_eq!(v["current"], 1);
        assert_eq!(items(&s), 1);
        assert_eq!(v["states"][2]["future"], true);
        s.execute("edit.history.go", &json!({"index": 0})).unwrap();
        assert_eq!(states(&mut s)["current"], 0);
        assert_eq!(items(&s), 0);
        let err = s.execute("edit.history.go", &json!({"index": 9})).unwrap_err();
        assert!(err.to_string().contains("out of range"), "{err}");
        assert_eq!(items(&s), 0);
        assert_eq!(s.doc().unwrap().history.undo.len(), 0);
    }

    #[test]
    fn delete_refuses_the_first_state_and_drops_the_current_state_onward() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        frame(&mut s);
        let err = s.execute("edit.history.delete", &json!({"index": 0})).unwrap_err();
        assert!(err.to_string().contains("first history state"), "{err}");
        assert_eq!(items(&s), 2);
        assert_eq!(s.doc().unwrap().history.undo.len(), 2);
        s.execute("edit.history.delete", &json!({"index": 2})).unwrap();
        let v = states(&mut s);
        assert_eq!(v["current"], 1);
        assert_eq!(labels(&v), ["New", "Create Frame"]);
        assert!(s.doc().unwrap().history.redo.is_empty());
        assert_eq!(items(&s), 1);
    }

    #[test]
    fn delete_of_a_future_state_truncates_redo_without_moving() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        frame(&mut s);
        s.execute("edit.history.go", &json!({"index": 0})).unwrap();
        assert_eq!(items(&s), 0);
        s.execute("edit.history.delete", &json!({"index": 2})).unwrap();
        let v = states(&mut s);
        assert_eq!(v["current"], 0);
        assert_eq!(items(&s), 0);
        assert_eq!(s.doc().unwrap().history.redo.len(), 1);
        assert_eq!(labels(&v), ["New", "Create Frame"]);
        assert_eq!(v["states"][1]["future"], true);
    }

    #[test]
    fn clear_keeps_the_document_and_names_the_remaining_state() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        frame(&mut s);
        s.execute("edit.history.clear", &json!({})).unwrap();
        let v = states(&mut s);
        assert_eq!(items(&s), 2);
        assert_eq!(labels(&v), ["Create Frame"]);
        assert_eq!(v["states"][0]["kind"], "action");
        assert_eq!(v["current"], 0);
        assert!(s.doc().unwrap().history.undo.is_empty());
        assert!(s.doc().unwrap().history.redo.is_empty());
    }

    #[test]
    fn clear_of_an_undone_document_with_no_origin_stays_new() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        s.execute("edit.undo", &json!({})).unwrap();
        s.execute("edit.history.clear", &json!({})).unwrap();
        let v = states(&mut s);
        assert_eq!(labels(&v), ["New"]);
        assert_eq!(v["states"][0]["kind"], "new");
        assert!(s.doc().unwrap().history.origin.is_none());
        assert!(s.doc().unwrap().history.redo.is_empty());
        assert_eq!(items(&s), 0);
    }

    #[test]
    fn new_document_copies_one_state_and_leaves_the_source_alone() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        frame(&mut s);
        let untitled = s.untitled;
        let made = s.execute("edit.history.newDocument", &json!({"index": 1})).unwrap();
        assert_eq!(made["index"], 1);
        assert_eq!(s.active_index(), Some(1));
        assert_eq!(s.untitled, untitled + 1);
        let new_doc = &s.documents()[1];
        assert_eq!(new_doc.title(), "Untitled-2");
        assert!(new_doc.is_dirty());
        assert!(new_doc.path.is_none());
        assert!(new_doc.history.undo.is_empty());
        assert!(new_doc.history.redo.is_empty());
        assert!(new_doc.fonts.is_none());
        assert_eq!(new_doc.doc.spreads[0].items.len(), 1);
        let source = &s.documents()[0];
        assert_eq!(source.history.undo.len(), 2);
        assert!(source.history.redo.is_empty());
        assert_eq!(source.doc.spreads[0].items.len(), 2);
    }

    #[test]
    fn a_bad_new_document_index_does_not_take_an_untitled_number() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let untitled = s.untitled;
        assert!(s.execute("edit.history.newDocument", &json!({"index": 4})).is_err());
        assert_eq!(s.untitled, untitled);
        assert_eq!(s.documents().len(), 1);
    }

    #[test]
    fn the_limit_names_the_dropped_state_and_never_empties_undo() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.doc_mut().unwrap().history.limit = 1;
        frame(&mut s);
        frame(&mut s);
        frame(&mut s);
        assert_eq!(s.doc().unwrap().history.undo.len(), 1);
        assert_eq!(s.doc().unwrap().history.origin.as_deref(), Some("Create Frame"));
        let v = states(&mut s);
        assert_eq!(v["states"][0]["label"], "Create Frame");
        assert_eq!(v["states"][0]["kind"], "action");
        assert_ne!(v["states"][0]["label"], "New");
        s.doc_mut().unwrap().history.limit = 0;
        frame(&mut s);
        assert_eq!(s.doc().unwrap().history.undo.len(), 1);
    }

    #[test]
    fn a_saved_path_names_the_first_state_until_history_is_trimmed() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.doc_mut().unwrap().path = Some("/tmp/Sample.designcraft".into());
        let v = states(&mut s);
        assert_eq!(v["states"][0]["label"], "Sample.designcraft");
        assert_eq!(v["states"][0]["kind"], "file");
    }

    #[test]
    fn hostile_indexes_are_rejected() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        frame(&mut s);
        for (cmd, value) in [
            ("edit.history.go", json!({"index": -1})),
            ("edit.history.go", json!({"index": 1.5})),
            ("edit.history.go", json!({"index": u64::MAX})),
            ("edit.history.go", json!({})),
            ("edit.history.delete", json!({"index": -1})),
            ("edit.history.delete", json!({"index": 1.5})),
        ] {
            assert!(s.execute(cmd, &value).is_err(), "{cmd} {value}");
        }
        assert_eq!(s.doc().unwrap().history.undo.len(), 1);
        assert_eq!(items(&s), 1);
    }
}
