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
        "insertTable" => "Create Table",
        "textFrameOptions" => "Text Frame Options",
        "documentSetup" => "Document Setup",
        "findChange" => "Find/Change",
        "paragraphStyleOptions" => "Paragraph Style Options",
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
            "paragraphStyleOptions" => paragraph_style_options(app, ui, &mut d),
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
        "insertTable" => app.run(
            "table.insert",
            json!({"rows": d.n("bodyRows").unwrap_or(4.0).max(1.0) as u64, "cols": d.n("columns").unwrap_or(4.0).max(1.0) as u64,
                "headerRows": d.n("headerRows").unwrap_or(0.0).max(0.0) as u64, "footerRows": d.n("footerRows").unwrap_or(0.0).max(0.0) as u64}),
        ),
        "textFrameOptions" => app.run(
            "object.textFrameOptions",
            json!({"columns": d.n("columns").unwrap_or(1.0) as u64, "gutter": d.m("gutter").unwrap_or(12.0), "inset": d.m("inset").unwrap_or(0.0), "verticalJustification": d.s("verticalJustification")}),
        ),
        "documentSetup" => app.run("layout.documentSetup", json!({"width": d.m("width"), "height": d.m("height")})),
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
        "findChange" => app.run("find.change", json!({"find": d.s("find"), "change": d.s("change"), "grep": d.b("grep"), "caseSensitive": d.b("caseSensitive"), "wholeWord": d.b("wholeWord"), "scope": d.s("scope")})),
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
