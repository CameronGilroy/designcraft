//! Small InDesign-style widgets: measurement fields with units and arithmetic, labelled
//! fields, swatch chips, section headers.

use designcraft_geom::Unit;
use designcraft_geom::units::{format_measure_prec, parse_measure};
use egui::{Color32, Response, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::theme::Tokens;

/// A measurement field: shows `value` (points) in `unit`; on Enter/focus loss parses any unit and
/// arithmetic. Returns the new value in points when edited.
pub fn measure(ui: &mut Ui, id: &str, value: Option<f64>, unit: Unit, width: f32) -> Option<f64> {
    let t = Tokens::get(ui.ctx());
    let key = ui.id().with(id);
    let mut buf: String =
        ui.data(|d| d.get_temp::<String>(key)).unwrap_or_else(|| value.map(|v| format_measure_prec(v, unit, 3)).unwrap_or_default());
    let r = ui.add_sized(
        [width, 21.0],
        egui::TextEdit::singleline(&mut buf).font(egui::FontId::proportional(12.0)).background_color(t.input).margin(egui::Margin::symmetric(4, 1)),
    );
    let mut out = None;
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(key, buf.clone()));
    }
    if r.lost_focus() {
        if let Ok(v) = parse_measure(&buf, unit)
            && value.is_none_or(|old| (old - v).abs() > 1e-9)
        {
            out = Some(v);
        }
        ui.data_mut(|d| d.remove::<String>(key));
    } else if !r.has_focus() {
        ui.data_mut(|d| d.remove::<String>(key));
    }
    out
}

/// A plain number field (percent, degrees, counts).
pub fn number(ui: &mut Ui, id: &str, value: Option<f64>, suffix: &str, width: f32, decimals: usize) -> Option<f64> {
    let t = Tokens::get(ui.ctx());
    let key = ui.id().with(id);
    let show = |v: f64| {
        let s = format!("{v:.decimals$}");
        let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
        format!("{s}{suffix}")
    };
    let mut buf: String = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_else(|| value.map(show).unwrap_or_default());
    let r = ui.add_sized(
        [width, 21.0],
        egui::TextEdit::singleline(&mut buf).font(egui::FontId::proportional(12.0)).background_color(t.input).margin(egui::Margin::symmetric(4, 1)),
    );
    let mut out = None;
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(key, buf.clone()));
    }
    if r.lost_focus() {
        let cleaned: String = buf.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-')).collect();
        if let Ok(v) = cleaned.parse::<f64>()
            && value.is_none_or(|old| (old - v).abs() > 1e-9)
        {
            out = Some(v);
        }
        ui.data_mut(|d| d.remove::<String>(key));
    } else if !r.has_focus() {
        ui.data_mut(|d| d.remove::<String>(key));
    }
    out
}

/// A small dim label (field captions like `X:` or `W:`).
pub fn caption(ui: &mut Ui, s: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(s).size(11.5).color(t.text_dim));
}

/// Panel section header (InDesign 2026: regular-weight white title, no disclosure chevron; a
/// 1.5 pt rule separates sections). Returns whether to show the section body (always true).
pub fn section(ui: &mut Ui, title: &str, _default_open: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(11.5), t.text_strong);
    true
}

pub fn divider(ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let r = ui.available_rect_before_wrap();
    ui.painter()
        .line_segment([egui::pos2(r.min.x - 10.0, r.min.y + 4.0), egui::pos2(r.max.x + 10.0, r.min.y + 4.0)], Stroke::new(1.5, t.section_divider));
    ui.add_space(8.0);
}

/// A swatch chip (colour square, [None] slash, gradient ramp).
pub fn swatch_chip(ui: &mut Ui, size: f32, color: Option<Color32>, gradient: Option<(Color32, Color32)>) -> Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    paint_chip(ui.painter(), r, color, gradient);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
    resp
}

pub fn paint_chip(p: &egui::Painter, r: egui::Rect, color: Option<Color32>, gradient: Option<(Color32, Color32)>) {
    match (color, gradient) {
        (_, Some((a, b))) => {
            let n = 12;
            for i in 0..n {
                let f = i as f32 / (n - 1) as f32;
                let c = Color32::from_rgb(
                    (a.r() as f32 * (1.0 - f) + b.r() as f32 * f) as u8,
                    (a.g() as f32 * (1.0 - f) + b.g() as f32 * f) as u8,
                    (a.b() as f32 * (1.0 - f) + b.b() as f32 * f) as u8,
                );
                let x0 = r.min.x + r.width() * i as f32 / n as f32;
                p.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, r.min.y), egui::pos2(x0 + r.width() / n as f32 + 0.5, r.max.y)), 0.0, c);
            }
        }
        (Some(c), None) => {
            p.rect_filled(r, 0.0, c);
        }
        (None, None) => {
            p.rect_filled(r, 0.0, Color32::WHITE);
            p.line_segment([r.left_bottom(), r.right_top()], Stroke::new(1.5, Color32::from_rgb(230, 30, 30)));
        }
    }
}

/// Resolve a swatch to a display colour (and gradient end colours).
pub fn swatch_colors(doc: &designcraft_doc::Document, name: &str, tint: f32) -> (Option<Color32>, Option<(Color32, Color32)>) {
    let conv = |c: designcraft_color::Color| {
        let [r, g, b, _] = c.to_rgba8(1.0);
        Color32::from_rgb(r, g, b)
    };
    if let Some(g) = designcraft_color::swatch::resolve_gradient(&doc.swatches, name)
        && let (Some(a), Some(b)) = (g.stops.first(), g.stops.last())
    {
        return (None, Some((conv(a.color), conv(b.color))));
    }
    (doc.resolve_color(name, tint).map(conv), None)
}

/// An icon toggle button (selected = darker well).
pub fn icon_toggle(ui: &mut Ui, icon: &str, on: bool, tip: &str) -> Response {
    crate::icons::button(ui, icon, 22.0, on, tip)
}
