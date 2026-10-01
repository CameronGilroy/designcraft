//! What is selected: page items (containers or their content) or a text range in a story.

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, StoryId};

/// A caret or text range in a story. `anchor` stays put while `focus` moves (Shift-arrows).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSel {
    pub story: StoryId,
    pub anchor: usize,
    pub focus: usize,
    /// The frame the caret is shown in (for carets at frame boundaries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<ItemId>,
}

impl TextSel {
    pub fn caret(story: StoryId, pos: usize) -> Self {
        TextSel { story, anchor: pos, focus: pos, frame: None }
    }
    pub fn range(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
    pub fn is_caret(&self) -> bool {
        self.anchor == self.focus
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    /// Selected items (top-level or, with the Direct Selection tool, nested items).
    pub items: Vec<ItemId>,
    /// Content selection (graphic inside its frame) instead of the container.
    #[serde(default)]
    pub content: bool,
    /// Direct selection: selected anchors as (item, subpath, anchor).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<(ItemId, usize, usize)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextSel>,
    /// Key object for Align.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<ItemId>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.text.is_none()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn items(ids: Vec<ItemId>) -> Self {
        Selection { items: ids, ..Default::default() }
    }
    pub fn text(t: TextSel) -> Self {
        Selection { text: Some(t), ..Default::default() }
    }
    pub fn contains(&self, id: ItemId) -> bool {
        self.items.contains(&id)
    }
}
