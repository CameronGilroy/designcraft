//! Swatches and Color panels.

use designcraft_color::{ColorType, SwatchValue};
use egui::{Sense, vec2};
use serde_json::json;

use crate::theme::Tokens;
use crate::{DesignApp, icons};

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let text = st.selection.text.is_some();
    let fill_cur = super::sel_info(app).map(|i| i.fill);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(if text { "Applies to text" } else { "Applies to fill" }).size(11.0).color(t.text_dim));
    });
    for sw in &doc.swatches {
        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
        if fill_cur.as_deref() == Some(sw.name.as_str()) {
            ui.painter().rect_filled(row, 0.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(row, 0.0, t.hover);
        }
        let (c, g) = crate::widgets::swatch_colors(&doc, &sw.name, 1.0);
        let chip = egui::Rect::from_min_size(row.min + vec2(4.0, 3.0), vec2(16.0, 16.0));
        crate::widgets::paint_chip(ui.painter(), chip, c, g);
        ui.painter().text(row.min + vec2(28.0, 11.0), egui::Align2::LEFT_CENTER, &sw.name, egui::FontId::proportional(12.5), t.text);
        // Type indicators: spot dot / process square, colour mode.
        let kind = match &sw.value {
            SwatchValue::Color { color, color_type } => {
                let m = match color {
                    designcraft_color::Color::Cmyk { .. } => "CMYK",
                    designcraft_color::Color::Rgb { .. } => "RGB",
                    designcraft_color::Color::Gray { .. } => "Gray",
                };
                format!("{}{}", if *color_type == ColorType::Spot { "● " } else { "" }, m)
            }
            SwatchValue::Gradient { .. } => "Gradient".into(),
            SwatchValue::Tint { tint, .. } => format!("{:.0}%", tint * 100.0),
            _ => String::new(),
        };
        if sw.locked {
            icons::paint(
                ui.painter(),
                egui::Rect::from_min_size(egui::pos2(row.max.x - 64.0, row.min.y + 4.0), vec2(13.0, 13.0)),
                "lock",
                t.text_dim,
            );
        }
        ui.painter().text(row.right_center() - vec2(6.0, 0.0), egui::Align2::RIGHT_CENTER, kind, egui::FontId::proportional(10.5), t.text_dim);
        if resp.clicked() {
            let r = if text { app.run("type.char", json!({"attrs": {"fill": sw.name}})) } else { app.run("object.fill", json!({"swatch": sw.name})) };
            let _ = r;
        }
        if resp.secondary_clicked() {
            let _ = app.run("object.stroke", json!({"swatch": sw.name}));
        }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Right-click applies to stroke").size(10.5).color(t.text_disabled));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::button(ui, "plus", 20.0, false, "New Swatch").clicked() {
                let _ = app.run("swatch.create", json!({"color": {"c": 0, "m": 50, "y": 100, "k": 0}}));
            }
        });
    });
}

pub fn color_panel(app: &mut DesignApp, ui: &mut egui::Ui) {
    let Some(i) = super::sel_info(app) else {
        ui.label("Select an object to edit its fill.");
        return;
    };
    let Some(doc) = app.session.active().map(|d| d.doc.clone()) else { return };
    let Some(c) = doc.resolve_color(&i.fill, 1.0) else {
        ui.label(format!("Fill: {}", i.fill));
        return;
    };
    let [mut cc, mut m, mut y, mut k] = c.to_cmyk();
    let mut changed = false;
    for (label, v) in [("C", &mut cc), ("M", &mut m), ("Y", &mut y), ("K", &mut k)] {
        ui.horizontal(|ui| {
            ui.label(label);
            let mut p = *v * 100.0;
            if ui.add(egui::Slider::new(&mut p, 0.0..=100.0).suffix("%").integer()).drag_stopped() {
                *v = p / 100.0;
                changed = true;
            }
        });
    }
    if changed && let Ok(r) = app.run("swatch.create", json!({"color": {"c": cc * 100.0, "m": m * 100.0, "y": y * 100.0, "k": k * 100.0}})) {
        let _ = app.run("object.fill", json!({"swatch": r["name"]}));
    }
}
