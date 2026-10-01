//! The Properties panel (context-sensitive) and small single-purpose panels (Stroke, Character,
//! Paragraph, Text Wrap, Align, Links, Info).

use designcraft_doc::Align;
use designcraft_geom::Unit;
use egui::vec2;
use serde_json::json;

use super::{SelInfo, sel_info, text_attrs};
use crate::theme::Tokens;
use crate::widgets::{caption, divider, measure, number, section};
use crate::{DesignApp, icons};

fn units(app: &DesignApp) -> Unit {
    app.session.active().map(|d| d.doc.settings.horizontal_units).unwrap_or(Unit::Picas)
}

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else {
        ui.label(egui::RichText::new("No document open").color(t.text_dim));
        return;
    };
    let has_text = st.selection.text.is_some();
    let info = sel_info(app);
    if has_text {
        ui.label(egui::RichText::new("Text").color(t.text_dim));
        character_panel(app, ui);
        divider(ui);
        paragraph_panel(app, ui);
        divider(ui);
        if section(ui, "Text Frame", false) {
            text_frame_section(app, ui);
        }
        return;
    }
    match info {
        Some(i) => {
            let kind = if i.count > 1 { format!("{} objects", i.count) } else { i.kind.trim_matches(['<', '>']).to_string() };
            ui.label(egui::RichText::new(capitalize(&kind)).color(t.text_dim));
            if section(ui, "Transform", true) {
                transform_section(app, ui, &i);
            }
            divider(ui);
            if section(ui, "Appearance", true) {
                appearance_section(app, ui, &i);
            }
            divider(ui);
            if i.count > 1 && section(ui, "Align", true) {
                align_panel(app, ui);
                divider(ui);
            }
            if i.is_text {
                if section(ui, "Text Frame", true) {
                    text_frame_section(app, ui);
                }
                divider(ui);
            }
            if i.is_graphic {
                if section(ui, "Frame Fitting", true) {
                    ui.horizontal(|ui| {
                        for (icon, mode, tip) in [
                            ("fit-fill", "fillProportionally", "Fill Frame Proportionally"),
                            ("fit-prop", "fitProportionally", "Fit Content Proportionally"),
                            ("fit-content", "fitContentToFrame", "Fit Content to Frame"),
                            ("fit-frame", "fitFrameToContent", "Fit Frame to Content"),
                            ("fit-center", "centerContent", "Center Content"),
                        ] {
                            if icons::button(ui, icon, 24.0, false, tip).clicked() {
                                let _ = app.run("object.fit", json!({"mode": mode}));
                            }
                        }
                    });
                }
                divider(ui);
            }
            if section(ui, "Text Wrap", false) {
                wrap_panel(app, ui);
            }
            divider(ui);
            if section(ui, "Quick Actions", true) {
                ui.horizontal_wrapped(|ui| {
                    for (label, cmd) in [
                        ("Group", "object.group"),
                        ("Ungroup", "object.ungroup"),
                        ("Lock", "object.lock"),
                        ("Bring to Front", "object.bringToFront"),
                        ("Send to Back", "object.sendToBack"),
                    ] {
                        if ui.button(label).clicked() {
                            let _ = app.run(cmd, json!({}));
                        }
                    }
                    if i.is_text && ui.button("Fill with Placeholder Text").clicked() {
                        let _ = app.run("type.fillWithPlaceholder", json!({}));
                    }
                });
            }
        }
        None => document_section(app, ui),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn document_section(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let u = units(app);
    let Some(st) = app.session.active() else { return };
    let d = st.doc.clone();
    ui.label(egui::RichText::new("Document").color(t.text_dim));
    if section(ui, "Document", true) {
        egui::Grid::new("docgrid").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
            caption(ui, "W");
            if let Some(v) = measure(ui, "dw", Some(d.settings.page_width), u, 70.0) {
                let _ = app.run("layout.documentSetup", json!({"width": v}));
            }
            caption(ui, "H");
            if let Some(v) = measure(ui, "dh", Some(d.settings.page_height), u, 70.0) {
                let _ = app.run("layout.documentSetup", json!({"height": v}));
            }
            ui.end_row();
            caption(ui, "Pages");
            ui.label(d.page_count().to_string());
            caption(ui, "");
            let mut facing = d.settings.facing_pages;
            if ui.checkbox(&mut facing, "Facing").changed() {
                let _ = app.run("layout.documentSetup", json!({"facingPages": facing}));
            }
            ui.end_row();
        });
        if ui.button("Document Setup…").clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::new("documentSetup", json!({})));
        }
    }
    divider(ui);
    if section(ui, "Margins", true) {
        let p = d.page(crate::canvas::current_page(app).unwrap_or(0)).cloned();
        if let Some(p) = p {
            egui::Grid::new("margins").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
                for (row, (a, av, b, bv)) in
                    [("Top", p.margins.top, "Bottom", p.margins.bottom), ("Inside", p.margins.inside, "Outside", p.margins.outside)]
                        .into_iter()
                        .enumerate()
                {
                    caption(ui, a);
                    if let Some(v) = measure(ui, &format!("m{row}a"), Some(av), u, 64.0) {
                        let mut m = p.margins;
                        if row == 0 {
                            m.top = v
                        } else {
                            m.inside = v
                        }
                        let _ = app.run("layout.marginsAndColumns", json!({"margins": m}));
                    }
                    caption(ui, b);
                    if let Some(v) = measure(ui, &format!("m{row}b"), Some(bv), u, 64.0) {
                        let mut m = p.margins;
                        if row == 0 {
                            m.bottom = v
                        } else {
                            m.outside = v
                        }
                        let _ = app.run("layout.marginsAndColumns", json!({"margins": m}));
                    }
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                caption(ui, "Columns");
                if let Some(v) = number(ui, "cols", Some(p.columns.count as f64), "", 36.0, 0) {
                    let _ = app.run("layout.marginsAndColumns", json!({"columns": v.max(1.0) as u64}));
                }
                caption(ui, "Gutter");
                if let Some(v) = measure(ui, "gut", Some(p.columns.gutter), u, 56.0) {
                    let _ = app.run("layout.marginsAndColumns", json!({"gutter": v}));
                }
            });
        }
    }
    divider(ui);
    if section(ui, "Rulers & Grids", true) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.ui.rulers, "Rulers");
            ui.checkbox(&mut app.ui.baseline_grid, "Baseline grid");
        });
    }
    divider(ui);
    if section(ui, "Guides", true) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.ui.guides, "Guides");
            ui.checkbox(&mut app.ui.frame_edges, "Frame edges");
        });
    }
    divider(ui);
    if section(ui, "Quick Actions", true) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Insert Page").clicked() {
                let _ = app.run("layout.pages.insert", json!({"count": 1}));
            }
            if ui.button("Place…").clicked() {
                let _ = app.run("app.placeDialog", json!({}));
            }
            if ui.button("Export PNG…").clicked() {
                let _ = app.run("app.exportPng", json!({}));
            }
        });
    }
}

fn transform_section(app: &mut DesignApp, ui: &mut egui::Ui, i: &SelInfo) {
    let u = units(app);
    egui::Grid::new("xf").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
        caption(ui, "X");
        if let Some(v) = measure(ui, "px", Some(i.page_rect.x0), u, 70.0) {
            let _ = app.run("transform.set", json!({"x": v}));
        }
        caption(ui, "W");
        if let Some(v) = measure(ui, "pw", Some(i.page_rect.width()), u, 70.0) {
            let _ = app.run("transform.set", json!({"width": v}));
        }
        ui.end_row();
        caption(ui, "Y");
        if let Some(v) = measure(ui, "py", Some(i.page_rect.y0), u, 70.0) {
            let _ = app.run("transform.set", json!({"y": v}));
        }
        caption(ui, "H");
        if let Some(v) = measure(ui, "ph", Some(i.page_rect.height()), u, 70.0) {
            let _ = app.run("transform.set", json!({"height": v}));
        }
        ui.end_row();
    });
    ui.horizontal(|ui| {
        caption(ui, "Rotate");
        if let Some(v) = number(ui, "rot", Some(0.0), "°", 46.0, 1) {
            let _ = app.run("transform.rotate", json!({"angle": v}));
        }
        if ui.small_button("⇋").on_hover_text("Flip Horizontal").clicked() {
            let _ = app.run("transform.flip", json!({"axis": "horizontal"}));
        }
        if ui.small_button("⇵").on_hover_text("Flip Vertical").clicked() {
            let _ = app.run("transform.flip", json!({"axis": "vertical"}));
        }
    });
}

fn appearance_section(app: &mut DesignApp, ui: &mut egui::Ui, i: &SelInfo) {
    ui.horizontal(|ui| {
        caption(ui, "Fill   ");
        super::swatch_picker(app, ui, "pfill", Some(i.fill.clone()), |app, n| {
            let _ = app.run("object.fill", json!({"swatch": n}));
        });
        ui.label(&i.fill);
    });
    ui.horizontal(|ui| {
        caption(ui, "Stroke");
        super::swatch_picker(app, ui, "pstroke", Some(i.stroke.clone()), |app, n| {
            let _ = app.run("object.stroke", json!({"swatch": n}));
        });
        if let Some(v) = number(ui, "psw", Some(i.stroke_weight), " pt", 50.0, 3) {
            let _ = app.run("object.stroke", json!({"weight": v}));
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Opacity");
        let mut o = (i.opacity * 100.0) as f32;
        if ui.add(egui::Slider::new(&mut o, 0.0..=100.0).suffix("%").integer()).drag_stopped() {
            let _ = app.run("object.opacity", json!({"opacity": o / 100.0}));
        }
    });
    ui.horizontal(|ui| {
        let mut on = i.shadow;
        if ui.checkbox(&mut on, "Drop shadow").changed() {
            let _ = app.run("object.dropShadow", json!({"on": on}));
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Corners");
        for (label, shape) in [("None", "none"), ("Rounded", "rounded"), ("Bevel", "bevel"), ("Inset", "inset")] {
            if ui.small_button(label).clicked() {
                let _ = app.run("object.cornerOptions", json!({"shape": shape, "size": 12.0}));
            }
        }
    });
}

fn text_frame_section(app: &mut DesignApp, ui: &mut egui::Ui) {
    let u = units(app);
    let Some(st) = app.session.active() else { return };
    let fid = st.selection.items.first().copied().or_else(|| st.selection.text.and_then(|t| t.frame));
    let Some(opts) = fid.and_then(|f| st.doc.item(f)).and_then(|i| i.text_frame()).map(|t| t.options.clone()) else { return };
    ui.horizontal(|ui| {
        caption(ui, "Columns");
        if let Some(v) = number(ui, "tfc", Some(opts.columns as f64), "", 36.0, 0) {
            let _ = app.run("object.textFrameOptions", json!({"columns": v.max(1.0) as u64}));
        }
        caption(ui, "Gutter");
        if let Some(v) = measure(ui, "tfg", Some(opts.gutter), u, 56.0) {
            let _ = app.run("object.textFrameOptions", json!({"gutter": v}));
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Inset");
        if let Some(v) = measure(ui, "tfi", Some(opts.inset[0]), u, 56.0) {
            let _ = app.run("object.textFrameOptions", json!({"inset": v}));
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Align");
        for (label, vj) in [("Top", "top"), ("Center", "center"), ("Bottom", "bottom"), ("Justify", "justify")] {
            let on = serde_json::to_value(opts.vertical_justification).ok().and_then(|v| v.as_str().map(|s| s == vj)).unwrap_or(false);
            if ui.selectable_label(on, label).clicked() {
                let _ = app.run("object.textFrameOptions", json!({"verticalJustification": vj}));
            }
        }
    });
    if ui.button("Text Frame Options…").clicked() {
        app.ui.dialog = Some(crate::dialogs::Dialog::new("textFrameOptions", json!({})));
    }
}

pub fn character_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let Some(a) = text_attrs(app) else {
        ui.label("Select text or a text frame.");
        return;
    };
    let c = a["chars"].clone();
    let fam = c["fontFamily"].as_str().unwrap_or("").to_string();
    let sty = c["fontStyle"].as_str().unwrap_or("").to_string();
    ui.label(egui::RichText::new("Character").strong());
    super::font_family_picker(app, ui, &fam, 220.0);
    super::font_style_picker(app, ui, &fam, &sty, 220.0);
    egui::Grid::new("chargrid").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
        caption(ui, "Size");
        if let Some(v) = number(ui, "pcs", c["size"].as_f64(), " pt", 60.0, 2) {
            let _ = app.run("type.char", json!({"attrs": {"size": v}}));
        }
        caption(ui, "Leading");
        let auto = c["leading"]["kind"].as_str() != Some("points");
        let lv = if auto { c["size"].as_f64().map(|s| s * a["para"]["autoLeading"].as_f64().unwrap_or(1.2)) } else { c["leading"]["value"].as_f64() };
        if let Some(v) = number(ui, "pcl", lv, if auto { " (auto)" } else { " pt" }, 70.0, 2) {
            let _ = app.run("type.char", json!({"attrs": {"leading": {"kind": "points", "value": v}}}));
        }
        ui.end_row();
        caption(ui, "Tracking");
        if let Some(v) = number(ui, "pct", c["tracking"].as_f64(), "", 60.0, 0) {
            let _ = app.run("type.char", json!({"attrs": {"tracking": v}}));
        }
        caption(ui, "Baseline");
        if let Some(v) = number(ui, "pcb", c["baselineShift"].as_f64(), " pt", 70.0, 2) {
            let _ = app.run("type.char", json!({"attrs": {"baselineShift": v}}));
        }
        ui.end_row();
    });
    ui.horizontal(|ui| {
        caption(ui, "Color");
        super::swatch_picker(app, ui, "tfill", c["fill"].as_str().map(str::to_string), |app, n| {
            let _ = app.run("type.char", json!({"attrs": {"fill": n}}));
        });
        ui.label(c["fill"].as_str().unwrap_or(""));
    });
    ui.horizontal(|ui| {
        let caps = c["capitalization"].as_str() == Some("allCaps");
        if ui.selectable_label(caps, "TT").on_hover_text("All Caps").clicked() {
            let _ = app.run("type.char", json!({"attrs": {"capitalization": if caps { "normal" } else { "allCaps" }}}));
        }
        let ul = c["underline"].as_bool().unwrap_or(false);
        if ui.selectable_label(ul, "U").on_hover_text("Underline").clicked() {
            let _ = app.run("type.char", json!({"attrs": {"underline": !ul}}));
        }
        let st = c["strikethrough"].as_bool().unwrap_or(false);
        if ui.selectable_label(st, "S").on_hover_text("Strikethrough").clicked() {
            let _ = app.run("type.char", json!({"attrs": {"strikethrough": !st}}));
        }
        let sup = c["position"].as_str() == Some("superscript");
        if ui.selectable_label(sup, "T¹").on_hover_text("Superscript").clicked() {
            let _ = app.run("type.char", json!({"attrs": {"position": if sup { "normal" } else { "superscript" }}}));
        }
    });
}

pub fn paragraph_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let Some(a) = text_attrs(app) else {
        ui.label("Select text or a text frame.");
        return;
    };
    let u = units(app);
    let p = a["para"].clone();
    ui.label(egui::RichText::new("Paragraph").strong());
    let cur: Align = serde_json::from_value(p["align"].clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        for (al, icon) in [
            (Align::Left, "align-left"),
            (Align::Center, "align-center"),
            (Align::Right, "align-right"),
            (Align::LeftJustified, "align-justify"),
            (Align::FullyJustified, "align-justify-all"),
        ] {
            if icons::button(ui, icon, 24.0, cur == al, al.label()).clicked() {
                let _ = app.run("type.para", json!({"attrs": {"align": al}}));
            }
        }
    });
    egui::Grid::new("paragrid").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
        for (row, ((la, ka), (lb, kb))) in [
            (("Left", "leftIndent"), ("Right", "rightIndent")),
            (("First", "firstLineIndent"), ("Last", "lastLineIndent")),
            (("Before", "spaceBefore"), ("After", "spaceAfter")),
        ]
        .into_iter()
        .enumerate()
        {
            caption(ui, la);
            if let Some(v) = measure(ui, &format!("pp{row}a"), p[ka].as_f64(), u, 64.0) {
                let _ = app.run("type.para", json!({"attrs": {ka: v}}));
            }
            caption(ui, lb);
            if let Some(v) = measure(ui, &format!("pp{row}b"), p[kb].as_f64(), u, 64.0) {
                let _ = app.run("type.para", json!({"attrs": {kb: v}}));
            }
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        let mut h = p["hyphenate"].as_bool().unwrap_or(true);
        if ui.checkbox(&mut h, "Hyphenate").changed() {
            let _ = app.run("type.para", json!({"attrs": {"hyphenate": h}}));
        }
        let mut grid = p["gridAlign"].as_str() == Some("allLines");
        if ui.checkbox(&mut grid, "Align to baseline grid").changed() {
            let _ = app.run("type.para", json!({"attrs": {"gridAlign": if grid { "allLines" } else { "none" }}}));
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Composer");
        let single = p["composer"].as_str() == Some("singleLine");
        if ui.selectable_label(!single, "Paragraph").clicked() {
            let _ = app.run("type.para", json!({"attrs": {"composer": "paragraph"}}));
        }
        if ui.selectable_label(single, "Single-line").clicked() {
            let _ = app.run("type.para", json!({"attrs": {"composer": "singleLine"}}));
        }
    });
    let ps = a["paragraphStyle"].as_str().unwrap_or("").to_string();
    let ov = a["paraOverrides"].as_u64().unwrap_or(0) + a["charOverrides"].as_u64().unwrap_or(0);
    ui.horizontal(|ui| {
        caption(ui, "Style");
        super::para_style_picker(app, ui, &ps, ov > 0, 190.0);
    });
}

pub fn stroke_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let Some(i) = sel_info(app) else {
        ui.label("Select an object.");
        return;
    };
    ui.horizontal(|ui| {
        caption(ui, "Weight");
        if let Some(v) = number(ui, "sw", Some(i.stroke_weight), " pt", 60.0, 3) {
            let _ = app.run("object.stroke", json!({"weight": v}));
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Align Stroke");
        for (label, a) in [("Center", "center"), ("Inside", "inside"), ("Outside", "outside")] {
            let on = serde_json::to_value(i.stroke_align).ok().and_then(|v| v.as_str().map(|s| s == a)).unwrap_or(false);
            if ui.selectable_label(on, label).clicked() {
                let _ = app.run("object.stroke", json!({"align": a}));
            }
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Type");
        for (label, ty) in [
            ("Solid", json!({"kind": "solid"})),
            ("Dashed", json!({"kind": "dashed", "pattern": [12.0, 4.0]})),
            ("Dotted", json!({"kind": "dotted"})),
        ] {
            if ui.small_button(label).clicked() {
                let _ = app.run("object.stroke", json!({"type": ty}));
            }
        }
    });
    ui.horizontal(|ui| {
        caption(ui, "Color");
        super::swatch_picker(app, ui, "spick", Some(i.stroke.clone()), |app, n| {
            let _ = app.run("object.stroke", json!({"swatch": n}));
        });
    });
}

pub fn wrap_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let info = sel_info(app);
    ui.horizontal(|ui| {
        for (icon, mode, tip) in [
            ("wrap-none", "none", "No Text Wrap"),
            ("wrap-bbox", "boundingBox", "Wrap Around Bounding Box"),
            ("wrap-contour", "contour", "Wrap Around Object Shape"),
            ("wrap-jump", "jumpObject", "Jump Object"),
            ("wrap-next", "jumpToNextColumn", "Jump to Next Column"),
        ] {
            let on = info.as_ref().is_some_and(|i| i.wrap == mode);
            if icons::button(ui, icon, 26.0, on, tip).clicked() {
                let _ = app.run("object.textWrap", json!({"mode": mode}));
            }
        }
    });
    let u = units(app);
    ui.horizontal(|ui| {
        caption(ui, "Offset");
        if let Some(v) = measure(ui, "wo", Some(0.0), u, 60.0) {
            let _ = app.run("object.textWrap", json!({"mode": info.as_ref().map(|i| i.wrap).unwrap_or("boundingBox"), "offset": v}));
        }
    });
}

pub fn align_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (edge, tip) in [
            ("left", "Align left edges"),
            ("hcenter", "Align horizontal centers"),
            ("right", "Align right edges"),
            ("top", "Align top edges"),
            ("vcenter", "Align vertical centers"),
            ("bottom", "Align bottom edges"),
        ] {
            if ui
                .small_button(match edge {
                    "left" => "⇤",
                    "hcenter" => "↔",
                    "right" => "⇥",
                    "top" => "⤒",
                    "vcenter" => "↕",
                    _ => "⤓",
                })
                .on_hover_text(tip)
                .clicked()
            {
                let _ = app.run("object.align", json!({"edge": edge}));
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.small_button("Distribute ↔").clicked() {
            let _ = app.run("object.distribute", json!({"axis": "horizontal"}));
        }
        if ui.small_button("Distribute ↕").clicked() {
            let _ = app.run("object.distribute", json!({"axis": "vertical"}));
        }
    });
}

pub fn links_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let Some(st) = app.session.active() else { return };
    let t = Tokens::get(ui.ctx());
    let assets: Vec<(String, Option<(u32, u32)>, bool)> = st.doc.assets.values().map(|a| (a.name.clone(), a.pixels, a.link.is_some())).collect();
    if assets.is_empty() {
        ui.label(egui::RichText::new("No placed graphics.").color(t.text_dim));
    }
    for (name, px, linked) in assets {
        ui.horizontal(|ui| {
            ui.label(&name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(if linked { "Linked" } else { "Embedded" }).color(t.text_dim).size(11.0));
                if let Some((w, h)) = px {
                    ui.label(egui::RichText::new(format!("{w}×{h}")).color(t.text_dim).size(11.0));
                }
            });
        });
    }
}

pub fn info_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let u = units(app);
    let t = Tokens::get(ui.ctx());
    match sel_info(app) {
        Some(i) => {
            let f = |v: f64| designcraft_geom::format_measure(v, u);
            ui.label(format!("X: {}   Y: {}", f(i.page_rect.x0), f(i.page_rect.y0)));
            ui.label(format!("W: {}   H: {}", f(i.page_rect.width()), f(i.page_rect.height())));
            ui.label(format!("Fill: {}   Stroke: {}", i.fill, i.stroke));
        }
        None => {
            if let Some(st) = app.session.active() {
                ui.label(format!("{} pages · {} stories · {} items", st.doc.page_count(), st.doc.stories.len(), st.doc.all_items().len()));
            }
        }
    }
    ui.label(egui::RichText::new(format!("Render {:.1} ms", app.perf.render_ms)).color(t.text_dim));
}

pub fn effects_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let Some(st) = app.session.active() else { return };
    let Some(it) = st.selection.items.first().and_then(|i| st.doc.item(*i)).cloned() else {
        ui.label("Select an object.");
        return;
    };
    use designcraft_color::BlendMode as B;
    const MODES: [B; 16] = [
        B::Normal,
        B::Multiply,
        B::Screen,
        B::Overlay,
        B::SoftLight,
        B::HardLight,
        B::ColorDodge,
        B::ColorBurn,
        B::Darken,
        B::Lighten,
        B::Difference,
        B::Exclusion,
        B::Hue,
        B::Saturation,
        B::Color,
        B::Luminosity,
    ];
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("blend").selected_text(it.blend.label()).width(130.0).show_ui(ui, |ui| {
            for m in MODES {
                if ui.selectable_label(m == it.blend, m.label()).clicked() {
                    let _ = app.run("object.opacity", json!({"opacity": it.opacity, "blend": m}));
                }
            }
        });
        caption(ui, "Opacity");
        if let Some(v) = number(ui, "eop", Some(it.opacity as f64 * 100.0), "%", 46.0, 0) {
            let _ = app.run("object.opacity", json!({"opacity": (v / 100.0).clamp(0.0, 1.0)}));
        }
    });
    divider(ui);
    let ds = it.effects.drop_shadow.clone();
    let mut on = ds.on;
    if ui.checkbox(&mut on, "Drop Shadow").changed() {
        let _ = app.run("object.dropShadow", json!({"on": on}));
    }
    if ds.on {
        egui::Grid::new("dsg").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
            caption(ui, "Distance");
            if let Some(v) = number(ui, "dsd", Some(ds.distance), " pt", 56.0, 1) {
                let _ = app.run("object.dropShadow", json!({"on": true, "distance": v}));
            }
            caption(ui, "Angle");
            if let Some(v) = number(ui, "dsa", Some(ds.angle), "°", 50.0, 0) {
                let _ = app.run("object.dropShadow", json!({"on": true, "angle": v}));
            }
            ui.end_row();
            caption(ui, "Opacity");
            if let Some(v) = number(ui, "dso", Some(ds.opacity as f64 * 100.0), "%", 56.0, 0) {
                let _ = app.run("object.dropShadow", json!({"on": true, "opacity": v / 100.0}));
            }
            caption(ui, "Size");
            if let Some(v) = number(ui, "dss", Some(ds.size), " pt", 50.0, 1) {
                let _ = app.run("object.dropShadow", json!({"on": true, "size": v}));
            }
            ui.end_row();
        });
    }
}
