//! The DesignCraft engine façade.
//!
//! Every user-visible action is a command with a stable id (`frame.create`, `text.insert`,
//! `layout.pages.insert`…) and JSON parameters. The egui UI, the CLI, the control channel and MCP
//! all go through [`Session::execute`]. Tools (pointer gestures) are hosted here and reduce to
//! commands, so every gesture is journaled and replayable.
#![forbid(unsafe_code)]

pub mod cmd;
pub mod sample;
mod tooling;

use std::sync::Arc;

use designcraft_compose::Cache;
use designcraft_doc::{Document, LayerId, Selection};
use serde_json::Value;

pub use cmd::{CommandInfo, CommandSpec, command_specs, find_command};
pub use designcraft_compose as compose;
pub use designcraft_doc as doc;
pub use designcraft_tools as tools;
pub use tooling::{UiRequest, ViewInfo};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("command `{0}` is not available right now: {1}")]
    Disabled(String, String),
    #[error("invalid parameters for `{cmd}`: {msg}")]
    BadParams { cmd: String, msg: String },
    #[error("no active document")]
    NoDocument,
    #[error("{0}")]
    Other(String),
}

impl From<designcraft_doc::DocError> for EngineError {
    fn from(e: designcraft_doc::DocError) -> Self {
        EngineError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub label: String,
    pub doc: Arc<Document>,
    pub selection: Selection,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub limit: usize,
}

#[derive(Clone, Debug)]
pub struct Interaction {
    pub label: String,
    pub doc: Arc<Document>,
    pub selection: Selection,
    pub preview: Option<(String, Value)>,
}

#[derive(Clone, Debug)]
pub struct DocState {
    pub doc: Arc<Document>,
    pub selection: Selection,
    pub history: History,
    pub path: Option<String>,
    pub revision: u64,
    pub saved_revision: u64,
    /// The document as last saved (dirty = the current document is a different allocation).
    pub saved_doc: Arc<Document>,
    pub active_layer: LayerId,
    pub interaction: Option<Interaction>,
    /// Editing parent spreads (Pages panel double-click on a parent).
    pub editing_parents: bool,
    pub uid: u64,
}

static NEXT_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl DocState {
    pub fn new(doc: Document, path: Option<String>) -> Self {
        let active_layer = doc.default_layer();
        let doc = Arc::new(doc);
        DocState {
            saved_doc: doc.clone(),
            doc,
            selection: Selection::default(),
            history: History { limit: 1000, ..Default::default() },
            path,
            revision: 1,
            saved_revision: 1,
            active_layer,
            interaction: None,
            editing_parents: false,
            uid: NEXT_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }
    pub fn is_dirty(&self) -> bool {
        !Arc::ptr_eq(&self.doc, &self.saved_doc)
    }
    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.doc.title.clone())
    }
}

/// Preferences that the engine needs (UI keeps its own).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    pub keyboard_increment: f64,
    pub show_hidden_characters: bool,
    pub typographers_quotes: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { keyboard_increment: 1.0, show_hidden_characters: false, typographers_quotes: true }
    }
}

pub struct Session {
    docs: Vec<DocState>,
    active: Option<usize>,
    pub prefs: Prefs,
    pub cache: Arc<Cache>,
    pub(crate) tool: Box<dyn designcraft_tools::Tool>,
    pub journal: Vec<(String, Value)>,
    pub clipboard: Option<Arc<Document>>,
    /// Requests for the UI (dialogs, view changes) produced by tools/commands.
    pub ui_requests: Vec<UiRequest>,
    /// Graphic loaded in the place cursor: (asset, natural size in points).
    pub loaded: Option<(designcraft_doc::AssetId, (f64, f64))>,
    pub(crate) untitled: u32,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Session {
            docs: vec![],
            active: None,
            prefs: Prefs::default(),
            cache: Arc::new(Cache::new()),
            tool: designcraft_tools::create("selection"),
            journal: vec![],
            clipboard: None,
            ui_requests: vec![],
            loaded: None,
            untitled: 0,
        }
    }

    pub fn documents(&self) -> &[DocState] {
        &self.docs
    }
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }
    pub fn active(&self) -> Option<&DocState> {
        self.active.and_then(|i| self.docs.get(i))
    }
    pub fn active_mut(&mut self) -> Option<&mut DocState> {
        self.active.and_then(|i| self.docs.get_mut(i))
    }
    pub fn doc(&self) -> Result<&DocState> {
        self.active().ok_or(EngineError::NoDocument)
    }
    pub fn doc_mut(&mut self) -> Result<&mut DocState> {
        self.active_mut().ok_or(EngineError::NoDocument)
    }
    pub fn set_active(&mut self, i: usize) {
        if i < self.docs.len() {
            self.active = Some(i);
        }
    }
    pub fn add_document(&mut self, d: DocState) -> usize {
        self.docs.push(d);
        self.active = Some(self.docs.len() - 1);
        self.docs.len() - 1
    }
    pub fn close_document(&mut self, i: usize) {
        if i < self.docs.len() {
            self.docs.remove(i);
            self.active = if self.docs.is_empty() { None } else { Some(i.min(self.docs.len() - 1)) };
        }
    }

    /// Run a command by id. Edits push one undo step (unless inside an interaction).
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value> {
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.into()))?;
        (spec.enabled)(self).map_err(|e| EngineError::Disabled(id.into(), e))?;
        let before = self.active().map(|d| (d.uid, d.doc.clone()));
        let r = (spec.run)(self, params)?;
        // Record undo if the document changed (and we're not previewing an interaction).
        if let (Some((uid, old)), Some(st)) = (before, self.active_mut())
            && st.uid == uid
            && !Arc::ptr_eq(&old, &st.doc)
            && st.interaction.is_none()
            && spec.undoable
        {
            let entry = HistoryEntry { label: spec.label.to_string(), doc: old, selection: st.selection.clone() };
            push_undo(st, entry);
        }
        if spec.journal {
            self.journal.push((id.to_string(), params.clone()));
        }
        Ok(r)
    }

    pub fn commands(&self) -> Vec<CommandInfo> {
        command_specs().iter().map(|c| c.info(self)).collect()
    }

    /// Mutate the active document (copy-on-write) and bump the revision.
    pub fn edit<T>(&mut self, f: impl FnOnce(&mut Document, &mut Selection) -> Result<T>) -> Result<T> {
        let st = self.doc_mut()?;
        let mut doc = (*st.doc).clone();
        let mut sel = st.selection.clone();
        let r = f(&mut doc, &mut sel)?;
        st.doc = Arc::new(doc);
        st.selection = sel;
        st.revision += 1;
        Ok(r)
    }
}

fn push_undo(st: &mut DocState, e: HistoryEntry) {
    st.history.undo.push(e);
    st.history.redo.clear();
    if st.history.undo.len() > st.history.limit.max(1) {
        st.history.undo.remove(0);
    }
}

pub(crate) use push_undo as record_undo;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_idml;
