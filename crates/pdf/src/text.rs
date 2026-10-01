//! Composed text → PDF text objects with embedded (subsetted) fonts.
//!
//! Each line is split into runs of glyphs sharing face, style, scale and baseline offset. A run is
//! drawn with one `draw_glyphs` call: the run origin, horizontal scale and skew go into the
//! transform, glyph advances are taken from the composed x positions (so justification, tracking
//! and kerning are exact), and every glyph carries the story text it came from so viewers can
//! select, search and copy it.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use designcraft_compose::{ComposedStory, FrameText, PlacedGlyph};
use designcraft_fonts::{FontDb, FontFace};
use designcraft_geom::kurbo::Shape as _;
use designcraft_geom::{Affine, BezPath, Rect};
use krilla::geom::Point;
use krilla::paint::Stroke;
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};

use crate::export::{Exporter, solid_fill, tf, to_path};

/// Private-use characters DesignCraft stores for markers (page numbers, section markers, breaks).
fn is_marker(c: char) -> bool {
    ('\u{E000}'..='\u{E0FF}').contains(&c)
}

fn same_run(a: &PlacedGlyph, b: &PlacedGlyph) -> bool {
    b.visible
        && a.face.id() == b.face.id()
        && a.style == b.style
        && (a.sx - b.sx).abs() < 1e-9
        && (a.sy - b.sy).abs() < 1e-9
        && (a.y - b.y).abs() < 1e-6
}

impl Exporter<'_> {
    pub(crate) fn font(&mut self, face: &FontFace) -> Option<Font> {
        self.fonts.entry(face.id()).or_insert_with(|| Font::new(face.data().to_vec().into(), face.index())).clone()
    }

    fn reverse_cmap(&mut self, face: &FontFace) -> Arc<HashMap<u32, char>> {
        self.reverse_cmaps
            .entry(face.id())
            .or_insert_with(|| {
                let mut m = HashMap::new();
                // Sorted by code point: the lowest code point wins (`-` before U+2010).
                for (c, g) in face.chars() {
                    m.entry(g).or_insert(c);
                }
                Arc::new(m)
            })
            .clone()
    }

    pub(crate) fn frame_text(&mut self, s: &mut Surface, cs: &ComposedStory, ft: &FrameText, story: &str) {
        // Paragraph shading and rules under the text.
        for d in &ft.decos {
            if let Some(c) = self.swatch_color(&d.color, d.tint) {
                self.fill_rect(s, d.rect, c);
            }
        }
        let mut deco: Vec<(u32, Rect)> = Vec::new();
        for l in &ft.lines {
            let gs = &l.glyphs;
            let mut i = 0;
            while i < gs.len() {
                let g = &gs[i];
                if !g.visible || g.sx <= 0.0 || g.sy <= 0.0 {
                    i += 1;
                    continue;
                }
                let mut j = i + 1;
                while j < gs.len() && same_run(g, &gs[j]) {
                    j += 1;
                }
                self.run(s, cs, &gs[i..j], l.baseline, story);
                i = j;
            }
            // Underline / strikethrough, merged per style along the line.
            for g in gs.iter().filter(|g| g.visible) {
                let st = &cs.styles[g.style as usize];
                let w = st.size / 14.0;
                let mut add = |y: f64| {
                    let r = Rect::new(g.x, y, g.x + g.adv, y + w);
                    match deco.last_mut() {
                        Some((si, last)) if *si == g.style && (last.y0 - r.y0).abs() < 1e-6 && (r.x0 - last.x1).abs() < 0.5 => last.x1 = r.x1,
                        _ => deco.push((g.style, r)),
                    }
                };
                if st.underline {
                    add(l.baseline + st.size * 0.12);
                }
                if st.strikethrough {
                    add(l.baseline - st.size * 0.3);
                }
            }
        }
        for (si, r) in deco {
            let st = &cs.styles[si as usize];
            if let Some(c) = self.swatch_color(&st.fill, st.fill_tint) {
                self.fill_rect(s, r, c);
            }
        }
    }

    fn fill_rect(&self, s: &mut Surface, r: Rect, c: krilla::color::Color) {
        if let Some(p) = to_path(&r.to_path(0.1)) {
            s.set_stroke(None);
            s.set_fill(Some(solid_fill(c, 1.0)));
            s.draw_path(&p);
            s.set_fill(None);
        }
    }

    /// Unicode text for a run's glyphs: (text, per-glyph byte range into it).
    fn run_text(&mut self, glyphs: &[PlacedGlyph], story: &str) -> (String, Vec<Range<usize>>) {
        let cmap = self.reverse_cmap(&glyphs[0].face);
        let mut text = String::new();
        let mut ranges: Vec<Range<usize>> = Vec::with_capacity(glyphs.len());
        // Byte of the previous glyph when it came straight from the story (cluster continuation).
        let mut prev_src: Option<usize> = None;
        for g in glyphs {
            let src = story.get(g.byte..g.byte + g.len).filter(|t| !t.is_empty() && !t.chars().any(is_marker));
            let range = if let Some(t) = src {
                prev_src = Some(g.byte);
                let a = text.len();
                text.push_str(t);
                a..text.len()
            } else if g.len == 0 && prev_src == Some(g.byte) && !ranges.is_empty() {
                // Second glyph of a multi-glyph cluster: share the cluster's text.
                ranges[ranges.len() - 1].clone()
            } else {
                // Inserted glyphs (page numbers, list labels, hyphens): map back through the font.
                prev_src = None;
                match cmap.get(&g.gid) {
                    Some(c) => {
                        let a = text.len();
                        text.push(*c);
                        a..text.len()
                    }
                    None => match ranges.last() {
                        Some(r) => r.clone(),
                        None => {
                            text.push('\u{FFFD}');
                            0..text.len()
                        }
                    },
                }
            };
            ranges.push(range);
        }
        (text, ranges)
    }

    fn run(&mut self, s: &mut Surface, cs: &ComposedStory, glyphs: &[PlacedGlyph], baseline: f64, story: &str) {
        let g0 = &glyphs[0];
        let st = &cs.styles[g0.style as usize];
        let fill = self.swatch_color(&st.fill, st.fill_tint);
        let stroke =
            if st.stroke != designcraft_color::swatch::NONE && st.stroke_weight > 0.0 { self.swatch_color(&st.stroke, st.stroke_tint) } else { None };
        if fill.is_none() && stroke.is_none() {
            return;
        }
        let face = g0.face;
        let size = g0.sy * face.upem;
        let hs = g0.sx / g0.sy;
        let skew = if st.skew != 0.0 { Affine::new([1.0, 0.0, -st.skew.to_radians().tan(), 1.0, 0.0, 0.0]) } else { Affine::IDENTITY };
        let origin = Affine::translate((g0.x, baseline + g0.y)) * skew * Affine::scale_non_uniform(hs, 1.0);
        let Some(font) = self.font(&face) else {
            self.warn(format!("font `{} {}` could not be embedded; its text was drawn as outlines", face.family, face.style));
            self.outline_run(s, glyphs, origin, size, fill, stroke, st.stroke_weight);
            return;
        };
        let (text, ranges) = self.run_text(glyphs, story);
        let k = 1.0 / (hs * size);
        let kg: Vec<KrillaGlyph> = glyphs
            .iter()
            .zip(ranges)
            .enumerate()
            .map(|(i, (g, r))| {
                let next = glyphs.get(i + 1).map_or(g.x + g.adv, |n| n.x);
                KrillaGlyph::new(GlyphId::new(g.gid), ((next - g.x) * k) as f32, 0.0, 0.0, 0.0, r, None)
            })
            .collect();
        s.push_transform(&tf(origin));
        s.set_fill(fill.map(|c| solid_fill(c, 1.0)));
        s.set_stroke(stroke.map(|c| Stroke { paint: c.into(), width: st.stroke_weight as f32, ..Default::default() }));
        s.draw_glyphs(Point::from_xy(0.0, 0.0), &kg, font, &text, size as f32, false);
        s.set_fill(None);
        s.set_stroke(None);
        s.pop();
    }

    /// Fallback for faces krilla can't embed: glyph outlines as paths.
    #[allow(clippy::too_many_arguments)]
    fn outline_run(
        &mut self,
        s: &mut Surface,
        glyphs: &[PlacedGlyph],
        origin: Affine,
        size: f64,
        fill: Option<krilla::color::Color>,
        stroke: Option<krilla::color::Color>,
        stroke_weight: f64,
    ) {
        let db = FontDb::global();
        let g0 = &glyphs[0];
        let k = size / g0.face.upem;
        let mut bp = BezPath::new();
        for g in glyphs {
            let a = Affine::translate(((g.x - g0.x) / (g0.sx / g0.sy), 0.0)) * Affine::scale(k);
            let mut o = (*db.outline(&g.face, g.gid)).clone();
            o.apply_affine(a);
            bp.extend(o.iter());
        }
        let Some(p) = to_path(&bp) else { return };
        s.push_transform(&tf(origin));
        s.set_fill(fill.map(|c| solid_fill(c, 1.0)));
        s.set_stroke(stroke.map(|c| Stroke { paint: c.into(), width: stroke_weight as f32, ..Default::default() }));
        s.draw_path(&p);
        s.set_fill(None);
        s.set_stroke(None);
        s.pop();
    }

    /// One line of plain text in the bundled UI face (page information mark).
    pub(crate) fn plain_text(&mut self, s: &mut Surface, text: &str, at: (f64, f64), size: f64, color: krilla::color::Color) {
        let face = FontDb::global().face(designcraft_fonts::FALLBACK_FAMILY, "Regular");
        let Some(font) = self.font(&face) else { return };
        let shaped = designcraft_fonts::shape(&face, text, &[], |c| c);
        let upem = face.upem as f32;
        let glyphs: Vec<KrillaGlyph> = shaped
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let end = shaped.iter().skip(i + 1).map(|n| n.cluster).find(|c| *c != g.cluster).unwrap_or(text.len());
                let end = if end > g.cluster { end } else { text.len() };
                KrillaGlyph::new(
                    GlyphId::new(g.gid),
                    g.x_advance as f32 / upem,
                    g.x_offset as f32 / upem,
                    g.y_offset as f32 / upem,
                    0.0,
                    g.cluster..end,
                    None,
                )
            })
            .collect();
        s.set_stroke(None);
        s.set_fill(Some(solid_fill(color, 1.0)));
        s.draw_glyphs(Point::from_xy(at.0 as f32, at.1 as f32), &glyphs, font, text, size as f32, false);
        s.set_fill(None);
    }
}
