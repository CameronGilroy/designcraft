//! Paragraph / character styles and swatches.

use designcraft_color::{Swatch, SwatchValue};
use designcraft_doc::{CharAttrs, CharacterStyle, ParaAttrs, ParagraphStyle, Styles};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, has_text_or_frames, ok, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("style.paragraph.apply", "Apply Paragraph Style", [], None, "{name, clearOverrides?: bool}", has_text_or_frames, apply_para),
        cmd!("style.character.apply", "Apply Character Style", [], None, "{name}", has_text_or_frames, apply_char),
        cmd!(
            "style.paragraph.create",
            "New Paragraph Style…",
            [],
            None,
            "{name, basedOn?, nextStyle?, para?: {…}, chars?: {…}, fromSelection?: bool}",
            has_doc,
            create_para
        ),
        cmd!(
            "style.paragraph.edit",
            "Paragraph Style Options…",
            [],
            None,
            "{name, rename?, basedOn?, nextStyle?, para?: {…}, chars?: {…}}",
            has_doc,
            edit_para
        ),
        cmd!("style.paragraph.delete", "Delete Paragraph Style", [], None, "{name, replaceWith?}", has_doc, delete_para),
        cmd!("style.character.create", "New Character Style…", [], None, "{name, basedOn?, chars?: {…}}", has_doc, create_char),
        cmd!("style.character.edit", "Character Style Options…", [], None, "{name, rename?, basedOn?, chars?}", has_doc, edit_char),
        cmd!(query "style.list", "List Styles", [], None, "{} → paragraph and character style names", has_doc, |s, _| {
            let st = s.doc()?;
            Ok(json!({
                "paragraph": st.doc.styles.paragraph.iter().map(|p| &p.name).collect::<Vec<_>>(),
                "character": st.doc.styles.character.iter().map(|p| &p.name).collect::<Vec<_>>(),
                "object": st.doc.styles.object.iter().map(|p| &p.name).collect::<Vec<_>>(),
            }))
        }),
        cmd!("style.object.apply", "Apply Object Style", [], None, "{name, ids?}", super::has_selection, apply_object),
        cmd!("style.object.create", "New Object Style…", [], None, "{name, fromSelection?: true, fill?, paragraphStyle?}", has_doc, create_object),
        cmd!(
            "swatch.create",
            "New Color Swatch…",
            [],
            None,
            "{name?, color: \"#rrggbb\"|{c,m,y,k}(0..100)|[r,g,b], spot?: bool}",
            has_doc,
            create_swatch
        ),
        cmd!("swatch.delete", "Delete Swatch", [], None, "{name}", has_doc, |s, p| {
            let name = str_param(p, "name").unwrap_or("").to_string();
            s.edit(|d, _| {
                if d.swatch(&name).is_some_and(|w| w.locked) {
                    return Err(bad("swatch.delete", "special swatches can't be deleted"));
                }
                d.swatches.retain(|w| w.name != name);
                ok()
            })
        }),
    ]
}

fn attrs<T: Default>(v: Option<&Value>, set: impl Fn(&mut T, &str, &Value) -> std::result::Result<(), String>) -> Result<T> {
    let mut a = T::default();
    if let Some(o) = v.and_then(Value::as_object) {
        for (k, v) in o {
            set(&mut a, k, v).map_err(|e| bad("style", e))?;
        }
    }
    Ok(a)
}

fn apply_para(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("style.paragraph.apply", "missing name"))?.to_string();
    let clear = p.get("clearOverrides").and_then(Value::as_bool).unwrap_or(false);
    if s.doc()?.doc.styles.para(&name).is_none() {
        return Err(bad("style.paragraph.apply", format!("no paragraph style `{name}`")));
    }
    let targets = super::text::format_targets_pub(s);
    s.edit(|d, _| {
        for t in &targets {
            let r = &t.range;
            if let Some(st) = d.text_story_mut(t.story, t.cell) {
                st.format_paras(r.clone(), |f| {
                    f.style = name.clone();
                    if clear {
                        f.para = ParaAttrs::default();
                        f.chars = CharAttrs::default();
                    }
                });
                if clear {
                    let (a, b) = (st.para_ranges()[st.para_at(r.start)].start, st.para_ranges()[st.para_at(r.end)].end);
                    st.format_chars(a..b, |f| f.over = CharAttrs::default());
                }
            }
        }
        ok()
    })
}

fn apply_char(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("style.character.apply", "missing name"))?.to_string();
    if s.doc()?.doc.styles.char_style(&name).is_none() {
        return Err(bad("style.character.apply", format!("no character style `{name}`")));
    }
    let targets = super::text::format_targets_pub(s);
    s.edit(|d, _| {
        for t in &targets {
            if let Some(st) = d.text_story_mut(t.story, t.cell) {
                st.format_chars(t.range.clone(), |f| f.style = name.clone());
            }
        }
        ok()
    })
}

fn create_para(s: &mut Session, p: &Value) -> Result<Value> {
    let base = str_param(p, "name").unwrap_or("Paragraph Style 1").to_string();
    let mut para: ParaAttrs = attrs(p.get("para"), |a: &mut ParaAttrs, k, v| a.set_json(k, v))?;
    let mut chars: CharAttrs = attrs(p.get("chars"), |a: &mut CharAttrs, k, v| a.set_json(k, v))?;
    if p.get("fromSelection").and_then(Value::as_bool).unwrap_or(false)
        && let Ok(cur) = s.execute("type.selectionAttrs", &json!({}))
        && !cur.is_null()
    {
        if let Ok(pp) = serde_json::from_value::<designcraft_doc::ParaProps>(cur["para"].clone()) {
            let mut full = ParaProps_to_attrs(&pp);
            full.merge(&para);
            para = full;
        }
        if let Ok(cp) = serde_json::from_value::<designcraft_doc::CharProps>(cur["chars"].clone()) {
            let mut full = CharProps_to_attrs(&cp);
            full.merge(&chars);
            chars = full;
        }
    }
    let based_on = str_param(p, "basedOn").map(str::to_string);
    let next = str_param(p, "nextStyle").map(str::to_string);
    s.edit(|d, _| {
        let name = Styles::unique_name(|n| d.styles.para(n).is_some(), &base);
        if let Some(b) = &based_on
            && d.styles.para(b).is_none()
        {
            return Err(bad("style.paragraph.create", format!("no style `{b}`")));
        }
        d.styles_mut().paragraph.push(ParagraphStyle { name: name.clone(), based_on, next_style: next, para, chars, shortcut: String::new() });
        Ok(json!({"name": name}))
    })
}

#[allow(non_snake_case)]
fn ParaProps_to_attrs(p: &designcraft_doc::ParaProps) -> ParaAttrs {
    designcraft_doc::ParaProps::common([p])
}
#[allow(non_snake_case)]
fn CharProps_to_attrs(p: &designcraft_doc::CharProps) -> CharAttrs {
    designcraft_doc::CharProps::common([p])
}

fn edit_para(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("style.paragraph.edit", "missing name"))?.to_string();
    let para: ParaAttrs = attrs(p.get("para"), |a: &mut ParaAttrs, k, v| a.set_json(k, v))?;
    let chars: CharAttrs = attrs(p.get("chars"), |a: &mut CharAttrs, k, v| a.set_json(k, v))?;
    let rename = str_param(p, "rename").map(str::to_string);
    let based = p.get("basedOn").cloned();
    s.edit(|d, _| {
        if let Some(Value::String(b)) = &based
            && d.styles.para_based_on_cycles(&name, b)
        {
            return Err(bad("style.paragraph.edit", "based-on would create a cycle"));
        }
        let st = d.styles_mut().para_mut(&name).ok_or_else(|| bad("style.paragraph.edit", format!("no style `{name}`")))?;
        st.para.merge(&para);
        st.chars.merge(&chars);
        match based {
            Some(Value::String(b)) => st.based_on = Some(b),
            Some(Value::Null) => st.based_on = None,
            _ => {}
        }
        if let Some(n) = rename.clone() {
            st.name = n.clone();
            for sid in d.stories.keys().copied().collect::<Vec<_>>() {
                if let Some(story) = d.story_mut(sid) {
                    for f in &mut story.paras {
                        if f.style == name {
                            f.style = n.clone();
                        }
                    }
                }
            }
        }
        ok()
    })
}

fn delete_para(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").unwrap_or("").to_string();
    let repl = str_param(p, "replaceWith").unwrap_or(designcraft_doc::BASIC_PARAGRAPH).to_string();
    if name.starts_with('[') {
        return Err(bad("style.paragraph.delete", "built-in styles can't be deleted"));
    }
    s.edit(|d, _| {
        d.styles_mut().paragraph.retain(|x| x.name != name);
        for sid in d.stories.keys().copied().collect::<Vec<_>>() {
            if let Some(story) = d.story_mut(sid) {
                for f in &mut story.paras {
                    if f.style == name {
                        f.style = repl.clone();
                    }
                }
            }
        }
        ok()
    })
}

fn create_char(s: &mut Session, p: &Value) -> Result<Value> {
    let base = str_param(p, "name").unwrap_or("Character Style 1").to_string();
    let chars: CharAttrs = attrs(p.get("chars"), |a: &mut CharAttrs, k, v| a.set_json(k, v))?;
    let based_on = str_param(p, "basedOn").map(str::to_string);
    s.edit(|d, _| {
        let name = Styles::unique_name(|n| d.styles.char_style(n).is_some(), &base);
        d.styles_mut().character.push(CharacterStyle { name: name.clone(), based_on, chars, shortcut: String::new() });
        Ok(json!({"name": name}))
    })
}

fn edit_char(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("style.character.edit", "missing name"))?.to_string();
    let chars: CharAttrs = attrs(p.get("chars"), |a: &mut CharAttrs, k, v| a.set_json(k, v))?;
    s.edit(|d, _| {
        let st = d.styles_mut().char_style_mut(&name).ok_or_else(|| bad("style.character.edit", "no such style"))?;
        st.chars.merge(&chars);
        ok()
    })
}

fn create_swatch(s: &mut Session, p: &Value) -> Result<Value> {
    let c = p.get("color").ok_or_else(|| bad("swatch.create", "missing color"))?;
    let color = parse_color(c).ok_or_else(|| bad("swatch.create", "bad color"))?;
    let spot = p.get("spot").and_then(Value::as_bool).unwrap_or(false);
    let name = str_param(p, "name").map(str::to_string).unwrap_or_else(|| match color {
        designcraft_color::Color::Cmyk { c, m, y, k } => designcraft_color::swatch::cmyk_name(c, m, y, k),
        other => {
            let [r, g, b] = other.to_rgb();
            format!("R={} G={} B={}", (r * 255.0).round(), (g * 255.0).round(), (b * 255.0).round())
        }
    });
    s.edit(|d, _| {
        let name = Styles::unique_name(|n| d.swatch(n).is_some(), &name);
        d.swatches.push(Swatch {
            name: name.clone(),
            value: SwatchValue::Color {
                color,
                color_type: if spot { designcraft_color::ColorType::Spot } else { designcraft_color::ColorType::Process },
            },
            locked: false,
            named: true,
        });
        Ok(json!({"name": name}))
    })
}

pub fn parse_color(v: &Value) -> Option<designcraft_color::Color> {
    use designcraft_color::Color;
    match v {
        Value::String(s) => Color::from_hex(s),
        Value::Array(a) if a.len() >= 3 => {
            let f = |i: usize| a[i].as_f64().map(|x| if x > 1.0 { x / 255.0 } else { x } as f32);
            Some(Color::rgb(f(0)?, f(1)?, f(2)?))
        }
        Value::Object(o) => {
            let g = |k: &str| o.get(k).and_then(Value::as_f64).map(|x| if x > 1.0 { x / 100.0 } else { x } as f32);
            Some(Color::cmyk(g("c")?, g("m")?, g("y")?, g("k")?))
        }
        _ => None,
    }
}

fn apply_object(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("style.object.apply", "missing name"))?.to_string();
    let os = s.doc()?.doc.styles.object_style(&name).cloned().ok_or_else(|| bad("style.object.apply", format!("no object style `{name}`")))?;
    let ids = super::targets(s, p)?;
    s.edit(|d, _| {
        let mut stories = Vec::new();
        for id in &ids {
            let Some(it) = d.item_mut(*id) else { continue };
            it.object_style = name.clone();
            if let Some(f) = &os.fill {
                it.fill = f.clone();
            }
            if let Some(st) = &os.stroke {
                it.stroke = st.clone();
            }
            if let Some(tf) = it.text_frame_mut() {
                if let Some(o) = &os.text_frame {
                    tf.options = o.clone();
                }
                stories.push(tf.story);
            }
        }
        if let Some(ps) = &os.paragraph_style {
            for sid in stories {
                if let Some(st) = d.story_mut(sid) {
                    let len = st.len();
                    st.format_paras(0..len, |f| f.style = ps.clone());
                }
            }
        }
        Ok(Value::Null)
    })
}

fn create_object(s: &mut Session, p: &Value) -> Result<Value> {
    let base = str_param(p, "name").unwrap_or("Object Style 1").to_string();
    let from_sel = p.get("fromSelection").and_then(Value::as_bool).unwrap_or(true);
    let src = if from_sel { s.doc()?.selection.items.first().and_then(|i| s.doc().ok()?.doc.item(*i).cloned()) } else { None };
    let mut os = designcraft_doc::ObjectStyle::default();
    if let Some(it) = &src {
        os.fill = Some(it.fill.clone());
        os.stroke = Some(it.stroke.clone());
        os.text_frame = it.text_frame().map(|t| t.options.clone());
    }
    if let Some(sw) = str_param(p, "fill") {
        os.fill = Some(designcraft_doc::Fill::swatch(sw));
    }
    if let Some(ps) = str_param(p, "paragraphStyle") {
        os.paragraph_style = Some(ps.to_string());
    }
    s.edit(|d, _| {
        let name = Styles::unique_name(|n| d.styles.object_style(n).is_some(), &base);
        os.name = name.clone();
        d.styles_mut().object.push(os.clone());
        Ok(json!({"name": name}))
    })
}
