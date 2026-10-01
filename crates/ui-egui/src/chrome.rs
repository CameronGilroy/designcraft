//! Window chrome: application bar (with menus), Control panel, document tabs, status bar, start screen.

use designcraft_doc::{Align, Content};
use designcraft_geom::Unit;
use egui::{Color32, Sense, Stroke, vec2};
use serde_json::json;

use crate::panels::{self, SelInfo};
use crate::theme::{Tokens, semibold};
use crate::widgets::{caption, measure, number};
use crate::{DesignApp, ScreenMode, icons};

pub fn app_bar(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let left = if app.integrated_titlebar { 78 } else { 8 };
    egui::Panel::top("app_bar")
        .exact_size(36.0)
        .frame(egui::Frame::NONE.fill(t.app_bar).inner_margin(egui::Margin { left, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                // Brand mark: our own "Dc" tile.
                let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                ui.painter().rect_filled(r, 5.0, Color32::from_rgb(214, 44, 104));
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "Dc", semibold(12.5), Color32::WHITE);
                ui.add_space(4.0);
                if icons::button(ui, "home", 24.0, app.session.active().is_none(), "Home").clicked() {
                    app.session_home();
                }
                ui.add_space(4.0);
                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
                ui.style_mut().visuals.widgets.hovered.bg_stroke = Stroke::NONE;
                ui.style_mut().spacing.button_padding = vec2(7.0, 3.0);
                crate::menus::menu_bar(app, ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    icons::button(ui, "search", 22.0, false, "Search");
                    ui.menu_button(egui::RichText::new("Essentials ▾").color(t.text), |ui| {
                        for w in ["Essentials", "Advanced", "Book", "Digital Publishing", "Printing and Proofing", "Typography"] {
                            if ui.button(w).clicked() {
                                ui.close();
                            }
                        }
                    });
                    if ui
                        .add(egui::Button::new(egui::RichText::new("Share").color(Color32::WHITE)).fill(t.accent_strong).corner_radius(12.0))
                        .clicked()
                    {
                        app.status("Sharing is not part of DesignCraft — export a PDF or package instead.");
                    }
                    ui.add_space(8.0);
                    // Screen mode.
                    let mode = app.ui.screen_mode;
                    for (m, icon, tip) in [(ScreenMode::Preview, "screen-preview", "Preview (W)"), (ScreenMode::Normal, "screen-normal", "Normal")] {
                        if icons::button(ui, icon, 22.0, mode == m, tip).clicked() {
                            app.ui.screen_mode = m;
                        }
                    }
                    ui.add_space(6.0);
                    // Zoom dropdown.
                    let z = app.view().map(|v| v.zoom).unwrap_or(1.0);
                    ui.menu_button(egui::RichText::new(format!("{:.0}% ▾", z * 100.0)).color(t.text), |ui| {
                        for p in [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0] {
                            if ui.button(format!("{:.0}%", p * 100.0)).clicked() {
                                let _ = app.run("view.zoom", json!({"zoom": p}));
                                ui.close();
                            }
                        }
                    });
                });
            });
        });
}

impl DesignApp {
    fn session_home(&mut self) {
        self.ui.status = "Home".into();
    }
}

fn vsep(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 46.0), Sense::hover());
    ui.painter().line_segment([r.center_top() + vec2(0.0, 4.0), r.center_bottom() - vec2(0.0, 4.0)], Stroke::new(1.0, t.divider));
}

/// Two-row context-sensitive Control panel.
pub fn control_bar(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("control_bar")
        .exact_size(64.0)
        .frame(
            egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 8, right: 8, top: 5, bottom: 3 }).stroke(Stroke::new(1.0, t.divider)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let text_mode = app.session.tool_id() == "type" || app.session.active().is_some_and(|d| d.selection.text.is_some());
                if text_mode {
                    control_text(app, ui);
                } else {
                    control_object(app, ui);
                }
            });
        });
}

fn control_object(app: &mut DesignApp, ui: &mut egui::Ui) {
    let units = app.session.active().map(|d| d.doc.settings.horizontal_units).unwrap_or(Unit::Picas);
    let info = panels::sel_info(app);
    // Reference point proxy.
    let (r, _) = ui.allocate_exact_size(vec2(30.0, 44.0), Sense::hover());
    let t = Tokens::get(ui.ctx());
    icons::paint(ui.painter(), egui::Rect::from_center_size(r.center(), vec2(26.0, 26.0)), "ref-point", t.icon);
    ui.painter().rect_filled(egui::Rect::from_center_size(r.center() + vec2(-7.8, -7.8), vec2(4.0, 4.0)), 0.0, t.text_strong);
    let (x, y, w, h) = match &info {
        Some(SelInfo { page_rect, .. }) => (Some(page_rect.x0), Some(page_rect.y0), Some(page_rect.width()), Some(page_rect.height())),
        None => (None, None, None, None),
    };
    egui::Grid::new("ctl_xywh").num_columns(4).spacing(vec2(4.0, 4.0)).show(ui, |ui| {
        caption(ui, "X:");
        if let Some(v) = measure(ui, "cx", x, units, 64.0) {
            let _ = app.run("transform.set", json!({"x": v}));
        }
        caption(ui, "W:");
        if let Some(v) = measure(ui, "cw", w, units, 64.0) {
            let _ = app.run("transform.set", json!({"width": v}));
        }
        ui.end_row();
        caption(ui, "Y:");
        if let Some(v) = measure(ui, "cy", y, units, 64.0) {
            let _ = app.run("transform.set", json!({"y": v}));
        }
        caption(ui, "H:");
        if let Some(v) = measure(ui, "ch", h, units, 64.0) {
            let _ = app.run("transform.set", json!({"height": v}));
        }
        ui.end_row();
    });
    vsep(ui);
    // Rotate / flip.
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            if icons::button(ui, "tool-rotate", 20.0, false, "Rotate 90° Counterclockwise").clicked() {
                let _ = app.run("transform.rotate", json!({"angle": 90}));
            }
            if ui.small_button("⇋").on_hover_text("Flip Horizontal").clicked() {
                let _ = app.run("transform.flip", json!({"axis": "horizontal"}));
            }
        });
        ui.horizontal(|ui| {
            if ui.small_button("↻").on_hover_text("Rotate 90° Clockwise").clicked() {
                let _ = app.run("transform.rotate", json!({"angle": -90}));
            }
            if ui.small_button("⇵").on_hover_text("Flip Vertical").clicked() {
                let _ = app.run("transform.flip", json!({"axis": "vertical"}));
            }
        });
    });
    vsep(ui);
    // Fill / stroke.
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            caption(ui, "Fill");
            panels::swatch_picker(app, ui, "ctlfill", info.as_ref().map(|i| i.fill.clone()), |app, name| {
                let _ = app.run("object.fill", json!({"swatch": name}));
            });
        });
        ui.horizontal(|ui| {
            caption(ui, "Stroke");
            panels::swatch_picker(app, ui, "ctlstroke", info.as_ref().map(|i| i.stroke.clone()), |app, name| {
                let _ = app.run("object.stroke", json!({"swatch": name}));
            });
            if let Some(v) = number(ui, "csw", info.as_ref().map(|i| i.stroke_weight), " pt", 46.0, 3) {
                let _ = app.run("object.stroke", json!({"weight": v}));
            }
        });
    });
    vsep(ui);
    // Opacity, fx.
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            caption(ui, "Opacity");
            if let Some(v) = number(ui, "cop", info.as_ref().map(|i| i.opacity * 100.0), "%", 42.0, 0) {
                let _ = app.run("object.opacity", json!({"opacity": v / 100.0}));
            }
        });
        ui.horizontal(|ui| {
            if icons::button(ui, "panel-effects", 20.0, info.as_ref().is_some_and(|i| i.shadow), "Drop Shadow").clicked() {
                let _ = app.run("object.dropShadow", json!({}));
            }
        });
    });
    vsep(ui);
    // Fitting.
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            for (icon, mode, tip) in [
                ("fit-fill", "fillProportionally", "Fill Frame Proportionally"),
                ("fit-prop", "fitProportionally", "Fit Content Proportionally"),
                ("fit-content", "fitContentToFrame", "Fit Content to Frame"),
            ] {
                if icons::button(ui, icon, 20.0, false, tip).clicked() {
                    let _ = app.run("object.fit", json!({"mode": mode}));
                }
            }
        });
        ui.horizontal(|ui| {
            for (icon, mode, tip) in [("fit-frame", "fitFrameToContent", "Fit Frame to Content"), ("fit-center", "centerContent", "Center Content")] {
                if icons::button(ui, icon, 20.0, false, tip).clicked() {
                    let _ = app.run("object.fit", json!({"mode": mode}));
                }
            }
        });
    });
    vsep(ui);
    // Text wrap.
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            for (icon, mode, tip) in [
                ("wrap-none", "none", "No Text Wrap"),
                ("wrap-bbox", "boundingBox", "Wrap Around Bounding Box"),
                ("wrap-contour", "contour", "Wrap Around Object Shape"),
            ] {
                let on = info.as_ref().is_some_and(|i| i.wrap == mode);
                if icons::button(ui, icon, 20.0, on, tip).clicked() {
                    let _ = app.run("object.textWrap", json!({"mode": mode}));
                }
            }
        });
        ui.horizontal(|ui| {
            for (icon, mode, tip) in [("wrap-jump", "jumpObject", "Jump Object"), ("wrap-next", "jumpToNextColumn", "Jump to Next Column")] {
                let on = info.as_ref().is_some_and(|i| i.wrap == mode);
                if icons::button(ui, icon, 20.0, on, tip).clicked() {
                    let _ = app.run("object.textWrap", json!({"mode": mode}));
                }
            }
        });
    });
    if let Some(i) = &info
        && let Some(cols) = i.columns
    {
        vsep(ui);
        ui.vertical(|ui| {
            caption(ui, "Columns");
            if let Some(v) = number(ui, "ccols", Some(cols as f64), "", 36.0, 0) {
                let _ = app.run("object.textFrameOptions", json!({"columns": v.max(1.0) as u64}));
            }
        });
    }
}

fn control_text(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let attrs = panels::text_attrs(app);
    let chars = attrs.as_ref().map(|a| a["chars"].clone()).unwrap_or_default();
    let para = attrs.as_ref().map(|a| a["para"].clone()).unwrap_or_default();
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("A").font(semibold(15.0)).color(t.text_strong));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("¶").font(semibold(15.0)).color(t.text_dim));
        });
    });
    vsep(ui);
    ui.vertical(|ui| {
        let fam = chars["fontFamily"].as_str().unwrap_or("").to_string();
        let sty = chars["fontStyle"].as_str().unwrap_or("").to_string();
        panels::font_family_picker(app, ui, &fam, 170.0);
        panels::font_style_picker(app, ui, &fam, &sty, 170.0);
    });
    vsep(ui);
    egui::Grid::new("ctl_char").num_columns(4).spacing(vec2(4.0, 3.0)).show(ui, |ui| {
        caption(ui, "𝐓");
        if let Some(v) = number(ui, "csize", chars["size"].as_f64(), " pt", 54.0, 2) {
            let _ = app.run("type.char", json!({"attrs": {"size": v}}));
        }
        caption(ui, "VA");
        if let Some(v) = number(ui, "ctrack", chars["tracking"].as_f64(), "", 48.0, 0) {
            let _ = app.run("type.char", json!({"attrs": {"tracking": v}}));
        }
        ui.end_row();
        caption(ui, "Ā");
        let lead = match chars["leading"]["kind"].as_str() {
            Some("points") => chars["leading"]["value"].as_f64(),
            _ => chars["size"].as_f64().map(|s| s * para["autoLeading"].as_f64().unwrap_or(1.2)),
        };
        if let Some(v) = number(ui, "clead", lead, " pt", 54.0, 2) {
            let _ = app.run("type.char", json!({"attrs": {"leading": {"kind": "points", "value": v}}}));
        }
        caption(ui, "IT");
        if let Some(v) = number(ui, "chs", chars["hScale"].as_f64().map(|v| v * 100.0), "%", 48.0, 0) {
            let _ = app.run("type.char", json!({"attrs": {"hScale": v / 100.0}}));
        }
        ui.end_row();
    });
    vsep(ui);
    ui.vertical(|ui| {
        let cur: Align = serde_json::from_value(para["align"].clone()).unwrap_or_default();
        ui.horizontal(|ui| {
            for (a, icon) in [(Align::Left, "align-left"), (Align::Center, "align-center"), (Align::Right, "align-right")] {
                if icons::button(ui, icon, 20.0, cur == a, a.label()).clicked() {
                    let _ = app.run("type.para", json!({"attrs": {"align": a}}));
                }
            }
        });
        ui.horizontal(|ui| {
            for (a, icon) in [(Align::LeftJustified, "align-justify"), (Align::FullyJustified, "align-justify-all")] {
                if icons::button(ui, icon, 20.0, cur == a, a.label()).clicked() {
                    let _ = app.run("type.para", json!({"attrs": {"align": a}}));
                }
            }
        });
    });
    vsep(ui);
    let units = app.session.active().map(|d| d.doc.settings.horizontal_units).unwrap_or(Unit::Picas);
    egui::Grid::new("ctl_para").num_columns(4).spacing(vec2(4.0, 3.0)).show(ui, |ui| {
        caption(ui, "→|");
        if let Some(v) = measure(ui, "cli", para["leftIndent"].as_f64(), units, 54.0) {
            let _ = app.run("type.para", json!({"attrs": {"leftIndent": v}}));
        }
        caption(ui, "↑¶");
        if let Some(v) = measure(ui, "csb", para["spaceBefore"].as_f64(), units, 54.0) {
            let _ = app.run("type.para", json!({"attrs": {"spaceBefore": v}}));
        }
        ui.end_row();
        caption(ui, "→¶");
        if let Some(v) = measure(ui, "cfi", para["firstLineIndent"].as_f64(), units, 54.0) {
            let _ = app.run("type.para", json!({"attrs": {"firstLineIndent": v}}));
        }
        caption(ui, "↓¶");
        if let Some(v) = measure(ui, "csa", para["spaceAfter"].as_f64(), units, 54.0) {
            let _ = app.run("type.para", json!({"attrs": {"spaceAfter": v}}));
        }
        ui.end_row();
    });
    vsep(ui);
    ui.vertical(|ui| {
        let ps = attrs.as_ref().and_then(|a| a["paragraphStyle"].as_str()).unwrap_or("").to_string();
        let ov = attrs.as_ref().and_then(|a| a["paraOverrides"].as_u64()).unwrap_or(0)
            + attrs.as_ref().and_then(|a| a["charOverrides"].as_u64()).unwrap_or(0);
        panels::para_style_picker(app, ui, &ps, ov > 0, 170.0);
        let hy = para["hyphenate"].as_bool().unwrap_or(true);
        let mut h = hy;
        if ui.checkbox(&mut h, "Hyphenate").changed() {
            let _ = app.run("type.para", json!({"attrs": {"hyphenate": h}}));
        }
    });
}

pub fn doc_tabs(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().rect_filled(bar, 0.0, t.panel_darker);
    let mut x = bar.min.x;
    let active = app.session.active_index();
    let zoom = app.view().map(|v| v.zoom).unwrap_or(1.0);
    let mut activate = None;
    let mut close = None;
    let titles: Vec<(String, bool)> = app.session.documents().iter().map(|d| (d.title(), d.is_dirty())).collect();
    for (i, (title, dirty)) in titles.iter().enumerate() {
        let is_active = Some(i) == active;
        let label = if is_active {
            format!("{}{} @ {:.0}%", if *dirty { "*" } else { "" }, title, zoom * 100.0)
        } else {
            format!("{}{}", if *dirty { "*" } else { "" }, title)
        };
        let galley = ui.painter().layout_no_wrap(label, egui::FontId::proportional(12.0), if is_active { t.text_strong } else { t.text_dim });
        let w = galley.size().x + 44.0;
        let r = egui::Rect::from_min_size(egui::pos2(x, bar.min.y), vec2(w, 26.0));
        let resp = ui.interact(r, ui.id().with(("doctab", i)), Sense::click());
        ui.painter().rect_filled(r, 0.0, if is_active { t.pasteboard } else { t.tab_inactive });
        if is_active {
            ui.painter().line_segment([r.left_top(), r.right_top()], Stroke::new(2.0, t.accent));
        }
        let cr = egui::Rect::from_center_size(egui::pos2(r.min.x + 13.0, r.center().y), vec2(14.0, 14.0));
        let cresp = ui.interact(cr, ui.id().with(("docclose", i)), Sense::click());
        ui.painter().text(
            cr.center(),
            egui::Align2::CENTER_CENTER,
            "×",
            egui::FontId::proportional(14.0),
            if cresp.hovered() { t.text_strong } else { t.text_dim },
        );
        ui.painter().galley(egui::pos2(r.min.x + 26.0, r.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
        ui.painter().line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, t.divider));
        if cresp.clicked() {
            close = Some(i);
        } else if resp.clicked() {
            activate = Some(i);
        }
        x += w;
    }
    if let Some(i) = close {
        let _ = app.run("file.close", json!({"index": i}));
    } else if let Some(i) = activate {
        let _ = app.run("file.activate", json!({"index": i}));
    }
}

pub fn status_bar(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("status_bar")
        .exact_size(24.0)
        .frame(egui::Frame::NONE.fill(t.panel_darker).inner_margin(egui::Margin::symmetric(8, 0)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                if app.session.active().is_none() {
                    ui.label(egui::RichText::new(&app.ui.status).size(11.5).color(t.text_dim));
                    return;
                }
                let z = app.view().map(|v| v.zoom).unwrap_or(1.0);
                ui.label(egui::RichText::new(format!("{:.0}%", z * 100.0)).size(11.5));
                ui.add_space(8.0);
                let n = app.session.active().map(|d| d.doc.page_count()).unwrap_or(1);
                let cur = crate::canvas::current_page(app).unwrap_or(0);
                if icons::button(ui, "first", 16.0, false, "First Spread").clicked() {
                    crate::canvas::go_to_page(app, 0);
                }
                if icons::button(ui, "prev", 16.0, false, "Previous Spread").clicked() {
                    crate::canvas::go_to_page(app, cur.saturating_sub(1));
                }
                let name = app.session.active().map(|d| d.doc.page_name(cur)).unwrap_or_default();
                ui.menu_button(egui::RichText::new(format!(" {name} ▾ ")).size(11.5), |ui| {
                    let names: Vec<String> = app.session.active().map(|d| (0..n).map(|i| d.doc.page_name(i)).collect()).unwrap_or_default();
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for (i, nm) in names.iter().enumerate() {
                            if ui.button(nm).clicked() {
                                crate::canvas::go_to_page(app, i);
                                ui.close();
                            }
                        }
                    });
                });
                if icons::button(ui, "next", 16.0, false, "Next Spread").clicked() {
                    crate::canvas::go_to_page(app, (cur + 1).min(n - 1));
                }
                if icons::button(ui, "last", 16.0, false, "Last Spread").clicked() {
                    crate::canvas::go_to_page(app, n - 1);
                }
                ui.add_space(14.0);
                // Preflight indicator: overset text is an error.
                let errors = panels::preflight_errors(app);
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(
                    r.center(),
                    4.5,
                    if errors == 0 { Color32::from_rgb(60, 190, 90) } else { Color32::from_rgb(230, 50, 50) },
                );
                ui.label(
                    egui::RichText::new(if errors == 0 {
                        "No errors".to_string()
                    } else {
                        format!("{errors} error{}", if errors == 1 { "" } else { "s" })
                    })
                    .size(11.5),
                );
                ui.add_space(14.0);
                ui.label(egui::RichText::new(&app.ui.status).size(11.5).color(t.text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{:.1} ms render · {:.0} fps", app.perf.render_ms, app.perf.fps))
                            .size(10.5)
                            .color(t.text_disabled),
                    );
                });
            });
        });
}

pub fn start_screen(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let r = ui.available_rect_before_wrap();
    ui.painter().rect_filled(r, 0.0, t.panel_darker);
    ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(48.0)), |ui| {
        ui.label(egui::RichText::new("Welcome to DesignCraft").font(semibold(28.0)).color(t.text_strong));
        ui.label(egui::RichText::new("Page layout for print and screen — fast, open and scriptable.").size(15.0).color(t.text_dim));
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("  New file  ").size(14.0).color(Color32::WHITE))
                        .fill(t.accent_strong)
                        .corner_radius(16.0)
                        .min_size(vec2(0.0, 32.0)),
                )
                .clicked()
            {
                let _ = app.run("app.newDocumentDialog", json!({}));
            }
            if ui.add(egui::Button::new(egui::RichText::new("  Open  ").size(14.0)).corner_radius(16.0).min_size(vec2(0.0, 32.0))).clicked() {
                let _ = app.run("app.openDialog", json!({}));
            }
            if ui
                .add(egui::Button::new(egui::RichText::new("  Open sample magazine  ").size(14.0)).corner_radius(16.0).min_size(vec2(0.0, 32.0)))
                .clicked()
            {
                let _ = app.run("file.newSample", json!({}));
            }
        });
        ui.add_space(28.0);
        ui.label(egui::RichText::new("Start a new document").font(semibold(15.0)).color(t.text_strong));
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for p in designcraft_doc::build::PRESETS.iter().take(10) {
                let (cr, resp) = ui.allocate_exact_size(vec2(120.0, 150.0), Sense::click());
                let hov = resp.hovered();
                ui.painter().rect_filled(cr, 6.0, if hov { t.hover } else { t.panel });
                let k = (80.0 / p.width.max(p.height)) as f32;
                let pr = egui::Rect::from_center_size(cr.center() - vec2(0.0, 14.0), vec2(p.width as f32 * k, p.height as f32 * k));
                ui.painter().rect_filled(pr, 0.0, Color32::from_gray(245));
                ui.painter().text(egui::pos2(cr.center().x, cr.max.y - 26.0), egui::Align2::CENTER_CENTER, p.name, semibold(12.0), t.text_strong);
                ui.painter().text(
                    egui::pos2(cr.center().x, cr.max.y - 11.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{} × {}", designcraft_geom::format_measure(p.width, p.units), designcraft_geom::format_measure(p.height, p.units)),
                    egui::FontId::proportional(10.0),
                    t.text_dim,
                );
                if resp.clicked() {
                    let _ = app.run("file.new", json!({"preset": p.name}));
                }
            }
        });
    });
    let _ = Content::Unassigned;
}
