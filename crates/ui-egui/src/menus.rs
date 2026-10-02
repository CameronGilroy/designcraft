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
    ("app.exportIdml", "Export IDML…", None, "{path?} — InDesign Markup (IDML) package"),
    ("app.exportPdf", "Export PDF…", None, "{path?, …file.exportPdf options} — asks for a path when none is given"),
    ("app.palette", "Command Palette…", Some("Cmd+K"), "{}"),
    ("app.findChange", "Find/Change…", Some("Cmd+F"), "{}"),
    ("app.insertTableDialog", "Create Table…", None, "{} — Insert Table dialog (body/header/footer rows, columns)"),
    ("app.footnoteOptionsDialog", "Document Footnote Options…", None, "{} — numbering, formatting and layout of footnotes"),
    ("app.insertXrefDialog", "Insert Cross-Reference…", None, "{} — New Cross-Reference dialog (paragraph or text anchor, format)"),
    ("app.deletePage", "Delete Page", None, "{} — deletes the page in view"),
    ("app.duplicateSpread", "Duplicate Spread", None, "{} — duplicates the spread in view (pages and items)"),
    ("app.tablePanel", "Table Panel", Some("Shift+F9"), "{}"),
    ("app.storyEditor", "Edit in Story Editor", Some("Cmd+Y"), "{story?}"),
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
        "{panel: properties|pages|layers|swatches|paragraphStyles|characterStyles|stroke|character|paragraph|textWrap|links|table}",
    ),
    ("window.controlBar", "Control", Some("Cmd+Alt+6"), "{}"),
    ("window.taskBar", "Contextual Task Bar", None, "{}"),
    ("help.discord", "Join the ArtCraft Discord…", None, "{} — opens https://discord.gg/artcraft in the browser"),
    ("help.appPage", "DesignCraft Website…", None, "{} — opens https://getartcraft.com/apps/designcraft"),
    ("help.github", "DesignCraft on GitHub…", None, "{} — opens https://github.com/storytold/designcraft"),
    ("help.issues", "Report an Issue…", None, "{} — opens the GitHub issue tracker"),
    ("help.website", "ArtCraft Website…", None, "{} — opens https://getartcraft.com"),
    ("help.app", "ArtCraft App Page…", None, "{app} — opens https://getartcraft.com/apps/{app} (e.g. photocraft)"),
    ("help.about", "About DesignCraft", None, "{open?: true} — the About splash with community links (open: false closes it)"),
    ("window.toolsDoubleColumn", "Tools: Double Column", None, "{}"),
    (
        "window.workspace",
        "Workspace",
        None,
        "{name: Essentials|Advanced|Book|Digital Publishing|Interactive for PDF|Printing and Proofing|Typography}",
    ),
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
            "ui:app.exportPdf",
            "ui:app.exportPng",
            "ui:app.exportIdml",
            "cmd:file.exportEpub",
            ">Export",
            "cmd:snippet.export",
            "<",
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
            "cmd:edit.stepAndRepeat",
            "-",
            "cmd:edit.selectAll",
            "cmd:edit.deselectAll",
            "-",
            "ui:app.findChange",
            "cmd:find.next",
            "ui:app.storyEditor",
            "-",
            ">Spelling",
            "cmd:spelling.check",
            "cmd:spelling.addWord",
            "<",
            "-",
            "ui:app.palette",
        ],
    ),
    (
        "Layout",
        &[
            ">Pages",
            "cmd:layout.pages.insert",
            "cmd:layout.pages.move",
            "ui:app.duplicateSpread",
            "ui:app.deletePage",
            "-",
            "cmd:layout.pages.applyParent",
            "cmd:layout.parents.new",
            "<",
            "cmd:layout.marginsAndColumns",
            "-",
            "ui:view.goToPage",
            "-",
            "cmd:layout.section",
            "cmd:toc.generate",
            "cmd:toc.update",
        ],
    ),
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
            "-",
            ">Hyperlinks & Cross-References",
            "cmd:hyperlink.create",
            "cmd:anchor.create",
            "-",
            "ui:app.insertXrefDialog",
            "cmd:xref.defineFormat",
            "<",
            ">Text Variables",
            "cmd:variables.define",
            "cmd:variables.insert",
            "<",
            "-",
            "cmd:footnote.insert",
            "ui:app.footnoteOptionsDialog",
            "cmd:footnote.goToReference",
        ],
    ),
    (
        "Object",
        &[
            ">Transform",
            "cmd:transform.move",
            "cmd:transform.scale",
            "cmd:transform.rotate",
            "cmd:transform.shear",
            "-",
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
            "cmd:object.innerShadow",
            "cmd:object.outerGlow",
            "cmd:object.feather",
            "<",
            ">Fitting",
            "cmd:object.fit|Fill Frame Proportionally|{\"mode\":\"fillProportionally\"}",
            "cmd:object.fit|Fit Content Proportionally|{\"mode\":\"fitProportionally\"}",
            "cmd:object.fit|Fit Frame to Content|{\"mode\":\"fitFrameToContent\"}",
            "cmd:object.fit|Fit Content to Frame|{\"mode\":\"fitContentToFrame\"}",
            "cmd:object.fit|Center Content|{\"mode\":\"centerContent\"}",
            "<",
            ">Content",
            "cmd:object.content|Graphic|{\"type\":\"graphic\"}",
            "cmd:object.content|Text|{\"type\":\"text\"}",
            "cmd:object.content|Unassigned|{\"type\":\"unassigned\"}",
            "<",
        ],
    ),
    (
        "Table",
        &[
            "ui:app.insertTableDialog",
            "cmd:table.convertFromText",
            "cmd:table.convertToText",
            "-",
            ">Insert",
            "cmd:table.insertRowAbove",
            "cmd:table.insertRowBelow",
            "cmd:table.insertColumnLeft",
            "cmd:table.insertColumnRight",
            "<",
            ">Delete",
            "cmd:table.deleteRow",
            "cmd:table.deleteColumn",
            "cmd:table.delete",
            "<",
            ">Select",
            "cmd:table.selectRow",
            "cmd:table.selectColumn",
            "cmd:table.selectTable",
            "<",
            "-",
            "cmd:table.merge",
            "cmd:table.unmerge",
            "cmd:table.distributeColumns",
            "-",
            "ui:app.tablePanel",
        ],
    ),
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
    (
        "Window",
        &[
            "ui:window.controlBar",
            "ui:window.taskBar",
            "ui:window.toolsDoubleColumn",
            "-",
            "ui:window.panel",
            "-",
            ">Utilities",
            "cmd:data.merge",
            "<",
            ">Output",
            "cmd:preflight.run",
            "<",
            "-",
            "ui:window.brightness",
        ],
    ),
    (
        "Help",
        &[
            "ui:help.discord",
            "-",
            "ui:help.appPage",
            "ui:help.github",
            "ui:help.issues",
            "ui:help.website",
            "-",
            "cmd:file.newSample",
            "-",
            "ui:help.about",
        ],
    ),
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
        "app.openDialog" => app.pick_and_open("open"),
        "app.placeDialog" => app.pick_and_open("place"),
        "app.save" | "app.saveDialog" if app.services.download.is_some() => download_document(app),
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
        "app.exportIdml" => export_idml(app, p),
        "app.storyEditor" => {
            let sid = p.get("story").and_then(Value::as_u64).map(designcraft_doc::StoryId).or_else(|| {
                let st = app.session.active()?;
                st.selection.text.map(|t| t.story).or_else(|| st.selection.items.iter().find_map(|i| st.doc.item(*i)?.text_frame().map(|t| t.story)))
            });
            match sid {
                Some(s) => {
                    app.story_editor = if app.story_editor == Some(s) { None } else { Some(s) };
                    Ok(Value::Null)
                }
                None => Err("select a text frame or place the cursor in text".into()),
            }
        }
        "app.findChange" => {
            app.ui.dialog = Some(crate::dialogs::Dialog::new("findChange", json!({})));
            Ok(Value::Null)
        }
        "app.insertTableDialog" => {
            if app.session.active().is_none_or(|d| d.selection.text.is_none()) {
                return Some(Err("place the insertion point in a text frame to create a table".into()));
            }
            app.ui.dialog = Some(crate::dialogs::Dialog::new("insertTable", p.clone()));
            Ok(Value::Null)
        }
        "app.footnoteOptionsDialog" => {
            let Some(st) = app.session.active() else { return Some(Err("no document open".into())) };
            let o = serde_json::to_value(&st.doc.footnote_options).unwrap_or_default();
            app.ui.dialog = Some(crate::dialogs::Dialog::new("footnoteOptions", o));
            Ok(Value::Null)
        }
        "app.insertXrefDialog" => {
            if app.session.active().is_none_or(|d| d.selection.text.is_none()) {
                return Some(Err("place the insertion point in text to insert a cross-reference".into()));
            }
            app.ui.dialog = Some(crate::dialogs::Dialog::new("insertXref", json!({})));
            Ok(Value::Null)
        }
        "app.deletePage" | "app.duplicateSpread" => {
            let Some(st) = app.session.active() else { return Some(Err("no document open".into())) };
            let page = crate::canvas::current_page(app).unwrap_or(0);
            let r = if id == "app.deletePage" {
                app.run("layout.pages.delete", json!({"pages": [page]}))
            } else {
                let spread = st.doc.page_loc(page).map_or(0, |l| l.0);
                app.run("layout.pages.duplicateSpread", json!({"spread": spread}))
            };
            return Some(r);
        }
        "app.tablePanel" => {
            app.ui.open_panel = if app.ui.open_panel.as_deref() == Some("table") { None } else { Some("table".into()) };
            Ok(Value::Null)
        }
        "app.exportPdf" => export_pdf(app, p),
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
        "window.taskBar" => flag(&mut app.ui.task_bar),
        "help.about" => {
            app.ui.about = p.get("open").and_then(Value::as_bool).unwrap_or(true);
            Ok(json!(app.ui.about))
        }
        "help.app" => match p.get("app").and_then(Value::as_str) {
            Some(slug) if !slug.is_empty() && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => {
                let url = format!("{}/apps/{slug}", designcraft_engine::links::WEBSITE);
                app.ui.pending_urls.push(url.clone());
                Ok(json!({"url": url}))
            }
            _ => Err("help.app: give `app` (e.g. \"photocraft\")".into()),
        },
        h if h.starts_with("help.") && designcraft_engine::links::get(&h[5..]).is_some() => {
            let url = designcraft_engine::links::get(&h[5..]).unwrap_or_default().to_string();
            app.ui.pending_urls.push(url.clone());
            Ok(json!({"url": url}))
        }
        "window.toolsDoubleColumn" => flag(&mut app.ui.tools_double_column),
        "window.workspace" => {
            let name = p.get("name").and_then(Value::as_str).unwrap_or("Essentials");
            // Workspaces choose which bars and panels are visible.
            app.ui.control_bar = matches!(name, "Advanced" | "Typography" | "Printing and Proofing" | "Book");
            app.ui.dock_tab = if name == "Typography" { "properties".into() } else { app.ui.dock_tab.clone() };
            app.ui.open_panel = match name {
                "Typography" => Some("paragraphStyles".into()),
                "Printing and Proofing" => Some("swatches".into()),
                _ => None,
            };
            app.ui.workspace = name.to_string();
            Ok(Value::Null)
        }
        "window.brightness" => {
            let b = p.get("brightness").and_then(Value::as_str).and_then(crate::theme::Brightness::parse).unwrap_or(crate::theme::Brightness::Dark);
            app.ui.brightness = b;
            app.restyle = true;
            Ok(Value::Null)
        }
        _ => return None,
    })
}

/// Save by handing the serialized document to the host as a download (web).
fn download_document(app: &mut DesignApp) -> Result<Value, String> {
    let name = app.session.active().map(|d| format!("{}.designcraft", d.doc.title)).ok_or("no document")?;
    let ser = app.run("file.serialize", json!({}))?;
    let bytes = ser.get("json").and_then(Value::as_str).unwrap_or_default().as_bytes().to_vec();
    if let Some(download) = app.services.download.as_mut() {
        download(&name, &bytes);
    }
    // Marks the document saved; on the web the engine doesn't touch a file system.
    app.run("file.saveAs", json!({"path": name}))
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

/// File › Export IDML…: ask for a path, export through `file.exportIdml` and write the bytes with
/// the platform writer (a download on the web).
fn export_idml(app: &mut DesignApp, p: &Value) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document")?;
    let path = match p.get("path").and_then(Value::as_str) {
        Some(s) => Some(s.to_string()),
        None => {
            let name = format!("{}.idml", st.doc.title);
            app.services.pick_save.as_mut().and_then(|f| f(&name))
        }
    };
    let Some(path) = path else { return Ok(Value::Null) };
    let r = app.run("file.exportIdml", json!({}))?;
    let bytes = designcraft_engine::cmd::base64_decode(r["base64"].as_str().unwrap_or_default());
    match app.services.write.as_mut() {
        Some(w) => w(&path, &bytes).map(|_| json!({"path": path, "bytes": bytes.len()})),
        None => Err("no writer".into()),
    }
}

/// File › Export PDF…: ask for a path, export through `file.exportPdf` and write the bytes with the
/// platform writer (a download on the web).
fn export_pdf(app: &mut DesignApp, p: &Value) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document")?;
    let path = match p.get("path").and_then(Value::as_str) {
        Some(s) => Some(s.to_string()),
        None => {
            let name = format!("{}.pdf", st.doc.title);
            app.services.pick_save.as_mut().and_then(|f| f(&name))
        }
    };
    let Some(path) = path else { return Ok(Value::Null) };
    let mut params = if p.is_object() { p.clone() } else { json!({}) };
    if let Some(o) = params.as_object_mut() {
        o.remove("path");
        o.entry("bleed").or_insert(json!(true));
    }
    let r = app.run("file.exportPdf", params)?;
    let bytes = designcraft_engine::cmd::base64_decode(r["base64"].as_str().unwrap_or_default());
    match app.services.write.as_mut() {
        Some(w) => w(&path, &bytes).map(|_| json!({"path": path, "bytes": bytes.len(), "pages": r["pages"], "warnings": r["warnings"]})),
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

/// A menu entry, shared by the in-window menu bar and the native macOS menu.
#[derive(Clone, Debug)]
pub enum Item {
    Sep,
    Sub(String, Vec<Item>),
    Cmd { label: String, id: String, params: Value, shortcut: Option<&'static str> },
}

fn cmd_item(id: &str, params: Value) -> Item {
    let (label, shortcut) = match ui_label(id) {
        Some((l, sc)) => (l.to_string(), sc),
        None => match designcraft_engine::find_command(id) {
            Some(c) => (c.label.to_string(), c.shortcut),
            None => (id.to_string(), None),
        },
    };
    Item::Cmd { label, id: id.to_string(), params, shortcut }
}

fn parse_entries(entries: &[&str]) -> Vec<Item> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let e = entries[i];
        i += 1;
        if e == "-" {
            out.push(Item::Sep);
        } else if let Some(name) = e.strip_prefix('>') {
            let start = i;
            while i < entries.len() && entries[i] != "<" {
                i += 1;
            }
            let sub = parse_entries(&entries[start..i]);
            i += 1;
            out.push(Item::Sub(name.to_string(), sub));
        } else if e == "ui:window.panel" {
            let items = crate::dock::DOCK_TABS
                .iter()
                .chain(crate::dock::ICON_PANELS)
                .map(|(id, label, _)| Item::Cmd { label: label.to_string(), id: "window.panel".into(), params: json!({"panel": id}), shortcut: None })
                .collect();
            out.push(Item::Sub("Panels".into(), items));
        } else if e == "ui:window.brightness" {
            let items = crate::theme::Brightness::ALL
                .iter()
                .map(|b| Item::Cmd {
                    label: b.label().to_string(),
                    id: "window.brightness".into(),
                    params: json!({"brightness": b.id()}),
                    shortcut: None,
                })
                .collect();
            out.push(Item::Sub("Interface Color Theme".into(), items));
        } else {
            let (_, id) = e.split_once(':').unwrap_or(("cmd", e));
            // `id|Label|{params}`: a fixed-parameter variant of a command.
            let mut parts = id.splitn(3, '|');
            let id = parts.next().unwrap_or(id);
            match (parts.next(), parts.next().and_then(|j| serde_json::from_str::<Value>(j).ok())) {
                (Some(label), Some(params)) => {
                    let mut it = cmd_item(id, params);
                    if let Item::Cmd { label: l, .. } = &mut it {
                        *l = label.to_string();
                    }
                    out.push(it);
                }
                _ => out.push(cmd_item(id, Value::Null)),
            }
        }
    }
    out
}

/// The whole menu tree (InDesign order).
pub fn menu_tree() -> Vec<(&'static str, Vec<Item>)> {
    MENUS.iter().map(|(m, e)| (*m, parse_entries(e))).collect()
}

/// Check state of a toggle command (None = not a toggle).
pub fn checked(app: &DesignApp, id: &str, params: &Value) -> Option<bool> {
    Some(match id {
        "view.rulers" => app.ui.rulers,
        "view.frameEdges" => app.ui.frame_edges,
        "view.guides" => app.ui.guides,
        "view.baselineGrid" => app.ui.baseline_grid,
        "view.textThreads" => app.ui.text_threads,
        "view.hiddenCharacters" => app.ui.hidden_characters,
        "window.controlBar" => app.ui.control_bar,
        "window.taskBar" => app.ui.task_bar,
        "window.toolsDoubleColumn" => app.ui.tools_double_column,
        "view.togglePreview" => app.ui.screen_mode == crate::ScreenMode::Preview,
        "window.brightness" => params.get("brightness").and_then(Value::as_str) == Some(app.ui.brightness.id()),
        _ => return None,
    })
}

/// Enablement for menu display (UI commands need a document unless they're app/window-level).
pub fn menu_enabled(app: &DesignApp, id: &str) -> bool {
    if ui_label(id).is_some() {
        return app.session.active().is_some() || id.starts_with("app.") || id.starts_with("window.");
    }
    enabled(app, id)
}

/// A menu item was chosen. An engine command whose label ends in "…" and that takes parameters
/// opens a dialog built from its parameter documentation (see [`crate::dialogs::command_fields`]).
pub fn activate(app: &mut DesignApp, id: &str, params: &Value) {
    if params.is_null()
        && ui_label(id).is_none()
        && let Some(c) = designcraft_engine::find_command(id)
        && !crate::dialogs::command_fields(c.params).is_empty()
        && (c.label.ends_with('…') || crate::dialogs::command_fields(c.params).iter().any(|f| !f.optional))
    {
        app.ui.dialog = Some(crate::dialogs::Dialog::new(&format!("cmd:{id}"), json!({})));
        return;
    }
    let p = if params.is_null() { json!({}) } else { params.clone() };
    let _ = app.run(id, p);
}

/// The menu bar contents (inside the app bar; macOS uses the native menu instead).
pub fn menu_bar(app: &mut DesignApp, ui: &mut egui::Ui) {
    for (menu, entries) in menu_tree() {
        ui.menu_button(menu, |ui| {
            ui.set_min_width(240.0);
            menu_items(app, ui, &entries);
        });
    }
}

fn menu_items(app: &mut DesignApp, ui: &mut egui::Ui, items: &[Item]) {
    for it in items {
        match it {
            Item::Sep => {
                ui.separator();
            }
            Item::Sub(name, children) => {
                ui.menu_button(name, |ui| menu_items(app, ui, children));
            }
            Item::Cmd { label, id, params, shortcut } => {
                let text = match checked(app, id, params) {
                    Some(true) => format!("✓ {label}"),
                    Some(false) => format!("   {label}"),
                    None => label.clone(),
                };
                let mut b = egui::Button::new(text);
                if let Some(sc) = shortcut {
                    b = b.shortcut_text(shortcut_text(sc));
                }
                if ui.add_enabled(menu_enabled(app, id), b).clicked() {
                    activate(app, id, params);
                    ui.close();
                }
            }
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
                if id == "edit.clear" || (typing && matches!(id, "edit.copy" | "edit.cut" | "edit.paste" | "edit.pasteInPlace")) {
                    continue; // Handled by the active tool / text clipboard events.
                }
                fired = Some(id);
                break;
            }
        }
        if let Some(id) = fired {
            if app.native_shortcuts.contains(id) {
                continue; // The native menu handles it.
            }
            let _ = app.run(id, json!({}));
            continue;
        }
        // Tool shortcuts (single keys, Shift+key).
        if !typing && !modifiers.command && !modifiers.alt {
            let name = key.name();
            let sc = if modifiers.shift { format!("Shift+{name}") } else { name.to_string() };
            if let Some(tool) = designcraft_tools::tool_for_shortcut(&sc) {
                log::debug!("tool shortcut {sc} -> {tool}");
                if std::env::var_os("DESIGNCRAFT_DEBUG_KEYS").is_some() {
                    eprintln!("tool shortcut {sc} ({key:?}, {modifiers:?}) -> {tool}");
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(items: &[Item], out: &mut Vec<(String, String, Value)>) {
        for it in items {
            match it {
                Item::Sub(_, c) => walk(c, out),
                Item::Cmd { label, id, params, .. } => out.push((label.clone(), id.clone(), params.clone())),
                Item::Sep => {}
            }
        }
    }

    #[test]
    fn every_menu_entry_is_a_command() {
        let mut all = Vec::new();
        for (_, items) in menu_tree() {
            walk(&items, &mut all);
        }
        assert!(all.len() > 80, "{}", all.len());
        for (label, id, params) in &all {
            assert!(ui_label(id).is_some() || designcraft_engine::find_command(id).is_some(), "menu entry {label}: unknown command {id}");
            assert!(!label.is_empty() && label != id, "menu entry {id} has no label");
            if let Some(o) = params.as_object() {
                assert!(!o.is_empty(), "{id}: empty fixed params");
            }
        }
        // Fixed-parameter variants keep their own labels.
        assert!(all.iter().any(|(l, id, p)| l == "Fill Frame Proportionally" && id == "object.fit" && p["mode"] == "fillProportionally"));
    }
}
