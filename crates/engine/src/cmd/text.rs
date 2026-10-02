//! Text editing (Type tool) and character/paragraph formatting.

use designcraft_compose as compose;
use designcraft_doc::{CellAddr, CharAttrs, Content, ItemId, ParaAttrs, Selection, StoryId, TextSel};
use designcraft_geom::Point;
use serde_json::{Value, json};

use super::{CommandSpec, bad, bool_or, cmd, has_doc, has_text, has_text_or_frames, id_param, ok, point_param, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "text.placeCaret", "Place Caret", [], None, "{frame, point: [x,y] (spread coords)} — converts empty frames to text frames", has_doc, |s, p| place(s, p, false)),
        cmd!(noundo "text.extendTo", "Extend Text Selection", [], None, "{frame, point}", has_doc, |s, p| place(s, p, true)),
        cmd!(noundo "text.selectWord", "Select Word", [], None, "{frame, point}", has_doc, |s, p| {
            place(s, p, false)?;
            let st = s.doc_mut()?;
            let Some(t) = st.selection.text else { return ok() };
            let text = &st.doc.text_story(t.story, t.cell).map(|x| x.text.clone()).unwrap_or_default();
            let (a, b) = word_bounds(text, t.focus);
            st.selection.text = Some(TextSel { anchor: a, focus: b, ..t });
            ok()
        }),
        cmd!(noundo "text.select", "Select Text", [], None, "{story, anchor, focus}", has_doc, |s, p| {
            let sid = StoryId(p.get("story").and_then(Value::as_u64).unwrap_or(0));
            let st = s.doc_mut()?;
            let len = st.doc.story(sid).ok_or_else(|| bad("text.select", "no such story"))?.len();
            let a = (p.get("anchor").and_then(Value::as_u64).unwrap_or(0) as usize).min(len);
            let f = (p.get("focus").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(a)).min(len);
            st.selection = Selection::text(TextSel { story: sid, anchor: a, focus: f, frame: None, cell: None });
            st.revision += 1;
            ok()
        }),
        cmd!("text.insert", "Type", [], None, "{text} — replaces the selected text", has_text, insert),
        cmd!("text.delete", "Delete Text", [], None, "{forward?: bool, word?: bool}", has_text, delete),
        cmd!(noundo "text.move", "Move Caret", [], None, "{dir: left|right|up|down|lineStart|lineEnd|storyStart|storyEnd, extend?, word?}", has_text, move_caret),
        cmd!(noundo "text.exitToFrame", "Select Frame", [], None, "{}", has_doc, |s, _| {
            let st = s.doc_mut()?;
            if let Some(t) = st.selection.text.take() {
                let f = t.frame.or_else(|| st.doc.story(t.story).and_then(|x| x.frames.first().copied()));
                st.selection.items = f.into_iter().collect();
            }
            st.selection.cells = None;
            st.revision += 1;
            ok()
        }),
        cmd!("story.setText", "Set Story Text", [], None, "{story, text} — replace a story's whole text", has_doc, |s, p| {
            let sid = StoryId(p.get("story").and_then(Value::as_u64).ok_or_else(|| bad("story.setText", "missing story"))?);
            let text = str_param(p, "text").unwrap_or("").to_string();
            s.edit(|d, sel| {
                d.set_story_text(sid, &text)?;
                if let Some(t) = sel.text.as_mut().filter(|t| t.story == sid) {
                    t.anchor = t.anchor.min(text.len());
                    t.focus = t.focus.min(text.len());
                }
                ok()
            })
        }),
        cmd!(
            "story.replaceRange",
            "Edit Story",
            [],
            None,
            "{story, start, end, text} — replace a byte range keeping surrounding formatting",
            has_doc,
            |s, p| {
                let sid = StoryId(p.get("story").and_then(Value::as_u64).ok_or_else(|| bad("story.replaceRange", "missing story"))?);
                let a = p.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
                let b = p.get("end").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(a);
                let text = str_param(p, "text").unwrap_or("").to_string();
                s.edit(|d, sel| {
                    let st = d.story_mut(sid).ok_or(designcraft_doc::DocError::NoStory(sid))?;
                    let (a, b) = (a.min(st.len()), b.min(st.len()).max(a.min(st.len())));
                    st.replace(a..b, &text);
                    let len = st.len();
                    if let Some(t) = sel.text.as_mut().filter(|t| t.story == sid) {
                        t.anchor = t.anchor.min(len);
                        t.focus = t.focus.min(len);
                    }
                    Ok(json!({"length": len}))
                })
            }
        ),
        cmd!(query "story.get", "Get Story", [], None, "{story? | frame?} → text, frames, paragraphs, overset", has_doc, |s, p| {
            let st = s.doc()?;
            let sid = story_of(s, p).ok_or_else(|| bad("story.get", "no story"))?;
            let cs = s.cache.get(&st.doc, sid, None);
            let mut v = st.doc.story_summary(sid).unwrap_or_default();
            v["overset"] = json!(cs.overset_at);
            v["lines"] = json!(cs.line_count());
            Ok(v)
        }),
        cmd!("type.fillWithPlaceholder", "Fill with Placeholder Text", ["Type"], None, "{frame?}", has_text_or_frames, fill_placeholder),
        cmd!(
            "type.char",
            "Character Formatting",
            [],
            None,
            "{attrs: {fontFamily?, fontStyle?, size?, leading?: {kind:auto}|{kind:points,value}, tracking?, kerning?, hScale?, vScale?, baselineShift?, fill?, capitalization?, underline?, …}}",
            has_text_or_frames,
            |s, p| format_chars(s, p.get("attrs").unwrap_or(p))
        ),
        cmd!(
            "type.para",
            "Paragraph Formatting",
            [],
            None,
            "{attrs: {align?, leftIndent?, firstLineIndent?, spaceBefore?, spaceAfter?, dropCapLines?, hyphenate?, composer?, tabs?, …}}",
            has_text_or_frames,
            |s, p| format_paras(s, p.get("attrs").unwrap_or(p))
        ),
        cmd!("type.align", "Align", [], None, "{align: left|center|right|leftJustified|…}", has_text_or_frames, |s, p| {
            format_paras(s, &json!({"align": p.get("align").cloned().unwrap_or(json!("left"))}))
        }),
        cmd!("type.alignLeft", "Align Left", [], Some("Cmd+Shift+L"), "{}", has_text_or_frames, |s, _| format_paras(s, &json!({"align": "left"}))),
        cmd!("type.alignCenter", "Align Center", [], Some("Cmd+Shift+C"), "{}", has_text_or_frames, |s, _| format_paras(
            s,
            &json!({"align": "center"})
        )),
        cmd!("type.alignRight", "Align Right", [], Some("Cmd+Shift+R"), "{}", has_text_or_frames, |s, _| format_paras(s, &json!({"align": "right"}))),
        cmd!("type.justify", "Justify Left", [], Some("Cmd+Shift+J"), "{}", has_text_or_frames, |s, _| format_paras(
            s,
            &json!({"align": "leftJustified"})
        )),
        cmd!("type.bold", "Bold", [], Some("Cmd+Shift+B"), "{}", has_text_or_frames, |s, _| format_chars(s, &json!({"fontStyle": "Bold"}))),
        cmd!("type.italic", "Italic", [], Some("Cmd+Shift+I"), "{}", has_text_or_frames, |s, _| format_chars(s, &json!({"fontStyle": "Italic"}))),
        cmd!("type.underline", "Underline", [], Some("Cmd+Shift+U"), "{}", has_text_or_frames, |s, _| format_chars(s, &json!({"underline": true}))),
        cmd!("type.allCaps", "All Caps", [], Some("Cmd+Shift+K"), "{}", has_text_or_frames, |s, _| format_chars(
            s,
            &json!({"capitalization": "allCaps"})
        )),
        cmd!("type.sizeUp", "Increase Point Size", [], Some("Cmd+Shift+."), "{}", has_text_or_frames, |s, _| step_size(s, 2.0)),
        cmd!("type.sizeDown", "Decrease Point Size", [], Some("Cmd+Shift+,"), "{}", has_text_or_frames, |s, _| step_size(s, -2.0)),
        cmd!(query "type.selectionAttrs", "Selection Attributes", [], None, "{} → resolved character/paragraph attributes at the text selection", has_doc, selection_attrs),
    ]
}

fn story_of(s: &Session, p: &Value) -> Option<StoryId> {
    let st = s.active()?;
    if let Some(v) = p.get("story").and_then(Value::as_u64) {
        return Some(StoryId(v));
    }
    let frame = id_param(p, "frame").or_else(|| st.selection.items.first().copied());
    if let Some(f) = frame {
        return st.doc.item(f).and_then(|i| i.text_frame()).map(|t| t.story);
    }
    st.selection.text.map(|t| t.story)
}

/// Story byte at spread point `pt` in text frame `frame` (in a table cell's story when the point is in a cell).
fn hit_byte(s: &Session, frame: ItemId, pt: Point) -> Option<(StoryId, usize, Option<CellAddr>)> {
    let st = s.active()?;
    let loc = st.doc.find(frame)?;
    let it = st.doc.item_at(&loc)?;
    let sid = it.text_frame()?.story;
    let xf = st.doc.parent_xf(&loc) * it.xf;
    let inner = xf.inverse() * pt;
    let cs = s.cache.get(&st.doc, sid, None);
    let fi = cs.frames.iter().position(|f| f.frame == frame)?;
    if let Some((table, row, col, b)) = compose::hit_cell(&cs, fi, inner) {
        return Some((sid, b, Some(CellAddr { table, row, col })));
    }
    if let Some((id, b)) = compose::hit_note(&cs, fi, inner) {
        return Some((sid, b, Some(CellAddr::footnote(id))));
    }
    let b = compose::hit(&cs, fi, inner).unwrap_or(0);
    Some((sid, b, None))
}

fn place(s: &mut Session, p: &Value, extend: bool) -> Result<Value> {
    let frame = id_param(p, "frame").ok_or_else(|| bad("text.placeCaret", "missing frame"))?;
    let pt = point_param(p, "point").unwrap_or(Point::ZERO);
    // Empty frames become text frames.
    let needs_convert = s.doc()?.doc.item(frame).is_some_and(|i| matches!(i.content, Content::Unassigned));
    if needs_convert {
        s.execute("object.content", &json!({"ids": [frame.0], "type": "text"}))?;
    }
    let (sid, b, cell) = hit_byte(s, frame, pt).ok_or_else(|| bad("text.placeCaret", "not a text frame"))?;
    let st = s.doc_mut()?;
    // Dragging from one cell into another selects cells.
    if extend
        && let (Some(t), Some(c)) = (st.selection.text, cell)
        && let Some(a) = t.cell
        && t.story == sid
        && a.table == c.table
        && (a.row, a.col) != (c.row, c.col)
    {
        let range = designcraft_doc::CellRange::new(a.row, a.col, c.row, c.col);
        st.selection.cells = Some(designcraft_doc::TableSel { story: sid, table: c.table, range });
        st.revision += 1;
        return Ok(json!({"story": sid.0, "cells": range}));
    }
    let t = match (extend, st.selection.text) {
        (true, Some(t)) if t.story == sid && t.cell == cell => TextSel { focus: b, frame: Some(frame), ..t },
        _ => TextSel { story: sid, anchor: b, focus: b, frame: Some(frame), cell },
    };
    st.selection = Selection::text(t);
    st.revision += 1;
    Ok(json!({"story": sid.0, "pos": b, "cell": cell}))
}

fn insert(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").unwrap_or("").to_string();
    // Tab in a table cell moves to the next cell (Shift-Tab: `table.prevCell`).
    if text == "\t" && s.doc()?.selection.text.is_some_and(|t| t.cell.is_some()) {
        return super::table::step_cell(s, true);
    }
    let text = if s.prefs.typographers_quotes { smart_quotes(s, &text) } else { text };
    s.edit(|d, sel| {
        let t = sel.text.ok_or_else(|| bad("text.insert", "no insertion point"))?;
        let st = d.text_story_mut(t.story, t.cell).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
        let r = t.range();
        let r = r.start.min(st.len())..r.end.min(st.len());
        // Typing next to a table anchor starts a paragraph of its own.
        let mut text = text.clone();
        let mut trail = 0;
        if t.cell.is_none() && !text.is_empty() && st.table_at(r.start).is_some() {
            let pi = st.para_at(r.start);
            let pr = st.para_ranges()[pi].clone();
            let anchor_at = pr.start + st.text[pr.clone()].find(designcraft_doc::TABLE_ANCHOR).unwrap_or(0);
            if r.start <= anchor_at {
                if !text.ends_with('\n') {
                    text.push('\n');
                    trail = 1;
                }
            } else if !text.starts_with('\n') {
                text.insert(0, '\n');
            }
        }
        st.replace(r.clone(), &text);
        let pos = r.start + text.len() - trail;
        sel.text = Some(TextSel { anchor: pos, focus: pos, ..t });
        Ok(json!({"pos": pos}))
    })
}

fn smart_quotes(s: &Session, text: &str) -> String {
    if !text.contains(['"', '\'']) {
        return text.to_string();
    }
    let prev = s
        .active()
        .and_then(|st| {
            let t = st.selection.text?;
            let story = st.doc.text_story(t.story, t.cell)?;
            story.text[..t.range().start.min(story.len())].chars().last()
        })
        .unwrap_or(' ');
    let mut out = String::with_capacity(text.len());
    let mut last = prev;
    for c in text.chars() {
        let open = last.is_whitespace() || matches!(last, '(' | '[' | '{' | '\u{2014}' | '\u{2013}');
        let r = match c {
            '"' => {
                if open {
                    '\u{201C}'
                } else {
                    '\u{201D}'
                }
            }
            '\'' => {
                if open {
                    '\u{2018}'
                } else {
                    '\u{2019}'
                }
            }
            c => c,
        };
        out.push(r);
        last = c;
    }
    out
}

pub(crate) fn delete_selection(s: &mut Session) -> Result<Value> {
    s.edit(|d, sel| {
        let t = sel.text.ok_or_else(|| bad("text.delete", "no text"))?;
        let r = t.range();
        if let Some(st) = d.text_story_mut(t.story, t.cell) {
            st.delete(r.clone());
        }
        sel.text = Some(TextSel { anchor: r.start, focus: r.start, ..t });
        ok()
    })
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let forward = bool_or(p, "forward", false);
    let word = bool_or(p, "word", false);
    s.edit(|d, sel| {
        let t = sel.text.ok_or_else(|| bad("text.delete", "no text"))?;
        let st = d.text_story_mut(t.story, t.cell).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
        let r = t.range();
        let r = if !r.is_empty() {
            r
        } else if forward {
            let end = if word { next_word(&st.text, r.start) } else { next_char(&st.text, r.start) };
            r.start..end
        } else {
            let start = if word { prev_word(&st.text, r.start) } else { prev_char(&st.text, r.start) };
            start..r.start
        };
        st.delete(r.clone());
        sel.text = Some(TextSel { anchor: r.start, focus: r.start, ..t });
        ok()
    })
}

pub fn next_char(s: &str, i: usize) -> usize {
    s[i.min(s.len())..].chars().next().map(|c| i + c.len_utf8()).unwrap_or(s.len())
}
pub fn prev_char(s: &str, i: usize) -> usize {
    s[..i.min(s.len())].chars().next_back().map(|c| i - c.len_utf8()).unwrap_or(0)
}
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '’'
}
pub fn next_word(s: &str, i: usize) -> usize {
    let mut j = i;
    let mut seen = false;
    for (k, c) in s[i..].char_indices() {
        if is_word(c) {
            seen = true;
        } else if seen {
            return i + k;
        }
        j = i + k + c.len_utf8();
    }
    j
}
pub fn prev_word(s: &str, i: usize) -> usize {
    let mut seen = false;
    for (k, c) in s[..i].char_indices().rev() {
        if is_word(c) {
            seen = true;
        } else if seen {
            return k + c.len_utf8();
        }
    }
    0
}
fn word_bounds(s: &str, i: usize) -> (usize, usize) {
    let a = s[..i.min(s.len())].char_indices().rev().take_while(|(_, c)| is_word(*c)).last().map(|(k, _)| k).unwrap_or(i);
    let b = s[i.min(s.len())..].char_indices().find(|(_, c)| !is_word(*c)).map(|(k, _)| i + k).unwrap_or(s.len());
    (a, b)
}

fn move_caret(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = str_param(p, "dir").unwrap_or("right").to_string();
    let extend = bool_or(p, "extend", false);
    let word = bool_or(p, "word", false);
    let st = s.doc()?;
    let t = st.selection.text.ok_or_else(|| bad("text.move", "no caret"))?;
    let story = st.doc.text_story(t.story, t.cell).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
    let text = story.text.clone();
    let cs = s.cache.get(&st.doc, t.story, None);
    // In a cell, lines come from the cell's own composition.
    let cs = match t.cell {
        Some(c) if c.footnote_id().is_some() => compose::find_note(&cs, c.row as u64).map(|(_, n)| n.text.clone()).unwrap_or(cs),
        Some(c) => compose::find_cell(&cs, c.table, c.row, c.col).map(|(_, _, pc)| pc.text.clone()).unwrap_or(cs),
        None => cs,
    };
    let pos = t.focus.min(text.len());
    let collapse_to = |left: bool| if left { t.range().start } else { t.range().end };
    let new = match dir.as_str() {
        "left" if !extend && !t.is_caret() => collapse_to(true),
        "right" if !extend && !t.is_caret() => collapse_to(false),
        "left" => {
            if word {
                prev_word(&text, pos)
            } else {
                prev_char(&text, pos)
            }
        }
        "right" => {
            if word {
                next_word(&text, pos)
            } else {
                next_char(&text, pos)
            }
        }
        "up" | "down" => vertical(&cs, pos, dir == "up").unwrap_or(pos),
        "lineStart" => line_of(&cs, pos).map(|l| l.0).unwrap_or(0),
        "lineEnd" => line_of(&cs, pos).map(|l| l.1).unwrap_or(text.len()),
        "storyStart" => 0,
        "storyEnd" => text.len(),
        other => return Err(bad("text.move", format!("unknown dir `{other}`"))),
    };
    let stm = s.doc_mut()?;
    stm.selection.text = Some(if extend { TextSel { focus: new, ..t } } else { TextSel { anchor: new, focus: new, ..t } });
    stm.revision += 1;
    Ok(json!({"pos": new}))
}

fn line_of(cs: &compose::ComposedStory, pos: usize) -> Option<(usize, usize)> {
    cs.frames.iter().flat_map(|f| f.lines.iter()).find(|l| pos >= l.range.start && pos <= l.range.end).map(|l| (l.range.start, l.range.end))
}

fn vertical(cs: &compose::ComposedStory, pos: usize, up: bool) -> Option<usize> {
    let (fi, x, baseline, _, _) = compose::caret(cs, pos)?;
    let lines: Vec<(usize, &compose::Line)> = cs.frames.iter().enumerate().flat_map(|(i, f)| f.lines.iter().map(move |l| (i, l))).collect();
    let cur = lines.iter().position(|(i, l)| *i == fi && (l.baseline - baseline).abs() < 0.01 && pos >= l.range.start && pos <= l.range.end)?;
    let target = if up { cur.checked_sub(1)? } else { cur + 1 };
    let (tf, tl) = lines.get(target)?;
    compose::hit(cs, *tf, Point::new(x, tl.baseline - 1.0))
}

/// Where a formatting command applies: a story (or a table cell's story) and a byte range.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub story: StoryId,
    pub cell: Option<CellAddr>,
    pub range: std::ops::Range<usize>,
}

/// Ranges and stories a formatting command applies to: selected table cells (whole cells), the
/// text selection, or whole stories of the selected text frames.
fn format_targets(s: &Session) -> Vec<Target> {
    let Some(st) = s.active() else { return vec![] };
    if let Some(ts) = st.selection.cells
        && let Some(t) = st.doc.story(ts.story).and_then(|x| x.tables.get(&ts.table))
    {
        let mut v = Vec::new();
        let owners = t.owners();
        for r in ts.range.r0..=ts.range.r1.min(t.nrows().saturating_sub(1)) {
            for c in ts.range.c0..=ts.range.c1.min(t.ncols().saturating_sub(1)) {
                if owners[r * t.ncols() + c] == (r, c) {
                    let len = t.cell(r, c).map_or(0, |x| x.text.len());
                    v.push(Target { story: ts.story, cell: Some(CellAddr { table: ts.table, row: r, col: c }), range: 0..len });
                }
            }
        }
        return v;
    }
    if let Some(t) = st.selection.text {
        return vec![Target { story: t.story, cell: t.cell, range: t.range() }];
    }
    let mut v: Vec<Target> = Vec::new();
    for id in &st.selection.items {
        if let Some(tf) = st.doc.item(*id).and_then(|i| i.text_frame()) {
            let len = st.doc.story(tf.story).map(|x| x.len()).unwrap_or(0);
            if !v.iter().any(|t| t.story == tf.story) {
                v.push(Target { story: tf.story, cell: None, range: 0..len });
            }
        }
    }
    v
}

pub(crate) fn format_chars(s: &mut Session, attrs: &Value) -> Result<Value> {
    let mut a = CharAttrs::default();
    if let Some(o) = attrs.as_object() {
        for (k, v) in o {
            a.set_json(k, v).map_err(|e| bad("type.char", e))?;
        }
    }
    let targets = format_targets(s);
    s.edit(|d, _| {
        for t in &targets {
            let r = &t.range;
            let Some(st) = d.text_story_mut(t.story, t.cell) else { continue };
            if r.is_empty() {
                // Caret: change the typing format (the empty run at the caret).
                let pos = r.start;
                if st.is_empty() {
                    st.chars[0].format.over.merge(&a);
                    st.rev += 1;
                } else {
                    let _ = pos;
                }
                continue;
            }
            st.format_chars(r.clone(), |f| f.over.merge(&a));
        }
        ok()
    })
}

pub(crate) fn format_paras(s: &mut Session, attrs: &Value) -> Result<Value> {
    let mut a = ParaAttrs::default();
    if let Some(o) = attrs.as_object() {
        for (k, v) in o {
            a.set_json(k, v).map_err(|e| bad("type.para", e))?;
        }
    }
    let targets = format_targets(s);
    s.edit(|d, _| {
        for t in &targets {
            if let Some(st) = d.text_story_mut(t.story, t.cell) {
                st.format_paras(t.range.clone(), |p| p.para.merge(&a));
            }
        }
        ok()
    })
}

fn step_size(s: &mut Session, delta: f64) -> Result<Value> {
    let cur = selection_attrs(s, &json!({}))?;
    let size = cur["chars"]["size"].as_f64().unwrap_or(12.0);
    format_chars(s, &json!({"size": (size + delta).clamp(0.1, 1296.0)}))
}

fn selection_attrs(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let targets = format_targets(s);
    let Some(Target { story: sid, cell, range: r }) = targets.first().cloned() else { return Ok(Value::Null) };
    let story = st.doc.text_story(sid, cell).ok_or(designcraft_doc::DocError::NoStory(sid))?;
    let pi = story.para_at(r.start);
    let pf = &story.paras[pi];
    let (pp, base) = st.doc.styles.resolve_para(pf);
    let cf = if r.is_empty() { story.char_format_at(r.start) } else { story.format_after(r.start) };
    let cp = st.doc.styles.resolve_char(&base, cf);
    Ok(json!({"story": sid.0, "paragraphStyle": pf.style, "characterStyle": cf.style, "para": pp, "chars": cp,
        "paraOverrides": pf.para.count(), "charOverrides": cf.over.count()}))
}

/// Our own filler text (not Adobe's).
pub const PLACEHOLDER: &str = "Ovid ellum quisque arcet velut tempora sint, ne veriora pareant laudemque fieri. Lorem novum strata ponet amicos, \
et tamen uti ferrum caelo tempus mollit. Quisque sinat oppida retro, nec tenuere longas animi semper vias. \
Sic erat, ut nostris ceperunt pectora verbis, mitescunt flamma solidos campos venti. Moventur iam nubes, aequora pontus, \
dum sidera fulgent tacito per inane meatu. Arbore sub magna ludunt pueri, linquunt aurea litora fluctus. \
Iamque tibi rursus ponet nova carmina vates, et longas umbras quaerit sub tegmine fagi. Omnia mutantur, nihil interit.";

fn fill_placeholder(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let frame = id_param(p, "frame").or_else(|| st.selection.items.first().copied()).or_else(|| st.selection.text.and_then(|t| t.frame));
    let sid = match frame.and_then(|f| st.doc.item(f)).and_then(|i| i.text_frame()) {
        Some(t) => t.story,
        None => {
            if let Some(f) = frame {
                s.execute("object.content", &json!({"ids": [f.0], "type": "text"}))?;
            }
            story_of(s, p).ok_or_else(|| bad("type.fillWithPlaceholder", "select a text frame"))?
        }
    };
    // Fill until overset: repeat the filler sentences.
    let mut text = String::new();
    let sentences: Vec<&str> = PLACEHOLDER.split_inclusive(". ").collect();
    let doc = s.doc()?.doc.clone();
    let mut d2 = (*doc).clone();
    for i in 0..400 {
        text.push_str(sentences[i % sentences.len()]);
        if i % 3 == 2 {
            d2.set_story_text(sid, &text)?;
            let cs = compose::compose_story(&d2, sid, &compose::ComposeOptions::default());
            if cs.is_overset() {
                // Trim to the last sentence that fits.
                let cut = cs.overset_at.unwrap_or(text.len());
                let keep = text[..cut.min(text.len())].rfind(". ").map(|k| k + 1).unwrap_or(cut);
                text.truncate(keep);
                break;
            }
        }
    }
    s.edit(|d, _| {
        d.set_story_text(sid, text.trim_end())?;
        ok()
    })
}

pub(crate) fn format_targets_pub(s: &Session) -> Vec<Target> {
    format_targets(s)
}
