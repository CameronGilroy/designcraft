//! Hyperlinks and Bookmarks panels (Window › Interactive).

use serde_json::{Value, json};

use crate::DesignApp;
use crate::theme::Tokens;

fn dest_text(d: &Value) -> String {
    if let Some(u) = d.get("url").and_then(Value::as_str) {
        return u.to_string();
    }
    if let Some(e) = d.get("email").and_then(Value::as_str) {
        return format!("mailto:{e}");
    }
    if let Some(p) = d.get("page").and_then(Value::as_u64) {
        return format!("Page {}", p + 1);
    }
    d.to_string()
}

pub fn hyperlinks(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::new("new_hyperlink_url");
    let mut url: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| "https://".into());
    let has_target = app.session.active().is_some_and(|d| d.selection.text.is_some_and(|t| !t.is_caret()) || !d.selection.items.is_empty());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut url).desired_width(170.0));
        if ui.add_enabled(has_target, egui::Button::new("New")).on_hover_text("Hyperlink from the selected text or frames").clicked() {
            let p = if let Some(m) = url.strip_prefix("mailto:") { json!({"email": m}) } else { json!({"url": url}) };
            if let Err(e) = app.run("hyperlink.create", p) {
                app.status(format!("Hyperlinks: {e}"));
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, url));
    ui.separator();
    let list = app.session.execute("hyperlink.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        ui.label(egui::RichText::new("No hyperlinks. Select text or a frame, type a URL and click New.").size(11.0).color(t.text_dim));
    }
    for h in list {
        let hid = h["id"].clone();
        ui.horizontal(|ui| {
            let r = ui.selectable_label(false, h["name"].as_str().unwrap_or("")).on_hover_text(dest_text(&h["dest"]));
            if r.clicked() {
                let _ = app.run("hyperlink.goToSource", json!({"id": hid}));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("hyperlink.delete", json!({"id": hid}));
                }
            });
        });
    }
}

pub fn bookmarks(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    if ui.button("New Bookmark for This Page").clicked() {
        let page = crate::canvas::current_page(app).unwrap_or(0) + 1;
        let _ = app.run("bookmark.add", json!({"page": page}));
    }
    ui.separator();
    let list = app.session.execute("bookmark.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        ui.label(egui::RichText::new("No bookmarks. They become the PDF outline.").size(11.0).color(t.text_dim));
    }
    for (i, b) in list.iter().enumerate() {
        let page = b["page"].as_u64().unwrap_or(0) as usize;
        ui.horizontal(|ui| {
            if ui.selectable_label(false, b["name"].as_str().unwrap_or("")).on_hover_text(format!("Page {}", app.session.page_label(page))).clicked()
            {
                crate::canvas::go_to_page(app, page);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("bookmark.delete", json!({"index": i}));
                }
            });
        });
    }
}

/// Articles panel: reading-order lists for EPUB / HTML export.
pub fn articles(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let has_sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    if ui.add_enabled(has_sel, egui::Button::new("New Article from Selection")).clicked() {
        let _ = app.run("article.new", json!({}));
    }
    ui.separator();
    let list = app.session.execute("article.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        ui.label(egui::RichText::new("No articles: exports follow page order.").size(11.0).color(t.text_dim));
    }
    for a in list {
        let name = a["name"].as_str().unwrap_or("").to_string();
        let mut export = a["export"].as_bool().unwrap_or(true);
        ui.horizontal(|ui| {
            if ui.checkbox(&mut export, "").on_hover_text("Include when exporting").changed() {
                let _ = app.run("article.options", json!({"name": name, "export": export}));
            }
            ui.label(egui::RichText::new(&name).strong());
            ui.label(egui::RichText::new(format!("{} objects", a["items"].as_array().map_or(0, Vec::len))).size(10.5).color(t.text_dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("article.delete", json!({"name": name}));
                }
                if ui.add_enabled(has_sel, egui::Button::new("Add").small()).on_hover_text("Add the selection").clicked() {
                    let _ = app.run("article.add", json!({"name": name}));
                }
            });
        });
    }
}

/// Tags panel and structure: tags (click to tag the selection), the structure in reading order,
/// XML export/import.
pub fn tags(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Ok(info) = app.session.execute("xml.tags", &json!({})) else { return };
    let has_sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    let id = egui::Id::new("new_tag_name");
    let mut name: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("New tag").desired_width(140.0));
        if ui.button("New").clicked() && !name.trim().is_empty() {
            match app.run("xml.newTag", json!({"name": name.trim()})) {
                Ok(_) => name.clear(),
                Err(e) => app.status(format!("Tags: {e}")),
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, name));
    for tag in info["tags"].as_array().cloned().unwrap_or_default() {
        let n = tag["name"].as_str().unwrap_or("").to_string();
        let c = tag["color"].as_array().map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect::<Vec<_>>()).unwrap_or_default();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            if c.len() == 3 {
                ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
            }
            if ui.add_enabled(has_sel, egui::Button::new(&n).frame(false)).on_hover_text("Tag the selection").clicked() {
                let _ = app.run("xml.tag", json!({"tag": n}));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("xml.deleteTag", json!({"name": n}));
                }
            });
        });
    }
    if has_sel && ui.small_button("Untag Selection").clicked() {
        let _ = app.run("xml.tag", json!({"tag": null}));
    }
    ui.separator();
    ui.label(egui::RichText::new("Structure").strong());
    let st = app.session.execute("xml.structure", &json!({})).unwrap_or_default();
    ui.label(egui::RichText::new(st["root"].as_str().unwrap_or("Root")).size(11.0).color(t.text_dim));
    for el in st["elements"].as_array().cloned().unwrap_or_default() {
        let text = el["text"].as_str().unwrap_or("").chars().take(40).collect::<String>();
        let label = format!("  <{}> {}", el["tag"].as_str().unwrap_or(""), text);
        if ui.selectable_label(false, egui::RichText::new(label).size(11.0)).clicked() {
            let _ = app.run("selection.set", json!({"ids": [el["id"]]}));
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Export XML…").clicked() {
            let _ = app.run("app.exportXml", json!({}));
        }
        if ui.button("Import XML…").clicked()
            && let Some(path) = app.services.pick_open.as_mut().and_then(|f| f("xml"))
            && let Err(e) = app.run("file.importXml", json!({"path": path}))
        {
            app.status(format!("Import XML: {e}"));
        }
    });
}
