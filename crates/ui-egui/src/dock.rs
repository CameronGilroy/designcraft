//! The right dock: an expanded panel group (Properties · Pages · Layers) and a collapsed icon
//! column whose panels open as flyouts beside the dock.

use egui::{Color32, Sense, Stroke, vec2};

use crate::theme::{Tokens, semibold};
use crate::{DesignApp, icons, panels};

/// (id, label, icon) of the tabs in the expanded group.
pub const DOCK_TABS: &[(&str, &str, &str)] =
    &[("properties", "Properties", "panel-properties"), ("pages", "Pages", "panel-pages"), ("layers", "Layers", "panel-layers")];

/// Panels in the collapsed icon column.
pub const ICON_PANELS: &[(&str, &str, &str)] = &[
    ("stroke", "Stroke", "panel-stroke"),
    ("swatches", "Swatches", "panel-swatches"),
    ("color", "Color", "panel-color"),
    ("effects", "Effects", "panel-effects"),
    ("paragraphStyles", "Paragraph Styles", "panel-pstyles"),
    ("characterStyles", "Character Styles", "panel-cstyles"),
    ("character", "Character", "panel-character"),
    ("paragraph", "Paragraph", "panel-paragraph"),
    ("textWrap", "Text Wrap", "panel-wrap"),
    ("align", "Align", "panel-align"),
    ("links", "Links", "panel-links"),
    ("info", "Info", "panel-info"),
];

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    // Expanded panel stack (outermost, 253 pt) — declared first so it sits at the far right.
    if app.ui.dock_expanded {
        egui::Panel::right("dock").default_size(253.0).size_range(220.0..=420.0).resizable(true).frame(egui::Frame::NONE.fill(t.panel)).show(
            ui,
            |ui| {
                dock_header(ui, &t, "»");
                // Tab strip (27 pt).
                let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 27.0), Sense::hover());
                ui.painter().rect_filled(strip, 0.0, t.tab_strip);
                let mut x = strip.min.x;
                for (id, label, _) in DOCK_TABS {
                    let active = app.ui.dock_tab == *id;
                    let g = ui.painter().layout_no_wrap(
                        label.to_string(),
                        semibold(11.5),
                        if active { Color32::from_rgb(0xf3, 0xf3, 0xf3) } else { t.text_dim },
                    );
                    let w = g.size().x + 26.0;
                    let r = egui::Rect::from_min_size(egui::pos2(x, strip.min.y), vec2(w, 27.0));
                    if active {
                        ui.painter().rect_filled(r, 0.0, t.panel);
                    }
                    ui.painter().galley(egui::pos2(r.min.x + 13.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
                    ui.painter().line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, t.border));
                    if ui.interact(r, ui.id().with(("docktab", *id)), Sense::click()).clicked() {
                        app.ui.dock_tab = id.to_string();
                    }
                    x += w;
                }
                let menu_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 24.0, strip.min.y + 4.0), vec2(18.0, 18.0));
                icons::paint(ui.painter(), menu_r, "menu", t.icon);
                egui::Frame::NONE.inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 6 }).show(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.ui.dock_tab.as_str() {
                        "pages" => panels::pages::show(app, ui),
                        "layers" => panels::layers::show(app, ui),
                        _ => panels::properties::show(app, ui),
                    });
                });
            },
        );
    }
    // Collapsed icon strip (37 pt) to the left of the panel stack.
    egui::Panel::right("icon_column").exact_size(37.0).resizable(false).frame(egui::Frame::NONE.fill(t.panel)).show(ui, |ui| {
        dock_header(ui, &t, "«");
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            for (id, label, icon) in ICON_PANELS {
                let open = app.ui.open_panel.as_deref() == Some(*id);
                if icons::button(ui, icon, 28.0, open, label).clicked() {
                    app.ui.open_panel = if open { None } else { Some(id.to_string()) };
                }
                ui.add_space(1.0);
            }
        });
    });
}

/// 11 pt dock header strip with a collapse chevron.
fn dock_header(ui: &mut egui::Ui, t: &Tokens, chevron: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 11.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.tab_strip);
    ui.painter().text(egui::pos2(r.max.x - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, chevron, egui::FontId::proportional(9.0), t.text_dim);
    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.border));
}

/// A panel opened from the icon column, shown as a floating flyout next to the dock.
pub fn flyout(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(id) = app.ui.open_panel.clone() else { return };
    let t = Tokens::get(ctx);
    let label = ICON_PANELS.iter().find(|p| p.0 == id).map(|p| p.1).unwrap_or("Panel");
    let screen = ctx.content_rect();
    let dock_w = if app.ui.dock_expanded { ctx.memory(|m| m.area_rect(egui::Id::new("dock")).map(|r| r.width())).unwrap_or(280.0) } else { 0.0 };
    let pos = egui::pos2(screen.max.x - 37.0 - dock_w - 262.0, 120.0);
    let mut open = true;
    egui::Area::new(egui::Id::new("panel_flyout")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).inner_margin(egui::Margin::same(0)).show(ui, |ui| {
            ui.set_width(256.0);
            let (strip, _) = ui.allocate_exact_size(vec2(256.0, 26.0), Sense::hover());
            ui.painter().rect_filled(strip, 0.0, t.panel_darker);
            ui.painter().text(strip.min + vec2(10.0, 13.0), egui::Align2::LEFT_CENTER, label, semibold(12.0), t.text_strong);
            let close = egui::Rect::from_min_size(egui::pos2(strip.max.x - 22.0, strip.min.y + 4.0), vec2(18.0, 18.0));
            if ui.interact(close, ui.id().with("flyclose"), Sense::click()).clicked() {
                open = false;
            }
            ui.painter().text(close.center(), egui::Align2::CENTER_CENTER, "×", egui::FontId::proportional(15.0), t.text_dim);
            egui::Frame::NONE.inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(screen.height() - 220.0).show(ui, |ui| match id.as_str() {
                    "swatches" => panels::swatches::show(app, ui),
                    "paragraphStyles" => panels::styles::paragraph(app, ui),
                    "characterStyles" => panels::styles::character(app, ui),
                    "stroke" => panels::properties::stroke_panel(app, ui),
                    "character" => panels::properties::character_panel(app, ui),
                    "paragraph" => panels::properties::paragraph_panel(app, ui),
                    "textWrap" => panels::properties::wrap_panel(app, ui),
                    "align" => panels::properties::align_panel(app, ui),
                    "color" => panels::swatches::color_panel(app, ui),
                    "effects" => panels::properties::effects_panel(app, ui),
                    "links" => panels::properties::links_panel(app, ui),
                    _ => panels::properties::info_panel(app, ui),
                });
            });
        });
    });
    if !open {
        app.ui.open_panel = None;
    }
}
