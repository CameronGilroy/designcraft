//! Modal dialogs. Each dialog keeps its fields in a JSON map so the control channel can set them
//! (`ui.dialog.set {field, value}`) and confirm them (`ui.dialog.confirm`) like a user would.

use designcraft_geom::{Unit, format_measure, units::parse_measure};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::DesignApp;
use crate::theme::semibold;

#[derive(Clone, Debug, Serialize)]
pub struct Dialog {
    pub id: String,
    pub fields: Map<String, Value>,
}

impl Dialog {
    pub fn new(id: &str, params: Value) -> Self {
        let mut fields = match params {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        let defaults = match id {
            "newDocument" => {
                json!({"preset": "Letter", "width": "51p0", "height": "66p0", "pages": 1, "facingPages": true, "columns": 1, "gutter": "1p0",
                "marginTop": "3p0", "marginBottom": "3p0", "marginInside": "3p0", "marginOutside": "3p0", "bleed": "0p0", "primaryTextFrame": false})
            }
            "goToPage" => json!({"page": 1}),
            "insertTable" => json!({"bodyRows": 4, "columns": 4, "headerRows": 0, "footerRows": 0}),
            "insertXref" => json!({"linkTo": "paragraph", "style": "", "target": "", "format": ""}),
            "findChange" => json!({"find": "", "change": "", "grep": false, "caseSensitive": false, "wholeWord": false, "scope": "document"}),
            "textFrameOptions" => json!({"columns": 1, "gutter": "1p0", "inset": "0p0", "verticalJustification": "top"}),
            "documentSetup" => json!({}),
            _ => json!({}),
        };
        if let Value::Object(d) = defaults {
            for (k, v) in d {
                fields.entry(k).or_insert(v);
            }
        }
        if id == "footnoteOptions" {
            // Flatten the rule and show special characters as InDesign metacharacters.
            if let Some(Value::Object(r)) = fields.remove("rule") {
                for (k, v) in r {
                    fields.insert(format!("rule.{k}"), v);
                }
            }
            let sep = fields.get("separator").and_then(Value::as_str).unwrap_or("").to_string();
            fields.insert("separator".into(), json!(sep.replace('\t', "^t").replace('\u{2003}', "^m").replace('\u{2002}', "^>")));
            fields.entry("tab".to_string()).or_insert(json!("numbering"));
        }
        if id == "paragraphStyleOptions" {
            fields.entry("section".to_string()).or_insert(json!("general"));
        }
        if id == "frameSize" {
            for k in ["width", "height"] {
                if let Some(v) = fields.get(k).and_then(Value::as_f64) {
                    fields.insert(k.into(), json!(format_measure(v, Unit::Picas)));
                }
            }
        }
        Dialog { id: id.into(), fields }
    }
    fn s(&self, k: &str) -> String {
        match self.fields.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        }
    }
    fn m(&self, k: &str) -> Option<f64> {
        match self.fields.get(k) {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => parse_measure(s, Unit::Picas).ok(),
            _ => None,
        }
    }
    /// A measure in points ("6", "6 pt", "0p6", "2 mm").
    fn pt(&self, k: &str) -> Option<f64> {
        match self.fields.get(k) {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => parse_measure(s, Unit::Points).ok(),
            _ => None,
        }
    }
    fn b(&self, k: &str) -> bool {
        self.fields.get(k).and_then(Value::as_bool).unwrap_or(false)
    }
    fn n(&self, k: &str) -> Option<f64> {
        match self.fields.get(k) {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

fn text_field(ui: &mut egui::Ui, d: &mut Dialog, key: &str, w: f32) {
    let mut s = d.s(key);
    if ui.add(egui::TextEdit::singleline(&mut s).desired_width(w)).changed() {
        d.fields.insert(key.into(), Value::String(s));
    }
}

/// File › Document Setup, filled from the document.
pub fn open_document_setup(app: &mut DesignApp) {
    let Ok(cur) = app.session.execute("layout.documentSetup", &json!({})) else { return };
    let units = app.session.active().map(|s| s.doc.settings.horizontal_units).unwrap_or(Unit::Picas);
    let fm = |v: &Value| json!(format_measure(v.as_f64().unwrap_or(0.0), units));
    let mut f = json!({"intent": cur["intent"], "pages": cur["pages"], "startPage": cur["startPage"], "facingPages": cur["facingPages"],
        "width": fm(&cur["width"]), "height": fm(&cur["height"])});
    for k in ["bleed", "slug"] {
        for (i, e) in ["Top", "Bottom", "Inside", "Outside"].iter().enumerate() {
            f[format!("{k}{e}")] = fm(&cur[k][i]);
        }
    }
    app.ui.dialog = Some(Dialog::new("documentSetup", f));
}

fn document_setup(ui: &mut egui::Ui, d: &mut Dialog) {
    egui::Grid::new("ds").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Intent:");
        let cur = d.s("intent");
        egui::ComboBox::from_id_salt("ds_intent")
            .selected_text(match cur.as_str() {
                "web" => "Web",
                "mobile" => "Mobile",
                _ => "Print",
            })
            .width(110.0)
            .show_ui(ui, |ui| {
                for (v, l) in [("print", "Print"), ("web", "Web"), ("mobile", "Mobile")] {
                    if ui.selectable_label(cur == v, l).clicked() {
                        d.fields.insert("intent".into(), json!(v));
                    }
                }
            });
        ui.label("");
        ui.label("");
        ui.end_row();
        ui.label("Number of Pages:");
        text_field(ui, d, "pages", 60.0);
        check(ui, d, "facingPages", "Facing Pages");
        ui.label("");
        ui.end_row();
        ui.label("Start Page #:");
        text_field(ui, d, "startPage", 60.0);
        ui.end_row();
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Page Size").font(semibold(12.0)));
    let (w, h) = (d.m("width").unwrap_or(612.0), d.m("height").unwrap_or(792.0));
    let preset =
        designcraft_doc::build::PRESETS.iter().find(|p| (p.width - w).abs() < 0.5 && (p.height - h).abs() < 0.5).map_or("Custom", |p| p.name);
    egui::Grid::new("ds_size").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Size:");
        egui::ComboBox::from_id_salt("ds_preset").selected_text(preset).width(140.0).show_ui(ui, |ui| {
            for p in designcraft_doc::build::PRESETS {
                if ui.selectable_label(p.name == preset, p.name).clicked() {
                    d.fields.insert("width".into(), json!(format_measure(p.width, p.units)));
                    d.fields.insert("height".into(), json!(format_measure(p.height, p.units)));
                }
            }
        });
        ui.label("Orientation:");
        ui.horizontal(|ui| {
            for (portrait, l) in [(true, "Portrait"), (false, "Landscape")] {
                if ui.selectable_label((h >= w) == portrait, l).clicked() && (h >= w) != portrait {
                    let (fw, fh) = (d.s("width"), d.s("height"));
                    d.fields.insert("width".into(), json!(fh));
                    d.fields.insert("height".into(), json!(fw));
                }
            }
        });
        ui.end_row();
        ui.label("Width:");
        text_field(ui, d, "width", 80.0);
        ui.label("Height:");
        text_field(ui, d, "height", 80.0);
        ui.end_row();
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Bleed and Slug").font(semibold(12.0)));
    egui::Grid::new("ds_bleed").num_columns(5).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("");
        for e in ["Top", "Bottom", "Inside", "Outside"] {
            ui.label(e);
        }
        ui.end_row();
        for (k, l) in [("bleed", "Bleed:"), ("slug", "Slug:")] {
            ui.label(l);
            for e in ["Top", "Bottom", "Inside", "Outside"] {
                text_field(ui, d, &format!("{k}{e}"), 60.0);
            }
            ui.end_row();
        }
    });
}

/// Preferences: a section list and the section's options. Application options always; units and
/// increments when a document is open (InDesign keeps those with the document).
fn preferences(ui: &mut egui::Ui, d: &mut Dialog) {
    let has_doc = d.fields.contains_key("horizontalUnits");
    let sections: &[(&str, &str)] = if has_doc {
        &[("general", "General"), ("type", "Type"), ("units", "Units & Increments")]
    } else {
        &[("general", "General"), ("type", "Type")]
    };
    let cur = d.s("section");
    ui.horizontal_top(|ui| {
        // A fixed height: the separator would otherwise take the whole window.
        ui.set_min_height(240.0);
        ui.set_max_height(240.0);
        ui.vertical(|ui| {
            ui.set_width(150.0);
            for (id, label) in sections {
                if ui.selectable_label(cur == *id, *label).clicked() {
                    d.fields.insert("section".into(), json!(id));
                }
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_min_width(300.0);
            match cur.as_str() {
                "type" => {
                    ui.label(egui::RichText::new("Type Options").font(semibold(12.0)));
                    check(ui, d, "typographersQuotes", "Use Typographer's Quotes");
                }
                "units" => {
                    ui.label(egui::RichText::new("Ruler Units").font(semibold(12.0)));
                    egui::Grid::new("pref_units").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                        for (key, label) in [("horizontalUnits", "Horizontal:"), ("verticalUnits", "Vertical:")] {
                            ui.label(label);
                            let cur: Unit = d.fields.get(key).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
                            egui::ComboBox::from_id_salt(key).selected_text(cur.label()).width(140.0).show_ui(ui, |ui| {
                                for u in Unit::ALL {
                                    if ui.selectable_label(u == cur, u.label()).clicked() {
                                        d.fields.insert(key.into(), json!(u));
                                    }
                                }
                            });
                            ui.end_row();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Keyboard Increments").font(semibold(12.0)));
                    ui.horizontal(|ui| {
                        ui.label("Cursor Key:");
                        text_field(ui, d, "keyboardIncrement", 80.0);
                    });
                }
                _ => {
                    ui.label(egui::RichText::new("When Scaling").font(semibold(12.0)));
                    check(ui, d, "scaleStrokes", "Include Stroke Weight");
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Transform").font(semibold(12.0)));
                    check(ui, d, "dimensionsIncludeStroke", "Dimensions Include Stroke Weight");
                }
            }
        });
    });
}

fn check(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    let mut b = d.b(key);
    if ui.checkbox(&mut b, label).changed() {
        d.fields.insert(key.into(), Value::Bool(b));
    }
}

pub fn show(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(mut d) = app.ui.dialog.clone() else { return };
    let mut result: Option<bool> = None;
    let title = match d.id.as_str() {
        "newDocument" => "New Document",
        "frameSize" => "Rectangle",
        "goToPage" => "Go to Page",
        "insertTable" => "Create Table",
        "textFrameOptions" => "Text Frame Options",
        "documentSetup" => "Document Setup",
        "findChange" => "Find/Change",
        "paragraphStyleOptions" => "Paragraph Style Options",
        "footnoteOptions" => "Footnote Options",
        "insertXref" => "New Cross-Reference",
        "findFont" => "Find/Replace Font",
        "polygonSettings" => "Polygon Settings",
        "preferences" => "Preferences",
        "colorPicker" => "Color Picker",
        id => match id.strip_prefix("cmd:").and_then(designcraft_engine::find_command) {
            Some(c) => c.label.trim_end_matches('…'),
            None => "Dialog",
        },
    };
    egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.set_max_width(640.0);
        ui.label(egui::RichText::new(title).font(semibold(16.0)));
        ui.add_space(10.0);
        match d.id.as_str() {
            "newDocument" => {
                ui.horizontal(|ui| {
                    ui.label("Preset");
                    let cur = d.s("preset");
                    egui::ComboBox::from_id_salt("preset").selected_text(&cur).width(180.0).show_ui(ui, |ui| {
                        for p in designcraft_doc::build::PRESETS {
                            if ui.selectable_label(cur == p.name, p.name).clicked() {
                                d.fields.insert("preset".into(), json!(p.name));
                                d.fields.insert("width".into(), json!(format_measure(p.width, p.units)));
                                d.fields.insert("height".into(), json!(format_measure(p.height, p.units)));
                                d.fields.insert("facingPages".into(), json!(p.intent == designcraft_doc::Intent::Print));
                            }
                        }
                    });
                });
                egui::Grid::new("nd").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Width");
                    text_field(ui, &mut d, "width", 80.0);
                    ui.label("Height");
                    text_field(ui, &mut d, "height", 80.0);
                    ui.end_row();
                    ui.label("Pages");
                    text_field(ui, &mut d, "pages", 80.0);
                    ui.label("");
                    check(ui, &mut d, "facingPages", "Facing Pages");
                    ui.end_row();
                    ui.label("Columns");
                    text_field(ui, &mut d, "columns", 80.0);
                    ui.label("Gutter");
                    text_field(ui, &mut d, "gutter", 80.0);
                    ui.end_row();
                    ui.label("Top");
                    text_field(ui, &mut d, "marginTop", 80.0);
                    ui.label("Bottom");
                    text_field(ui, &mut d, "marginBottom", 80.0);
                    ui.end_row();
                    ui.label("Inside");
                    text_field(ui, &mut d, "marginInside", 80.0);
                    ui.label("Outside");
                    text_field(ui, &mut d, "marginOutside", 80.0);
                    ui.end_row();
                    ui.label("Bleed");
                    text_field(ui, &mut d, "bleed", 80.0);
                    ui.label("");
                    check(ui, &mut d, "primaryTextFrame", "Primary Text Frame");
                    ui.end_row();
                });
            }
            "frameSize" => {
                egui::Grid::new("fs").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Width");
                    text_field(ui, &mut d, "width", 90.0);
                    ui.end_row();
                    ui.label("Height");
                    text_field(ui, &mut d, "height", 90.0);
                    ui.end_row();
                });
            }
            "findChange" => {
                ui.horizontal(|ui| {
                    for (label, grep) in [("Text", false), ("GREP", true)] {
                        if ui.selectable_label(d.b("grep") == grep, label).clicked() {
                            d.fields.insert("grep".into(), json!(grep));
                        }
                    }
                });
                egui::Grid::new("fc").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Find what:");
                    text_field(ui, &mut d, "find", 260.0);
                    ui.end_row();
                    ui.label("Change to:");
                    text_field(ui, &mut d, "change", 260.0);
                    ui.end_row();
                    ui.label("Search:");
                    let cur = d.s("scope");
                    egui::ComboBox::from_id_salt("fcscope").selected_text(&cur).show_ui(ui, |ui| {
                        for v in ["document", "story", "selection"] {
                            if ui.selectable_label(cur == v, v).clicked() {
                                d.fields.insert("scope".into(), json!(v));
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    check(ui, &mut d, "caseSensitive", "Case sensitive");
                    check(ui, &mut d, "wholeWord", "Whole word");
                });
                ui.horizontal(|ui| {
                    let params0 = json!({"find": d.s("find"), "change": d.s("change"), "grep": d.b("grep"), "caseSensitive": d.b("caseSensitive"), "wholeWord": d.b("wholeWord"), "scope": d.s("scope")});
                    let params = || params0.clone();
                    if ui.button("Find Next").clicked() {
                        let r = app.run("find.next", params());
                        let msg = match r {
                            Ok(Value::Null) => "No matches".to_string(),
                            Ok(_) => "Found".to_string(),
                            Err(e) => e,
                        };
                        d.fields.insert("status".into(), json!(msg));
                    }
                    if ui.button("Change All").clicked() {
                        let msg = match app.run("find.change", params()) {
                            Ok(v) => format!("{} replacement(s) made", v["count"]),
                            Err(e) => e,
                        };
                        d.fields.insert("status".into(), json!(msg));
                    }
                    if ui.button("Count").clicked() {
                        let msg = match app.run("find.find", params()) {
                            Ok(v) => format!("{} match(es)", v.as_array().map(|a| a.len()).unwrap_or(0)),
                            Err(e) => e,
                        };
                        d.fields.insert("status".into(), json!(msg));
                    }
                });
                if let Some(st) = d.fields.get("status").and_then(Value::as_str) {
                    ui.label(egui::RichText::new(st).color(crate::theme::Tokens::get(ui.ctx()).text_dim));
                }
            }
            "paragraphStyleOptions" => paragraph_style_options(app, ui, &mut d),
            "footnoteOptions" => footnote_options(app, ui, &mut d),
            "insertXref" => insert_xref(app, ui, &mut d),
            "findFont" => find_font(app, ui, &mut d),
            "colorPicker" => color_picker(ui, &mut d),
            "preferences" => preferences(ui, &mut d),
            "polygonSettings" => {
                egui::Grid::new("poly").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Number of Sides:");
                    text_field(ui, &mut d, "sides", 60.0);
                    ui.end_row();
                    ui.label("Star Inset:");
                    ui.horizontal(|ui| {
                        text_field(ui, &mut d, "starInset", 60.0);
                        ui.label("%");
                    });
                    ui.end_row();
                });
            }
            id if id.starts_with("cmd:") => command_form(ui, &mut d),
            "goToPage" => {
                ui.horizontal(|ui| {
                    ui.label("Page");
                    text_field(ui, &mut d, "page", 80.0);
                });
            }
            "insertTable" => {
                ui.label(egui::RichText::new("Table Dimensions").font(semibold(12.0)));
                egui::Grid::new("ins_table").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Body Rows");
                    text_field(ui, &mut d, "bodyRows", 60.0);
                    ui.label("Columns");
                    text_field(ui, &mut d, "columns", 60.0);
                    ui.end_row();
                    ui.label("Header Rows");
                    text_field(ui, &mut d, "headerRows", 60.0);
                    ui.label("Footer Rows");
                    text_field(ui, &mut d, "footerRows", 60.0);
                    ui.end_row();
                });
            }
            "textFrameOptions" => {
                egui::Grid::new("tfo").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Number of columns");
                    text_field(ui, &mut d, "columns", 80.0);
                    ui.end_row();
                    ui.label("Gutter");
                    text_field(ui, &mut d, "gutter", 80.0);
                    ui.end_row();
                    ui.label("Inset spacing");
                    text_field(ui, &mut d, "inset", 80.0);
                    ui.end_row();
                    ui.label("Vertical justification");
                    let cur = d.s("verticalJustification");
                    egui::ComboBox::from_id_salt("vj").selected_text(&cur).show_ui(ui, |ui| {
                        for v in ["top", "center", "bottom", "justify"] {
                            if ui.selectable_label(cur == v, v).clicked() {
                                d.fields.insert("verticalJustification".into(), json!(v));
                            }
                        }
                    });
                    ui.end_row();
                });
            }
            "documentSetup" => document_setup(ui, &mut d),
            _ => {}
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("  OK  ").color(egui::Color32::WHITE))
                            .fill(crate::theme::Tokens::get(ui.ctx()).accent_strong),
                    )
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(false);
                }
            });
        });
    });
    app.ui.dialog = Some(d);
    match result {
        Some(true) => {
            let _ = confirm(app);
        }
        Some(false) => app.ui.dialog = None,
        None => {}
    }
}

/// Apply the open dialog.
pub fn confirm(app: &mut DesignApp) -> Result<Value, String> {
    let Some(d) = app.ui.dialog.take() else { return Err("no dialog open".into()) };
    match d.id.as_str() {
        "newDocument" => {
            let margins = json!({"top": d.m("marginTop").unwrap_or(36.0), "bottom": d.m("marginBottom").unwrap_or(36.0), "inside": d.m("marginInside").unwrap_or(36.0), "outside": d.m("marginOutside").unwrap_or(36.0)});
            app.run(
                "file.new",
                json!({"width": d.m("width"), "height": d.m("height"), "pages": d.n("pages").unwrap_or(1.0) as u64, "facingPages": d.b("facingPages"),
                    "columns": d.n("columns").unwrap_or(1.0) as u64, "gutter": d.m("gutter").unwrap_or(12.0), "margins": margins, "bleed": d.m("bleed").unwrap_or(0.0),
                    "primaryTextFrame": d.b("primaryTextFrame")}),
            )
        }
        "frameSize" => {
            let x = d.fields.get("x").and_then(Value::as_f64).unwrap_or(0.0);
            let y = d.fields.get("y").and_then(Value::as_f64).unwrap_or(0.0);
            let w = d.m("width").unwrap_or(72.0);
            let h = d.m("height").unwrap_or(72.0);
            app.run(
                "frame.create",
                json!({"spread": d.fields.get("spread").cloned().unwrap_or(json!(0)), "shape": d.s("shape"), "content": d.s("content"), "rect": [x, y, x + w, y + h]}),
            )
        }
        "goToPage" => {
            crate::canvas::go_to_page(app, (d.n("page").unwrap_or(1.0) as usize).saturating_sub(1));
            Ok(Value::Null)
        }
        "insertTable" => app.run(
            "table.insert",
            json!({"rows": d.n("bodyRows").unwrap_or(4.0).max(1.0) as u64, "cols": d.n("columns").unwrap_or(4.0).max(1.0) as u64,
                "headerRows": d.n("headerRows").unwrap_or(0.0).max(0.0) as u64, "footerRows": d.n("footerRows").unwrap_or(0.0).max(0.0) as u64}),
        ),
        "textFrameOptions" => app.run(
            "object.textFrameOptions",
            json!({"columns": d.n("columns").unwrap_or(1.0) as u64, "gutter": d.m("gutter").unwrap_or(12.0), "inset": d.m("inset").unwrap_or(0.0), "verticalJustification": d.s("verticalJustification")}),
        ),
        "documentSetup" => {
            let edges = |k: &str| json!(["Top", "Bottom", "Inside", "Outside"].map(|e| d.m(&format!("{k}{e}")).unwrap_or(0.0)));
            app.run(
                "layout.documentSetup",
                json!({"intent": d.s("intent"), "pages": d.n("pages").unwrap_or(1.0).max(1.0) as u64, "startPage": d.n("startPage").unwrap_or(1.0).max(1.0) as u64,
                    "facingPages": d.b("facingPages"), "width": d.m("width"), "height": d.m("height"), "bleed": edges("bleed"), "slug": edges("slug")}),
            )
        }
        "paragraphStyleOptions" => {
            let name = d.s("name");
            let mut para = serde_json::Map::new();
            let mut chars = serde_json::Map::new();
            for (k, v) in &d.fields {
                if let Some(a) = k.strip_prefix("p.") {
                    para.insert(a.into(), v.clone());
                } else if let Some(a) = k.strip_prefix("c.") {
                    chars.insert(a.into(), v.clone());
                }
            }
            let mut params = json!({"name": name, "para": para, "chars": chars});
            let based = d.s("basedOn");
            if !based.is_empty() {
                params["basedOn"] = if based == "[No Paragraph Style]" { Value::Null } else { json!(based) };
            }
            let rename = d.s("rename");
            if !rename.is_empty() && rename != name {
                params["rename"] = json!(rename);
            }
            app.run("style.paragraph.edit", params)
        }
        "colorPicker" => {
            let hex = d.s("hex");
            app.run("object.color", json!({"color": hex, "target": d.s("target")}))
        }
        "preferences" => {
            app.run(
                "prefs.set",
                json!({"scaleStrokes": d.b("scaleStrokes"), "dimensionsIncludeStroke": d.b("dimensionsIncludeStroke"), "typographersQuotes": d.b("typographersQuotes")}),
            )?;
            if !d.fields.contains_key("horizontalUnits") {
                return Ok(Value::Null);
            }
            let mut doc = json!({"horizontalUnits": d.fields["horizontalUnits"], "verticalUnits": d.fields["verticalUnits"]});
            if let Some(v) = d.fields.get("keyboardIncrement").and_then(Value::as_str).and_then(|s| parse_measure(s, Unit::Points).ok()) {
                doc["keyboardIncrement"] = json!(v.max(0.001));
            }
            app.run("document.preferences", doc)
        }
        "polygonSettings" => app.run("tool.polygonSettings", json!({"sides": d.n("sides").unwrap_or(6.0) as u64, "starInset": d.n("starInset").unwrap_or(0.0)})),
        "findFont" => {
            let (f, st) = (d.s("family"), d.s("style"));
            if f.is_empty() || d.s("toFamily").is_empty() {
                return Ok(Value::Null);
            }
            app.run("font.replace", json!({"family": f, "style": st, "toFamily": d.s("toFamily"), "toStyle": d.s("toStyle")}))
        }
        "insertXref" => {
            let format = d.s("format");
            let target = d.s("target");
            let params = match target.split_once(':') {
                Some(("a", id)) => json!({"anchor": id.parse::<u64>().unwrap_or(0), "format": format}),
                Some((sid, pi)) => json!({"story": sid.parse::<u64>().unwrap_or(0), "para": pi.parse::<u64>().unwrap_or(0), "format": format}),
                None => {
                    let mut d = d.clone();
                    d.fields.insert("status".into(), json!("Choose a destination paragraph or text anchor."));
                    app.ui.dialog = Some(d);
                    return Err("no destination chosen".into());
                }
            };
            app.run("xref.insert", params)
        }
        "footnoteOptions" => {
            let pt = |k: &str| d.pt(k).map_or(Value::Null, |v| json!(v));
            let mut p = json!({
                "style": d.s("style"), "startAt": d.n("startAt").unwrap_or(1.0).max(0.0) as u32, "restart": d.s("restart"),
                "prefix": d.s("prefix"), "suffix": d.s("suffix"), "affixIn": d.s("affixIn"), "refPosition": d.s("refPosition"),
                "refCharStyle": d.s("refCharStyle"), "paraStyle": d.s("paraStyle"),
                "separator": d.s("separator").replace("^t", "\t").replace("^m", "\u{2003}").replace("^>", "\u{2002}"),
                "spaceBefore": pt("spaceBefore"), "spaceBetween": pt("spaceBetween"), "firstBaseline": d.s("firstBaseline"),
                "firstBaselineMin": pt("firstBaselineMin"), "spanColumns": d.b("spanColumns"),
                "rule": {"on": d.b("rule.on"), "weight": pt("rule.weight"), "color": d.s("rule.color"), "width": pt("rule.width"),
                    "offset": pt("rule.offset"), "leftIndent": pt("rule.leftIndent")},
            });
            // Unparsable measures keep their current values.
            if let Some(o) = p.as_object_mut() {
                o.retain(|_, v| !v.is_null());
            }
            if let Some(r) = p.get_mut("rule").and_then(Value::as_object_mut) {
                r.retain(|_, v| !v.is_null());
            }
            app.run("footnote.options", p)
        }
        "findChange" => app.run("find.change", json!({"find": d.s("find"), "change": d.s("change"), "grep": d.b("grep"), "caseSensitive": d.b("caseSensitive"), "wholeWord": d.b("wholeWord"), "scope": d.s("scope")})),
        id if id.starts_with("cmd:") => {
            let cid = &id[4..];
            let doc = designcraft_engine::find_command(cid).map_or("", |c| c.params);
            let mut p = Map::new();
            for f in command_fields(doc) {
                match d.fields.get(&f.key) {
                    Some(Value::String(v)) if !v.trim().is_empty() => {
                        let v = v.trim();
                        // Numbers, booleans, arrays and objects as JSON; anything else is a string.
                        let parsed = serde_json::from_str::<Value>(v).ok().filter(|x| !x.is_string());
                        p.insert(f.key, parsed.unwrap_or_else(|| json!(v)));
                    }
                    Some(Value::Bool(b)) => {
                        p.insert(f.key, json!(b));
                    }
                    _ => {}
                }
            }
            let r = app.run(cid, Value::Object(p));
            if let Err(e) = &r {
                // Keep the dialog open with the error.
                let mut d = d.clone();
                d.fields.insert("status".into(), json!(e));
                app.ui.dialog = Some(d);
            }
            r
        }
        other => Err(format!("unknown dialog {other}")),
    }
}

/// Paragraph Style Options: sections in a left list (like InDesign), fields on the right.
/// Edited values are stored as `p.<attr>` / `c.<attr>` fields and applied on OK.
fn paragraph_style_options(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    let name = d.s("name");
    let Some(st) = app.session.active() else { return };
    let Some(style) = st.doc.styles.para(&name).cloned() else {
        ui.label(format!("No style named {name}"));
        return;
    };
    let names: Vec<String> = st.doc.styles.paragraph.iter().map(|p| p.name.clone()).filter(|n| *n != name).collect();
    let (pp, cp) = st.doc.styles.resolve_para_style(&name);
    let units = st.doc.settings.horizontal_units;
    let pv = serde_json::to_value(&pp).unwrap_or_default();
    let cv = serde_json::to_value(&cp).unwrap_or_default();
    let cur = |d: &Dialog, k: &str, base: &Value| d.fields.get(k).cloned().unwrap_or_else(|| base.clone());
    ui.set_min_width(560.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(170.0);
            for (id, label) in [
                ("general", "General"),
                ("chars", "Basic Character Formats"),
                ("indents", "Indents and Spacing"),
                ("hyph", "Hyphenation"),
                ("justify", "Justification"),
                ("color", "Character Color"),
            ] {
                if ui.selectable_label(d.s("section") == id, label).clicked() {
                    d.fields.insert("section".into(), json!(id));
                }
            }
        });
        ui.separator();
        ui.vertical(|ui| match d.s("section").as_str() {
            "chars" => {
                egui::Grid::new("psc").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Font Family:");
                    let fam = cur(d, "c.fontFamily", &cv["fontFamily"]).as_str().unwrap_or("").to_string();
                    egui::ComboBox::from_id_salt("psfam").selected_text(&fam).width(200.0).show_ui(ui, |ui| {
                        for f in designcraft_fonts::FontDb::global().families() {
                            if ui.selectable_label(f == fam, &f).clicked() {
                                d.fields.insert("c.fontFamily".into(), json!(f));
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Font Style:");
                    let sty = cur(d, "c.fontStyle", &cv["fontStyle"]).as_str().unwrap_or("").to_string();
                    egui::ComboBox::from_id_salt("pssty").selected_text(&sty).width(200.0).show_ui(ui, |ui| {
                        for s in designcraft_fonts::FontDb::global().styles(&fam) {
                            if ui.selectable_label(s == sty, &s).clicked() {
                                d.fields.insert("c.fontStyle".into(), json!(s));
                            }
                        }
                    });
                    ui.end_row();
                    for (label, key, suffix) in [("Size:", "size", " pt"), ("Tracking:", "tracking", "")] {
                        ui.label(label);
                        let v = cur(d, &format!("c.{key}"), &cv[key]).as_f64();
                        if let Some(n) = crate::widgets::number(ui, &format!("ps{key}"), v, suffix, 80.0, 2) {
                            d.fields.insert(format!("c.{key}"), json!(n));
                        }
                        ui.end_row();
                    }
                    ui.label("Leading:");
                    let lv = match cur(d, "c.leading", &cv["leading"]) {
                        v if v["kind"] == "points" => v["value"].as_f64(),
                        _ => None,
                    };
                    if let Some(n) = crate::widgets::number(ui, "pslead", lv, " pt", 80.0, 2) {
                        d.fields.insert("c.leading".into(), json!({"kind": "points", "value": n}));
                    }
                    ui.end_row();
                });
            }
            "indents" => {
                egui::Grid::new("psi").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Alignment:");
                    let a: designcraft_doc::Align = serde_json::from_value(cur(d, "p.align", &pv["align"])).unwrap_or_default();
                    egui::ComboBox::from_id_salt("psalign").selected_text(a.label()).width(240.0).show_ui(ui, |ui| {
                        for al in designcraft_doc::Align::ALL {
                            if ui.selectable_label(al == a, al.label()).clicked() {
                                d.fields.insert("p.align".into(), serde_json::to_value(al).unwrap_or_default());
                            }
                        }
                    });
                    ui.end_row();
                    for (label, key) in [
                        ("Left Indent:", "leftIndent"),
                        ("First Line Indent:", "firstLineIndent"),
                        ("Right Indent:", "rightIndent"),
                        ("Space Before:", "spaceBefore"),
                        ("Space After:", "spaceAfter"),
                    ] {
                        ui.label(label);
                        let v = cur(d, &format!("p.{key}"), &pv[key]).as_f64();
                        if let Some(n) = crate::widgets::measure(ui, &format!("ps{key}"), v, units, 80.0) {
                            d.fields.insert(format!("p.{key}"), json!(n));
                        }
                        ui.end_row();
                    }
                });
            }
            "hyph" => {
                let mut h = cur(d, "p.hyphenate", &pv["hyphenate"]).as_bool().unwrap_or(true);
                if ui.checkbox(&mut h, "Hyphenate").changed() {
                    d.fields.insert("p.hyphenate".into(), json!(h));
                }
                egui::Grid::new("psh").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    for (label, key) in [
                        ("Words with at Least:", "hyphMinWord"),
                        ("After First:", "hyphAfterFirst"),
                        ("Before Last:", "hyphBeforeLast"),
                        ("Hyphen Limit:", "hyphLimit"),
                    ] {
                        ui.label(label);
                        let v = cur(d, &format!("p.{key}"), &pv[key]).as_f64();
                        if let Some(n) = crate::widgets::number(ui, &format!("ps{key}"), v, "", 60.0, 0) {
                            d.fields.insert(format!("p.{key}"), json!(n.max(0.0) as u64));
                        }
                        ui.end_row();
                    }
                });
            }
            "justify" => {
                egui::Grid::new("psj").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("");
                    ui.label("Minimum");
                    ui.label("Desired");
                    ui.label("Maximum");
                    ui.end_row();
                    for (label, base) in [("Word Spacing:", "wordSpace"), ("Letter Spacing:", "letterSpace"), ("Glyph Scaling:", "glyphScale")] {
                        ui.label(label);
                        for suffix in ["Min", "Desired", "Max"] {
                            let key = format!("{base}{suffix}");
                            let v = cur(d, &format!("p.{key}"), &pv[key.as_str()]).as_f64().map(|x| x * 100.0);
                            if let Some(n) = crate::widgets::number(ui, &format!("ps{key}"), v, "%", 60.0, 0) {
                                d.fields.insert(format!("p.{key}"), json!(n / 100.0));
                            }
                        }
                        ui.end_row();
                    }
                    ui.label("Composer:");
                    let single = cur(d, "p.composer", &pv["composer"]).as_str() == Some("singleLine");
                    if ui.selectable_label(!single, "Paragraph Composer").clicked() {
                        d.fields.insert("p.composer".into(), json!("paragraph"));
                    }
                    if ui.selectable_label(single, "Single-line Composer").clicked() {
                        d.fields.insert("p.composer".into(), json!("singleLine"));
                    }
                    ui.end_row();
                });
            }
            "color" => {
                let cur_fill = cur(d, "c.fill", &cv["fill"]).as_str().unwrap_or("").to_string();
                let swatches: Vec<String> = st.doc.swatches.iter().map(|s| s.name.clone()).collect();
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for sw in swatches {
                        let (c, g) = crate::widgets::swatch_colors(&st.doc, &sw, 1.0);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            crate::widgets::paint_chip(ui.painter(), r, c, g);
                            if ui.selectable_label(sw == cur_fill, &sw).clicked() {
                                d.fields.insert("c.fill".into(), json!(sw));
                            }
                        });
                    }
                });
            }
            _ => {
                egui::Grid::new("psg").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Style Name:");
                    if !d.fields.contains_key("rename") {
                        d.fields.insert("rename".into(), json!(name));
                    }
                    let mut rn = d.s("rename");
                    if ui.add(egui::TextEdit::singleline(&mut rn).desired_width(220.0)).changed() {
                        d.fields.insert("rename".into(), json!(rn));
                    }
                    ui.end_row();
                    ui.label("Based On:");
                    let based = d
                        .fields
                        .get("basedOn")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or(style.based_on.clone())
                        .unwrap_or_else(|| "[No Paragraph Style]".into());
                    egui::ComboBox::from_id_salt("psbased").selected_text(&based).width(220.0).show_ui(ui, |ui| {
                        for n in &names {
                            if ui.selectable_label(*n == based, n).clicked() {
                                d.fields.insert("basedOn".into(), json!(n));
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("{} {} {:.1} pt · {}", cp.font_family, cp.font_style, cp.size, pp.align.label()))
                        .color(crate::theme::Tokens::get(ui.ctx()).text_dim),
                );
            }
        });
    });
}

fn combo(ui: &mut egui::Ui, d: &mut Dialog, key: &str, opts: &[(&str, &str)]) {
    let cur = d.s(key);
    let shown = opts.iter().find(|o| o.0 == cur).map_or(cur.as_str(), |o| o.1).to_string();
    egui::ComboBox::from_id_salt(key).selected_text(shown).width(170.0).show_ui(ui, |ui| {
        for (v, label) in opts {
            if ui.selectable_label(cur == *v, *label).clicked() {
                d.fields.insert(key.into(), json!(v));
            }
        }
    });
}

/// Style name picker (paragraph or character styles of the active document).
fn style_combo(app: &DesignApp, ui: &mut egui::Ui, d: &mut Dialog, key: &str, character: bool) {
    let names: Vec<String> = app
        .session
        .active()
        .map(|st| {
            if character {
                st.doc.styles.character.iter().map(|s| s.name.clone()).collect()
            } else {
                st.doc.styles.paragraph.iter().map(|s| s.name.clone()).collect()
            }
        })
        .unwrap_or_default();
    let mut opts: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), n.as_str())).collect();
    if character && !names.iter().any(|n| n == designcraft_doc::NO_CHAR_STYLE) {
        opts.insert(0, (designcraft_doc::NO_CHAR_STYLE, designcraft_doc::NO_CHAR_STYLE));
    }
    combo(ui, d, key, &opts);
}

/// Document Footnote Options: Numbering and Formatting / Layout tabs.
fn footnote_options(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    ui.horizontal(|ui| {
        for (tab, label) in [("numbering", "Numbering and Formatting"), ("layout", "Layout")] {
            if ui.selectable_label(d.s("tab") == tab, label).clicked() {
                d.fields.insert("tab".into(), json!(tab));
            }
        }
    });
    ui.separator();
    let head = |ui: &mut egui::Ui, t: &str| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(t).font(semibold(12.0)));
    };
    if d.s("tab") == "layout" {
        head(ui, "Spacing Options");
        egui::Grid::new("fn_sp").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Minimum Space Before First Footnote:");
            text_field(ui, d, "spaceBefore", 70.0);
            ui.end_row();
            ui.label("Space Between Footnotes:");
            text_field(ui, d, "spaceBetween", 70.0);
            ui.end_row();
        });
        head(ui, "First Baseline");
        egui::Grid::new("fn_fb").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Offset:");
            combo(
                ui,
                d,
                "firstBaseline",
                &[("ascent", "Ascent"), ("capHeight", "Cap Height"), ("leading", "Leading"), ("xHeight", "x Height"), ("fixed", "Fixed")],
            );
            ui.end_row();
            ui.label("Min:");
            text_field(ui, d, "firstBaselineMin", 70.0);
            ui.end_row();
        });
        check(ui, d, "spanColumns", "Span Footnotes Across Columns");
        head(ui, "Rule Above");
        check(ui, d, "rule.on", "Rule On");
        egui::Grid::new("fn_rule").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Weight:");
            text_field(ui, d, "rule.weight", 60.0);
            ui.label("Color:");
            let swatches: Vec<String> = app.session.active().map(|st| st.doc.swatches.iter().map(|s| s.name.clone()).collect()).unwrap_or_default();
            let opts: Vec<(&str, &str)> = swatches.iter().map(|n| (n.as_str(), n.as_str())).collect();
            combo(ui, d, "rule.color", &opts);
            ui.end_row();
            ui.label("Width:");
            text_field(ui, d, "rule.width", 60.0);
            ui.label("Offset:");
            text_field(ui, d, "rule.offset", 60.0);
            ui.end_row();
            ui.label("Left Indent:");
            text_field(ui, d, "rule.leftIndent", 60.0);
            ui.end_row();
        });
        return;
    }
    head(ui, "Numbering");
    egui::Grid::new("fn_num").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Style:");
        combo(
            ui,
            d,
            "style",
            &[
                ("arabic", "1, 2, 3, 4..."),
                ("upperRoman", "I, II, III, IV..."),
                ("lowerRoman", "i, ii, iii, iv..."),
                ("upperLetters", "A, B, C, D..."),
                ("lowerLetters", "a, b, c, d..."),
                ("arabicLeadingZero", "01, 02, 03..."),
                ("symbols", "*, †, ‡, §..."),
            ],
        );
        ui.end_row();
        ui.label("Start at:");
        text_field(ui, d, "startAt", 60.0);
        ui.end_row();
        ui.label("Restart Numbering Every:");
        combo(ui, d, "restart", &[("never", "Never (continuous)"), ("page", "Page"), ("spread", "Spread"), ("section", "Section")]);
        ui.end_row();
        ui.label("Show Prefix/Suffix in:");
        combo(ui, d, "affixIn", &[("none", "None"), ("reference", "Footnote Reference"), ("text", "Footnote Text"), ("both", "Both")]);
        ui.end_row();
        ui.label("Prefix:");
        text_field(ui, d, "prefix", 60.0);
        ui.end_row();
        ui.label("Suffix:");
        text_field(ui, d, "suffix", 60.0);
        ui.end_row();
    });
    head(ui, "Footnote Reference Number in Text");
    egui::Grid::new("fn_ref").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Position:");
        combo(
            ui,
            d,
            "refPosition",
            &[
                ("superscript", "Apply Superscript"),
                ("subscript", "Apply Subscript"),
                ("normal", "Apply Normal"),
                ("otSuperscript", "OpenType Superscript"),
            ],
        );
        ui.end_row();
        ui.label("Character Style:");
        style_combo(app, ui, d, "refCharStyle", true);
        ui.end_row();
    });
    head(ui, "Footnote Formatting");
    egui::Grid::new("fn_fmt").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Paragraph Style:");
        style_combo(app, ui, d, "paraStyle", false);
        ui.end_row();
        ui.label("Separator:");
        text_field(ui, d, "separator", 60.0);
        ui.end_row();
    });
}

/// One parameter of a command, parsed from its params documentation
/// (`{name, type?: a|b|c, count?, flag?: bool, …}`).
#[derive(Clone, Debug, PartialEq)]
pub struct CommandField {
    pub key: String,
    pub optional: bool,
    /// Enumerated values (`a|b|c`).
    pub choices: Vec<String>,
    pub boolean: bool,
    /// The documented value spec (shown as a hint).
    pub hint: String,
}

/// Parameters of a command from its documentation string: the top-level keys of the first `{…}`.
pub fn command_fields(doc: &str) -> Vec<CommandField> {
    let Some(start) = doc.find('{') else { return vec![] };
    let mut depth = 0i32;
    let mut end = doc.len();
    for (i, c) in doc[start..].char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &doc[start + 1..end];
    let mut parts = Vec::new();
    let (mut depth, mut from) = (0i32, 0usize);
    for (i, c) in body.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&body[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[from..]);
    let ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !s.starts_with(|c: char| c.is_ascii_digit());
    let mut out = Vec::new();
    for part in parts {
        let part = part.trim();
        let (k, spec) = match part.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (part, ""),
        };
        let optional = k.ends_with('?');
        let key = k.trim_end_matches('?');
        if !ident(key) || out.iter().any(|f: &CommandField| f.key == key) {
            continue;
        }
        let word = spec.split_whitespace().next().unwrap_or("");
        let choices: Vec<String> =
            if word.contains('|') && word.split('|').all(ident) { word.split('|').map(str::to_string).collect() } else { vec![] };
        let boolean = spec.starts_with("bool") || spec.starts_with("true|false") || spec.starts_with("true") && spec.len() <= 5;
        out.push(CommandField { key: key.into(), optional, choices: if boolean { vec![] } else { choices }, boolean, hint: spec.into() });
    }
    out
}

fn humanize(key: &str) -> String {
    let mut s = String::new();
    for (i, c) in key.chars().enumerate() {
        if i == 0 {
            s.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            s.push(' ');
            s.extend(c.to_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

/// A form for any command, from its parameter documentation.
fn command_form(ui: &mut egui::Ui, d: &mut Dialog) {
    let doc = designcraft_engine::find_command(&d.id[4..]).map_or("", |c| c.params);
    let fields = command_fields(doc);
    let dim = crate::theme::Tokens::get(ui.ctx()).text_dim;
    egui::Grid::new("cmdform").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        for f in &fields {
            ui.label(format!("{}{}", humanize(&f.key), if f.optional { "" } else { " *" }));
            if f.boolean {
                check(ui, d, &f.key, "");
            } else if !f.choices.is_empty() {
                let mut opts: Vec<(&str, &str)> = vec![("", "—")];
                opts.extend(f.choices.iter().map(|c| (c.as_str(), c.as_str())));
                combo(ui, d, &f.key, &opts);
            } else {
                let mut s = d.s(&f.key);
                let hint = if f.hint.is_empty() { String::new() } else { f.hint.clone() };
                if ui.add(egui::TextEdit::singleline(&mut s).hint_text(egui::RichText::new(hint).color(dim)).desired_width(240.0)).changed() {
                    d.fields.insert(f.key.clone(), Value::String(s));
                }
            }
            ui.end_row();
        }
    });
    if let Some(st) = d.fields.get("status").and_then(Value::as_str) {
        ui.label(egui::RichText::new(st).color(crate::theme::Tokens::get(ui.ctx()).text_dim));
    }
}

/// New Cross-Reference: link to a paragraph (paragraph styles on the left, their paragraphs on
/// the right) or a text anchor, with a cross-reference format.
fn insert_xref(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    let Some(st) = app.session.active() else { return };
    let doc = &st.doc;
    let all = "[All Paragraphs]";
    ui.horizontal(|ui| {
        ui.label("Link To:");
        combo(ui, d, "linkTo", &[("paragraph", "Paragraph"), ("anchor", "Text Anchor")]);
    });
    ui.add_space(6.0);
    let dim = crate::theme::Tokens::get(ui.ctx()).text_dim;
    if d.s("linkTo") == "anchor" {
        egui::ScrollArea::vertical().id_salt("xr_anchors").max_height(220.0).max_width(460.0).show(ui, |ui| {
            let mut any = false;
            for st in doc.stories.values() {
                for a in &st.anchors {
                    any = true;
                    let key = format!("a:{}", a.id);
                    if ui.selectable_label(d.s("target") == key, &a.name).clicked() {
                        d.fields.insert("target".into(), json!(key));
                    }
                }
            }
            if !any {
                ui.label(egui::RichText::new("No text anchors in this document.").color(dim));
            }
        });
    } else {
        let mut styles: Vec<String> = vec![all.to_string()];
        styles.extend(doc.styles.paragraph.iter().map(|s| s.name.clone()));
        let cur_style = if d.s("style").is_empty() { all.to_string() } else { d.s("style") };
        ui.horizontal_top(|ui| {
            let col = |w: f32| (egui::vec2(w, 240.0), egui::Layout::top_down(egui::Align::Min));
            let (sz, lay) = col(180.0);
            ui.allocate_ui_with_layout(sz, lay, |ui| {
                egui::ScrollArea::vertical().id_salt("xr_styles").auto_shrink([false, false]).show(ui, |ui| {
                    for name in &styles {
                        if ui.selectable_label(*name == cur_style, name).clicked() {
                            d.fields.insert("style".into(), json!(name));
                        }
                    }
                })
            });
            ui.add_space(8.0);
            let (sz, lay) = col(330.0);
            ui.allocate_ui_with_layout(sz, lay, |ui| {
                egui::ScrollArea::vertical().id_salt("xr_paras").auto_shrink([false, false]).show(ui, |ui| {
                    for st in doc.stories.values().filter(|s| !s.frames.is_empty()) {
                        for (pi, r) in st.para_ranges().into_iter().enumerate() {
                            if cur_style != all && st.paras[pi].style != cur_style {
                                continue;
                            }
                            let text = designcraft_doc::xref::clean(&st.text[r]);
                            if text.is_empty() {
                                continue;
                            }
                            let short: String = text.chars().take(52).collect();
                            let key = format!("{}:{pi}", st.id.0);
                            if ui.selectable_label(d.s("target") == key, short).clicked() {
                                d.fields.insert("target".into(), json!(key));
                            }
                        }
                    }
                })
            });
        });
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Cross-Reference Format").font(semibold(12.0)));
    let names: Vec<String> = doc.xref_formats.iter().map(|f| f.name.clone()).collect();
    if d.s("format").is_empty() {
        d.fields.insert("format".into(), json!(names.first().cloned().unwrap_or_default()));
    }
    let opts: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), n.as_str())).collect();
    ui.horizontal(|ui| {
        ui.label("Format:");
        combo(ui, d, "format", &opts);
    });
    if let Some(m) = d.fields.get("status").and_then(Value::as_str) {
        ui.label(egui::RichText::new(m).color(dim));
    }
}

/// Color Picker: saturation/brightness field and hue slider, with RGB, CMYK and hex fields.
fn color_picker(ui: &mut egui::Ui, d: &mut Dialog) {
    let hex = d.s("hex");
    let mut c = designcraft_color::Color::from_hex(&hex).map(|c| c.to_rgb()).map_or(egui::Color32::BLACK, |[r, g, b]| {
        egui::Color32::from_rgb((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
    });
    let before = c;
    ui.horizontal_top(|ui| {
        egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque);
        ui.vertical(|ui| {
            let mut rgb = [c.r() as f32, c.g() as f32, c.b() as f32];
            egui::Grid::new("cp_rgb").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
                for (k, l) in ["R", "G", "B"].iter().enumerate() {
                    ui.label(*l);
                    ui.add(egui::DragValue::new(&mut rgb[k]).range(0.0..=255.0));
                    ui.end_row();
                }
            });
            c = egui::Color32::from_rgb(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8);
            let col = designcraft_color::Color::rgb8(c.r(), c.g(), c.b());
            let [cc, m, y, k] = col.to_cmyk();
            ui.add_space(6.0);
            ui.label(format!("C {:.0}%  M {:.0}%  Y {:.0}%  K {:.0}%", cc * 100.0, m * 100.0, y * 100.0, k * 100.0));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("#");
                let mut h = format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
                if ui.add(egui::TextEdit::singleline(&mut h).desired_width(70.0)).changed()
                    && let Some(n) = designcraft_color::Color::from_hex(&h)
                {
                    let [r, g, b] = n.to_rgb();
                    c = egui::Color32::from_rgb((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8);
                }
            });
        });
    });
    if c != before {
        d.fields.insert("hex".into(), json!(format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())));
    }
}

/// Find/Replace Font: the document's fonts (missing ones first, flagged) and a replacement.
fn find_font(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    // Listed once when the dialog opens (it scans every story).
    if !d.fields.contains_key("_fonts") {
        let list = app.session.execute("font.list", &json!({})).unwrap_or_default();
        d.fields.insert("_fonts".into(), list);
    }
    let fonts = d.fields.get("_fonts").and_then(Value::as_array).cloned().unwrap_or_default();
    let missing = fonts.iter().filter(|f| f["missing"] == true || f["styleMissing"] == true).count();
    ui.label(format!("Fonts in Document: {}    Missing: {missing}", fonts.len()));
    egui::ScrollArea::vertical().id_salt("ff_list").max_height(180.0).show(ui, |ui| {
        for f in &fonts {
            let (fam, st) = (f["family"].as_str().unwrap_or(""), f["style"].as_str().unwrap_or(""));
            let warn = f["missing"] == true || f["styleMissing"] == true;
            let label = format!("{}{fam} {st}", if warn { "\u{26A0} " } else { "" });
            let on = d.s("family") == fam && d.s("style") == st;
            let mut text = egui::RichText::new(label);
            if warn {
                text = text.color(egui::Color32::from_rgb(0xe5, 0x4b, 0x4b));
            }
            if ui.selectable_label(on, text).clicked() {
                d.fields.insert("family".into(), json!(fam));
                d.fields.insert("style".into(), json!(st));
            }
        }
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Replace With").font(semibold(12.0)));
    let db = designcraft_fonts::FontDb::global();
    let families = db.families();
    let fam_opts: Vec<(&str, &str)> = families.iter().map(|f| (f.as_str(), f.as_str())).collect();
    egui::Grid::new("ff_to").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Font Family:");
        combo(ui, d, "toFamily", &fam_opts);
        ui.end_row();
        let styles = db.styles(&d.s("toFamily"));
        let st_opts: Vec<(&str, &str)> = styles.iter().map(|s| (s.as_str(), s.as_str())).collect();
        ui.label("Font Style:");
        combo(ui, d, "toStyle", &st_opts);
        ui.end_row();
    });
    ui.label(
        egui::RichText::new("OK changes all: text and paragraph/character styles.").color(crate::theme::Tokens::get(ui.ctx()).text_dim).size(11.0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_params() {
        let f = command_fields("{name, type: custom|lastPageNumber|chapterNumber, text?, rule?: {on, weight}, flag?: bool} — creates");
        let keys: Vec<&str> = f.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, ["name", "type", "text", "rule", "flag"]);
        assert_eq!(f[1].choices, ["custom", "lastPageNumber", "chapterNumber"]);
        assert!(!f[0].optional && f[2].optional);
        assert!(f[4].boolean);
        assert!(command_fields("{}").is_empty());
        assert!(command_fields("no params").is_empty());
        // Every command with a "…" label parses without panicking.
        for c in designcraft_engine::command_specs() {
            let _ = command_fields(c.params);
        }
    }
}
