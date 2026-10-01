//! Paragraph, character and object styles with based-on inheritance.

use serde::{Deserialize, Serialize};

use crate::attrs::{CharAttrs, CharProps, ParaAttrs, ParaProps};
use crate::item::{Fill, Stroke, TextFrameOptions};
use crate::story::{BASIC_PARAGRAPH, CharFormat, NO_CHAR_STYLE, ParaFormat};

pub const NO_PARA_STYLE: &str = "[No Paragraph Style]";
pub const BASIC_GRAPHICS_FRAME: &str = "[Basic Graphics Frame]";
pub const BASIC_TEXT_FRAME: &str = "[Basic Text Frame]";
pub const NO_OBJECT_STYLE: &str = "[None]";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphStyle {
    /// Unique name; groups are expressed as `Group/Name`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_style: Option<String>,
    #[serde(default)]
    pub para: ParaAttrs,
    #[serde(default)]
    pub chars: CharAttrs,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shortcut: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterStyle {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(default)]
    pub chars: CharAttrs,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shortcut: String,
}

/// Object style: each attribute group is optional ("not included in style" when `None`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ObjectStyle {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_frame: Option<TextFrameOptions>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Styles {
    pub paragraph: Vec<ParagraphStyle>,
    pub character: Vec<CharacterStyle>,
    pub object: Vec<ObjectStyle>,
    /// Default styles for new text/frames (the style selected with nothing selected).
    pub default_paragraph: String,
    pub default_character: String,
    pub default_text_frame: String,
    pub default_graphic_frame: String,
}

impl Default for Styles {
    fn default() -> Self {
        Styles {
            paragraph: vec![
                ParagraphStyle {
                    name: NO_PARA_STYLE.into(),
                    based_on: None,
                    next_style: None,
                    para: ParaAttrs::default(),
                    chars: CharAttrs::default(),
                    shortcut: String::new(),
                },
                ParagraphStyle {
                    name: BASIC_PARAGRAPH.into(),
                    based_on: None,
                    next_style: Some(BASIC_PARAGRAPH.into()),
                    para: ParaAttrs::default(),
                    chars: CharAttrs::default(),
                    shortcut: String::new(),
                },
            ],
            character: vec![CharacterStyle { name: NO_CHAR_STYLE.into(), based_on: None, chars: CharAttrs::default(), shortcut: String::new() }],
            object: vec![
                ObjectStyle { name: NO_OBJECT_STYLE.into(), ..Default::default() },
                ObjectStyle { name: BASIC_GRAPHICS_FRAME.into(), stroke: Some(Stroke::default()), ..Default::default() },
                ObjectStyle {
                    name: BASIC_TEXT_FRAME.into(),
                    fill: Some(Fill::none()),
                    stroke: Some(Stroke::none()),
                    paragraph_style: Some(BASIC_PARAGRAPH.into()),
                    ..Default::default()
                },
            ],
            default_paragraph: BASIC_PARAGRAPH.into(),
            default_character: NO_CHAR_STYLE.into(),
            default_text_frame: BASIC_TEXT_FRAME.into(),
            default_graphic_frame: BASIC_GRAPHICS_FRAME.into(),
        }
    }
}

const MAX_DEPTH: usize = 32;

impl Styles {
    pub fn para(&self, name: &str) -> Option<&ParagraphStyle> {
        self.paragraph.iter().find(|s| s.name == name)
    }
    pub fn para_mut(&mut self, name: &str) -> Option<&mut ParagraphStyle> {
        self.paragraph.iter_mut().find(|s| s.name == name)
    }
    pub fn char_style(&self, name: &str) -> Option<&CharacterStyle> {
        self.character.iter().find(|s| s.name == name)
    }
    pub fn char_style_mut(&mut self, name: &str) -> Option<&mut CharacterStyle> {
        self.character.iter_mut().find(|s| s.name == name)
    }
    pub fn object_style(&self, name: &str) -> Option<&ObjectStyle> {
        self.object.iter().find(|s| s.name == name)
    }

    /// The paragraph style chain from root to `name` (cycle-safe).
    fn para_chain(&self, name: &str) -> Vec<&ParagraphStyle> {
        let mut chain = Vec::new();
        let mut cur = self.para(name);
        while let Some(s) = cur {
            if chain.len() >= MAX_DEPTH || chain.iter().any(|c: &&ParagraphStyle| c.name == s.name) {
                break;
            }
            chain.push(s);
            cur = s.based_on.as_deref().and_then(|b| self.para(b));
        }
        chain.reverse();
        chain
    }

    fn char_chain(&self, name: &str) -> Vec<&CharacterStyle> {
        let mut chain = Vec::new();
        let mut cur = self.char_style(name);
        while let Some(s) = cur {
            if chain.len() >= MAX_DEPTH || chain.iter().any(|c: &&CharacterStyle| c.name == s.name) {
                break;
            }
            chain.push(s);
            cur = s.based_on.as_deref().and_then(|b| self.char_style(b));
        }
        chain.reverse();
        chain
    }

    /// Resolved paragraph attributes and the paragraph's base character attributes.
    pub fn resolve_para(&self, p: &ParaFormat) -> (ParaProps, CharProps) {
        let (mut pp, mut cp) = (ParaProps::default(), CharProps::default());
        for s in self.para_chain(&p.style) {
            pp.apply(&s.para);
            cp.apply(&s.chars);
        }
        pp.apply(&p.para);
        cp.apply(&p.chars);
        (pp, cp)
    }

    /// Paragraph-style-only resolution (no local overrides).
    pub fn resolve_para_style(&self, name: &str) -> (ParaProps, CharProps) {
        self.resolve_para(&ParaFormat { style: name.into(), ..Default::default() })
    }

    /// Character attributes for a run: paragraph base ← character style chain ← local overrides.
    pub fn resolve_char(&self, para_chars: &CharProps, f: &CharFormat) -> CharProps {
        let mut cp = para_chars.clone();
        for s in self.char_chain(&f.style) {
            cp.apply(&s.chars);
        }
        cp.apply(&f.over);
        cp
    }

    /// Would making `name` based on `parent` create a cycle?
    pub fn para_based_on_cycles(&self, name: &str, parent: &str) -> bool {
        name == parent || self.para_chain(parent).iter().any(|s| s.name == name)
    }
    pub fn char_based_on_cycles(&self, name: &str, parent: &str) -> bool {
        name == parent || self.char_chain(parent).iter().any(|s| s.name == name)
    }

    /// A free name `base`, `base 2`, `base 3`…
    pub fn unique_name(existing: impl Fn(&str) -> bool, base: &str) -> String {
        if !existing(base) {
            return base.to_string();
        }
        (2..).map(|i| format!("{base} {i}")).find(|n| !existing(n)).expect("infinite")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn based_on_chain_resolves_in_order() {
        let mut st = Styles::default();
        st.paragraph.push(ParagraphStyle {
            name: "Body".into(),
            based_on: Some(BASIC_PARAGRAPH.into()),
            next_style: None,
            para: ParaAttrs { space_after: Some(6.0), ..Default::default() },
            chars: CharAttrs { size: Some(10.0), ..Default::default() },
            shortcut: String::new(),
        });
        st.paragraph.push(ParagraphStyle {
            name: "Body First".into(),
            based_on: Some("Body".into()),
            next_style: Some("Body".into()),
            para: ParaAttrs { first_line_indent: Some(0.0), ..Default::default() },
            chars: CharAttrs { size: Some(11.0), ..Default::default() },
            shortcut: String::new(),
        });
        let (pp, cp) = st.resolve_para_style("Body First");
        assert_eq!(pp.space_after, 6.0);
        assert_eq!(cp.size, 11.0);
        let f = CharFormat { style: NO_CHAR_STYLE.into(), over: CharAttrs { tracking: Some(10.0), ..Default::default() } };
        let r = st.resolve_char(&cp, &f);
        assert_eq!((r.size, r.tracking), (11.0, 10.0));
    }

    #[test]
    fn cycles_are_detected_and_safe() {
        let mut st = Styles::default();
        for (n, b) in [("A", "B"), ("B", "A")] {
            st.paragraph.push(ParagraphStyle {
                name: n.into(),
                based_on: Some(b.into()),
                next_style: None,
                para: ParaAttrs::default(),
                chars: CharAttrs::default(),
                shortcut: String::new(),
            });
        }
        let _ = st.resolve_para_style("A"); // terminates
        assert!(st.para_based_on_cycles("A", "B"));
        assert!(!st.para_based_on_cycles(BASIC_PARAGRAPH, NO_PARA_STYLE));
    }

    #[test]
    fn unique_names() {
        let names = ["Body", "Body 2"];
        assert_eq!(Styles::unique_name(|n| names.contains(&n), "Body"), "Body 3");
        assert_eq!(Styles::unique_name(|n| names.contains(&n), "Head"), "Head");
    }
}
