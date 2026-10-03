//! The menu bar tree (InDesign order), UI-only commands, keyboard shortcuts and Quick Apply.

use serde_json::{Value, json};

use crate::{DesignApp, ScreenMode};

/// UI commands: (id, label, shortcut, params).
pub const UI_COMMANDS: &[(&str, &str, Option<&str>, &str)] = &[
    ("app.newDocumentDialog", "New Document…", Some("Cmd+N"), "{}"),
    ("app.openDialog", "Open…", Some("Cmd+O"), "{}"),
    ("app.placeDialog", "Place…", Some("Cmd+D"), "{}"),
    ("app.saveDialog", "Save As…", Some("Cmd+Shift+S"), "{}"),
    ("app.save", "Save", Some("Cmd+S"), "{}"),
    ("app.saveCopyDialog", "Save a Copy…", Some("Cmd+Alt+S"), "{} — choose where to write a copy (the document stays as it is)"),
    ("app.loadSwatches", "Load Swatches…", None, "{} — pick a swatch exchange (.ase) file"),
    ("app.packageDialog", "Package…", Some("Cmd+Alt+Shift+P"), "{} — choose the package folder to create (file.package)"),
    ("app.saveSwatches", "Save Swatches for Exchange…", None, "{path?} — write the colour swatches as .ase"),
    ("app.exportPng", "Export Page as PNG…", Some("Cmd+E"), "{}"),
    ("app.exportIdml", "Export IDML…", None, "{path?} — InDesign Markup (IDML) package"),
    ("app.exportEpub", "Export EPUB…", None, "{path?} — reflowable EPUB 3"),
    ("app.exportHtml", "Export HTML…", None, "{path?} — one self-contained web page"),
    ("app.qrCode", "Generate QR Code…", None, "{} — the QR Code dialog (object.qrCode does the work)"),
    ("app.exportText", "Export Text…", None, "{path?} — the story being edited, as Text Only (.txt) or Rich Text Format (.rtf)"),
    ("app.exportPdf", "Export PDF…", None, "{path?, …file.exportPdf options} — asks for a path when none is given"),
    ("app.palette", "Quick Apply…", Some("Cmd+Return"), "{} — search styles and commands"),
    ("app.preferences", "Preferences…", Some("Cmd+K"), "{}"),
    ("window.uiScale", "UI Scaling", None, "{scale: 0.5–3 (1 = 100%)} — the size of the whole interface"),
    ("app.keyboardShortcuts", "Keyboard Shortcuts…", None, "{} — view and change the shortcut of any command"),
    ("window.richBlack", "Appearance of Black", None, "{on: bool} — show 100% black as rich black on screen (off: accurately)"),
    ("window.setShortcut", "Set Shortcut", None, "{id, shortcut: \"Cmd+Alt+J\" | \"\" (none) | null (default)} → {shortcut, conflicts: [ids]}"),
    ("window.resetShortcuts", "Reset Shortcuts", None, "{} — every command back to its default shortcut"),
    ("app.findChange", "Find/Change…", Some("Cmd+F"), "{}"),
    ("app.insertTableDialog", "Create Table…", None, "{} — Insert Table dialog (body/header/footer rows, columns)"),
    ("app.footnoteOptionsDialog", "Document Footnote Options…", None, "{} — numbering, formatting and layout of footnotes"),
    ("app.findFontDialog", "Find/Replace Font…", None, "{} — fonts used (missing ones flagged) and replacing them"),
    ("app.insertXrefDialog", "Insert Cross-Reference…", None, "{} — New Cross-Reference dialog (paragraph or text anchor, format)"),
    ("app.deleteAllGuides", "Delete All Guides on Spread", None, "{} — the spread in view"),
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
    ("view.overprintPreview", "Overprint Preview", Some("Cmd+Alt+Shift+Y"), "{} — show how overprinting inks mix"),
    ("view.fastDisplay", "Fast Display", Some("Cmd+Alt+Shift+Z"), "{} — placed graphics as grey boxes, no effects"),
    ("view.typicalDisplay", "Typical Display", Some("Cmd+Alt+Z"), "{} — low-resolution image proxies"),
    ("view.highQualityDisplay", "High Quality Display", Some("Cmd+Alt+H"), "{} — full-resolution images"),
    ("view.goToPage", "Go to Page…", Some("Cmd+J"), "{page: number (position) | \"name\" (section page name, or \"+n\" for a position)}"),
    (
        "window.panel",
        "Show Panel",
        None,
        "{panel: properties|pages|layers|swatches|paragraphStyles|characterStyles|stroke|character|paragraph|textWrap|links|table}",
    ),
    ("window.floatPanel", "Float Panel", None, "{panel, x?, y?} — the panel in its own movable window"),
    ("window.dockPanel", "Dock Panel", None, "{panel} — back into the dock's icon column"),
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
            "ui:app.saveCopyDialog",
            "cmd:file.revert",
            "-",
            "ui:app.placeDialog",
            "-",
            "ui:app.exportPdf",
            "ui:app.packageDialog",
            "ui:app.exportPng",
            "ui:app.exportIdml",
            "ui:app.exportEpub",
            "ui:app.exportHtml",
            "ui:app.exportText",
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
            "cmd:edit.pasteWithoutFormatting",
            "cmd:edit.pasteInto",
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
            "ui:app.keyboardShortcuts",
            "ui:app.preferences",
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
            "cmd:layout.createGuides",
            "-",
            "ui:view.goToPage",
            "-",
            "cmd:layout.section",
            "cmd:toc.generate",
            "cmd:toc.update",
            ">Index",
            "cmd:index.addReference",
            "cmd:index.generate",
            "cmd:index.update",
            "<",
        ],
    ),
    (
        "Type",
        &[
            "ui:app.findFontDialog",
            "-",
            "cmd:type.createOutlines",
            ">Bulleted and Numbered Lists",
            "cmd:list.define",
            "<",
            ">Type on a Path",
            "cmd:type.pathOptions",
            "cmd:type.pathOptions|Delete Type from Path|{\"delete\": true}",
            "<",
            "ui:window.panel|Glyphs|{\"panel\": \"glyphs\"}",
            "cmd:type.fillWithPlaceholder",
            ">Insert Special Character",
            ">Symbols",
            "cmd:text.insert|Bullet Character|{\"text\": \"\u{2022}\", \"raw\": true}",
            "cmd:text.insert|Copyright Symbol|{\"text\": \"\u{a9}\", \"raw\": true}",
            "cmd:text.insert|Ellipsis|{\"text\": \"\u{2026}\", \"raw\": true}",
            "cmd:text.insert|Paragraph Symbol|{\"text\": \"\u{b6}\", \"raw\": true}",
            "cmd:text.insert|Registered Trademark Symbol|{\"text\": \"\u{ae}\", \"raw\": true}",
            "cmd:text.insert|Section Symbol|{\"text\": \"\u{a7}\", \"raw\": true}",
            "cmd:text.insert|Trademark Symbol|{\"text\": \"\u{2122}\", \"raw\": true}",
            "<",
            ">Markers",
            "cmd:text.insert|Current Page Number|{\"text\": \"\u{e000}\", \"raw\": true}",
            "cmd:text.insert|Next Page Number|{\"text\": \"\u{e007}\", \"raw\": true}",
            "cmd:text.insert|Previous Page Number|{\"text\": \"\u{e008}\", \"raw\": true}",
            "cmd:text.insert|Section Marker|{\"text\": \"\u{e001}\", \"raw\": true}",
            "<",
            ">Hyphens and Dashes",
            "cmd:text.insert|Em Dash|{\"text\": \"\u{2014}\", \"raw\": true}",
            "cmd:text.insert|En Dash|{\"text\": \"\u{2013}\", \"raw\": true}",
            "cmd:text.insert|Discretionary Hyphen|{\"text\": \"\u{ad}\", \"raw\": true}",
            "cmd:text.insert|Nonbreaking Hyphen|{\"text\": \"\u{2011}\", \"raw\": true}",
            "<",
            ">Quotation Marks",
            "cmd:text.insert|Double Left Quotation Marks|{\"text\": \"\u{201c}\", \"raw\": true}",
            "cmd:text.insert|Double Right Quotation Marks|{\"text\": \"\u{201d}\", \"raw\": true}",
            "cmd:text.insert|Single Left Quotation Mark|{\"text\": \"\u{2018}\", \"raw\": true}",
            "cmd:text.insert|Single Right Quotation Mark|{\"text\": \"\u{2019}\", \"raw\": true}",
            "cmd:text.insert|Straight Double Quotation Marks|{\"text\": \"\\\"\", \"raw\": true}",
            "cmd:text.insert|Straight Single Quotation Mark (Apostrophe)|{\"text\": \"'\", \"raw\": true}",
            "<",
            ">Other",
            "cmd:text.insert|Tab|{\"text\": \"\\t\", \"raw\": true}",
            "cmd:text.insert|Right Indent Tab|{\"text\": \"\u{e006}\", \"raw\": true}",
            "cmd:text.insert|Indent to Here|{\"text\": \"\u{e005}\", \"raw\": true}",
            "<",
            "<",
            ">Insert White Space",
            "cmd:text.insert|Em Space|{\"text\": \"\u{2003}\", \"raw\": true}",
            "cmd:text.insert|En Space|{\"text\": \"\u{2002}\", \"raw\": true}",
            "cmd:text.insert|Nonbreaking Space|{\"text\": \"\u{a0}\", \"raw\": true}",
            "cmd:text.insert|Hair Space|{\"text\": \"\u{200a}\", \"raw\": true}",
            "cmd:text.insert|Sixth Space|{\"text\": \"\u{2006}\", \"raw\": true}",
            "cmd:text.insert|Thin Space|{\"text\": \"\u{2009}\", \"raw\": true}",
            "cmd:text.insert|Quarter Space|{\"text\": \"\u{2005}\", \"raw\": true}",
            "cmd:text.insert|Third Space|{\"text\": \"\u{2004}\", \"raw\": true}",
            "cmd:text.insert|Punctuation Space|{\"text\": \"\u{2008}\", \"raw\": true}",
            "cmd:text.insert|Figure Space|{\"text\": \"\u{2007}\", \"raw\": true}",
            "cmd:text.insert|Flush Space|{\"text\": \"\u{2001}\", \"raw\": true}",
            "<",
            ">Insert Break Character",
            "cmd:text.insert|Column Break|{\"text\": \"\u{e002}\", \"raw\": true}",
            "cmd:text.insert|Frame Break|{\"text\": \"\u{e003}\", \"raw\": true}",
            "cmd:text.insert|Page Break|{\"text\": \"\u{e004}\", \"raw\": true}",
            "cmd:text.insert|Paragraph Return|{\"text\": \"\\n\", \"raw\": true}",
            "cmd:text.insert|Forced Line Break|{\"text\": \"\u{2028}\", \"raw\": true}",
            "<",
            "-",
            ">Change Case",
            "cmd:type.changeCase|UPPERCASE|{\"case\": \"upper\"}",
            "cmd:type.changeCase|lowercase|{\"case\": \"lower\"}",
            "cmd:type.changeCase|Title Case|{\"case\": \"title\"}",
            "cmd:type.changeCase|Sentence case|{\"case\": \"sentence\"}",
            "<",
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
            "-",
            "cmd:transform.clear",
            "<",
            ">Transform Again",
            "cmd:transform.again",
            "cmd:transform.again|Transform Again Individually|{\"individually\": true}",
            "cmd:transform.again|Transform Sequence Again|{\"sequence\": true}",
            "cmd:transform.again|Transform Sequence Again Individually|{\"sequence\": true, \"individually\": true}",
            "<",
            ">Paths",
            "cmd:object.makeCompoundPath",
            "cmd:object.releaseCompoundPath",
            "<",
            ">Pathfinder",
            "cmd:object.pathfinder|Add|{\"op\": \"add\"}",
            "cmd:object.pathfinder|Subtract|{\"op\": \"subtract\"}",
            "cmd:object.pathfinder|Intersect|{\"op\": \"intersect\"}",
            "cmd:object.pathfinder|Exclude Overlap|{\"op\": \"exclude\"}",
            "cmd:object.pathfinder|Minus Back|{\"op\": \"minusBack\"}",
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
            ">Select",
            "cmd:select.firstAbove",
            "cmd:select.nextAbove",
            "cmd:select.nextBelow",
            "cmd:select.lastBelow",
            "-",
            "cmd:select.container",
            "cmd:select.content",
            "-",
            "cmd:select.previousInGroup",
            "cmd:select.nextInGroup",
            "<",
            ">Convert Shape",
            "cmd:object.convertShape|Rectangle|{\"to\": \"rectangle\"}",
            "cmd:object.convertShape|Rounded Rectangle|{\"to\": \"roundedRectangle\"}",
            "cmd:object.convertShape|Beveled Rectangle|{\"to\": \"beveledRectangle\"}",
            "cmd:object.convertShape|Inverse Rounded Rectangle|{\"to\": \"inverseRoundedRectangle\"}",
            "cmd:object.convertShape|Ellipse|{\"to\": \"ellipse\"}",
            "cmd:object.convertShape|Triangle|{\"to\": \"triangle\"}",
            "cmd:object.convertShape|Polygon|{\"to\": \"polygon\"}",
            "cmd:object.convertShape|Line|{\"to\": \"line\"}",
            "cmd:object.convertShape|Orthogonal Line|{\"to\": \"orthogonalLine\"}",
            "-",
            "cmd:object.convertShape|Open Path|{\"to\": \"openPath\"}",
            "cmd:object.convertShape|Closed Path|{\"to\": \"closedPath\"}",
            "<",
            "ui:app.qrCode",
            ">Captions",
            "cmd:object.caption",
            "<",
            ">Anchored Object",
            "cmd:anchored.options",
            "cmd:anchored.release",
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
            "cmd:table.splitHorizontally",
            "cmd:table.splitVertically",
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
            "ui:view.overprintPreview",
            "ui:view.togglePreview",
            ">Display Performance",
            "ui:view.fastDisplay",
            "ui:view.typicalDisplay",
            "ui:view.highQualityDisplay",
            "<",
            "-",
            "ui:view.rulers",
            "ui:view.frameEdges",
            "ui:view.textThreads",
            "ui:view.hiddenCharacters",
            "-",
            ">Grids & Guides",
            "ui:view.guides",
            "ui:view.baselineGrid",
            "-",
            "ui:app.deleteAllGuides",
            "<",
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
            "ui:window.panel|Attributes|{\"panel\": \"attributes\"}",
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
        "app.saveCopyDialog" => {
            let name = app.session.active().map(|d| format!("{} copy.designcraft", d.doc.title)).unwrap_or_default();
            if let Some(path) = app.services.pick_save.as_mut().and_then(|f| f(&name)) {
                return Some(app.run("file.saveACopy", json!({"path": path})));
            }
            Ok(Value::Null)
        }
        "app.exportPng" => export_png(app, p),
        "app.exportIdml" => export_idml(app, p),
        "app.exportEpub" => export_bytes(app, p, "epub", "file.exportEpub"),
        "app.exportHtml" => export_bytes(app, p, "html", "file.exportHtml"),
        "app.qrCode" => {
            app.ui.dialog = Some(crate::dialogs::Dialog::new("qrCode", json!({})));
            Ok(Value::Null)
        }
        "app.exportText" => export_bytes(app, p, "rtf", "file.exportText"),
        "app.packageDialog" => {
            let name = app.session.active().map(|d| format!("{} Folder", d.doc.title)).unwrap_or_default();
            match app.services.pick_save.as_mut().and_then(|f| f(&name)) {
                Some(dir) => {
                    let r = app.run("file.package", json!({"dir": dir}));
                    if let Ok(v) = &r {
                        app.status(format!(
                            "Packaged {} files into {}",
                            v["files"].as_array().map_or(0, |f| f.len()),
                            v["dir"].as_str().unwrap_or("")
                        ));
                    }
                    r
                }
                None => Ok(Value::Null),
            }
        }
        "app.loadSwatches" => {
            if let Some(pick) = app.services.pick_open.as_mut() {
                return Some(match pick("swatches") {
                    Some(path) => app.run("swatch.load", json!({"path": path})),
                    None => Ok(Value::Null),
                });
            }
            if let Some(open) = app.services.open_async.as_mut() {
                open("swatches");
            }
            Ok(Value::Null)
        }
        "app.saveSwatches" => {
            let path = match p.get("path").and_then(Value::as_str) {
                Some(s) => Some(s.to_string()),
                None => app.services.pick_save.as_mut().and_then(|f| f("Swatches.ase")),
            };
            let Some(path) = path else { return Some(Ok(Value::Null)) };
            let r = match app.run("swatch.save", json!({})) {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            let bytes = designcraft_engine::cmd::base64_decode(r["base64"].as_str().unwrap_or_default());
            match app.services.write.as_mut() {
                Some(w) => w(&path, &bytes).map(|_| json!({"path": path, "bytes": bytes.len()})),
                None => Err("no writer".into()),
            }
        }
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
        "app.findFontDialog" => {
            if app.session.active().is_none() {
                return Some(Err("no document open".into()));
            }
            app.ui.dialog = Some(crate::dialogs::Dialog::new("findFont", json!({})));
            Ok(Value::Null)
        }
        "app.insertXrefDialog" => {
            if app.session.active().is_none_or(|d| d.selection.text.is_none()) {
                return Some(Err("place the insertion point in text to insert a cross-reference".into()));
            }
            app.ui.dialog = Some(crate::dialogs::Dialog::new("insertXref", json!({})));
            Ok(Value::Null)
        }
        "app.deleteAllGuides" => {
            let Some(st) = app.session.active() else { return Some(Err("no document open".into())) };
            let page = crate::canvas::current_page(app).unwrap_or(0);
            let spread = if st.editing_parents { json!({"kind": "parent", "index": 0}) } else { json!(st.doc.page_loc(page).map_or(0, |l| l.0)) };
            return Some(app.run("guide.deleteAll", json!({"spread": spread})));
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
        "app.preferences" => {
            let mut f = app.session.execute("prefs.set", &json!({})).unwrap_or_default();
            if app.session.active().is_some() {
                let doc = app.session.execute("document.preferences", &json!({})).unwrap_or_default();
                f["horizontalUnits"] = doc["horizontalUnits"].clone();
                for k in ["superscriptSize", "superscriptPosition", "subscriptSize", "subscriptPosition"] {
                    f[format!("adv.{k}")] = json!(format!("{}", doc["advancedType"][k].as_f64().unwrap_or(0.0)));
                }
                f["verticalUnits"] = doc["verticalUnits"].clone();
                let inc = doc["keyboardIncrement"].as_f64().unwrap_or(1.0);
                f["keyboardIncrement"] = json!(designcraft_geom::format_measure(inc, designcraft_geom::Unit::Points));
                let units = app.session.active().map(|d| d.doc.settings.horizontal_units).unwrap_or_default();
                let m = |v: &Value| json!(designcraft_geom::format_measure(v.as_f64().unwrap_or(0.0), units));
                let (bg, g) = (&doc["baselineGrid"], &doc["grid"]);
                f["bg.start"] = m(&bg["start"]);
                f["bg.increment"] = m(&bg["increment"]);
                f["bg.relativeTo"] = bg["relativeTo"].clone();
                f["bg.viewThreshold"] = json!((bg["viewThreshold"].as_f64().unwrap_or(0.75) * 100.0).round());
                f["bg.color"] = bg["color"].clone();
                f["grid.horizontal"] = m(&g["horizontal"]);
                f["grid.vertical"] = m(&g["vertical"]);
                f["grid.subdivisions"] = g["subdivisions"].clone();
                f["grid.inBack"] = g["inBack"].clone();
                f["grid.color"] = g["color"].clone();
                for k in ["marginColor", "columnColor", "bleedColor", "slugColor"] {
                    f[k] = doc[k].clone();
                }
                f["pasteboard.h"] = m(&doc["pasteboard"][0]);
                f["pasteboard.v"] = m(&doc["pasteboard"][1]);
            }
            f["displayQuality"] = json!(match app.ui.display_quality {
                designcraft_render::DisplayQuality::Fast => "fast",
                designcraft_render::DisplayQuality::Typical => "typical",
                designcraft_render::DisplayQuality::High => "high",
            });
            f["uiScale"] = json!((app.ui.ui_scale * 100.0).round());
            f["richBlack"] = json!(app.ui.rich_black);
            f["section"] = json!("general");
            app.ui.dialog = Some(crate::dialogs::Dialog::new("preferences", f));
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
        "view.overprintPreview" => {
            app.canvas.shown = None;
            flag(&mut app.ui.overprint_preview)
        }
        "view.fastDisplay" | "view.typicalDisplay" | "view.highQualityDisplay" => {
            use designcraft_render::DisplayQuality as Q;
            app.ui.display_quality = match id {
                "view.fastDisplay" => Q::Fast,
                "view.typicalDisplay" => Q::Typical,
                _ => Q::High,
            };
            // Redraw everything at the new quality.
            app.canvas.shown = None;
            Ok(Value::Null)
        }
        "view.rulers" => flag(&mut app.ui.rulers),
        "view.guides" => flag(&mut app.ui.guides),
        "view.baselineGrid" => flag(&mut app.ui.baseline_grid),
        "view.textThreads" => flag(&mut app.ui.text_threads),
        "view.hiddenCharacters" => flag(&mut app.ui.hidden_characters),
        "view.goToPage" => {
            match p.get("page") {
                Some(Value::Number(n)) => crate::canvas::go_to_page(app, (n.as_u64().unwrap_or(1) as usize).saturating_sub(1)),
                Some(Value::String(s)) => match app.session.resolve_page(s) {
                    Some(abs) => crate::canvas::go_to_page(app, abs),
                    None => return Some(Err(format!("no page \"{s}\""))),
                },
                _ => app.ui.dialog = Some(crate::dialogs::Dialog::new("goToPage", json!({}))),
            }
            Ok(Value::Null)
        }
        "window.panel" => {
            let panel = p.get("panel").and_then(Value::as_str).unwrap_or("properties").to_string();
            if crate::dock::DOCK_TABS.iter().any(|(id, _, _)| *id == panel) {
                app.ui.dock_tab = panel;
                app.ui.dock_expanded = true;
                app.ui.open_panel = None;
            } else if !app.ui.floating.iter().any(|(p, _)| *p == panel) {
                app.ui.open_panel = if app.ui.open_panel.as_deref() == Some(panel.as_str()) { None } else { Some(panel) };
            }
            Ok(Value::Null)
        }
        "app.keyboardShortcuts" => {
            app.ui.dialog = Some(crate::dialogs::Dialog::new("keyboardShortcuts", json!({"query": "", "recording": ""})));
            Ok(Value::Null)
        }
        "window.setShortcut" => {
            let Some(cmd) = p.get("id").and_then(Value::as_str) else { return Some(Err("missing id".into())) };
            if designcraft_engine::find_command(cmd).is_none() && ui_label(cmd).is_none() {
                return Some(Err(format!("unknown command `{cmd}`")));
            }
            match p.get("shortcut") {
                None | Some(Value::Null) => {
                    app.ui.shortcuts.remove(cmd);
                }
                Some(Value::String(sc)) if sc.is_empty() => {
                    app.ui.shortcuts.insert(cmd.to_string(), String::new());
                }
                Some(Value::String(sc)) => {
                    if parse_shortcut(sc).is_none() {
                        return Some(Err(format!("can't read shortcut `{sc}`")));
                    }
                    app.ui.shortcuts.insert(cmd.to_string(), sc.clone());
                }
                Some(_) => return Some(Err("shortcut: a string or null".into())),
            }
            let sc = shortcut_of(app, cmd);
            let conflicts: Vec<String> =
                effective_shortcuts(app).into_iter().filter(|(i, s)| i != cmd && Some(s) == sc.as_ref()).map(|(i, _)| i).collect();
            Ok(json!({"shortcut": sc, "conflicts": conflicts}))
        }
        "window.resetShortcuts" => {
            app.ui.shortcuts.clear();
            Ok(Value::Null)
        }
        "window.richBlack" => {
            app.ui.rich_black = p.get("on").and_then(Value::as_bool).unwrap_or(!app.ui.rich_black);
            app.canvas.shown = None;
            Ok(json!({"on": app.ui.rich_black}))
        }
        "window.uiScale" => {
            let v = p.get("scale").and_then(Value::as_f64).unwrap_or(1.0) as f32;
            app.ui.ui_scale = v.clamp(0.5, 3.0);
            Ok(json!({"scale": app.ui.ui_scale}))
        }
        "window.floatPanel" | "window.dockPanel" => {
            let panel = p.get("panel").and_then(Value::as_str).unwrap_or("");
            if !crate::dock::ICON_PANELS.iter().any(|(id, _, _)| *id == panel) {
                return Some(Err(format!("{id}: unknown panel `{panel}`")));
            }
            if id == "window.floatPanel" {
                let at = egui::pos2(
                    p.get("x").and_then(Value::as_f64).unwrap_or(400.0) as f32,
                    p.get("y").and_then(Value::as_f64).unwrap_or(160.0) as f32,
                );
                crate::dock::float_panel(app, panel, at);
            } else {
                crate::dock::dock_panel(app, panel);
            }
            Ok(json!({"floating": app.ui.floating.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>()}))
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

/// Export EPUB / Text: ask for a path (default extension `ext`), run `cmd` without one and write
/// what it returns (`base64` or `text`) with the platform writer.
fn export_bytes(app: &mut DesignApp, p: &Value, ext: &str, cmd: &str) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document")?;
    let path = match p.get("path").and_then(Value::as_str) {
        Some(s) => Some(s.to_string()),
        None => {
            let name = format!("{}.{ext}", st.doc.title);
            app.services.pick_save.as_mut().and_then(|f| f(&name))
        }
    };
    let Some(path) = path else { return Ok(Value::Null) };
    let mut params = json!({});
    if cmd == "file.exportText" {
        let txt = path.to_ascii_lowercase().ends_with(".txt");
        params["format"] = json!(if txt { "txt" } else { "rtf" });
    }
    let r = app.run(cmd, params)?;
    let bytes = match r["text"].as_str() {
        Some(t) => t.as_bytes().to_vec(),
        None => designcraft_engine::cmd::base64_decode(r["base64"].as_str().unwrap_or_default()),
    };
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
        o.entry("tagged").or_insert(json!(true));
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

/// The default shortcut of a command.
pub fn default_shortcut(id: &str) -> Option<&'static str> {
    designcraft_engine::find_command(id).and_then(|c| c.shortcut).or_else(|| ui_label(id).and_then(|(_, s)| s))
}

/// The shortcut in effect for a command (the user's, else the default).
pub fn shortcut_of(app: &DesignApp, id: &str) -> Option<String> {
    match app.ui.shortcuts.get(id) {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s.clone()),
        None => default_shortcut(id).map(str::to_string),
    }
}

/// Every command with a shortcut in effect: (id, shortcut). UI commands come first: where one
/// shares keys with an engine command (⌘N, ⌘O…) it's the one with the dialog.
pub fn effective_shortcuts(app: &DesignApp) -> Vec<(String, String)> {
    UI_COMMANDS
        .iter()
        .map(|c| c.0)
        .chain(designcraft_engine::command_specs().iter().map(|c| c.id))
        .filter_map(|id| shortcut_of(app, id).map(|s| (id.to_string(), s)))
        .collect()
}

/// A shortcut as written in this app ("Cmd+Alt+Shift+K") from a key press.
pub fn shortcut_string(m: egui::Modifiers, key: egui::Key) -> String {
    let mut s = String::new();
    if m.command {
        s += "Cmd+";
    }
    if m.alt {
        s += "Alt+";
    }
    if m.shift {
        s += "Shift+";
    }
    s += key.name();
    s
}

/// A shortcut for display: ⌃⌥⇧⌘ order on macOS, Ctrl+Alt+Shift+ elsewhere.
pub fn shortcut_text(sc: &str) -> String {
    let mac = cfg!(target_os = "macos");
    // The key is the last part ("Cmd+-" ends in an empty part before "-").
    let (mods, key) = match sc.rsplit_once('+') {
        Some((m, "")) => (m.trim_end_matches('+'), "+"),
        Some((m, k)) => (m, k),
        None => ("", sc),
    };
    let has = |m: &str| mods.split('+').any(|x| x == m);
    let mut out = String::new();
    for (m, sym, word) in [("Ctrl", "⌃", "Ctrl+"), ("Alt", "⌥", "Alt+"), ("Shift", "⇧", "Shift+"), ("Cmd", "⌘", "Ctrl+")] {
        if has(m) && !(m == "Cmd" && !mac && has("Ctrl")) {
            out += if mac { sym } else { word };
        }
    }
    out + key
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
            // Find the matching `<` (submenus nest).
            let start = i;
            let mut depth = 1;
            while i < entries.len() {
                if entries[i].starts_with('>') {
                    depth += 1;
                } else if entries[i] == "<" {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
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
        "view.overprintPreview" => app.ui.overprint_preview,
        "view.fastDisplay" => app.ui.display_quality == designcraft_render::DisplayQuality::Fast,
        "view.typicalDisplay" => app.ui.display_quality == designcraft_render::DisplayQuality::Typical,
        "view.highQualityDisplay" => app.ui.display_quality == designcraft_render::DisplayQuality::High,
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
    if params.is_null() && id == "layout.documentSetup" {
        crate::dialogs::open_document_setup(app);
        return;
    }
    if params.is_null() && id == "file.print" {
        crate::dialogs::open_print(app);
        return;
    }
    if params.is_null() && id == "object.textFrameOptions" {
        app.ui.dialog = Some(crate::dialogs::Dialog::new("textFrameOptions", json!({})));
        return;
    }
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
            Item::Cmd { label, id, params, shortcut: _ } => {
                let text = match checked(app, id, params) {
                    Some(true) => format!("✓ {label}"),
                    Some(false) => format!("   {label}"),
                    None => label.clone(),
                };
                let mut b = egui::Button::new(text);
                if let Some(sc) = shortcut_of(app, id) {
                    b = b.shortcut_text(shortcut_text(&sc));
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
        let mut fired: Option<String> = None;
        let all = effective_shortcuts(app);
        for (id, sc) in &all {
            let id = id.as_str();
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
                fired = Some(id.to_string());
                break;
            }
        }
        if let Some(id) = fired {
            if app.native_shortcuts.contains(&id) && !app.ui.shortcuts.contains_key(&id) {
                continue; // The native menu handles it.
            }
            // Like choosing the menu item: "…" commands open their dialog.
            activate(app, &id, &Value::Null);
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

/// Quick Apply (⌘Return): the document's styles and every command, searched by name.
pub fn palette(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(mut q) = app.ui.palette.clone() else { return };
    let mut close = false;
    let mut run: Option<(String, Value)> = None;
    egui::Modal::new(egui::Id::new("palette")).show(ctx, |ui| {
        ui.set_width(480.0);
        let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search styles and commands…").desired_width(f32::INFINITY));
        r.request_focus();
        let items = quick_apply_items(&app.session, &q);
        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
            for (i, it) in items.iter().enumerate() {
                let mut b = egui::Button::new(it.label.as_str()).frame(false);
                if let Some(sc) = &it.shortcut {
                    b = b.shortcut_text(shortcut_text(sc));
                } else if !it.kind.is_empty() {
                    b = b.shortcut_text(it.kind);
                }
                if ui.add_sized([460.0, 22.0], b).clicked() || (i == 0 && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    run = Some((it.id.clone(), it.params.clone()));
                }
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
    });
    app.ui.palette = if close || run.is_some() { None } else { Some(q) };
    if let Some((id, params)) = run {
        let _ = app.run(&id, params);
    }
}

/// One Quick Apply entry: a style to apply or a command to run.
#[derive(Clone, Debug, PartialEq)]
pub struct QuickItem {
    pub label: String,
    pub id: String,
    pub params: Value,
    pub shortcut: Option<String>,
    /// "Paragraph Style", "Character Style", "Object Style", or "" for commands.
    pub kind: &'static str,
}

/// Quick Apply matches (styles first, as InDesign lists them), at most 14.
pub fn quick_apply_items(session: &designcraft_engine::Session, query: &str) -> Vec<QuickItem> {
    let ql = query.to_lowercase();
    let hit = |label: &str, id: &str| ql.is_empty() || label.to_lowercase().contains(&ql) || id.to_lowercase().contains(&ql);
    let mut out = Vec::new();
    if let Some(st) = session.active() {
        let styles = &st.doc.styles;
        let mut add = |name: &str, id: &str, kind: &'static str| {
            if name.starts_with('[') || !hit(name, "") {
                return;
            }
            out.push(QuickItem { label: name.to_string(), id: id.into(), params: json!({"name": name}), shortcut: None, kind });
        };
        for p in &styles.paragraph {
            add(&p.name, "style.paragraph.apply", "Paragraph Style");
        }
        for c in &styles.character {
            add(&c.name, "style.character.apply", "Character Style");
        }
        for o in &styles.object {
            add(&o.name, "style.object.apply", "Object Style");
        }
    }
    // Styles need a query, else they would crowd out the commands.
    if ql.is_empty() {
        out.clear();
    }
    let commands = designcraft_engine::command_specs()
        .iter()
        .filter(|c| !c.menu.is_empty() || c.shortcut.is_some())
        .map(|c| (c.id, c.label, c.shortcut))
        .chain(UI_COMMANDS.iter().map(|c| (c.0, c.1, c.2)))
        .filter(|(id, l, _)| hit(l, id))
        .map(|(id, l, sc)| QuickItem { label: l.to_string(), id: id.to_string(), params: json!({}), shortcut: sc.map(str::to_string), kind: "" });
    out.extend(commands);
    out.truncate(14);
    out
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
    fn custom_shortcuts_override_defaults() {
        let mut app = crate::DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        assert_eq!(shortcut_of(&app, "app.preferences").as_deref(), Some("Cmd+K"));
        let r = run_ui(&mut app, "window.setShortcut", &json!({"id": "app.preferences", "shortcut": "Cmd+Alt+Shift+9"})).unwrap().unwrap();
        assert_eq!(r["shortcut"], "Cmd+Alt+Shift+9");
        assert!(effective_shortcuts(&app).contains(&("app.preferences".into(), "Cmd+Alt+Shift+9".into())));
        // Taking another command's keys reports the conflict.
        let r = run_ui(&mut app, "window.setShortcut", &json!({"id": "app.palette", "shortcut": "Cmd+Alt+Shift+9"})).unwrap().unwrap();
        assert_eq!(r["conflicts"], json!(["app.preferences"]));
        run_ui(&mut app, "window.setShortcut", &json!({"id": "app.palette", "shortcut": ""})).unwrap().unwrap();
        assert_eq!(shortcut_of(&app, "app.palette"), None);
        assert!(run_ui(&mut app, "window.setShortcut", &json!({"id": "app.palette", "shortcut": "Cmd+Nope"})).unwrap().is_err());
        run_ui(&mut app, "window.resetShortcuts", &json!({})).unwrap().unwrap();
        // ⌘N is the New Document dialog, not a bare file.new.
        let first_cmd_n = effective_shortcuts(&app).into_iter().find(|(_, s)| s == "Cmd+N").map(|(i, _)| i);
        assert_eq!(first_cmd_n.as_deref(), Some("app.newDocumentDialog"));
        assert_eq!(shortcut_of(&app, "app.palette").as_deref(), Some("Cmd+Return"));
        assert_eq!(shortcut_string(egui::Modifiers { command: true, shift: true, ..Default::default() }, egui::Key::K), "Cmd+Shift+K");
        if cfg!(target_os = "macos") {
            assert_eq!(shortcut_text("Cmd+Alt+3"), "⌥⌘3");
            assert_eq!(shortcut_text("Cmd+Shift+."), "⇧⌘.");
        } else {
            assert_eq!(shortcut_text("Cmd+Alt+3"), "Alt+Ctrl+3");
        }
    }

    #[test]
    fn panels_float_and_dock() {
        let mut app = crate::DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        let r = run_ui(&mut app, "window.floatPanel", &json!({"panel": "swatches", "x": 50, "y": 60})).unwrap().unwrap();
        assert_eq!(r["floating"], json!(["swatches"]));
        assert_eq!(app.ui.floating[0].1, [50.0, 60.0]);
        // Showing a floating panel doesn't also open its flyout.
        run_ui(&mut app, "window.panel", &json!({"panel": "swatches"})).unwrap().unwrap();
        assert_eq!(app.ui.open_panel, None);
        run_ui(&mut app, "window.dockPanel", &json!({"panel": "swatches"})).unwrap().unwrap();
        assert!(app.ui.floating.is_empty());
        assert!(run_ui(&mut app, "window.floatPanel", &json!({"panel": "nope"})).unwrap().is_err());
    }

    #[test]
    fn quick_apply_lists_styles_then_commands() {
        let mut s = designcraft_engine::Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("style.paragraph.create", &json!({"name": "Body Copy"})).unwrap();
        let items = quick_apply_items(&s, "body");
        assert_eq!(items[0].id, "style.paragraph.apply");
        assert_eq!(items[0].params, json!({"name": "Body Copy"}));
        assert_eq!(items[0].kind, "Paragraph Style");
        assert!(quick_apply_items(&s, "").iter().all(|i| i.kind.is_empty()));
        assert!(quick_apply_items(&s, "preferences").iter().any(|i| i.id == "app.preferences"));
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
        // Nested submenus parse (Type › Insert Special Character › Symbols › …).
        let ty = menu_tree().into_iter().find(|(m, _)| *m == "Type").unwrap().1;
        let special = ty.iter().find_map(|i| match i {
            Item::Sub(n, c) if n == "Insert Special Character" => Some(c.clone()),
            _ => None,
        });
        assert!(special.is_some_and(|c| c.iter().any(|i| matches!(i, Item::Sub(n, _) if n == "Symbols"))));
        // Fixed-parameter variants keep their own labels.
        assert!(all.iter().any(|(l, id, p)| l == "Fill Frame Proportionally" && id == "object.fit" && p["mode"] == "fillProportionally"));
    }
}
