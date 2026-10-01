//! DesignCraft's egui frontend: an InDesign-style UI over `designcraft-engine`.
//!
//! The UI is thin: every action goes through [`DesignApp::run`], which dispatches UI commands
//! (view/window) here and everything else to the engine. Menus, shortcuts, the ⌘K palette and
//! the control channel ([`control`]) share that entry point.
#![forbid(unsafe_code)]

pub mod canvas;
pub mod chrome;
pub mod control;
pub mod dialogs;
pub mod dock;
pub mod icons;
pub mod menus;
pub mod panels;
pub mod theme;
pub mod toolbar;
pub mod widgets;

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

use designcraft_engine::{Session, UiRequest, ViewInfo};
use designcraft_geom::{Point, Unit};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use control::{ControlRequest, ControlResponse};

pub type ReadFn = Box<dyn Fn(&str) -> Result<Vec<u8>, String>>;
pub type WriteFn = Box<dyn FnMut(&str, &[u8]) -> Result<(), String>>;
pub type PickFn = Box<dyn FnMut(&str) -> Option<String>>;
pub type OpenAsyncFn = Box<dyn FnMut(&str)>;
pub type DownloadFn = Box<dyn FnMut(&str, &[u8])>;
/// Files `(name, bytes)` delivered asynchronously by the host (web file picker, dropped files).
pub type Inbox = std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

/// Platform services injected by the host (desktop or web).
#[derive(Default)]
pub struct Services {
    /// Open-file dialog for a purpose (`open`, `place`) → path.
    pub pick_open: Option<PickFn>,
    /// Save dialog with a suggested name → path.
    pub pick_save: Option<PickFn>,
    pub read: Option<ReadFn>,
    pub write: Option<WriteFn>,
    /// Asynchronous open dialog for a purpose (`open`, `place`); the chosen file arrives later
    /// through [`Services::inbox`]. Used when `pick_open` is unset (web).
    pub open_async: Option<OpenAsyncFn>,
    /// Hand bytes to the user as a named file (browser download). When set, Save uses it
    /// instead of writing to a path.
    pub download: Option<DownloadFn>,
    /// Files delivered asynchronously, drained every frame: `.designcraft` → `file.openBytes`,
    /// anything else → `file.place`.
    pub inbox: Option<Inbox>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenMode {
    #[default]
    Normal,
    Preview,
    Bleed,
    Slug,
    Presentation,
}

/// Persisted UI state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiState {
    pub brightness: theme::Brightness,
    pub screen_mode: ScreenMode,
    pub frame_edges: bool,
    pub rulers: bool,
    pub guides: bool,
    pub baseline_grid: bool,
    pub document_grid: bool,
    pub text_threads: bool,
    pub hidden_characters: bool,
    pub control_bar: bool,
    pub tools_double_column: bool,
    /// Expanded right-dock panel group tab.
    pub dock_tab: String,
    /// Panel opened from the collapsed icon column (flyout).
    pub open_panel: Option<String>,
    pub dock_expanded: bool,
    pub units: Unit,
    #[serde(skip)]
    pub status: String,
    #[serde(skip)]
    pub dialog: Option<dialogs::Dialog>,
    #[serde(skip)]
    pub palette: Option<String>,
    #[serde(skip)]
    pub flyout: Option<usize>,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            brightness: theme::Brightness::Dark,
            screen_mode: ScreenMode::Normal,
            frame_edges: true,
            rulers: true,
            guides: true,
            baseline_grid: false,
            document_grid: false,
            text_threads: false,
            hidden_characters: false,
            control_bar: true,
            tools_double_column: false,
            dock_tab: "properties".into(),
            open_panel: None,
            dock_expanded: true,
            units: Unit::Picas,
            status: String::new(),
            dialog: None,
            palette: None,
            flyout: None,
        }
    }
}

/// Per-document view: zoom (screen points per document point) and the canvas point at the
/// top-left of the canvas area.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct View {
    pub zoom: f64,
    pub origin: Point,
    pub fitted: bool,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: 0.5, origin: Point::new(-100.0, -100.0), fitted: false }
    }
}

pub struct CanvasCache {
    pub renderer: designcraft_render::Renderer,
    pub texture: Option<egui::TextureHandle>,
    pub key: Option<(u64, u64, u64, u64, u64, u32, u32, u8)>,
    pub last_ms: f64,
}

#[derive(Default, Clone, Copy, Debug, Serialize)]
pub struct Perf {
    pub frame_ms: f64,
    pub render_ms: f64,
    pub fps: f64,
}

pub struct DesignApp {
    pub session: Session,
    pub ui: UiState,
    pub services: Services,
    pub views: HashMap<u64, View>,
    pub canvas: CanvasCache,
    pub canvas_rect: Option<egui::Rect>,
    pub perf: Perf,
    pub synthetic: Vec<egui::Event>,
    control_rx: Option<Receiver<ControlRequest>>,
    pending_shots: Vec<(u64, Option<String>, Sender<ControlResponse>, f64)>,
    queued_shots: Vec<(u64, f64, u32)>,
    shot_token: u64,
    styled: bool,
    pub restyle: bool,
    fonts_ready: bool,
    pub integrated_titlebar: bool,
    last_time: f64,
}

impl DesignApp {
    pub fn new(session: Session, services: Services) -> Self {
        let mut renderer = designcraft_render::Renderer::new();
        renderer.threads = designcraft_render::default_threads();
        DesignApp {
            session,
            ui: UiState::default(),
            services,
            views: HashMap::new(),
            canvas: CanvasCache { renderer, texture: None, key: None, last_ms: 0.0 },
            canvas_rect: None,
            perf: Perf::default(),
            synthetic: vec![],
            control_rx: None,
            pending_shots: vec![],
            queued_shots: vec![],
            shot_token: 0,
            styled: false,
            restyle: false,
            fonts_ready: false,
            integrated_titlebar: false,
            last_time: 0.0,
        }
    }

    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control_rx = Some(rx);
        self
    }

    pub fn view(&self) -> Option<&View> {
        let uid = self.session.active()?.uid;
        self.views.get(&uid)
    }
    pub fn view_mut(&mut self) -> Option<&mut View> {
        let uid = self.session.active()?.uid;
        Some(self.views.entry(uid).or_default())
    }
    pub fn view_info(&self) -> ViewInfo {
        ViewInfo { zoom: self.view().map(|v| v.zoom).unwrap_or(1.0) }
    }

    pub fn status(&mut self, s: impl Into<String>) {
        self.ui.status = s.into();
    }

    /// THE entry point for every action (menus, shortcuts, palette, panels, control channel).
    pub fn run(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if let Some(r) = menus::run_ui(self, id, &params) {
            return r;
        }
        let r = self.session.execute(id, &params).map_err(|e| e.to_string());
        self.after_engine();
        if let Err(e) = &r {
            self.status(e.clone());
        }
        r
    }

    /// Handle requests produced by tools/commands (dialogs, view changes, file pickers).
    pub fn after_engine(&mut self) {
        let reqs = std::mem::take(&mut self.session.ui_requests);
        for r in reqs {
            match r {
                UiRequest::Dialog { id, params } => self.ui.dialog = Some(dialogs::Dialog::new(&id, params)),
                UiRequest::View { params } => canvas::apply_view_request(self, &params),
                UiRequest::Pick { purpose, params } => {
                    let _ = params;
                    let _ = self.pick_and_open(&purpose);
                }
            }
        }
        // New documents get a fitted view.
        if let Some(st) = self.session.active() {
            let uid = st.uid;
            self.views.entry(uid).or_default();
        }
    }

    /// Ask the host for a file to open (`open`) or place (`place`). Synchronous pickers run the
    /// command right away; asynchronous ones (web) deliver the file through the inbox.
    pub fn pick_and_open(&mut self, purpose: &str) -> Result<Value, String> {
        let cmd = if purpose == "place" { "file.place" } else { "file.open" };
        if let Some(pick) = self.services.pick_open.as_mut() {
            return match pick(purpose) {
                Some(path) => self.run(cmd, json!({"path": path})),
                None => Ok(Value::Null),
            };
        }
        if let Some(open) = self.services.open_async.as_mut() {
            open(purpose);
        }
        Ok(Value::Null)
    }

    /// Open or place files delivered through the inbox.
    fn drain_inbox(&mut self) {
        let Some(inbox) = self.services.inbox.clone() else { return };
        let files = std::mem::take(&mut *inbox.lock().unwrap_or_else(|e| e.into_inner()));
        for (name, bytes) in files {
            let b64 = designcraft_engine::cmd::base64_encode(&bytes);
            let r = if name.to_ascii_lowercase().ends_with(".designcraft") {
                let title = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
                self.run("file.openBytes", json!({"name": title, "base64": b64}))
            } else {
                self.run("file.place", json!({"name": name, "base64": b64}))
            };
            if let Err(e) = r {
                self.status(format!("{name}: {e}"));
            }
        }
    }

    pub fn select_tool(&mut self, id: &str) {
        let _ = self.run("tool.select", json!({"tool": id}));
        self.ui.flyout = None;
    }

    /// Per-frame logic before layout.
    pub fn logic(&mut self, ctx: &egui::Context) {
        if !self.styled {
            theme::install_fonts(ctx);
            self.styled = true;
            self.restyle = true;
        } else {
            self.fonts_ready = true;
        }
        if self.restyle {
            theme::apply(ctx, &theme::Tokens::for_brightness(self.ui.brightness));
            self.restyle = false;
        }
        let now = ctx.input(|i| i.time);
        let dt = now - self.last_time;
        if dt > 0.0 {
            self.perf.fps = self.perf.fps * 0.9 + (1.0 / dt).min(240.0) * 0.1;
        }
        self.last_time = now;
        self.drain_control(ctx);
        if self.fonts_ready {
            self.drain_inbox();
        }
        if !self.synthetic.is_empty() {
            ctx.request_repaint();
        }
        self.collect_screenshots(ctx);
        self.issue_screenshots(ctx);
        if self.fonts_ready {
            menus::shortcuts(self, ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            {
                let p = f.path().to_string_lossy().to_string();
                if p.is_empty() {
                    continue;
                }
                let cmd = if p.ends_with(".designcraft") { "file.open" } else { "file.place" };
                let _ = self.run(cmd, json!({"path": p}));
            }
        }
    }

    /// Inject synthetic events (one pointer event per frame).
    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        if self.synthetic.is_empty() {
            return;
        }
        let n = match self.synthetic[0] {
            egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. } => 1,
            _ => self.synthetic.iter().position(|e| matches!(e, egui::Event::Key { pressed: false, .. })).map_or(self.synthetic.len(), |i| i + 1),
        };
        if let Some(egui::Event::PointerMoved(p) | egui::Event::PointerButton { pos: p, .. }) = self.synthetic.first() {
            raw.events.push(egui::Event::PointerMoved(*p));
        }
        raw.events.extend(self.synthetic.drain(..n));
    }

    /// Lay out the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.fonts_ready {
            ctx.request_repaint();
            return;
        }
        let t0 = now_ms();
        let t = theme::Tokens::get(&ctx);
        let presenting = self.ui.screen_mode == ScreenMode::Presentation;
        if !presenting {
            chrome::app_bar(self, ui);
            if self.ui.control_bar && self.session.active().is_some() {
                chrome::control_bar(self, ui);
            }
            chrome::status_bar(self, ui);
            toolbar::show(self, ui);
            dock::show(self, ui);
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.pasteboard)).show(ui, |ui| {
            if self.session.active().is_none() {
                chrome::start_screen(self, ui);
                return;
            }
            if !presenting {
                chrome::doc_tabs(self, ui);
            }
            canvas::show(self, ui);
        });
        dock::flyout(self, &ctx);
        dialogs::show(self, &ctx);
        menus::palette(self, &ctx);
        self.perf.frame_ms = now_ms() - t0;
    }

    fn drain_control(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.control_rx.take() else { return };
        while let Ok(req) = rx.try_recv() {
            let reply = req.reply.clone();
            match control::handle(self, ctx, &req) {
                control::Outcome::Done(v) => {
                    let _ = reply.send(v);
                }
                control::Outcome::Screenshot { path } => {
                    self.shot_token += 1;
                    let token = self.shot_token;
                    self.queued_shots.push((token, now_ms() + 120.0, 0));
                    self.pending_shots.push((token, path, reply, now_ms() + 8000.0));
                }
            }
        }
        self.control_rx = Some(rx);
    }

    fn issue_screenshots(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        self.queued_shots.retain_mut(|(token, at, frames)| {
            *frames += 1;
            if now >= *at && *frames >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(*token)));
                false
            } else {
                true
            }
        });
        if !self.queued_shots.is_empty() || !self.pending_shots.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn collect_screenshots(&mut self, ctx: &egui::Context) {
        if self.pending_shots.is_empty() {
            return;
        }
        let events: Vec<_> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { user_data, image, .. } => {
                        let token = user_data.data.as_ref().and_then(|d| d.downcast_ref::<u64>()).copied()?;
                        Some((token, image.clone()))
                    }
                    _ => None,
                })
                .collect()
        });
        for (token, image) in events {
            if let Some(i) = self.pending_shots.iter().position(|(t, ..)| *t == token) {
                let (_, path, reply, _) = self.pending_shots.remove(i);
                let _ = reply.send(control::save_screenshot(self, &image, path.as_deref()));
            }
        }
        let now = now_ms();
        self.pending_shots.retain(|(_, _, reply, deadline)| {
            if now < *deadline {
                return true;
            }
            let _ = reply.send(json!({"ok": false, "error": "no frame was presented (screen locked or window hidden); use ui.render"}));
            false
        });
    }
}

pub fn now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}
