//! The DesignCraft document model (pure data + serde).
//!
//! A [`Document`] holds spreads of pages, parent ("master") spreads, layers, stories, styles,
//! swatches, sections and embedded assets. Spreads, items and stories are `Arc`-shared so an
//! edit clones only what it touches and undo snapshots are O(1).
//!
//! Coordinates are points, y down. Each spread has its own space: pages sit side by side from
//! x = 0 with their tops at y = 0. Items live in spread space via `Item::xf`.
#![forbid(unsafe_code)]

pub mod attrs;
pub mod build;
mod edit;
pub mod ids;
pub mod item;
pub mod notes;
pub mod page;
pub mod selection;
pub mod story;
pub mod styles;
pub mod table;
pub mod vars;

use std::collections::BTreeMap;
use std::sync::Arc;

pub use attrs::*;
pub use designcraft_color as color;
pub use designcraft_geom as geom;
pub use edit::{ItemLoc, ItemPath, SpreadRef, item_hit as edit_hit};
pub use ids::*;
pub use item::*;
pub use notes::{FOOTNOTE_REF, FOOTNOTE_TABLE, Footnote, FootnoteOptions};
pub use page::*;
pub use selection::*;
pub use story::*;
pub use styles::*;
pub use table::*;

use designcraft_color::Swatch;
use designcraft_geom::Unit;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DocError {
    #[error("no such item {0}")]
    NoItem(ItemId),
    #[error("no such story {0}")]
    NoStory(StoryId),
    #[error("no such page {0}")]
    NoPage(usize),
    #[error("no such layer {0}")]
    NoLayer(LayerId),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, DocError>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Intent {
    #[default]
    Print,
    Web,
    Mobile,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GridRelative {
    #[default]
    TopOfPage,
    TopMargin,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BaselineGrid {
    pub start: f64,
    pub increment: f64,
    pub relative_to: GridRelative,
    pub color: [u8; 3],
    /// Hidden below this zoom (1.0 = 100%).
    pub view_threshold: f64,
}

impl Default for BaselineGrid {
    fn default() -> Self {
        BaselineGrid { start: 36.0, increment: 12.0, relative_to: GridRelative::TopOfPage, color: [140, 205, 230], view_threshold: 0.75 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocumentGrid {
    pub horizontal: f64,
    pub vertical: f64,
    pub subdivisions: u32,
    pub color: [u8; 3],
    pub in_back: bool,
}

impl Default for DocumentGrid {
    fn default() -> Self {
        DocumentGrid { horizontal: 72.0, vertical: 72.0, subdivisions: 8, color: [200, 200, 200], in_back: true }
    }
}

/// File → Document Setup + the guide/grid/unit preferences stored with the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocSettings {
    pub intent: Intent,
    pub page_width: f64,
    pub page_height: f64,
    pub facing_pages: bool,
    pub primary_text_frame: bool,
    /// Bleed: top, bottom, inside, outside.
    pub bleed: [f64; 4],
    pub slug: [f64; 4],
    pub horizontal_units: Unit,
    pub vertical_units: Unit,
    pub baseline_grid: BaselineGrid,
    pub grid: DocumentGrid,
    /// Pasteboard extent around spreads (horizontal, vertical).
    pub pasteboard: (f64, f64),
    pub margin_color: [u8; 3],
    pub column_color: [u8; 3],
    pub bleed_color: [u8; 3],
    pub slug_color: [u8; 3],
    pub keyboard_increment: f64,
    /// Chapter number (Numbering & Section Options › Document Chapter Numbering).
    pub chapter_number: u32,
}

impl Default for DocSettings {
    fn default() -> Self {
        DocSettings {
            intent: Intent::Print,
            page_width: 612.0,
            page_height: 792.0,
            facing_pages: true,
            primary_text_frame: false,
            bleed: [0.0; 4],
            slug: [0.0; 4],
            horizontal_units: Unit::Picas,
            vertical_units: Unit::Picas,
            baseline_grid: BaselineGrid::default(),
            grid: DocumentGrid::default(),
            pasteboard: (72.0, 72.0),
            margin_color: [255, 56, 255],
            column_color: [166, 40, 255],
            bleed_color: [255, 72, 103],
            slug_color: [100, 188, 221],
            keyboard_increment: 1.0,
            chapter_number: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    /// Selection / frame-edge colour.
    pub color: [u8; 3],
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "yes")]
    pub printable: bool,
    #[serde(default = "yes")]
    pub show_guides: bool,
    #[serde(default)]
    pub suppress_wrap_when_hidden: bool,
}

fn yes() -> bool {
    true
}

/// InDesign's layer colour sequence (names are the conventional colour names).
pub const LAYER_COLORS: &[(&str, [u8; 3])] = &[
    ("Light Blue", [43, 155, 255]),
    ("Red", [255, 0, 0]),
    ("Green", [79, 255, 79]),
    ("Blue", [0, 0, 255]),
    ("Yellow", [255, 255, 79]),
    ("Magenta", [255, 79, 255]),
    ("Cyan", [0, 255, 255]),
    ("Gray", [128, 128, 128]),
    ("Black", [0, 0, 0]),
    ("Orange", [255, 102, 0]),
    ("Dark Green", [0, 84, 0]),
    ("Teal", [0, 153, 153]),
    ("Tan", [204, 153, 102]),
    ("Brown", [153, 51, 0]),
    ("Violet", [153, 51, 255]),
    ("Gold", [255, 153, 0]),
];

/// An embedded file (placed image / PDF). Bytes are stored next to the JSON in the native format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    pub mime: String,
    /// Original location on disk (Links panel), if linked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip)]
    pub data: Arc<Vec<u8>>,
    /// Pixel size for raster images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixels: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub title: String,
    pub settings: DocSettings,
    pub spreads: Vec<Arc<Spread>>,
    pub parents: Vec<Arc<Spread>>,
    /// Top of the list = frontmost layer (Layers panel order).
    pub layers: Vec<Layer>,
    pub stories: BTreeMap<StoryId, Arc<Story>>,
    pub styles: Arc<Styles>,
    pub swatches: Vec<Swatch>,
    pub sections: Vec<Section>,
    #[serde(default)]
    pub assets: BTreeMap<AssetId, Arc<Asset>>,
    /// Hyperlinks (Window → Interactive → Hyperlinks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hyperlinks: Vec<Hyperlink>,
    /// PDF bookmarks (Window → Interactive → Bookmarks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bookmarks: Vec<Bookmark>,
    /// Words added to the document's user dictionary (spelling).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_words: Vec<String>,
    /// The generated table of contents (Layout → Table of Contents), kept for Update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toc: Option<Toc>,
    /// Text variable definitions (instances in stories are `vars::var_char(index)`).
    #[serde(default = "vars::defaults")]
    pub text_variables: Vec<vars::TextVariable>,
    /// Type › Document Footnote Options.
    #[serde(default)]
    pub footnote_options: FootnoteOptions,
    /// Creation / last save time (Unix seconds, UTC).
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub modified: i64,
    pub next_id: u64,
}

/// What a hyperlink is attached to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum HyperlinkSource {
    Item { id: ItemId },
    Text { story: StoryId, start: usize, end: usize },
}

/// Where a hyperlink goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum HyperlinkDest {
    Url(String),
    Email(String),
    /// Absolute page index.
    Page(usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hyperlink {
    pub id: u64,
    pub name: String,
    pub source: HyperlinkSource,
    pub dest: HyperlinkDest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TocEntry {
    /// Paragraph style whose paragraphs are listed.
    pub style: String,
    /// 1-based nesting level.
    #[serde(default = "one")]
    pub level: u8,
}

fn one() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Toc {
    pub story: StoryId,
    pub title: String,
    pub entries: Vec<TocEntry>,
    #[serde(default)]
    pub page_numbers: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub name: String,
    /// Absolute page index.
    pub page: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Bookmark>,
}

impl Document {
    /// Allocate a fresh id.
    pub fn alloc(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn page_count(&self) -> usize {
        self.spreads.iter().map(|s| s.pages.len()).sum()
    }

    /// (spread index, page index within spread) for an absolute page index.
    pub fn page_loc(&self, abs: usize) -> Option<(usize, usize)> {
        let mut n = 0;
        for (si, s) in self.spreads.iter().enumerate() {
            if abs < n + s.pages.len() {
                return Some((si, abs - n));
            }
            n += s.pages.len();
        }
        None
    }

    /// Absolute index of the first page of spread `si`.
    pub fn first_page_of_spread(&self, si: usize) -> usize {
        self.spreads[..si.min(self.spreads.len())].iter().map(|s| s.pages.len()).sum()
    }

    pub fn page(&self, abs: usize) -> Option<&Page> {
        let (s, p) = self.page_loc(abs)?;
        self.spreads[s].pages.get(p)
    }

    /// Iterate all pages with their absolute index and spread index.
    pub fn pages(&self) -> impl Iterator<Item = (usize, usize, &Page)> {
        self.spreads.iter().enumerate().flat_map(|(si, s)| s.pages.iter().map(move |p| (si, p))).enumerate().map(|(i, (si, p))| (i, si, p))
    }

    /// The section containing absolute page `abs`.
    pub fn section_of(&self, abs: usize) -> Option<&Section> {
        self.sections.iter().filter(|s| s.start <= abs).max_by_key(|s| s.start)
    }

    /// The page number (before formatting) of absolute page `abs`, per sections. A section without
    /// a start number continues from the previous section.
    pub fn page_number(&self, abs: usize) -> u32 {
        let Some(sec) = self.section_of(abs) else { return abs as u32 + 1 };
        let start = match sec.start_number {
            Some(n) => n,
            None if sec.start == 0 => 1,
            None => self.page_number(sec.start - 1) + 1,
        };
        start + (abs - sec.start) as u32
    }

    /// The displayed page name ("1", "iv", "A-3"…) for an absolute page index, per sections.
    pub fn page_name(&self, abs: usize) -> String {
        let Some(sec) = self.section_of(abs) else { return (abs + 1).to_string() };
        let num = sec.style.format(self.page_number(abs));
        if sec.include_prefix { format!("{}{}", sec.prefix, num) } else { num }
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }
    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }
    /// Stacking rank of a layer: 0 = backmost.
    pub fn layer_rank(&self, id: LayerId) -> usize {
        self.layers.iter().position(|l| l.id == id).map(|i| self.layers.len() - 1 - i).unwrap_or(0)
    }
    pub fn default_layer(&self) -> LayerId {
        self.layers.first().map(|l| l.id).unwrap_or_default()
    }

    pub fn story(&self, id: StoryId) -> Option<&Story> {
        self.stories.get(&id).map(|s| &**s)
    }
    pub fn story_mut(&mut self, id: StoryId) -> Option<&mut Story> {
        self.stories.get_mut(&id).map(Arc::make_mut)
    }

    pub fn swatch(&self, name: &str) -> Option<&Swatch> {
        self.swatches.iter().find(|s| s.name == name)
    }

    /// Resolve a swatch reference to a display colour.
    pub fn resolve_color(&self, swatch: &str, tint: f32) -> Option<designcraft_color::Color> {
        designcraft_color::swatch::resolve(&self.swatches, swatch, tint)
    }

    /// Validate structural invariants (ids unique, threads consistent, stories well-formed).
    pub fn check(&self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for sp in self.spreads.iter().chain(self.parents.iter()) {
            for it in &sp.items {
                let mut dup = None;
                it.walk(&mut |i| {
                    if !ids.insert(i.id.0) {
                        dup = Some(i.id);
                    }
                });
                if let Some(d) = dup {
                    return Err(DocError::Invalid(format!("duplicate item id {d}")));
                }
            }
        }
        for (sid, st) in &self.stories {
            st.check().map_err(DocError::Invalid)?;
            if st.id != *sid {
                return Err(DocError::Invalid(format!("story key {sid} != id {}", st.id)));
            }
            for f in &st.frames {
                let item = self.find(*f).ok_or(DocError::NoItem(*f))?;
                match self.item_at(&item).and_then(|i| i.text_frame().map(|t| t.story)) {
                    Some(s) if s == *sid => {}
                    other => return Err(DocError::Invalid(format!("frame {f} in story {sid} points at {other:?}"))),
                }
            }
        }
        // Every text frame is in its story's thread.
        for sp in self.spreads.iter().chain(self.parents.iter()) {
            for it in &sp.items {
                let mut bad = None;
                it.walk(&mut |i| {
                    if let Some(t) = i.text_frame()
                        && !self.stories.get(&t.story).is_some_and(|s| s.frames.contains(&i.id))
                    {
                        bad = Some(i.id);
                    }
                });
                if let Some(b) = bad {
                    return Err(DocError::Invalid(format!("text frame {b} not in its story's thread")));
                }
            }
        }
        if self.next_id < ids.iter().copied().max().unwrap_or(0) {
            return Err(DocError::Invalid("next_id behind existing ids".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

impl Document {
    /// Mutable styles (copy-on-write).
    pub fn styles_mut(&mut self) -> &mut Styles {
        Arc::make_mut(&mut self.styles)
    }
}
