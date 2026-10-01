//! Paragraph Styles and Character Styles panels.

use egui::{Sense, vec2};
use serde_json::json;

use crate::theme::Tokens;
use crate::{DesignApp, icons};

fn list(app: &mut DesignApp, ui: &mut egui::Ui, para: bool) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let names: Vec<String> = if para {
        st.doc.styles.paragraph.iter().map(|s| s.name.clone()).collect()
    } else {
        st.doc.styles.character.iter().map(|s| s.name.clone()).collect()
    };
    let attrs = super::text_attrs(app);
    let current = attrs.as_ref().and_then(|a| a[if para { "paragraphStyle" } else { "characterStyle" }].as_str().map(str::to_string));
    let overrides = attrs.as_ref().map(|a| a[if para { "paraOverrides" } else { "charOverrides" }].as_u64().unwrap_or(0)).unwrap_or(0);
    ui.label(
        egui::RichText::new(match &current {
            Some(c) => format!("{c}{}", if overrides > 0 { "+" } else { "" }),
            None => "No text selected".into(),
        })
        .size(11.0)
        .color(t.text_dim),
    );
    for n in &names {
        if para && n == designcraft_doc::NO_PARA_STYLE {
            continue;
        }
        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
        if current.as_deref() == Some(n.as_str()) {
            ui.painter().rect_filled(row, 0.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(row, 0.0, t.hover);
        }
        ui.painter().text(row.min + vec2(8.0, 11.0), egui::Align2::LEFT_CENTER, n, egui::FontId::proportional(12.5), t.text);
        if resp.clicked() {
            let cmd = if para { "style.paragraph.apply" } else { "style.character.apply" };
            let clear = ui.input(|i| i.modifiers.alt);
            let _ = app.run(cmd, json!({"name": n, "clearOverrides": clear}));
        }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("⌥-click clears overrides").size(10.5).color(t.text_disabled));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::button(ui, "plus", 20.0, false, "Create New Style from Selection").clicked() {
                let cmd = if para { "style.paragraph.create" } else { "style.character.create" };
                let base = if para { "Paragraph Style 1" } else { "Character Style 1" };
                let _ = app.run(cmd, json!({"name": base, "fromSelection": true}));
            }
        });
    });
}

pub fn paragraph(app: &mut DesignApp, ui: &mut egui::Ui) {
    list(app, ui, true);
}

pub fn character(app: &mut DesignApp, ui: &mut egui::Ui) {
    list(app, ui, false);
}
