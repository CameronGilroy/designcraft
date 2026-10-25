//! History panel: one row per undo state, with New Document and Delete in the footer.

use egui::{Color32, Sense, Stroke, vec2};
use serde_json::{Value, json};

use crate::DesignApp;
use crate::theme::Tokens;

/// The list of states. The footer stays put while the list scrolls.
pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language.clone();
    let value = match app.session.execute("document.history.states", &json!({})) {
        Ok(value) => value,
        Err(_) => {
            dim(ui, &t, crate::i18n::tr(&lang, "No document"));
            return;
        }
    };
    let current = value.get("current").and_then(Value::as_u64).unwrap_or(0);
    let Some(states) = value.get("states").and_then(Value::as_array) else {
        dim(ui, &t, crate::i18n::tr(&lang, "No document"));
        return;
    };
    let footer_h = 28.0;
    let list_h = (ui.available_height() - footer_h).max(48.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
        for state in states {
            let index = state.get("index").and_then(Value::as_u64).unwrap_or(0);
            let raw = state.get("label").and_then(Value::as_str).unwrap_or("");
            let kind = state.get("kind").and_then(Value::as_str).unwrap_or("action");
            let future = state.get("future").and_then(Value::as_bool).unwrap_or(false);
            let selected = state.get("current").and_then(Value::as_bool).unwrap_or(false);
            let owned = raw.to_string();
            let shown = if kind == "file" { owned } else { crate::i18n::tr(&lang, &owned).to_string() };
            let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
            if selected {
                ui.painter().rect_filled(rect, 2.0, t.row_selected);
            } else if response.hovered() {
                ui.painter().rect_filled(rect, 2.0, t.hover);
            }
            let icon = egui::Rect::from_min_size(rect.min + vec2(4.0, 3.0), vec2(16.0, 16.0));
            let color = if future {
                t.text_dim
            } else if selected {
                t.text_strong
            } else {
                t.text
            };
            crate::icons::paint(ui.painter(), icon, "panel-history", if future { t.text_disabled } else { t.icon });
            crate::rtl::paint(
                ui.painter(),
                rect.left_center() + vec2(24.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &shown,
                egui::FontId::proportional(12.0),
                color,
            );
            if response.clicked()
                && let Err(error) = app.run("edit.history.go", json!({"index": index}))
            {
                app.status_error("History", error);
            }
        }
    });
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([line.left_center(), line.right_center()], Stroke::new(1.0, t.border));
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            if footer_button(ui, &t, &lang, "Delete", current > 0, true).clicked()
                && let Err(error) = app.run("edit.history.delete", json!({}))
            {
                app.status_error("History", error);
            }
            if footer_button(ui, &t, &lang, "New Document", true, false).clicked()
                && let Err(error) = app.run("edit.history.newDocument", json!({}))
            {
                app.status_error("History", error);
            }
        });
    });
}

/// History commands at the top of the panel menu.
pub fn panel_menu(app: &mut DesignApp, ui: &mut egui::Ui) {
    let lang = app.ui.language.clone();
    let (back, forward, delete, clear, has_doc) = match app.session.active() {
        Some(st) => {
            let undo = !st.history.undo.is_empty();
            let redo = !st.history.redo.is_empty();
            (undo, redo, undo, undo || redo, true)
        }
        None => (false, false, false, false, false),
    };
    let mut command = |ui: &mut egui::Ui, label: &str, on: bool, id: &str| {
        let text = crate::rtl::widget(ui, crate::i18n::tr(&lang, label));
        if ui.add_enabled(on, egui::Button::new(text)).clicked() {
            if let Err(error) = app.run(id, json!({})) {
                app.status_error("History", error);
            }
            ui.close();
        }
    };
    command(ui, "Step Backward", back, "edit.undo");
    command(ui, "Step Forward", forward, "edit.redo");
    command(ui, "Delete", delete, "edit.history.delete");
    command(ui, "Clear History", clear, "edit.history.clear");
    ui.separator();
    command(ui, "New Document", has_doc, "edit.history.newDocument");
}

fn dim(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    ui.label(egui::RichText::new(text).color(t.text_dim));
}

/// Footer icon. `trash` paints the trash can; otherwise a page with a plus.
fn footer_button(ui: &mut egui::Ui, t: &Tokens, lang: &str, tip: &str, enabled: bool, trash: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 3.0, t.hover);
    }
    let color = if enabled { t.icon } else { t.text_disabled };
    if trash {
        crate::icons::paint(ui.painter(), rect.shrink(3.0), "trash", color);
    } else {
        page_plus(ui.painter(), rect, color);
    }
    let tip = crate::i18n::tr(lang, tip).to_string();
    response.on_hover_ui(move |ui| {
        crate::rtl::label(ui, tip);
    })
}

fn page_plus(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let page = egui::Rect::from_center_size(rect.center() + vec2(-2.0, 0.0), vec2(10.0, 12.0));
    painter.rect_stroke(page, 1.0, Stroke::new(1.0, color), egui::StrokeKind::Middle);
    let center = page.right_bottom() + vec2(-1.0, -1.0);
    painter.line_segment([center + vec2(-3.5, 0.0), center + vec2(3.5, 0.0)], Stroke::new(1.2, color));
    painter.line_segment([center + vec2(0.0, -3.5), center + vec2(0.0, 3.5)], Stroke::new(1.2, color));
}
