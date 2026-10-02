//! Composed text → pixels.
//!
//! Glyph outlines of a frame are transformed into frame space and batched per line and run style
//! once, then cached per (composed story allocation, frame id): a composed story is immutable, so
//! the cache entry stays valid for as long as the composition cache hands out the same `Arc`.
//! Each render only fills the lines that intersect the visible rectangle, and lines smaller than
//! the greeking threshold become grey bars without touching any outline.

use std::collections::HashMap;
use std::sync::Arc;

use designcraft_compose::{ComposedStory, FrameText, Line};
use designcraft_doc::ItemId;
use designcraft_fonts::FontDb;
use designcraft_geom::{Affine, BezPath, Rect, Shape};
use vello_cpu::RenderContext;
use vello_cpu::kurbo;
use vello_cpu::peniko;

use crate::{Frame, Renderer, color_of, rect_overlaps};

/// Cached outlines of one composed line.
pub(crate) struct LineGlyphs {
    /// Frame-space bounds of everything drawn for the line (empty lines: `None`).
    pub bounds: Option<Rect>,
    /// Glyph outlines batched per run style.
    pub runs: Vec<(u32, BezPath)>,
    /// Underline / strikethrough bars.
    pub decos: Vec<(u32, Rect)>,
    pub glyphs: usize,
}

pub(crate) struct FrameGlyphs {
    pub lines: Vec<LineGlyphs>,
    /// Path elements held (cache budget).
    pub elements: usize,
}

struct Entry {
    /// Keeps the composed story alive so its address can't be reused while cached.
    _story: Arc<ComposedStory>,
    glyphs: Arc<FrameGlyphs>,
    stamp: u64,
}

/// Path cache keyed by (composed story address, frame id).
#[derive(Default)]
pub(crate) struct GlyphCache {
    map: HashMap<(usize, ItemId), Entry>,
    elements: usize,
    clock: u64,
}

/// Roughly 56 bytes per element: ~110 MB of cached outlines.
const MAX_ELEMENTS: usize = 2_000_000;

impl GlyphCache {
    pub fn tick(&mut self) {
        self.clock += 1;
    }

    fn get(&mut self, cs: &Arc<ComposedStory>, ft: &FrameText) -> Arc<FrameGlyphs> {
        let key = (Arc::as_ptr(cs) as usize, ft.frame);
        let stamp = self.clock;
        if let Some(e) = self.map.get_mut(&key) {
            e.stamp = stamp;
            return e.glyphs.clone();
        }
        let g = Arc::new(build(cs, ft));
        self.elements += g.elements;
        self.map.insert(key, Entry { _story: cs.clone(), glyphs: g.clone(), stamp });
        if self.elements > MAX_ELEMENTS {
            self.evict();
        }
        g
    }

    /// Drop least recently used entries (never the ones used by the current render) until the
    /// cache is at half its budget.
    fn evict(&mut self) {
        let mut stamps: Vec<(u64, usize, (usize, ItemId))> = self.map.iter().map(|(k, e)| (e.stamp, e.glyphs.elements, *k)).collect();
        stamps.sort_unstable_by_key(|s| s.0);
        for (stamp, n, k) in stamps {
            if self.elements <= MAX_ELEMENTS / 2 || stamp >= self.clock {
                break;
            }
            self.map.remove(&k);
            self.elements -= n;
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.elements = 0;
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

fn transform_el(a: Affine, el: kurbo::PathEl) -> kurbo::PathEl {
    use kurbo::PathEl::*;
    match el {
        MoveTo(p) => MoveTo(a * p),
        LineTo(p) => LineTo(a * p),
        QuadTo(p1, p2) => QuadTo(a * p1, a * p2),
        CurveTo(p1, p2, p3) => CurveTo(a * p1, a * p2, a * p3),
        ClosePath => ClosePath,
    }
}

fn union(r: Option<Rect>, b: Rect) -> Option<Rect> {
    Some(r.map_or(b, |r| r.union(b)))
}

fn build_line(db: &FontDb, cs: &ComposedStory, l: &Line) -> LineGlyphs {
    let mut runs: Vec<(u32, BezPath)> = Vec::new();
    let mut decos = Vec::new();
    let mut glyphs = 0;
    for g in &l.glyphs {
        if !g.visible {
            continue;
        }
        let style = &cs.styles[g.style as usize];
        if style.underline || style.strikethrough {
            let w = style.size / 14.0;
            if style.underline {
                decos.push((g.style, Rect::new(g.x, l.baseline + style.size * 0.12, g.x + g.adv, l.baseline + style.size * 0.12 + w)));
            }
            if style.strikethrough {
                decos.push((g.style, Rect::new(g.x, l.baseline - style.size * 0.3, g.x + g.adv, l.baseline - style.size * 0.3 + w)));
            }
        }
        let outline = db.outline(&g.face, g.gid);
        if outline.elements().is_empty() {
            continue;
        }
        let skew = if style.skew != 0.0 { Affine::new([1.0, 0.0, -style.skew.to_radians().tan(), 1.0, 0.0, 0.0]) } else { Affine::IDENTITY };
        let a = Affine::translate((g.x, l.baseline + g.y)) * skew * Affine::scale_non_uniform(g.sx, g.sy);
        let bp = match runs.iter_mut().find(|r| r.0 == g.style) {
            Some(r) => &mut r.1,
            None => {
                runs.push((g.style, BezPath::new()));
                &mut runs.last_mut().expect("pushed").1
            }
        };
        for el in outline.elements() {
            bp.push(transform_el(a, *el));
        }
        glyphs += 1;
    }
    let mut bounds = None;
    for (si, bp) in &runs {
        let st = &cs.styles[*si as usize];
        let sw = if st.stroke != designcraft_color::swatch::NONE { st.stroke_weight } else { 0.0 };
        bounds = union(bounds, bp.bounding_box().inflate(sw, sw));
    }
    for (_, r) in &decos {
        bounds = union(bounds, *r);
    }
    LineGlyphs { bounds, runs, decos, glyphs }
}

fn build(cs: &ComposedStory, ft: &FrameText) -> FrameGlyphs {
    let db = FontDb::global();
    let lines: Vec<LineGlyphs> = ft.lines.iter().map(|l| build_line(db, cs, l)).collect();
    let elements = lines.iter().flat_map(|l| &l.runs).map(|(_, bp)| bp.elements().len()).sum();
    FrameGlyphs { lines, elements }
}

/// Conservative frame-space box of a line from its metrics (no outlines needed).
fn line_box(l: &Line) -> Option<Rect> {
    let first = l.glyphs.first()?;
    let x0 = l.glyphs.iter().map(|g| g.x).fold(first.x, f64::min);
    let x1 = l.glyphs.iter().map(|g| g.x + g.adv.max(0.0)).fold(l.end_x, f64::max);
    let pad = l.ascent.max(1.0);
    Some(Rect::new(x0 - pad, l.baseline - l.ascent - pad, x1 + pad, l.baseline + l.descent.max(0.0) + pad))
}

impl Renderer {
    pub(crate) fn draw_text(&mut self, ctx: &mut RenderContext, f: &Frame, cs: &Arc<ComposedStory>, ft: &FrameText, xf: Affine) {
        let doc = f.doc;
        let m = f.view * xf;
        // Visible area in frame space.
        if xf.determinant().abs() < 1e-12 {
            return;
        }
        let vis = xf.inverse().transform_rect_bbox(f.visible);
        // Decorations under text (shading) and rules.
        for d in &ft.decos {
            if !rect_overlaps(d.rect, vis) {
                continue;
            }
            if let Some(c) = doc.resolve_color(&d.color, d.tint) {
                ctx.set_transform(m);
                ctx.set_paint(color_of(&c, 1.0));
                ctx.fill_rect(&d.rect);
            }
        }
        if !ft.tables.is_empty() {
            self.draw_tables(ctx, f, ft, xf);
        }
        let scale = m.determinant().abs().sqrt();
        let greek_px = f.opts.greek_below_px;
        let mut greek = BezPath::new();
        let mut shown: Vec<usize> = Vec::new();
        for (i, l) in ft.lines.iter().enumerate() {
            let Some(b) = line_box(l) else { continue };
            if !rect_overlaps(b, vis) {
                continue;
            }
            if greek_px > 0.0 && l.ascent * scale < greek_px {
                if let Some(a) = l.glyphs.first() {
                    let r = kurbo::Rect::new(a.x, l.baseline - l.ascent * 0.5, l.end_x, l.baseline);
                    if r.width() > 0.0 {
                        greek.extend(r.path_elements(0.1));
                    }
                }
                continue;
            }
            shown.push(i);
        }
        ctx.set_transform(m);
        if !shown.is_empty() {
            let fg = self.glyphs.get(cs, ft);
            // Resolve each style's paints once per frame.
            let mut fills: Vec<Option<Option<peniko::Color>>> = vec![None; cs.styles.len()];
            let mut fill_of = |si: u32| -> Option<peniko::Color> {
                *fills[si as usize].get_or_insert_with(|| {
                    let st = &cs.styles[si as usize];
                    doc.resolve_color(&st.fill, st.fill_tint).map(|c| color_of(&c, 1.0))
                })
            };
            for i in shown {
                let lg = &fg.lines[i];
                let Some(b) = lg.bounds else { continue };
                if !rect_overlaps(b, vis) {
                    continue;
                }
                self.stats.glyphs += lg.glyphs;
                for (si, bp) in &lg.runs {
                    if let Some(c) = fill_of(*si) {
                        ctx.set_paint(c);
                        ctx.fill_path(bp);
                    }
                    let st = &cs.styles[*si as usize];
                    if st.stroke != designcraft_color::swatch::NONE
                        && let Some(c) = doc.resolve_color(&st.stroke, st.stroke_tint)
                    {
                        ctx.set_paint(color_of(&c, 1.0));
                        ctx.set_stroke(kurbo::Stroke::new(st.stroke_weight));
                        ctx.stroke_path(bp);
                    }
                }
                for (si, r) in &lg.decos {
                    ctx.set_paint(fill_of(*si).unwrap_or(peniko::Color::BLACK));
                    ctx.fill_rect(r);
                }
            }
        }
        if !greek.elements().is_empty() {
            ctx.set_paint(color_of(&designcraft_color::Color::gray(0.35), 0.5));
            ctx.fill_path(&greek);
        }
        for n in &ft.notes {
            if let Some(nft) = n.text.frames.first() {
                self.draw_text(ctx, f, &n.text, nft, xf * Affine::translate(n.origin.to_vec2()));
            }
        }
    }
}

impl Renderer {
    /// Table fragments: cell fills, cell text, then cell edges and the border.
    fn draw_tables(&mut self, ctx: &mut RenderContext, f: &Frame, ft: &FrameText, xf: Affine) {
        let doc = f.doc;
        for t in &ft.tables {
            for c in &t.cells {
                if let Some((sw, tint)) = &c.fill
                    && let Some(col) = doc.resolve_color(sw, *tint)
                {
                    ctx.set_transform(f.view * xf);
                    ctx.set_paint(color_of(&col, 1.0));
                    // Overlap neighbours by half a device pixel so adjacent fills show no seams.
                    let h = 0.5 * f.px;
                    ctx.fill_rect(&c.rect.inflate(h, h));
                }
            }
            for c in &t.cells {
                if let Some(cft) = c.text.frames.first() {
                    self.draw_text(ctx, f, &c.text, cft, xf * Affine::translate(c.origin.to_vec2()));
                }
            }
            ctx.set_transform(f.view * xf);
            for s in &t.strokes {
                let Some(col) = doc.resolve_color(&s.stroke.color, s.stroke.tint) else { continue };
                ctx.set_paint(color_of(&col, 1.0));
                ctx.set_stroke(cell_stroke(&s.stroke));
                ctx.stroke_path(&kurbo::Line::new(s.a, s.b).to_path(0.1));
            }
        }
    }
}

/// Kurbo stroke for a table edge.
fn cell_stroke(s: &designcraft_doc::CellStroke) -> kurbo::Stroke {
    let st = kurbo::Stroke::new(s.weight).with_caps(kurbo::Cap::Square);
    match &s.kind {
        designcraft_doc::StrokeType::Dashed { pattern } if !pattern.is_empty() => {
            st.with_caps(kurbo::Cap::Butt).with_dashes(0.0, pattern.iter().copied())
        }
        designcraft_doc::StrokeType::Dotted => st.with_dashes(0.0, [0.0, s.weight * 2.0]).with_caps(kurbo::Cap::Round),
        _ => st,
    }
}
