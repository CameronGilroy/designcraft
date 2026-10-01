//! Panels and shared panel helpers.

pub mod layers;
pub mod pages;
pub mod properties;
pub mod styles;
pub mod swatches;

use designcraft_doc::{Content, StrokeAlign, WrapMode};
use designcraft_geom::Rect;
use egui::vec2;
use serde_json::{Value, json};

use crate::DesignApp;
use crate::theme::Tokens;

/// Summary of the current object selection (for the Control panel and Properties).
#[derive(Clone, Debug)]
pub struct SelInfo {
    /// Bounds relative to the page the selection is on.
    pub page_rect: Rect,
    pub count: usize,
    pub fill: String,
    pub stroke: String,
    pub stroke_weight: f64,
    pub stroke_align: StrokeAlign,
    pub opacity: f64,
    pub shadow: bool,
    pub wrap: &'static str,
    pub columns: Option<u32>,
    pub kind: &'static str,
    pub is_graphic: bool,
    pub is_text: bool,
}

pub fn sel_info(app: &DesignApp) -> Option<SelInfo> {
    let st = app.session.active()?;
    if st.selection.items.is_empty() {
        return None;
    }
    let d = &st.doc;
    let mut b: Option<Rect> = None;
    for id in &st.selection.items {
        let it = d.item(*id)?;
        b = Some(b.map_or(it.bounds(), |r| r.union(it.bounds())));
    }
    let b = b?;
    let first = d.item(st.selection.items[0])?;
    let loc = d.find(first.id)?;
    let sp = d.spread(loc.spread)?;
    let pi = sp.page_at_x(b.center().x).unwrap_or(0);
    let px = sp.pages.get(pi).map(|p| p.x).unwrap_or(0.0);
    let wrap = match first.wrap.mode {
        WrapMode::None => "none",
        WrapMode::BoundingBox => "boundingBox",
        WrapMode::Contour => "contour",
        WrapMode::JumpObject => "jumpObject",
        WrapMode::JumpToNextColumn => "jumpToNextColumn",
    };
    Some(SelInfo {
        page_rect: Rect::new(b.x0 - px, b.y0, b.x1 - px, b.y1),
        count: st.selection.items.len(),
        fill: first.fill.swatch.clone(),
        stroke: first.stroke.swatch.clone(),
        stroke_weight: first.stroke.weight,
        stroke_align: first.stroke.align,
        opacity: first.opacity as f64,
        shadow: first.effects.drop_shadow.on,
        wrap,
        columns: first.text_frame().map(|t| t.options.columns),
        kind: first.default_label(),
        is_graphic: matches!(first.content, Content::Graphic(_)),
        is_text: first.is_text_frame(),
    })
}

/// Resolved attributes at the text selection (or the selected text frames).
pub fn text_attrs(app: &mut DesignApp) -> Option<Value> {
    let v = app.session.execute("type.selectionAttrs", &json!({})).ok()?;
    if v.is_null() { None } else { Some(serde_json::to_value(v).ok()?) }
}

/// Count preflight errors: overset stories (more checks land with the Preflight panel).
pub fn preflight_errors(app: &DesignApp) -> usize {
    let Some(st) = app.session.active() else { return 0 };
    st.doc.stories.keys().filter(|sid| app.session.cache.get(&st.doc, **sid, None).is_overset()).count()
}

/// A swatch dropdown showing a chip and name; `on_pick` gets the chosen swatch name.
pub fn swatch_picker(app: &mut DesignApp, ui: &mut egui::Ui, id: &str, current: Option<String>, on_pick: impl FnOnce(&mut DesignApp, String)) {
    let Some(doc) = app.session.active().map(|d| d.doc.clone()) else { return };
    let cur = current.unwrap_or_default();
    let (c, g) = crate::widgets::swatch_colors(&doc, &cur, 1.0);
    let mut picked = None;
    ui.push_id(id, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(30.0, 18.0), egui::Sense::click());
        crate::widgets::paint_chip(
            ui.painter(),
            egui::Rect::from_min_size(r.min, vec2(18.0, 18.0)),
            if cur.is_empty() { Some(egui::Color32::GRAY) } else { c },
            g,
        );
        crate::icons::paint(
            ui.painter(),
            egui::Rect::from_min_size(r.min + vec2(18.0, 3.0), vec2(12.0, 12.0)),
            "chevron-down",
            Tokens::get(ui.ctx()).icon,
        );
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(200.0);
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for sw in &doc.swatches {
                    let (c, g) = crate::widgets::swatch_colors(&doc, &sw.name, 1.0);
                    let row = ui.horizontal(|ui| {
                        let (cr, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
                        crate::widgets::paint_chip(ui.painter(), cr, c, g);
                        ui.add(egui::Button::new(&sw.name).frame(false).selected(sw.name == cur))
                    });
                    if row.inner.clicked() {
                        picked = Some(sw.name.clone());
                        ui.close();
                    }
                }
            });
        });
    });
    if let Some(p) = picked {
        on_pick(app, p);
    }
}

pub fn font_family_picker(app: &mut DesignApp, ui: &mut egui::Ui, current: &str, width: f32) {
    let fams = designcraft_fonts::FontDb::global().families();
    let mut pick = None;
    egui::ComboBox::from_id_salt("font_family").selected_text(if current.is_empty() { "—" } else { current }).width(width).show_ui(ui, |ui| {
        for f in &fams {
            if ui.selectable_label(f == current, f).clicked() {
                pick = Some(f.clone());
            }
        }
    });
    if let Some(f) = pick {
        let styles = designcraft_fonts::FontDb::global().styles(&f);
        let style = if styles.iter().any(|s| s == "Regular") { "Regular".to_string() } else { styles.first().cloned().unwrap_or_default() };
        let _ = app.run("type.char", json!({"attrs": {"fontFamily": f, "fontStyle": style}}));
    }
}

pub fn font_style_picker(app: &mut DesignApp, ui: &mut egui::Ui, family: &str, current: &str, width: f32) {
    let styles = designcraft_fonts::FontDb::global().styles(family);
    let mut pick = None;
    egui::ComboBox::from_id_salt("font_style").selected_text(if current.is_empty() { "—" } else { current }).width(width).show_ui(ui, |ui| {
        for s in &styles {
            if ui.selectable_label(s == current, s).clicked() {
                pick = Some(s.clone());
            }
        }
    });
    if let Some(s) = pick {
        let _ = app.run("type.char", json!({"attrs": {"fontStyle": s}}));
    }
}

pub fn para_style_picker(app: &mut DesignApp, ui: &mut egui::Ui, current: &str, overridden: bool, width: f32) {
    let names: Vec<String> = app.session.active().map(|d| d.doc.styles.paragraph.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
    let mut pick = None;
    let label = if overridden { format!("{current}+") } else { current.to_string() };
    egui::ComboBox::from_id_salt("para_style").selected_text(label).width(width).show_ui(ui, |ui| {
        for n in names.iter().filter(|n| *n != designcraft_doc::NO_PARA_STYLE) {
            if ui.selectable_label(n == current, n).clicked() {
                pick = Some(n.clone());
            }
        }
    });
    if let Some(n) = pick {
        let _ = app.run("style.paragraph.apply", json!({"name": n, "clearOverrides": false}));
    }
}
