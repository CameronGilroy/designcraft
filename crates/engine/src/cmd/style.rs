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
        cmd!(
            "object.color",
            "Apply Color",
            [],
            None,
            "{color: \"#rrggbb\"|{c,m,y,k}(0..100)|[r,g,b](0..255), target?: fill|stroke, ids?} — an unnamed colour (not in the Swatches panel until Add to Swatches) on the selection",
            super::has_selection,
            apply_color
        ),
        cmd!(
            "swatch.addToSwatches",
            "Add to Swatches",
            [],
            None,
            "{swatch?: name (default: the selection's fill), target?: fill|stroke, name?: new name} — makes an unnamed colour a swatch",
            has_doc,
            add_to_swatches
        ),
        cmd!("swatch.addUnnamed", "Add Unnamed Colors", [], None, "{} — every unnamed colour used becomes a swatch", has_doc, |s, _| {
            s.edit(|d, _| {
                let mut n = 0;
                for w in &mut d.swatches {
                    if w.hidden {
                        w.hidden = false;
                        n += 1;
                    }
                }
                Ok(json!({"added": n}))
            })
        }),
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
            hidden: false,
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

/// The swatch name of a colour value ("C=… M=… Y=… K=…" / "R=… G=… B=…").
fn value_name(color: designcraft_color::Color) -> String {
    match color {
        designcraft_color::Color::Cmyk { c, m, y, k } => designcraft_color::swatch::cmyk_name(c, m, y, k),
        other => {
            let [r, g, b] = other.to_rgb();
            format!("R={} G={} B={}", (r * 255.0).round(), (g * 255.0).round(), (b * 255.0).round())
        }
    }
}

fn apply_color(s: &mut Session, p: &Value) -> Result<Value> {
    let color = p.get("color").and_then(parse_color).ok_or_else(|| bad("object.color", "missing or bad color"))?;
    let stroke = str_param(p, "target") == Some("stroke");
    let ids = super::targets(s, p)?;
    s.edit(|d, _| {
        // Reuse a swatch holding exactly this colour, else add an unnamed one.
        let existing = d
            .swatches
            .iter()
            .find(|w| matches!(&w.value, SwatchValue::Color { color: c, color_type: designcraft_color::ColorType::Process } if *c == color))
            .map(|w| w.name.clone());
        let name = match existing {
            Some(n) => n,
            None => {
                let name = Styles::unique_name(|n| d.swatch(n).is_some(), &value_name(color));
                d.swatches.push(Swatch {
                    name: name.clone(),
                    value: SwatchValue::Color { color, color_type: designcraft_color::ColorType::Process },
                    locked: false,
                    named: false,
                    hidden: true,
                });
                name
            }
        };
        for id in &ids {
            if let Some(it) = d.item_mut(*id) {
                if stroke {
                    it.stroke.swatch = name.clone();
                    it.stroke.tint = 1.0;
                } else {
                    it.fill.swatch = name.clone();
                    it.fill.tint = 1.0;
                }
            }
        }
        Ok(json!({"swatch": name}))
    })
}

fn add_to_swatches(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let name = match str_param(p, "swatch") {
        Some(n) => n.to_string(),
        None => {
            let id = st.selection.items.first().copied().ok_or_else(|| bad("swatch.addToSwatches", "select an object or give `swatch`"))?;
            let it = st.doc.item(id).ok_or_else(|| bad("swatch.addToSwatches", "no such item"))?;
            if str_param(p, "target") == Some("stroke") { it.stroke.swatch.clone() } else { it.fill.swatch.clone() }
        }
    };
    let rename = str_param(p, "name").map(str::to_string);
    s.edit(|d, _| {
        let w = d.swatches.iter_mut().find(|w| w.name == name).ok_or_else(|| bad("swatch.addToSwatches", format!("no swatch `{name}`")))?;
        if w.locked {
            return Err(bad("swatch.addToSwatches", "special swatches are already listed"));
        }
        w.hidden = false;
        let Some(new) = rename.filter(|n| *n != name) else { return Ok(json!({"name": name})) };
        if d.swatches.iter().any(|w| w.name == new) {
            return Err(bad("swatch.addToSwatches", format!("a swatch named `{new}` exists")));
        }
        let w = d.swatches.iter_mut().find(|w| w.name == name).expect("found above");
        w.name = new.clone();
        w.named = true;
        // Everything using the colour follows the rename.
        for sp in d.spreads.iter_mut().chain(d.parents.iter_mut()) {
            let sp = std::sync::Arc::make_mut(sp);
            for top in &mut sp.items {
                rename_in(std::sync::Arc::make_mut(top), &name, &new);
            }
        }
        Ok(json!({"name": new}))
    })
}

/// Point fills and strokes of an item (and its children) at a renamed swatch.
fn rename_in(it: &mut designcraft_doc::Item, from: &str, to: &str) {
    if it.fill.swatch == from {
        it.fill.swatch = to.to_string();
    }
    if it.stroke.swatch == from {
        it.stroke.swatch = to.to_string();
    }
    if let Some(kids) = it.children_mut() {
        for k in kids {
            rename_in(std::sync::Arc::make_mut(k), from, to);
        }
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;

    #[test]
    fn unnamed_colors_then_add_to_swatches() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [0, 0, 50, 50]})).unwrap()["id"].as_u64().unwrap();
        s.execute("selection.set", &json!({"ids": [id]})).unwrap();
        let listed = |s: &Session| s.doc().unwrap().doc.swatches.iter().filter(|w| !w.hidden).count();
        let before = listed(&s);
        let r = s.execute("object.color", &json!({"color": {"c": 10, "m": 20, "y": 30, "k": 0}})).unwrap();
        assert_eq!(r["swatch"], "C=10 M=20 Y=30 K=0");
        assert_eq!(listed(&s), before, "unnamed: not in the Swatches panel");
        // The same colour again reuses it; the stroke target works too.
        let n = s.doc().unwrap().doc.swatches.len();
        s.execute("object.color", &json!({"color": {"c": 10, "m": 20, "y": 30, "k": 0}, "target": "stroke"})).unwrap();
        assert_eq!(s.doc().unwrap().doc.swatches.len(), n);
        let d = &s.doc().unwrap().doc;
        let it = d.item(designcraft_doc::ItemId(id)).unwrap();
        assert_eq!(it.fill.swatch, it.stroke.swatch);
        // Add to Swatches with a name: listed, and the object follows the rename.
        s.execute("swatch.addToSwatches", &json!({"name": "Sand"})).unwrap();
        assert_eq!(listed(&s), before + 1);
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.item(designcraft_doc::ItemId(id)).unwrap().fill.swatch, "Sand");
        // A colour equal to an existing swatch applies that swatch.
        s.execute("object.color", &json!({"color": {"c": 10, "m": 20, "y": 30, "k": 0}})).unwrap();
        assert_eq!(s.doc().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().fill.swatch, "Sand");
    }
}
