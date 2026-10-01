//! Layers panel.

use egui::{Color32, Sense, Stroke, StrokeKind, vec2};
use serde_json::json;

use crate::theme::Tokens;
use crate::{DesignApp, icons};

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let active = st.active_layer;
    let sel_layers: Vec<_> = st.selection.items.iter().filter_map(|i| doc.item(*i).map(|it| it.layer)).collect();
    for l in &doc.layers {
        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        if l.id == active {
            ui.painter().rect_filled(row, 0.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(row, 0.0, t.hover);
        }
        let eye = egui::Rect::from_min_size(row.min + vec2(2.0, 4.0), vec2(18.0, 18.0));
        let lock = egui::Rect::from_min_size(row.min + vec2(24.0, 4.0), vec2(18.0, 18.0));
        if l.visible {
            icons::paint(ui.painter(), eye, "eye", t.icon);
        }
        if l.locked {
            icons::paint(ui.painter(), lock, "lock", t.icon);
        }
        ui.painter().rect_stroke(lock.expand(1.0), 0.0, Stroke::new(1.0, t.divider), StrokeKind::Inside);
        let chip = egui::Rect::from_min_size(row.min + vec2(48.0, 8.0), vec2(4.0, 10.0));
        ui.painter().rect_filled(chip, 0.0, Color32::from_rgb(l.color[0], l.color[1], l.color[2]));
        ui.painter().text(row.min + vec2(58.0, 13.0), egui::Align2::LEFT_CENTER, &l.name, egui::FontId::proportional(12.5), t.text);
        // Selection square in the layer colour.
        if sel_layers.contains(&l.id) {
            let sq = egui::Rect::from_center_size(egui::pos2(row.max.x - 12.0, row.center().y), vec2(7.0, 7.0));
            ui.painter().rect_filled(sq, 0.0, Color32::from_rgb(l.color[0], l.color[1], l.color[2]));
        }
        let eye_r = ui.interact(eye, ui.id().with(("eye", l.id.0)), Sense::click());
        let lock_r = ui.interact(lock, ui.id().with(("lock", l.id.0)), Sense::click());
        if eye_r.clicked() {
            let _ = app.run("layer.set", json!({"id": l.id.0, "visible": !l.visible}));
        } else if lock_r.clicked() {
            let _ = app.run("layer.set", json!({"id": l.id.0, "locked": !l.locked}));
        } else if resp.clicked() {
            let _ = app.run("layer.activate", json!({"id": l.id.0}));
        }
        resp.context_menu(|ui| {
            if ui.button("Delete Layer").clicked() {
                let _ = app.run("layer.delete", json!({"id": l.id.0}));
                ui.close();
            }
            if ui.button("Move Selection Here").clicked() {
                let _ = app.run("object.setLayer", json!({"layer": l.id.0}));
                ui.close();
            }
        });
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} Layer{}", doc.layers.len(), if doc.layers.len() == 1 { "" } else { "s" })).size(11.0).color(t.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::button(ui, "trash", 20.0, false, "Delete Layer").clicked() {
                let _ = app.run("layer.delete", json!({"id": active.0}));
            }
            if icons::button(ui, "plus", 20.0, false, "Create New Layer").clicked() {
                let _ = app.run("layer.new", json!({}));
            }
        });
    });
}
