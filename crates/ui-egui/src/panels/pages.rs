//! Pages panel: parents at the top, document spreads with live thumbnails below.

use std::collections::HashMap;
use std::sync::Mutex;

use designcraft_doc::SpreadRef;
use egui::{Color32, Sense, Stroke, StrokeKind, vec2};
use serde_json::json;

use crate::theme::{Tokens, semibold};
use crate::{DesignApp, icons};

type ThumbKey = (u64, u64, usize);

static THUMBS: Mutex<Option<HashMap<ThumbKey, egui::TextureHandle>>> = Mutex::new(None);

fn thumb(app: &mut DesignApp, ctx: &egui::Context, abs: usize, h: f32) -> Option<egui::TextureHandle> {
    let st = app.session.active()?;
    let key = (st.uid, st.revision, abs);
    let mut g = THUMBS.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    if let Some(t) = map.get(&key) {
        return Some(t.clone());
    }
    let page = st.doc.page(abs)?;
    let scale = (h as f64 * ctx.pixels_per_point() as f64) / page.height;
    let mut r = designcraft_render::Renderer::new();
    r.threads = 0;
    let img = r.render_page(
        &st.doc,
        &app.session.cache,
        abs,
        scale,
        false,
        &designcraft_render::RenderOptions { printing_only: true, ..Default::default() },
    )?;
    let ci = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
    let tex = ctx.load_texture(format!("thumb{abs}"), ci, egui::TextureOptions::LINEAR);
    // Drop thumbnails of older revisions.
    let (uid, rev) = (st.uid, st.revision);
    map.retain(|k, _| !(k.0 == uid && k.1 != rev));
    map.insert(key, tex.clone());
    Some(tex)
}

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let editing_parents = st.editing_parents;
    // Parents.
    ui.label(egui::RichText::new("Parents").font(semibold(11.5)).color(t.text_dim));
    ui.horizontal_wrapped(|ui| {
        let none = ui.add(egui::Button::new("[None]").frame(false));
        if none.clicked() {
            let _ = app.run("layout.parents.edit", json!({"on": false}));
        }
        for p in &doc.parents {
            let label = p.parent.as_ref().map(|i| i.label()).unwrap_or_default();
            let resp =
                ui.add(egui::Button::new(egui::RichText::new(&label).color(if editing_parents { t.text_strong } else { t.text })).frame(false));
            if resp.double_clicked() || resp.clicked() {
                let _ = app.run("layout.parents.edit", json!({"on": true}));
                app.views.clear();
            }
        }
    });
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([egui::pos2(r.min.x, r.min.y + 2.0), egui::pos2(r.max.x, r.min.y + 2.0)], Stroke::new(1.0, t.divider));
    ui.add_space(8.0);
    let cur = crate::canvas::current_page(app).unwrap_or(0);
    let th = 76.0;
    let center = ui.available_width() / 2.0;
    for (si, sp) in doc.spreads.iter().enumerate() {
        let first = doc.first_page_of_spread(si);
        let scale = th / doc.settings.page_height.max(1.0) as f32;
        // Pages relative to the spine.
        let spine = sp.spine_x() as f32;
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), th + 22.0), Sense::hover());
        for (pi, p) in sp.pages.iter().enumerate() {
            let abs = first + pi;
            let x0 = row.min.x + center + (p.x as f32 - spine) * scale;
            let pr = egui::Rect::from_min_size(egui::pos2(x0, row.min.y + 2.0), vec2(p.width as f32 * scale, p.height as f32 * scale));
            let resp = ui.interact(pr, ui.id().with(("page", abs)), Sense::click());
            if let Some(tex) = thumb(app, ui.ctx(), abs, th) {
                ui.painter().image(tex.id(), pr, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            } else {
                ui.painter().rect_filled(pr, 0.0, Color32::WHITE);
            }
            let selected = abs == cur && !editing_parents;
            ui.painter().rect_stroke(
                pr,
                0.0,
                Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { t.accent } else { t.divider }),
                StrokeKind::Outside,
            );
            // Parent prefix in the top corner.
            if let Some(pid) = p.parent
                && let Some(pp) = doc.parents.iter().find(|x| x.id == pid)
            {
                let prefix = pp.parent.as_ref().map(|i| i.prefix.clone()).unwrap_or_default();
                let corner =
                    if p.side == designcraft_doc::PageSide::Left { pr.left_top() + vec2(3.0, 2.0) } else { pr.right_top() + vec2(-9.0, 2.0) };
                ui.painter().text(corner, egui::Align2::LEFT_TOP, prefix, egui::FontId::proportional(9.0), Color32::from_gray(90));
            }
            if resp.clicked() || resp.double_clicked() {
                if editing_parents {
                    let _ = app.run("layout.parents.edit", json!({"on": false}));
                    app.views.clear();
                }
                crate::canvas::go_to_page(app, abs);
            }
            resp.context_menu(|ui| {
                if ui.button("Insert Page After").clicked() {
                    let _ = app.run("layout.pages.insert", json!({"after": abs, "count": 1}));
                    ui.close();
                }
                if ui.button("Delete Page").clicked() {
                    let _ = app.run("layout.pages.delete", json!({"pages": [abs]}));
                    ui.close();
                }
                if ui.button("Duplicate Spread").clicked() {
                    let _ = app.run("layout.pages.duplicateSpread", json!({"spread": si}));
                    ui.close();
                }
                ui.menu_button("Apply Parent", |ui| {
                    if ui.button("[None]").clicked() {
                        let _ = app.run("layout.pages.applyParent", json!({"pages": [abs], "parent": null}));
                        ui.close();
                    }
                    for pp in &doc.parents {
                        let l = pp.parent.as_ref().map(|i| i.label()).unwrap_or_default();
                        if ui.button(&l).clicked() {
                            let _ =
                                app.run("layout.pages.applyParent", json!({"pages": [abs], "parent": pp.parent.as_ref().map(|i| i.prefix.clone())}));
                            ui.close();
                        }
                    }
                });
            });
        }
        // Page numbers under the spread.
        let names: Vec<String> = (0..sp.pages.len()).map(|i| doc.page_name(first + i)).collect();
        let label = if names.len() > 1 { format!("{}–{}", names[0], names[names.len() - 1]) } else { names[0].clone() };
        ui.painter().text(
            egui::pos2(row.min.x + center, row.max.y - 8.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            t.text_dim,
        );
        let _ = SpreadRef::Doc(si);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} Pages in {} Spreads", doc.page_count(), doc.spreads.len())).size(11.0).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::button(ui, "trash", 20.0, false, "Delete Page").clicked() {
                let _ = app.run("layout.pages.delete", json!({"pages": [cur]}));
            }
            if icons::button(ui, "plus", 20.0, false, "Insert Page").clicked() {
                let _ = app.run("layout.pages.insert", json!({"after": cur, "count": 1}));
            }
        });
    });
}
