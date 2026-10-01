//! The Tools panel (left): single or double column, tool groups with flyouts, fill/stroke proxy,
//! screen mode buttons.

use designcraft_tools::TOOL_GROUPS;
use egui::{Color32, Sense, Stroke, StrokeKind, vec2};
use serde_json::json;

use crate::theme::Tokens;
use crate::{DesignApp, icons};

const BTN: f32 = 26.0;

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let cols = if app.ui.tools_double_column { 2 } else { 1 };
    let width = 10.0 + BTN * cols as f32 + 4.0 * (cols as f32 - 1.0) + 10.0;
    egui::Panel::left("tools").exact_size(width).resizable(false).frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.divider))).show(
        ui,
        |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(3.0);
            // Collapse chevrons.
            ui.horizontal(|ui| {
                ui.add_space(width / 2.0 - 8.0);
                if icons::button(ui, if cols == 1 { "double-chevron-right" } else { "double-chevron-left" }, 16.0, false, "Toggle double column")
                    .clicked()
                {
                    app.ui.tools_double_column = !app.ui.tools_double_column;
                }
            });
            ui.add_space(2.0);
            let current = app.session.tool_id();
            let mut chosen: Option<&'static str> = None;
            let mut row_items = Vec::new();
            for (gi, g) in TOOL_GROUPS.iter().enumerate() {
                if g.is_empty() {
                    row_items.push(None);
                    continue;
                }
                row_items.push(Some(gi));
            }
            let mut i = 0;
            while i < row_items.len() {
                match row_items[i] {
                    None => {
                        let r = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [egui::pos2(r.min.x + 6.0, r.min.y + 2.0), egui::pos2(r.max.x - 6.0, r.min.y + 2.0)],
                            Stroke::new(1.0, t.divider),
                        );
                        ui.add_space(5.0);
                        i += 1;
                    }
                    Some(_) => {
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for _ in 0..cols {
                                let Some(Some(gi)) = row_items.get(i).copied() else { break };
                                let g = TOOL_GROUPS[gi];
                                let shown = g.iter().find(|tl| tl.id == current).unwrap_or(&g[0]);
                                let active = g.iter().any(|tl| tl.id == current);
                                let tip = match shown.shortcut {
                                    Some(s) => format!("{} ({s})", shown.label),
                                    None => shown.label.to_string(),
                                };
                                let resp = icons::button(ui, shown.icon, BTN, active, &tip);
                                if g.len() > 1 {
                                    // Flyout triangle.
                                    let r = resp.rect;
                                    let p = r.right_bottom() - vec2(3.0, 3.0);
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![p, p - vec2(4.0, 0.0), p - vec2(0.0, 4.0)],
                                        t.icon,
                                        Stroke::NONE,
                                    ));
                                }
                                if resp.clicked() {
                                    chosen = Some(shown.id);
                                }
                                if g.len() > 1
                                    && (resp.secondary_clicked()
                                        || resp.long_touched()
                                        || (resp.is_pointer_button_down_on()
                                            && ui.input(|i| i.pointer.press_start_time().is_some_and(|s| i.time - s > 0.35))))
                                {
                                    app.ui.flyout = Some(gi);
                                }
                                if app.ui.flyout == Some(gi) {
                                    let pos = resp.rect.right_top() + vec2(4.0, 0.0);
                                    egui::Area::new(egui::Id::new(("flyout", gi))).order(egui::Order::Foreground).fixed_pos(pos).show(
                                        ui.ctx(),
                                        |ui| {
                                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                                for tl in g {
                                                    ui.horizontal(|ui| {
                                                        let (r, row) = ui.allocate_exact_size(vec2(210.0, 24.0), Sense::click());
                                                        if row.hovered() {
                                                            ui.painter().rect_filled(r, 2.0, t.hover);
                                                        }
                                                        icons::paint(
                                                            ui.painter(),
                                                            egui::Rect::from_min_size(r.min + vec2(4.0, 3.0), vec2(18.0, 18.0)),
                                                            tl.icon,
                                                            t.icon,
                                                        );
                                                        if tl.id == current {
                                                            ui.painter().circle_filled(r.min + vec2(-2.0, 12.0), 2.0, t.accent);
                                                        }
                                                        ui.painter().text(
                                                            r.min + vec2(28.0, 12.0),
                                                            egui::Align2::LEFT_CENTER,
                                                            tl.label,
                                                            egui::FontId::proportional(12.5),
                                                            t.text,
                                                        );
                                                        if let Some(sc) = tl.shortcut {
                                                            ui.painter().text(
                                                                r.right_center() - vec2(6.0, 0.0),
                                                                egui::Align2::RIGHT_CENTER,
                                                                sc,
                                                                egui::FontId::proportional(11.5),
                                                                t.text_dim,
                                                            );
                                                        }
                                                        if row.clicked() {
                                                            chosen = Some(tl.id);
                                                        }
                                                    });
                                                }
                                            });
                                        },
                                    );
                                }
                                i += 1;
                                if matches!(row_items.get(i), Some(None) | None) {
                                    break;
                                }
                            }
                        });
                    }
                }
            }
            if let Some(id) = chosen {
                app.select_tool(id);
            }
            if app.ui.flyout.is_some() && ui.input(|i| i.pointer.any_click()) && chosen.is_none() && !ui.ctx().is_pointer_over_egui() {
                app.ui.flyout = None;
            }
            ui.add_space(8.0);
            fill_stroke_proxy(app, ui, width);
        },
    );
}

/// Fill/stroke proxy squares with default/swap, and apply color / gradient / none.
fn fill_stroke_proxy(app: &mut DesignApp, ui: &mut egui::Ui, width: f32) {
    let t = Tokens::get(ui.ctx());
    let info = crate::panels::sel_info(app);
    let doc = app.session.active().map(|d| d.doc.clone());
    let (r, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
    let base = egui::pos2(r.center().x - 15.0, r.min.y + 4.0);
    let fill_r = egui::Rect::from_min_size(base, vec2(20.0, 20.0));
    let stroke_r = egui::Rect::from_min_size(base + vec2(10.0, 10.0), vec2(20.0, 20.0));
    let (fc, fg) = match (&doc, &info) {
        (Some(d), Some(i)) => crate::widgets::swatch_colors(d, &i.fill, 1.0),
        _ => (None, None),
    };
    let (sc, _) = match (&doc, &info) {
        (Some(d), Some(i)) => crate::widgets::swatch_colors(d, &i.stroke, 1.0),
        _ => (Some(Color32::BLACK), None),
    };
    let p = ui.painter();
    // Stroke square (ring) behind, fill square in front.
    match sc {
        Some(c) => {
            p.rect_filled(stroke_r, 0.0, c);
            p.rect_filled(stroke_r.shrink(5.0), 0.0, t.panel);
        }
        None => crate::widgets::paint_chip(p, stroke_r, None, None),
    }
    p.rect_stroke(stroke_r, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
    crate::widgets::paint_chip(p, fill_r, fc, fg);
    p.rect_stroke(fill_r, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
    let resp = ui.interact(r, ui.id().with("proxy_swap"), Sense::click());
    if resp.double_clicked() {
        let _ = app.run("window.panel", json!({"panel": "swatches"}));
    }
    ui.horizontal(|ui| {
        ui.add_space((width - 3.0 * 18.0 - 8.0) / 2.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        for (icon, tip, sw) in
            [("fill-proxy", "Apply Color (,)", "[Black]"), ("panel-gradient", "Apply Gradient (.)", ""), ("none", "Apply None (/)", "[None]")]
        {
            if icons::button(ui, icon, 18.0, false, tip).clicked() && !sw.is_empty() {
                let _ = app.run("object.fill", json!({"swatch": sw}));
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space((width - 2.0 * 22.0 - 4.0) / 2.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        for (m, icon, tip) in
            [(crate::ScreenMode::Normal, "screen-normal", "Normal (W)"), (crate::ScreenMode::Preview, "screen-preview", "Preview (W)")]
        {
            if icons::button(ui, icon, 22.0, app.ui.screen_mode == m, tip).clicked() {
                app.ui.screen_mode = m;
            }
        }
    });
}
