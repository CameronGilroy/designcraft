//! Object Library panel: the open library's items; add the selection, place or delete an item;
//! New / Open library.

use serde_json::json;

use crate::DesignApp;
use crate::theme::Tokens;

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        if ui.button("New Library…").clicked() {
            let path = app.services.pick_save.as_mut().and_then(|f| f("Library.dclib"));
            let _ = app.run("library.new", json!({"path": path}));
        }
        if ui.button("Open…").clicked()
            && let Some(path) = app.services.pick_open.as_mut().and_then(|f| f("library"))
            && let Err(e) = app.run("library.open", json!({"path": path}))
        {
            app.status(format!("Library: {e}"));
        }
    });
    let Ok(items) = app.session.execute("library.list", &json!({})) else {
        ui.label(egui::RichText::new("No library open.").size(11.0).color(t.text_dim));
        return;
    };
    let has_sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    if ui.add_enabled(has_sel, egui::Button::new("Add Selection")).clicked()
        && let Err(e) = app.run("library.add", json!({}))
    {
        app.status(format!("Library: {e}"));
    }
    ui.separator();
    let items = items.as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        ui.label(egui::RichText::new("The library is empty. Select objects and Add Selection.").size(11.0).color(t.text_dim));
    }
    for it in items {
        let i = it["index"].clone();
        ui.horizontal(|ui| {
            ui.label(it["name"].as_str().unwrap_or(""));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("library.remove", json!({"index": i}));
                }
                if ui.small_button("Place").clicked()
                    && let Err(e) = app.run("library.place", json!({"index": i}))
                {
                    app.status(format!("Library: {e}"));
                }
            });
        });
    }
}
