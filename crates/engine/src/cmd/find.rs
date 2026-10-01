//! Edit → Find/Change: plain text and GREP (regular expressions) with InDesign's metacharacters,
//! optional formatting in the change, scoped to the document, a story or the selection.

use designcraft_doc::{StoryId, TextSel, story};
use serde_json::{Value, json};

use super::{CommandSpec, bad, bool_or, cmd, has_doc, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "find.find", "Find", ["Edit", "Find/Change"], None,
            "{find, grep?: bool, caseSensitive?: bool, wholeWord?: bool, scope?: document|story|selection, story?} → matches [{story, start, end, text}]",
            has_doc, find),
        cmd!(
            "find.change",
            "Change All",
            ["Edit", "Find/Change"],
            None,
            "{find, change, grep?, caseSensitive?, wholeWord?, scope?, story?, first?: bool (change only the first match), attrs?: {character attributes applied to changed text}} → {count}",
            has_doc,
            change
        ),
        cmd!(noundo "find.next", "Find Next", ["Edit"], Some("Cmd+Alt+F"), "{find, grep?, …} — selects the next match after the caret", has_doc, find_next),
    ]
}

/// Translate InDesign text/GREP metacharacters to our story encoding.
fn metachars(s: &str, grep: bool) -> String {
    let pairs: &[(&str, &str)] = if grep {
        &[
            ("~b", "\n"),
            ("\\r", "\n"),
            ("~#", "\u{E000}"),
            ("~x", "\u{E001}"),
            ("~M", "\u{E002}"),
            ("~R", "\u{E003}"),
            ("~P", "\u{E004}"),
            ("~=", "\u{2014}"),
            ("~_", "\u{2014}"),
            ("~-", "\u{AD}"),
            ("~S", "\u{A0}"),
            ("~n", "\u{2028}"),
        ]
    } else {
        &[
            ("^p", "\n"),
            ("^t", "\t"),
            ("^n", "\u{2028}"),
            ("^#", "\u{E000}"),
            ("^_", "\u{2014}"),
            ("^=", "\u{2013}"),
            ("^-", "\u{AD}"),
            ("^s", "\u{A0}"),
            ("^^", "^"),
        ]
    };
    let mut out = s.to_string();
    for (a, b) in pairs {
        out = out.replace(a, b);
    }
    out
}

fn build(p: &Value) -> Result<regex::Regex> {
    let f = str_param(p, "find").ok_or_else(|| bad("find", "missing `find`"))?;
    if f.is_empty() {
        return Err(bad("find", "empty search"));
    }
    let grep = bool_or(p, "grep", false);
    let mut pat = if grep { metachars(f, true) } else { regex::escape(&metachars(f, false)) };
    if bool_or(p, "wholeWord", false) {
        pat = format!(r"\b(?:{pat})\b");
    }
    regex::RegexBuilder::new(&pat)
        .case_insensitive(!bool_or(p, "caseSensitive", false))
        .multi_line(true)
        .build()
        .map_err(|e| bad("find", format!("bad GREP: {e}")))
}

fn scope(s: &Session, p: &Value) -> Result<Vec<(StoryId, std::ops::Range<usize>)>> {
    let st = s.doc()?;
    let d = &st.doc;
    if let Some(sid) = p.get("story").and_then(Value::as_u64) {
        let sid = StoryId(sid);
        let len = d.story(sid).map(|x| x.len()).ok_or_else(|| bad("find", "no such story"))?;
        return Ok(vec![(sid, 0..len)]);
    }
    match str_param(p, "scope").unwrap_or("document") {
        "story" => {
            let t = st.selection.text.ok_or_else(|| bad("find", "no story selected"))?;
            Ok(vec![(t.story, 0..d.story(t.story).map(|x| x.len()).unwrap_or(0))])
        }
        "selection" => {
            let t = st.selection.text.ok_or_else(|| bad("find", "no text selected"))?;
            Ok(vec![(t.story, t.range())])
        }
        _ => Ok(d.stories.values().map(|x| (x.id, 0..x.len())).collect()),
    }
}

fn find(s: &mut Session, p: &Value) -> Result<Value> {
    let re = build(p)?;
    let st = s.doc()?;
    let mut out = Vec::new();
    for (sid, r) in scope(s, p)? {
        let Some(story) = st.doc.story(sid) else { continue };
        let text = &story.text[r.clone()];
        for m in re.find_iter(text) {
            out.push(json!({"story": sid.0, "start": r.start + m.start(), "end": r.start + m.end(), "text": m.as_str()}));
            if out.len() >= 10_000 {
                break;
            }
        }
    }
    Ok(Value::Array(out))
}

/// Expand `$1`… (GREP) or text metacharacters in the change string.
fn replacement(p: &Value) -> String {
    let c = str_param(p, "change").unwrap_or("");
    metachars(c, bool_or(p, "grep", false))
}

fn change(s: &mut Session, p: &Value) -> Result<Value> {
    let re = build(p)?;
    let grep = bool_or(p, "grep", false);
    let rep = replacement(p);
    let first_only = bool_or(p, "first", false);
    let attrs = p.get("attrs").cloned();
    let mut over = designcraft_doc::CharAttrs::default();
    if let Some(o) = attrs.as_ref().and_then(Value::as_object) {
        for (k, v) in o {
            over.set_json(k, v).map_err(|e| bad("find.change", e))?;
        }
    }
    let targets = scope(s, p)?;
    s.edit(|d, sel| {
        let mut count = 0usize;
        for (sid, r) in targets {
            let Some(story) = d.story(sid) else { continue };
            // Collect matches first (positions refer to the original text), apply back to front.
            let text = story.text[r.clone()].to_string();
            let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
            for caps in re.captures_iter(&text) {
                let m = caps.get(0).expect("match");
                let mut out = String::new();
                if grep {
                    caps.expand(&rep, &mut out);
                } else {
                    out = rep.clone();
                }
                edits.push((r.start + m.start()..r.start + m.end(), out));
                if first_only {
                    break;
                }
            }
            let st = d.story_mut(sid).expect("story");
            for (range, new) in edits.iter().rev() {
                st.replace(range.clone(), new);
                if !over.is_empty() && !new.is_empty() {
                    st.format_chars(range.start..range.start + new.len(), |f| f.over.merge(&over));
                }
                count += 1;
            }
            if let Some(t) = sel.text.as_mut().filter(|t| t.story == sid) {
                let len = st.len();
                t.anchor = t.anchor.min(len);
                t.focus = t.focus.min(len);
            }
        }
        Ok(json!({"count": count}))
    })
}

fn find_next(s: &mut Session, p: &Value) -> Result<Value> {
    let re = build(p)?;
    let st = s.doc()?;
    let (start_story, start_pos) = st.selection.text.map(|t| (Some(t.story), t.range().end)).unwrap_or((None, 0));
    let mut order: Vec<StoryId> = st.doc.stories.keys().copied().collect();
    if let Some(ss) = start_story
        && let Some(i) = order.iter().position(|x| *x == ss)
    {
        order.rotate_left(i);
    }
    let mut hit = None;
    for (k, sid) in order.iter().enumerate() {
        let Some(story) = st.doc.story(*sid) else { continue };
        let from = if k == 0 && start_story == Some(*sid) { start_pos.min(story.len()) } else { 0 };
        if let Some(m) = re.find_at(&story.text, from) {
            hit = Some((*sid, m.start(), m.end()));
            break;
        }
    }
    let Some((sid, a, b)) = hit else { return Ok(Value::Null) };
    let frame = st.doc.story(sid).and_then(|x| x.frames.first().copied());
    let stm = s.doc_mut()?;
    stm.selection = designcraft_doc::Selection::text(TextSel { story: sid, anchor: a, focus: b, frame });
    stm.revision += 1;
    let _ = story::PAGE_NUMBER;
    Ok(json!({"story": sid.0, "start": a, "end": b}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn find_and_change_text_and_grep() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("frame.create", &json!({"rect": [36, 36, 300, 300], "content": "text", "text": "Call 555-1234 or 555-9876.\nColour and colour."}))
            .unwrap();
        let m = s.execute("find.find", &json!({"find": "colour"})).unwrap();
        assert_eq!(m.as_array().unwrap().len(), 2);
        let m = s.execute("find.find", &json!({"find": "colour", "caseSensitive": true})).unwrap();
        assert_eq!(m.as_array().unwrap().len(), 1);
        let r = s.execute("find.change", &json!({"find": r"(\d{3})-(\d{4})", "change": "($1) $2", "grep": true})).unwrap();
        assert_eq!(r["count"], 2);
        let r = s.execute("find.change", &json!({"find": "colour", "change": "color", "attrs": {"fontStyle": "Italic"}})).unwrap();
        assert_eq!(r["count"], 2);
        let d = s.execute("document.inspect", &json!({})).unwrap();
        let sid = d["stories"][0]["id"].as_u64().unwrap();
        let text = s.execute("story.get", &json!({"story": sid})).unwrap()["text"].as_str().unwrap().to_string();
        assert_eq!(text, "Call (555) 1234 or (555) 9876.\ncolor and color.");
        // Paragraph metacharacter in text mode.
        assert_eq!(s.execute("find.find", &json!({"find": ".^pc"})).unwrap().as_array().unwrap().len(), 1);
        s.execute("find.next", &json!({"find": "or"})).unwrap();
        assert!(s.doc().unwrap().selection.text.is_some());
    }
}
