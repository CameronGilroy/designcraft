//! File › Place of text files (plain text, Word, RTF): into the text insertion point, the
//! selected frame, or a new frame on the page; with autoflow, pages and threaded frames are added
//! until the whole story fits (InDesign's Shift-click with a loaded text cursor).

use designcraft_doc::{CharacterStyle, Content, Document, ParaFormat, ParagraphStyle, SpreadRef, Story, StoryId, TextSel};
use designcraft_geom::Rect;
use serde_json::{Value, json};

use super::bad;
use crate::{Result, Session};

/// Add the imported styles the document doesn't have yet (existing styles win, as in InDesign's
/// default style-conflict option).
fn add_styles(d: &mut Document, imp: &designcraft_textimport::Imported) {
    let st = d.styles_mut();
    for s in &imp.para_styles {
        if st.para(&s.name).is_none() {
            st.paragraph.push(ParagraphStyle {
                name: s.name.clone(),
                based_on: s.based_on.clone().or_else(|| Some(designcraft_doc::story::BASIC_PARAGRAPH.into())),
                next_style: None,
                para: s.para.clone(),
                chars: s.chars.clone(),
                shortcut: String::new(),
            });
        }
    }
    for s in &imp.char_styles {
        if !st.character.iter().any(|c| c.name == s.name) {
            st.character.push(CharacterStyle { name: s.name.clone(), based_on: s.based_on.clone(), chars: s.chars.clone(), shortcut: String::new() });
        }
    }
}

/// "Remove styles and formatting": every paragraph [Basic Paragraph], no local formatting.
fn strip(story: &mut Story) {
    let n = story.len();
    story.format_paras(0..n, |p| *p = ParaFormat { table: p.table, ..Default::default() });
    story.format_chars(0..n, |f| *f = Default::default());
}

/// Is the story overset in its frames, and how much of it do its frames show?
fn fit(d: &Document, sid: StoryId) -> (Option<usize>, usize) {
    let cs = designcraft_compose::compose_story(d, sid, &Default::default());
    let shown = cs.frames.iter().map(|f| f.range.end).max().unwrap_or(0);
    (cs.overset_at, shown)
}

/// Thread new frames on new pages (margin rectangles) until the story fits. Returns pages added.
fn autoflow(d: &mut Document, sid: StoryId, max_pages: usize) -> Result<usize> {
    let mut added = 0;
    loop {
        let (overset, shown) = fit(d, sid);
        let Some(at) = overset else { return Ok(added) };
        if added >= max_pages {
            return Err(bad("file.place", format!("autoflow stopped after {max_pages} pages")));
        }
        let len = d.story(sid).map_or(0, |s| s.len());
        let frames = d.story(sid).map_or(1, |s| s.frames.len()).max(1);
        // Frames still needed at the rate the existing ones are filled (at least one).
        let per_frame = (shown.max(at) / frames).max(1);
        let want = (len - at).div_ceil(per_frame).clamp(1, max_pages - added);
        let last = *d.story(sid).and_then(|s| s.frames.last()).ok_or_else(|| bad("file.place", "the story has no frame"))?;
        let mut page = d.page_of_item(last).unwrap_or(d.page_count().saturating_sub(1));
        let parent = d.page_loc(page).and_then(|(si, pi)| d.spreads[si].pages[pi].parent);
        d.insert_pages(Some(page), want, parent)?;
        let layer = d.default_layer();
        let mut prev = last;
        for _ in 0..want {
            page += 1;
            let (si, pi) = d.page_loc(page).ok_or_else(|| bad("file.place", "page insert failed"))?;
            let r = d.spreads[si].pages[pi].margin_rect();
            let (fid, _) = d.add_text_frame(SpreadRef::Doc(si), r, layer, "", ParaFormat::default())?;
            d.thread(prev, fid)?;
            prev = fid;
        }
        added += want;
    }
}

pub(super) fn place_text(s: &mut Session, p: &Value, name: &str, bytes: &[u8]) -> Result<Value> {
    let mut imp = designcraft_textimport::import(name, bytes).map_err(|e| bad("file.place", e.to_string()))?;
    let remove = p.get("removeStyles").and_then(Value::as_bool).unwrap_or(false);
    if remove {
        strip(&mut imp.story);
        imp.para_styles.clear();
        imp.char_styles.clear();
    }
    let autoflow_on = p.get("autoflow").and_then(Value::as_bool).unwrap_or(false);
    let st = s.doc()?;
    let caret = st.selection.text.filter(|t| t.cell.is_none());
    let frame = super::id_param(p, "frame").or_else(|| {
        st.selection.items.iter().copied().find(|i| st.doc.item(*i).is_some_and(|it| matches!(it.content, Content::Unassigned | Content::Text(_))))
    });
    let page = p.get("page").and_then(Value::as_u64).map(|v| (v as usize).saturating_sub(1));
    let rect = super::rect_param(p, "rect");
    let warnings = imp.warnings.clone();
    s.edit(|d, sel| {
        add_styles(d, &imp);
        let sid = if let Some(t) = caret {
            // Into the text at the insertion point (replacing the selection).
            let story = d.story_mut(t.story).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
            let r = t.range();
            story.delete(r.clone());
            let end = story.insert_story(r.start, &imp.story);
            sel.text = Some(TextSel { anchor: end, focus: end, ..t });
            t.story
        } else {
            let fid = match frame {
                Some(f) => f,
                None => {
                    let pg = page.unwrap_or(0).min(d.page_count().saturating_sub(1));
                    let (si, pi) = d.page_loc(pg).ok_or_else(|| bad("file.place", "no such page"))?;
                    let r: Rect = rect.unwrap_or_else(|| d.spreads[si].pages[pi].margin_rect());
                    let layer = d.default_layer();
                    d.add_text_frame(SpreadRef::Doc(si), r, layer, "", ParaFormat::default())?.0
                }
            };
            // Empty frames become text frames.
            let sid = match d.item(fid).map(|i| i.content.clone()) {
                Some(Content::Text(t)) => t.story,
                Some(_) => {
                    let sid = StoryId(d.alloc());
                    let mut ns = Story::new(sid);
                    ns.frames = vec![fid];
                    d.stories.insert(sid, std::sync::Arc::new(ns));
                    if let Some(it) = d.item_mut(fid) {
                        it.content = Content::Text(designcraft_doc::TextFrame { story: sid, options: Default::default() });
                    }
                    sid
                }
                None => return Err(bad("file.place", "no such frame")),
            };
            let story = d.story_mut(sid).ok_or(designcraft_doc::DocError::NoStory(sid))?;
            let n = story.len();
            story.delete(0..n);
            story.insert_story(0, &imp.story);
            if let Some(first) = imp.story.paras.first() {
                story.paras[0] = ParaFormat { table: story.paras[0].table, ..first.clone() };
            }
            *sel = designcraft_doc::Selection::items(vec![fid]);
            sid
        };
        let pages = if autoflow_on { autoflow(d, sid, 2000)? } else { 0 };
        let overset = fit(d, sid).0.is_some();
        Ok(json!({"story": sid.0, "pagesAdded": pages, "overset": overset, "warnings": warnings}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_text_file_with_autoflow() {
        let dir = std::env::temp_dir().join(format!("dc-place-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("long.txt");
        let para = "The quick brown fox jumps over the lazy dog, again and again, to fill the page. ".repeat(12);
        std::fs::write(&path, vec![para.as_str(); 60].join("\n")).unwrap();
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 1})).unwrap();
        let r = s.execute("file.place", &json!({"path": path.to_string_lossy(), "autoflow": true})).unwrap();
        assert!(r["pagesAdded"].as_u64().unwrap() >= 2, "{r}");
        assert_eq!(r["overset"], false);
        let d = &s.doc().unwrap().doc;
        let sid = StoryId(r["story"].as_u64().unwrap());
        assert_eq!(d.story(sid).unwrap().frames.len(), d.page_count());
        assert_eq!(d.story(sid).unwrap().paras.len(), 60);
        // Into the insertion point of another frame.
        let fr = s.execute("frame.create", &json!({"rect": [72, 72, 300, 200], "content": "text", "text": "Start  end"})).unwrap();
        let sid2 = fr["story"].as_u64().unwrap();
        s.execute("text.select", &json!({"story": sid2, "anchor": 6, "focus": 6})).unwrap();
        let small = dir.join("small.rtf");
        std::fs::write(&small, br"{\rtf1 {\b middle}}").unwrap();
        s.execute("file.place", &json!({"path": small.to_string_lossy()})).unwrap();
        let st = s.doc().unwrap().doc.story(StoryId(sid2)).unwrap().clone();
        assert_eq!(st.text, "Start middle end");
        assert_eq!(st.format_after(6).over.font_style.as_deref(), Some("Bold"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
