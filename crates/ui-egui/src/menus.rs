//! The menu bar tree (InDesign order), UI-only commands, keyboard shortcuts and the ⌘K palette.

use serde_json::{Value, json};

use crate::{DesignApp, ScreenMode};

/// UI commands: (id, label, shortcut, params).
pub const UI_COMMANDS: &[(&str, &str, Option<&str>, &str)] = &[
    ("app.newDocumentDialog", "New Document…", Some("Cmd+N"), "{}"),
    ("app.openDialog", "Open…", Some("Cmd+O"), "{}"),
    ("app.placeDialog", "Place…", Some("Cmd+D"), "{}"),
    ("app.saveDialog", "Save As…", Some("Cmd+Shift+S"), "{}"),
    ("app.save", "Save", Some("Cmd+S"), "{}"),
    ("app.exportPng", "Export Page as PNG…", Some("Cmd+E"), "{}"),
    ("app.palette", "Command Palette…", Some("Cmd+K"), "{}"),
    ("view.zoomIn", "Zoom In", Some("Cmd+="), "{}"),
    ("view.zoomOut", "Zoom Out", Some("Cmd+-"), "{}"),
    ("view.fitPage", "Fit Page in Window", Some("Cmd+0"), "{}"),
    ("view.fitSpread", "Fit Spread in Window", Some("Cmd+Alt+0"), "{}"),
    ("view.actualSize", "Actual Size", Some("Cmd+1"), "{}"),
    ("view.entirePasteboard", "Entire Pasteboard", Some("Cmd+Alt+Shift+0"), "{}"),
    ("view.zoom", "Zoom To", None, "{zoom: 1.0 = 100%}"),
    ("view.screenMode", "Screen Mode", None, "{mode: normal|preview|bleed|slug|presentation}"),
    ("view.togglePreview", "Toggle Normal/Preview", Some("W"), "{}"),
    ("view.frameEdges", "Show/Hide Frame Edges", Some("Cmd+H"), "{}"),
    ("view.rulers", "Show/Hide Rulers", Some("Cmd+R"), "{}"),
    ("view.guides", "Show/Hide Guides", Some("Cmd+;"), "{}"),
    ("view.baselineGrid", "Show/Hide Baseline Grid", Some("Cmd+Alt+'"), "{}"),
    ("view.textThreads", "Show/Hide Text Threads", Some("Cmd+Alt+Y"), "{}"),
    ("view.hiddenCharacters", "Show/Hide Hidden Characters", Some("Cmd+Alt+I"), "{}"),
    ("view.goToPage", "Go to Page…", Some("Cmd+J"), "{page}"),
    (
        "window.panel",
        "Show Panel",
        None,
        "{panel: properties|pages|layers|swatches|paragraphStyles|characterStyles|stroke|character|paragraph|textWrap|links}",
    ),
    ("window.controlBar", "Control", Some("Cmd+Alt+6"), "{}"),
    ("window.toolsDoubleColumn", "Tools: Double Column", None, "{}"),
    ("window.brightness", "Interface Color Theme", None, "{brightness: dark|mediumDark|mediumLight|light}"),
];

/// Menu bar: (menu, entries). Entries: `cmd:<id>`, `ui:<id>`, `-` separator, `>Submenu` … `<`.
pub const MENUS: &[(&str, &[&str])] = &[
    (
        "File",
        &[
            "ui:app.newDocumentDialog",
            "ui:app.openDialog",
            "-",
            "cmd:file.close",
            "ui:app.save",
            "ui:app.saveDialog",
            "-",
            "ui:app.placeDialog",
            "-",
            "ui:app.exportPng",
            "-",
            "cmd:layout.documentSetup",
        ],
    ),
    (
        "Edit",
        &[
            "cmd:edit.undo",
            "cmd:edit.redo",
            "-",
            "cmd:edit.cut",
            "cmd:edit.copy",
            "cmd:edit.paste",
            "cmd:edit.pasteInPlace",
            "cmd:edit.clear",
            "-",
            "cmd:edit.duplicate",
            "-",
            "cmd:edit.selectAll",
            "cmd:edit.deselectAll",
            "-",
            "ui:app.palette",
        ],
    ),
    ("Layout", &[">Pages", "cmd:layout.pages.insert", "cmd:layout.parents.new", "<", "cmd:layout.marginsAndColumns", "-", "ui:view.goToPage"]),
    (
        "Type",
        &[
            "cmd:type.fillWithPlaceholder",
            "-",
            "cmd:type.alignLeft",
            "cmd:type.alignCenter",
            "cmd:type.alignRight",
            "cmd:type.justify",
            "-",
            "cmd:type.bold",
            "cmd:type.italic",
            "cmd:type.underline",
            "cmd:type.allCaps",
            "-",
            "cmd:type.sizeUp",
            "cmd:type.sizeDown",
            "-",
            "ui:view.hiddenCharacters",
        ],
    ),
    (
        "Object",
        &[
            ">Transform",
            "cmd:transform.flip",
            "<",
            ">Arrange",
            "cmd:object.bringToFront",
            "cmd:object.bringForward",
            "cmd:object.sendBackward",
            "cmd:object.sendToBack",
            "<",
            "-",
            "cmd:object.group",
            "cmd:object.ungroup",
            "cmd:object.lock",
            "cmd:object.unlockAll",
            "cmd:object.hide",
            "cmd:object.showAll",
            "-",
            "cmd:object.textFrameOptions",
            "cmd:object.cornerOptions",
            "-",
            ">Effects",
            "cmd:object.dropShadow",
            "<",
        ],
    ),
    ("Table", &[]),
    (
        "View",
        &[
            "ui:view.zoomIn",
            "ui:view.zoomOut",
            "ui:view.fitPage",
            "ui:view.fitSpread",
            "ui:view.actualSize",
            "ui:view.entirePasteboard",
            "-",
            "ui:view.togglePreview",
            "-",
            "ui:view.rulers",
            "ui:view.frameEdges",
            "ui:view.textThreads",
            "ui:view.hiddenCharacters",
            "-",
            "ui:view.guides",
            "ui:view.baselineGrid",
        ],
    ),
    ("Window", &["ui:window.controlBar", "ui:window.toolsDoubleColumn", "-", "ui:window.panel", "-", "ui:window.brightness"]),
    ("Help", &["cmd:file.newSample"]),
];

pub fn ui_label(id: &str) -> Option<(&'static str, Option<&'static str>)> {
    UI_COMMANDS.iter().find(|c| c.0 == id).map(|c| (c.1, c.2))
}

/// Run a UI command; `None` if `id` isn't one.
pub fn run_ui(app: &mut DesignApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    let rect = app.canvas_rect;
    let flag = |b: &mut bool| {
        *b = !*b;
        Ok(json!(*b))
    };
    Some(match id {
        "app.newDocumentDialog" => {
            app.ui.dialog = Some(crate::dialogs::Dialog::new("newDocument", json!({})));
            Ok(Value::Null)
        }
        "app.openDialog" => {
            if let Some(path) = app.services.pick_open.as_mut().and_then(|f| f("open")) {
                return Some(app.run("file.open", json!({"path": path})));
            }
            Ok(Value::Null)
        }
        "app.placeDialog" => {
            if let Some(path) = app.services.pick_open.as_mut().and_then(|f| f("place")) {
                return Some(app.run("file.place", json!({"path": path})));
            }
            Ok(Value::Null)
        }
        "app.save" => {
            if app.session.active().is_some_and(|d| d.path.is_some()) {
                return Some(app.run("file.save", json!({})));
            }
            return run_ui(app, "app.saveDialog", p);
        }
        "app.saveDialog" => {
            let name = app.session.active().map(|d| format!("{}.designcraft", d.doc.title)).unwrap_or_default();
            if let Some(path) = app.services.pick_save.as_mut().and_then(|f| f(&name)) {
                return Some(app.run("file.saveAs", json!({"path": path})));
            }
            Ok(Value::Null)
        }
        "app.exportPng" => export_png(app, p),
        "app.palette" => {
            app.ui.palette = Some(String::new());
            Ok(Value::Null)
        }
        "view.zoomIn" | "view.zoomOut" => {
            if let Some(r) = rect {
                crate::canvas::zoom_at(app, r.center(), if id == "view.zoomIn" { 1.5 } else { 1.0 / 1.5 });
            }
            Ok(Value::Null)
        }
        "view.fitPage" | "view.fitSpread" | "view.entirePasteboard" => {
            if let Some(r) = rect {
                crate::canvas::fit(
                    app,
                    r,
                    if id == "view.fitPage" {
                        "page"
                    } else if id == "view.fitSpread" {
                        "spread"
                    } else {
                        "all"
                    },
                );
            }
            Ok(Value::Null)
        }
        "view.actualSize" => {
            crate::canvas::set_zoom(app, 1.0);
            Ok(Value::Null)
        }
        "view.zoom" => {
            crate::canvas::set_zoom(app, p.get("zoom").and_then(Value::as_f64).unwrap_or(1.0));
            Ok(Value::Null)
        }
        "view.screenMode" => {
            app.ui.screen_mode = match p.get("mode").and_then(Value::as_str).unwrap_or("normal") {
                "preview" => ScreenMode::Preview,
                "bleed" => ScreenMode::Bleed,
                "slug" => ScreenMode::Slug,
                "presentation" => ScreenMode::Presentation,
                _ => ScreenMode::Normal,
            };
            Ok(Value::Null)
        }
        "view.togglePreview" => {
            app.ui.screen_mode = if app.ui.screen_mode == ScreenMode::Normal { ScreenMode::Preview } else { ScreenMode::Normal };
            Ok(Value::Null)
        }
        "view.frameEdges" => flag(&mut app.ui.frame_edges),
        "view.rulers" => flag(&mut app.ui.rulers),
        "view.guides" => flag(&mut app.ui.guides),
        "view.baselineGrid" => flag(&mut app.ui.baseline_grid),
        "view.textThreads" => flag(&mut app.ui.text_threads),
        "view.hiddenCharacters" => flag(&mut app.ui.hidden_characters),
        "view.goToPage" => {
            match p.get("page").and_then(Value::as_u64) {
                Some(n) => crate::canvas::go_to_page(app, (n as usize).saturating_sub(1)),
                None => app.ui.dialog = Some(crate::dialogs::Dialog::new("goToPage", json!({}))),
            }
            Ok(Value::Null)
        }
        "window.panel" => {
            let panel = p.get("panel").and_then(Value::as_str).unwrap_or("properties").to_string();
            if crate::dock::DOCK_TABS.iter().any(|(id, _, _)| *id == panel) {
                app.ui.dock_tab = panel;
                app.ui.dock_expanded = true;
                app.ui.open_panel = None;
            } else {
                app.ui.open_panel = if app.ui.open_panel.as_deref() == Some(panel.as_str()) { None } else { Some(panel) };
            }
            Ok(Value::Null)
        }
        "window.controlBar" => flag(&mut app.ui.control_bar),
        "window.toolsDoubleColumn" => flag(&mut app.ui.tools_double_column),
        "window.brightness" => {
            let b = p.get("brightness").and_then(Value::as_str).and_then(crate::theme::Brightness::parse).unwrap_or(crate::theme::Brightness::Dark);
            app.ui.brightness = b;
            app.restyle = true;
            Ok(Value::Null)
        }
        _ => return None,
    })
}

fn export_png(app: &mut DesignApp, p: &Value) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document")?;
    let page = p.get("page").and_then(Value::as_u64).map(|v| v as usize).or_else(|| crate::canvas::current_page(app)).unwrap_or(0);
    let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(2.0);
    let mut r = designcraft_render::Renderer::new();
    let img = r
        .render_page(
            &st.doc,
            &app.session.cache,
            page,
            scale,
            false,
            &designcraft_render::RenderOptions { printing_only: true, ..Default::default() },
        )
        .ok_or("no such page")?;
    let png = img.to_png();
    let path = match p.get("path").and_then(Value::as_str) {
        Some(s) => Some(s.to_string()),
        None => {
            let name = format!("{}-p{}.png", st.doc.title, page + 1);
            app.services.pick_save.as_mut().and_then(|f| f(&name))
        }
    };
    let Some(path) = path else { return Ok(Value::Null) };
    match app.services.write.as_mut() {
        Some(w) => w(&path, &png).map(|_| json!({"path": path, "width": img.width, "height": img.height})),
        None => Err("no writer".into()),
    }
}

/// Is a command enabled (for menu greying)?
pub fn enabled(app: &DesignApp, id: &str) -> bool {
    match designcraft_engine::find_command(id) {
        Some(c) => (c.enabled)(&app.session).is_ok(),
        None => true,
    }
}

fn shortcut_text(sc: &str) -> String {
    let mac = cfg!(target_os = "macos");
    sc.split('+')
        .map(|k| match k {
            "Cmd" => if mac { "⌘" } else { "Ctrl+" }.to_string(),
            "Shift" => if mac { "⇧" } else { "Shift+" }.to_string(),
            "Alt" => if mac { "⌥" } else { "Alt+" }.to_string(),
            k => k.to_string(),
        })
        .collect()
}

/// The menu bar contents (inside the app bar).
pub fn menu_bar(app: &mut DesignApp, ui: &mut egui::Ui) {
    for (menu, entries) in MENUS {
        ui.menu_button(*menu, |ui| {
            ui.set_min_width(240.0);
            menu_entries(app, ui, entries);
        });
    }
}

fn menu_entries(app: &mut DesignApp, ui: &mut egui::Ui, entries: &[&str]) {
    let mut i = 0;
    while i < entries.len() {
        let e = entries[i];
        i += 1;
        if e == "-" {
            ui.separator();
            continue;
        }
        if let Some(name) = e.strip_prefix('>') {
            let start = i;
            while i < entries.len() && entries[i] != "<" {
                i += 1;
            }
            let sub = &entries[start..i];
            i += 1;
            ui.menu_button(name, |ui| menu_entries(app, ui, sub));
            continue;
        }
        if e == "ui:window.panel" {
            ui.menu_button("Panels", |ui| {
                for (id, label, _) in crate::dock::DOCK_TABS.iter().chain(crate::dock::ICON_PANELS) {
                    if ui.button(*label).clicked() {
                        let _ = app.run("window.panel", json!({"panel": id}));
                        ui.close();
                    }
                }
            });
            continue;
        }
        if e == "ui:window.brightness" {
            ui.menu_button("Interface Color Theme", |ui| {
                for b in crate::theme::Brightness::ALL {
                    if ui.radio(app.ui.brightness == b, b.label()).clicked() {
                        let _ = app.run("window.brightness", json!({"brightness": b.id()}));
                        ui.close();
                    }
                }
            });
            continue;
        }
        let (kind, id) = e.split_once(':').unwrap_or(("cmd", e));
        let (label, sc, en) = if kind == "ui" {
            let (l, sc) = ui_label(id).unwrap_or((id, None));
            (l.to_string(), sc, app.session.active().is_some() || id.starts_with("app.") || id.starts_with("window."))
        } else {
            match designcraft_engine::find_command(id) {
                Some(c) => (c.label.to_string(), c.shortcut, enabled(app, id)),
                None => (id.to_string(), None, false),
            }
        };
        let checked = match id {
            "view.rulers" => Some(app.ui.rulers),
            "view.frameEdges" => Some(app.ui.frame_edges),
            "view.guides" => Some(app.ui.guides),
            "view.baselineGrid" => Some(app.ui.baseline_grid),
            "view.textThreads" => Some(app.ui.text_threads),
            "view.hiddenCharacters" => Some(app.ui.hidden_characters),
            "window.controlBar" => Some(app.ui.control_bar),
            _ => None,
        };
        let text = match checked {
            Some(true) => format!("✓ {label}"),
            Some(false) => format!("   {label}"),
            None => label,
        };
        let mut b = egui::Button::new(text);
        if let Some(sc) = sc {
            b = b.shortcut_text(shortcut_text(sc));
        }
        if ui.add_enabled(en, b).clicked() {
            let _ = app.run(id, json!({}));
            ui.close();
        }
    }
}

fn parse_shortcut(sc: &str) -> Option<(egui::Modifiers, egui::Key)> {
    let mut m = egui::Modifiers::NONE;
    let mut key = None;
    for part in sc.split('+').filter(|s| !s.is_empty()) {
        match part {
            "Cmd" => m.command = true,
            "Shift" => m.shift = true,
            "Alt" => m.alt = true,
            "Ctrl" => m.ctrl = true,
            "=" => key = Some(egui::Key::Equals),
            "-" => key = Some(egui::Key::Minus),
            "[" => key = Some(egui::Key::OpenBracket),
            "]" => key = Some(egui::Key::CloseBracket),
            ";" => key = Some(egui::Key::Semicolon),
            "'" => key = Some(egui::Key::Quote),
            "," => key = Some(egui::Key::Comma),
            "." => key = Some(egui::Key::Period),
            "\\" => key = Some(egui::Key::Backslash),
            "Delete" => key = Some(egui::Key::Delete),
            k => key = egui::Key::from_name(k),
        }
    }
    if sc.ends_with("+-") || sc == "-" {
        key = Some(egui::Key::Minus);
    }
    key.map(|k| (m, k))
}

/// Global keyboard shortcuts: menu commands and single-key tool shortcuts.
pub fn shortcuts(app: &mut DesignApp, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() || app.ui.dialog.is_some() || app.ui.palette.is_some() {
        return;
    }
    let typing = app.session.wants_text();
    let events = ctx.input(|i| i.events.clone());
    for e in events {
        let egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } = e else { continue };
        // Command shortcuts.
        let mut fired = None;
        let all = designcraft_engine::command_specs()
            .iter()
            .filter_map(|c| c.shortcut.map(|s| (c.id, s)))
            .chain(UI_COMMANDS.iter().filter_map(|c| c.2.map(|s| (c.0, s))));
        for (id, sc) in all {
            if let Some((m, k)) = parse_shortcut(sc)
                && k == key
                && m.command == modifiers.command
                && m.shift == modifiers.shift
                && m.alt == modifiers.alt
                && (m.command || m.alt || !typing)
                && !(id == "edit.clear" && typing)
            {
                // Single-key shortcuts (W) only when not typing.
                if !m.command && !m.alt && !m.shift && typing {
                    continue;
                }
                if id == "edit.clear" {
                    continue; // Delete is handled by the active tool.
                }
                fired = Some(id);
                break;
            }
        }
        if let Some(id) = fired {
            let _ = app.run(id, json!({}));
            continue;
        }
        // Tool shortcuts (single keys, Shift+key).
        if !typing && !modifiers.command && !modifiers.alt {
            let name = key.name();
            let sc = if modifiers.shift { format!("Shift+{name}") } else { name.to_string() };
            if let Some(tool) = designcraft_tools::tool_for_shortcut(&sc) {
                app.select_tool(tool);
            } else if key == egui::Key::Escape && app.session.tool_id() != "selection" && !app.session.tool_busy() {
                app.select_tool("selection");
            }
        }
    }
}

/// ⌘K command palette.
pub fn palette(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(mut q) = app.ui.palette.clone() else { return };
    let mut close = false;
    let mut run: Option<String> = None;
    egui::Modal::new(egui::Id::new("palette")).show(ctx, |ui| {
        ui.set_width(480.0);
        let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search commands…").desired_width(f32::INFINITY));
        r.request_focus();
        let ql = q.to_lowercase();
        let mut items: Vec<(String, String, Option<&str>)> = designcraft_engine::command_specs()
            .iter()
            .filter(|c| !c.menu.is_empty() || c.shortcut.is_some())
            .map(|c| (c.id.to_string(), c.label.to_string(), c.shortcut))
            .chain(UI_COMMANDS.iter().map(|c| (c.0.to_string(), c.1.to_string(), c.2)))
            .filter(|(id, l, _)| ql.is_empty() || l.to_lowercase().contains(&ql) || id.to_lowercase().contains(&ql))
            .collect();
        items.truncate(14);
        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
            for (i, (id, l, sc)) in items.iter().enumerate() {
                let mut b = egui::Button::new(l.as_str()).frame(false);
                if let Some(sc) = sc {
                    b = b.shortcut_text(shortcut_text(sc));
                }
                if ui.add_sized([460.0, 22.0], b).clicked() || (i == 0 && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    run = Some(id.clone());
                }
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
    });
    app.ui.palette = if close || run.is_some() { None } else { Some(q) };
    if let Some(id) = run {
        let _ = app.run(&id, json!({}));
    }
}
