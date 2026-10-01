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
        "textFrameOptions" => "Text Frame Options",
        "documentSetup" => "Document Setup",
        "findChange" => "Find/Change",
        _ => "Dialog",
    };
    egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
        ui.set_min_width(380.0);
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
            "goToPage" => {
                ui.horizontal(|ui| {
                    ui.label("Page");
                    text_field(ui, &mut d, "page", 80.0);
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
            "documentSetup" => {
                let st = app.session.active();
                let (w, h) = st.map(|s| (s.doc.settings.page_width, s.doc.settings.page_height)).unwrap_or((612.0, 792.0));
                d.fields.entry("width".to_string()).or_insert(json!(format_measure(w, Unit::Picas)));
                d.fields.entry("height".to_string()).or_insert(json!(format_measure(h, Unit::Picas)));
                egui::Grid::new("ds").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Width");
                    text_field(ui, &mut d, "width", 90.0);
                    ui.end_row();
                    ui.label("Height");
                    text_field(ui, &mut d, "height", 90.0);
                    ui.end_row();
                });
            }
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
        "textFrameOptions" => app.run(
            "object.textFrameOptions",
            json!({"columns": d.n("columns").unwrap_or(1.0) as u64, "gutter": d.m("gutter").unwrap_or(12.0), "inset": d.m("inset").unwrap_or(0.0), "verticalJustification": d.s("verticalJustification")}),
        ),
        "documentSetup" => app.run("layout.documentSetup", json!({"width": d.m("width"), "height": d.m("height")})),
        "findChange" => app.run("find.change", json!({"find": d.s("find"), "change": d.s("change"), "grep": d.b("grep"), "caseSensitive": d.b("caseSensitive"), "wholeWord": d.b("wholeWord"), "scope": d.s("scope")})),
        other => Err(format!("unknown dialog {other}")),
    }
}
