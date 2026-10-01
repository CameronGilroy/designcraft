//! DesignCraft text composition.
//!
//! [`compose_story`] lays a story out through its chain of threaded frames:
//! style resolution → shaping ([`shape`]) → line breaking ([`breaker`]: the paragraph composer is
//! Knuth–Plass total fit, the single-line composer is greedy) → placement in columns and frames
//! (first-baseline offset, leading, space before/after, baseline grid, text wrap, column/frame
//! breaks, vertical justification) → overset detection.
//!
//! Output coordinates are each frame's *inner* space (apply the frame item's `xf` for spread space).
#![forbid(unsafe_code)]

pub mod breaker;
mod cache;
pub mod hyphen;
pub mod shape;

use std::ops::Range;
use std::sync::Arc;

use designcraft_doc::{
    Align, Composer, Document, FirstBaseline, GridAlign, ItemId, ParaProps, SpanColumns, StartParagraph, Story, StoryId, TabAlign, TextFrameOptions,
    VerticalJustification, WrapMode, story,
};
use designcraft_fonts::{FontDb, FontFace};
use designcraft_geom::{Point, Rect};

use crate::breaker::{Break, Spacing};
pub use crate::cache::Cache;
use crate::shape::{Glyph, StyleTable, SubstCtx};

/// Character appearance shared by many glyphs (indexed from [`PlacedGlyph::style`]).
#[derive(Clone, Debug, PartialEq)]
pub struct RunStyle {
    pub fill: String,
    pub fill_tint: f32,
    pub stroke: String,
    pub stroke_tint: f32,
    pub stroke_weight: f64,
    pub underline: bool,
    pub strikethrough: bool,
    pub skew: f64,
    pub size: f64,
}

/// A positioned glyph. `x` is absolute in frame inner space; `y` is relative to the line baseline.
#[derive(Clone, Debug)]
pub struct PlacedGlyph {
    pub face: Arc<FontFace>,
    pub gid: u32,
    pub x: f64,
    pub y: f64,
    pub adv: f64,
    pub sx: f64,
    pub sy: f64,
    pub style: u32,
    pub byte: usize,
    pub len: usize,
    /// Control characters (tabs, breaks, markers' carriers) are not drawn.
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub column: u32,
    /// Baseline y in frame inner space.
    pub baseline: f64,
    /// Line slot (column minus wrap) horizontal extent.
    pub x0: f64,
    pub x1: f64,
    pub ascent: f64,
    pub descent: f64,
    pub leading: f64,
    /// Story byte range (excluding the paragraph separator).
    pub range: Range<usize>,
    pub para: usize,
    pub glyphs: Vec<PlacedGlyph>,
    pub hyphenated: bool,
    pub first_in_para: bool,
    pub last_in_para: bool,
    /// x after the last glyph (caret at line end).
    pub end_x: f64,
    /// Word-space ratio actually used vs desired (H&J violation highlighting), 1.0 = desired.
    pub spacing: f64,
}

/// A paragraph rule or shading rectangle in frame inner space.
#[derive(Clone, Debug, PartialEq)]
pub struct Deco {
    pub rect: Rect,
    pub color: String,
    pub tint: f32,
}

#[derive(Clone, Debug, Default)]
pub struct FrameText {
    pub frame: ItemId,
    pub lines: Vec<Line>,
    pub decos: Vec<Deco>,
    /// Byte range of the story shown in this frame.
    pub range: Range<usize>,
    /// Column rectangles (inner space) for drawing column guides / hit testing.
    pub columns: Vec<Rect>,
    /// Height the text needs (for auto-sizing), from the text area top.
    pub content_height: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ComposedStory {
    pub story: StoryId,
    pub rev: u64,
    pub frames: Vec<FrameText>,
    /// First byte that didn't fit (overset text), if any.
    pub overset_at: Option<usize>,
    pub styles: Vec<RunStyle>,
    pub text_len: usize,
}

impl ComposedStory {
    pub fn is_overset(&self) -> bool {
        self.overset_at.is_some()
    }
    pub fn frame(&self, id: ItemId) -> Option<&FrameText> {
        self.frames.iter().find(|f| f.frame == id)
    }
    /// Number of lines in all frames.
    pub fn line_count(&self) -> usize {
        self.frames.iter().map(|f| f.lines.len()).sum()
    }
}

/// A text-wrap exclusion in a frame's inner space.
#[derive(Clone, Debug, PartialEq)]
pub struct Exclusion {
    pub rect: Rect,
    pub mode: WrapMode,
}

/// One frame of the thread, ready for composition.
#[derive(Clone, Debug)]
pub struct FrameSpec {
    pub id: ItemId,
    /// Text area (inner space, after inset).
    pub area: Rect,
    pub opts: TextFrameOptions,
    pub exclusions: Vec<Exclusion>,
    pub page_name: Option<String>,
    /// Baseline grid in inner space: (first grid line y, increment).
    pub grid: Option<(f64, f64)>,
    /// The frame is on a left page (for towards/away-from-spine alignment).
    pub left_page: bool,
}

impl FrameSpec {
    pub fn columns(&self) -> Vec<Rect> {
        let a = self.area;
        let n = self.opts.columns.max(1);
        match self.opts.columns_kind {
            designcraft_doc::ColumnsKind::FixedWidth if self.opts.column_width > 0.0 => (0..n)
                .map(|i| {
                    let x = a.x0 + i as f64 * (self.opts.column_width + self.opts.gutter);
                    Rect::new(x, a.y0, x + self.opts.column_width, a.y1)
                })
                .collect(),
            _ => designcraft_doc::page::column_rects(a, n, self.opts.gutter),
        }
    }
}

/// Composition inputs that are not in the story itself.
#[derive(Clone, Debug, Default)]
pub struct ComposeOptions {
    /// Page name for page-number markers when the story is composed for a specific page (parent items).
    pub page_name: Option<String>,
}

/// Build the frame specs of a story from the document (geometry, wrap, page names, grid).
pub fn frame_specs(doc: &Document, sid: StoryId) -> Vec<FrameSpec> {
    let Some(st) = doc.story(sid) else { return vec![] };
    let mut out = Vec::with_capacity(st.frames.len());
    for &fid in &st.frames {
        let Some(loc) = doc.find(fid) else { continue };
        let Some(item) = doc.item_at(&loc) else { continue };
        let Some(tf) = item.text_frame() else { continue };
        let xf = doc.parent_xf(&loc) * item.xf;
        let inv = xf.inverse();
        let mut exclusions = Vec::new();
        let spread = doc.spread(loc.spread);
        if !tf.options.ignore_wrap
            && let Some(sp) = spread
        {
            for other in &sp.items {
                if other.id == item.id || other.wrap.mode == WrapMode::None || other.hidden {
                    continue;
                }
                if doc.layer(other.layer).is_some_and(|l| !l.visible) {
                    continue;
                }
                let o = other.wrap.offsets;
                let b = other.bounds();
                let r = Rect::new(b.x0 - o[1], b.y0 - o[0], b.x1 + o[3], b.y1 + o[2]);
                let inner = inv.transform_rect_bbox(r);
                exclusions.push(Exclusion { rect: inner, mode: other.wrap.mode });
            }
        }
        let (page_name, left_page) = match (loc.spread, spread) {
            (designcraft_doc::SpreadRef::Doc(si), Some(sp)) => {
                let c = item.bounds().center();
                let pi = sp.page_at_x(c.x).unwrap_or(0);
                let abs = doc.first_page_of_spread(si) + pi;
                (Some(doc.page_name(abs)), sp.pages.get(pi).is_some_and(|p| p.side == designcraft_doc::PageSide::Left))
            }
            (designcraft_doc::SpreadRef::Parent(pi), Some(sp)) => {
                let prefix = sp.parent.as_ref().map(|p| p.prefix.clone()).unwrap_or_else(|| "A".into());
                let _ = pi;
                (Some(prefix), false)
            }
            _ => (None, false),
        };
        let g = &doc.settings.baseline_grid;
        let (inc, start) = tf.options.baseline_grid.unwrap_or((g.increment, g.start));
        // Grid lines are at spread y = start + n·inc (page tops are y = 0); map into inner space (translation only).
        let ty = xf.translation().y;
        let grid = (inc > 0.0).then_some((start - ty, inc));
        out.push(FrameSpec { id: fid, area: item.text_area(), opts: tf.options.clone(), exclusions, page_name, grid, left_page });
    }
    out
}

/// Compose a story of `doc` through its frames.
pub fn compose_story(doc: &Document, sid: StoryId, opts: &ComposeOptions) -> ComposedStory {
    let specs = frame_specs(doc, sid);
    match doc.story(sid) {
        Some(st) => compose(doc, st, &specs, opts),
        None => ComposedStory { story: sid, ..Default::default() },
    }
}

const DEFAULT_TAB: f64 = 36.0;

/// Compose `story` into `frames`.
pub fn compose(doc: &Document, story: &Story, frames: &[FrameSpec], opts: &ComposeOptions) -> ComposedStory {
    let db = FontDb::global();
    let mut out = ComposedStory { story: story.id, rev: story.rev, text_len: story.text.len(), ..Default::default() };
    let mut styles_tab: Vec<RunStyle> = Vec::new();
    out.frames = frames.iter().map(|f| FrameText { frame: f.id, columns: f.columns(), ..Default::default() }).collect();
    let cols: Vec<Vec<Rect>> = frames.iter().map(FrameSpec::columns).collect();
    let mut cur = Cursor { fi: 0, col: 0, last_baseline: None, last_descent: 0.0, pending: 0.0 };
    let para_ranges = story.para_ranges();
    let mut list_counter: u32 = 0;
    'paras: for (pi, prange) in para_ranges.iter().enumerate() {
        let pf = &story.paras[pi];
        let (pp, base_chars) = doc.styles.resolve_para(pf);
        // Bullets & numbering: generated prefix (shaped as its own glyphs, mapped to the paragraph start).
        let sub = SubstCtx {
            page_name: opts.page_name.clone().or_else(|| frames.get(cur.fi.min(frames.len().saturating_sub(1))).and_then(|f| f.page_name.clone())),
            section_marker: None,
        };
        let mut table = StyleTable { styles: &mut styles_tab };
        let mut sp = shape::shape_para(db, &doc.styles, story, pi, prange.clone(), &base_chars, pp.auto_leading, &sub, &mut table);
        match pp.list_type {
            designcraft_doc::ListType::Numbers => {
                list_counter += 1;
                let label = format!("{}.{}", pp.number_style.format(list_counter), pp.list_separator);
                prepend_label(db, &mut sp.glyphs, &label, prange.start, &base_chars, &pp, &mut table);
            }
            designcraft_doc::ListType::Bullets => {
                let label = format!("{}{}", pp.bullet_char, pp.list_separator);
                prepend_label(db, &mut sp.glyphs, &label, prange.start, &base_chars, &pp, &mut table);
            }
            designcraft_doc::ListType::None => list_counter = 0,
        }
        let glyphs = sp.glyphs;
        let hyph_after = hyphenation_points(&story.text, &glyphs, &pp);
        let base_size = base_chars.size;
        let base_leading = match base_chars.leading {
            designcraft_doc::Leading::Auto => base_size * pp.auto_leading,
            designcraft_doc::Leading::Points(v) => v,
        };
        let spacing = Spacing {
            justify: pp.align.is_justified(),
            word_min: pp.word_space_min,
            word_desired: pp.word_space_desired,
            word_max: pp.word_space_max,
            hyphen_penalty: 50.0 + 450.0 * pp.hyph_weight,
            hyphen_limit: pp.hyph_limit,
            ragged_stretch: base_size * 2.0,
        };
        // Paragraph start options.
        if cur.last_baseline.is_some() {
            match pp.start_paragraph {
                StartParagraph::NextColumn => cur.next_column(&cols),
                StartParagraph::NextFrame | StartParagraph::NextPage | StartParagraph::NextOddPage | StartParagraph::NextEvenPage => cur.next_frame(),
                StartParagraph::Anywhere => {}
            }
        }
        if cur.last_baseline.is_some() {
            cur.pending += pp.space_before;
        }
        // Rule above / shading track the paragraph's first line.
        let mut g0 = 0usize;
        let mut line_no = 0usize;
        let has_tabs = glyphs.iter().any(|g| g.ch == '\t');
        let mut first_line_rect: Option<(usize, f64, f64, f64)> = None; // frame, baseline, ascent, x-span
        loop {
            if cur.fi >= frames.len() {
                out.overset_at = Some(glyphs.get(g0).map(|g| g.byte).unwrap_or(prange.start));
                break 'paras;
            }
            let f = &frames[cur.fi];
            let col = cols[cur.fi][cur.col.min(cols[cur.fi].len() - 1)];
            let col = match pp.span_columns {
                SpanColumns::Span(n) if cur.col == 0 => {
                    let n = if n == 0 { cols[cur.fi].len() } else { (n as usize).min(cols[cur.fi].len()) };
                    Rect::new(col.x0, col.y0, cols[cur.fi][n - 1].x1, col.y1)
                }
                _ => col,
            };
            // Estimate slots for the breaker with the paragraph's base leading.
            let est_first = cur.next_baseline(f, col, base_leading, base_chars.size * 0.75, &pp);
            let slots = estimate_slots(f, col, est_first, base_leading, base_chars.size, &glyphs[g0..], &pp, line_no);
            let width = |j: usize| -> f64 {
                let (x0, x1) = slots.get(j).copied().unwrap_or((col.x0, col.x1));
                let ind = pp.left_indent + pp.right_indent + if line_no + j == 0 { pp.first_line_indent } else { 0.0 };
                (x1 - x0 - ind).max(1.0)
            };
            let rest = &glyphs[g0..];
            let rest_h = &hyph_after[g0..];
            let breaks: Vec<Break> = if pp.composer == Composer::SingleLine || has_tabs || rest.len() > 4000 {
                breaker::greedy(rest, rest_h, &spacing, &width)
            } else {
                breaker::knuth_plass(rest, rest_h, &spacing, &width)
            };
            let mut moved = false;
            for (k, b) in breaks.iter().enumerate() {
                let (s, e) = (g0 + b.start, g0 + b.end);
                let line_glyphs = &glyphs[s..e.max(s)];
                let (asc, desc, lead) = line_metrics(line_glyphs, &glyphs, s, base_leading, base_chars.size, db, &base_chars);
                let mut baseline = cur.next_baseline(f, col, lead, asc, &pp);
                // Baseline grid.
                if let Some((g_start, inc)) = f.grid
                    && (pp.grid_align == GridAlign::AllLines || (pp.grid_align == GridAlign::FirstLineOnly && line_no == 0))
                    && inc > 0.0
                {
                    let n = ((baseline - g_start) / inc - 1e-6).ceil();
                    baseline = g_start + n * inc;
                }
                // Wrap: push the line down until a slot exists.
                let (mut x0, mut x1) = (col.x0, col.x1);
                if !f.exclusions.is_empty() {
                    let mut tries = 0;
                    loop {
                        match free_slot(f, col, baseline - asc, baseline + desc, base_size) {
                            Some((a, b)) => {
                                x0 = a;
                                x1 = b;
                                break;
                            }
                            None => {
                                baseline += 1.0;
                                tries += 1;
                                if baseline > col.y1 || tries > 4000 {
                                    break;
                                }
                            }
                        }
                    }
                }
                let fits = baseline + desc <= col.y1 + 0.01 || (cur.last_baseline.is_none() && baseline <= col.y1 && false);
                if !fits {
                    // Next column / frame; re-break the rest of the paragraph there.
                    g0 = s;
                    cur.next_column(&cols);
                    moved = true;
                    let _ = k;
                    break;
                }
                let ind_l = pp.left_indent + if line_no == 0 { pp.first_line_indent } else { 0.0 };
                let lx0 = x0 + ind_l;
                let lx1 = x1 - pp.right_indent;
                let last = k + 1 == breaks.len();
                let (placed, end_x, ratio) = layout_line(&glyphs, s, e, b.hyphen, lx0, lx1, col.x0, &pp, last, b.forced && !last, f.left_page);
                let range_end = if last { prange.end } else { glyphs.get(g0 + b.next).map(|g| g.byte).unwrap_or(prange.end) };
                let range_start = glyphs.get(s).map(|g| g.byte).unwrap_or(prange.start).min(range_end);
                let range_start = if line_no == 0 { prange.start } else { range_start };
                let ft = &mut out.frames[cur.fi];
                if line_no == 0 {
                    first_line_rect = Some((cur.fi, baseline, asc, x0));
                }
                ft.lines.push(Line {
                    column: cur.col as u32,
                    baseline,
                    x0,
                    x1,
                    ascent: asc,
                    descent: desc,
                    leading: lead,
                    range: range_start..range_end,
                    para: pi,
                    glyphs: placed,
                    hyphenated: b.hyphen,
                    first_in_para: line_no == 0,
                    last_in_para: last,
                    end_x,
                    spacing: ratio,
                });
                cur.last_baseline = Some(baseline);
                cur.last_descent = desc;
                cur.pending = 0.0;
                line_no += 1;
                // Column / frame / page break characters.
                if b.forced && e < glyphs.len() + 1 {
                    let brk = glyphs.get(g0 + b.next.saturating_sub(1)).map(|g| g.ch);
                    match brk {
                        Some(story::COLUMN_BREAK) => cur.next_column(&cols),
                        Some(story::FRAME_BREAK) | Some(story::PAGE_BREAK) => cur.next_frame(),
                        _ => {}
                    }
                }
            }
            if !moved {
                break;
            }
        }
        // Rules and shading for the paragraph.
        if let Some((fi, bl, asc, _)) = first_line_rect {
            let ft = &mut out.frames[fi];
            if pp.rule_above.on {
                let r = &pp.rule_above;
                let y = bl - asc - r.offset;
                let col = ft.lines.iter().rev().find(|l| l.para == pi).map(|l| (l.x0, l.x1)).unwrap_or((0.0, 0.0));
                ft.decos.push(Deco { rect: Rect::new(col.0 + r.left_indent, y - r.weight, col.1 - r.right_indent, y), color: r.color.clone(), tint: r.tint });
            }
            if pp.shading_on {
                let lines: Vec<&Line> = ft.lines.iter().filter(|l| l.para == pi).collect();
                if let (Some(a), Some(z)) = (lines.first(), lines.last()) {
                    ft.decos.insert(0, Deco { rect: Rect::new(a.x0, a.baseline - a.ascent, a.x1, z.baseline + z.descent), color: pp.shading_color.clone(), tint: pp.shading_tint });
                }
            }
        }
        if pp.rule_below.on
            && let Some(ft) = out.frames.get_mut(cur.fi.min(frames.len().saturating_sub(1)))
            && let Some(l) = ft.lines.iter().rev().find(|l| l.para == pi)
        {
            let r = &pp.rule_below;
            let y = l.baseline + r.offset;
            let (x0, x1) = if r.column_width { (l.x0, l.x1) } else { (l.x0, l.end_x) };
            ft.decos.push(Deco { rect: Rect::new(x0 + r.left_indent, y, x1 - r.right_indent, y + r.weight), color: r.color.clone(), tint: r.tint });
        }
        cur.pending += pp.space_after;
    }
    // Ranges, content heights and vertical justification.
    for (fi, ft) in out.frames.iter_mut().enumerate() {
        let f = &frames[fi];
        if let (Some(a), Some(z)) = (ft.lines.first(), ft.lines.last()) {
            ft.range = a.range.start..z.range.end;
            ft.content_height = z.baseline + z.descent - f.area.y0;
        } else {
            let p = ft_prev_end(&out.overset_at, story.text.len());
            ft.range = p..p;
        }
        vertical_justify(ft, f);
    }
    // Frame ranges for empty frames after the text: start at the end of the text shown.
    let mut last_end = 0;
    for ft in &mut out.frames {
        if ft.lines.is_empty() {
            let p = out.overset_at.unwrap_or(last_end).max(last_end);
            ft.range = p..p;
        } else {
            last_end = ft.range.end;
        }
    }
    out.styles = styles_tab;
    out
}

fn ft_prev_end(overset: &Option<usize>, len: usize) -> usize {
    overset.unwrap_or(len)
}

struct Cursor {
    fi: usize,
    col: usize,
    last_baseline: Option<f64>,
    last_descent: f64,
    /// Space before/after waiting to be added to the next line.
    pending: f64,
}

impl Cursor {
    fn next_column(&mut self, cols: &[Vec<Rect>]) {
        self.col += 1;
        if self.fi < cols.len() && self.col >= cols[self.fi].len() {
            self.col = 0;
            self.fi += 1;
        }
        self.last_baseline = None;
        self.pending = 0.0;
    }
    fn next_frame(&mut self) {
        self.fi += 1;
        self.col = 0;
        self.last_baseline = None;
        self.pending = 0.0;
    }
    /// Baseline for the next line with leading `lead` and ascent `asc`.
    fn next_baseline(&self, f: &FrameSpec, col: Rect, lead: f64, asc: f64, _pp: &ParaProps) -> f64 {
        match self.last_baseline {
            Some(b) => b + lead + self.pending,
            None => {
                let off = match f.opts.first_baseline {
                    FirstBaseline::Ascent => asc,
                    FirstBaseline::CapHeight => asc * 0.72,
                    FirstBaseline::XHeight => asc * 0.5,
                    FirstBaseline::Leading => lead,
                    FirstBaseline::Fixed => 0.0,
                };
                col.y0 + off.max(f.opts.first_baseline_min)
            }
        }
    }
}

fn line_metrics(line: &[Glyph], all: &[Glyph], s: usize, base_leading: f64, base_size: f64, db: &FontDb, base: &designcraft_doc::CharProps) -> (f64, f64, f64) {
    let _ = db;
    let src: &[Glyph] = if line.is_empty() { all.get(s..(s + 1).min(all.len())).unwrap_or(&[]) } else { line };
    if src.is_empty() {
        let face = FontDb::global().face(&base.font_family, &base.font_style);
        let k = base_size / face.upem;
        return (face.ascent * k, face.descent * k, base_leading);
    }
    let mut asc: f64 = 0.0;
    let mut desc: f64 = 0.0;
    let mut lead: f64 = 0.0;
    for g in src {
        asc = asc.max(g.ascent + g.shift.max(0.0));
        desc = desc.max(g.descent - g.shift.min(0.0));
        lead = lead.max(g.leading);
    }
    (asc, desc, lead)
}

/// Widest free horizontal interval of `col` in the band, or None if blocked.
fn free_slot(f: &FrameSpec, col: Rect, y0: f64, y1: f64, size: f64) -> Option<(f64, f64)> {
    let mut free = vec![(col.x0, col.x1)];
    for ex in &f.exclusions {
        let r = ex.rect;
        let overlaps_band = r.y0 < y1 && r.y1 > y0;
        match ex.mode {
            WrapMode::JumpToNextColumn if y1 > r.y0 && r.x0 < col.x1 && r.x1 > col.x0 => return None,
            WrapMode::JumpObject if overlaps_band && r.x0 < col.x1 && r.x1 > col.x0 => return None,
            WrapMode::BoundingBox | WrapMode::Contour if overlaps_band => {
                let mut next = Vec::new();
                for (a, b) in free {
                    if r.x1 <= a || r.x0 >= b {
                        next.push((a, b));
                        continue;
                    }
                    if r.x0 > a {
                        next.push((a, r.x0));
                    }
                    if r.x1 < b {
                        next.push((r.x1, b));
                    }
                }
                free = next;
            }
            _ => {}
        }
    }
    free.into_iter().filter(|(a, b)| b - a >= size * 1.5).max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0)))
}

#[allow(clippy::too_many_arguments)]
fn estimate_slots(f: &FrameSpec, col: Rect, first: f64, lead: f64, size: f64, glyphs: &[Glyph], pp: &ParaProps, line_no: usize) -> Vec<(f64, f64)> {
    let _ = (pp, line_no);
    if f.exclusions.is_empty() {
        return vec![];
    }
    // Rough upper bound on lines: total advance / column width × 2.
    let total: f64 = glyphs.iter().map(|g| g.adv).sum();
    let n = ((total / (col.width().max(10.0) * 0.5)).ceil() as usize + 2).min(2000);
    let mut v = Vec::with_capacity(n);
    let mut b = first;
    for _ in 0..n {
        let mut tries = 0;
        let slot = loop {
            if let Some(s) = free_slot(f, col, b - size * 0.8, b + size * 0.25, size) {
                break s;
            }
            b += 1.0;
            tries += 1;
            if b > col.y1 || tries > 4000 {
                break (col.x0, col.x1);
            }
        };
        v.push(slot);
        b += lead;
    }
    v
}

/// Position glyphs `s..e` within `[x0, x1]`; returns (glyphs, end x, word-space ratio).
#[allow(clippy::too_many_arguments)]
fn layout_line(
    glyphs: &[Glyph],
    s: usize,
    e: usize,
    hyphen: bool,
    x0: f64,
    x1: f64,
    tab_origin: f64,
    pp: &ParaProps,
    last: bool,
    forced_mid: bool,
    left_page: bool,
) -> (Vec<PlacedGlyph>, f64, f64) {
    let mut line: Vec<Glyph> = glyphs[s..e.max(s)].to_vec();
    if hyphen && let Some(g) = line.last() {
        let h = shape::hyphen_after(g);
        line.push(h);
    }
    // Tabs: compute widths left to right.
    let measure = x1 - x0;
    let mut x = 0.0;
    let mut i = 0;
    while i < line.len() {
        if line[i].ch == '\t' {
            let abs = x0 + x - tab_origin;
            let stop = pp.tabs.iter().find(|t| t.position > abs + 0.01).cloned();
            let (pos, align) = match &stop {
                Some(t) => (t.position, t.align),
                None => (((abs / DEFAULT_TAB).floor() + 1.0) * DEFAULT_TAB, TabAlign::Left),
            };
            // Width of the text after this tab up to the next tab / line end.
            let seg_end = line[i + 1..].iter().position(|g| g.ch == '\t').map_or(line.len(), |p| i + 1 + p);
            let seg_w: f64 = line[i + 1..seg_end].iter().map(|g| g.adv).sum();
            let target = pos + tab_origin - x0;
            let w = match align {
                TabAlign::Left => target - x,
                TabAlign::Right => target - x - seg_w,
                TabAlign::Center => target - x - seg_w / 2.0,
                TabAlign::Char => {
                    let ch = stop.as_ref().and_then(|t| t.align_on.chars().next()).unwrap_or('.');
                    let before: f64 = line[i + 1..seg_end].iter().take_while(|g| g.ch != ch).map(|g| g.adv).sum();
                    target - x - before
                }
            };
            line[i].adv = w.max(0.0);
        } else if line[i].ch == story::RIGHT_INDENT_TAB {
            let rest: f64 = line[i + 1..].iter().map(|g| g.adv).sum();
            line[i].adv = (measure - x - rest).max(0.0);
        }
        x += line[i].adv;
        i += 1;
    }
    let natural: f64 = line.iter().map(|g| g.adv).sum();
    let extra = measure - natural;
    let spaces: Vec<usize> = line.iter().enumerate().filter(|(_, g)| g.is_space() && !g.no_break).map(|(i, _)| i).collect();
    let has_tab = line.iter().any(|g| g.ch == '\t' || g.ch == story::RIGHT_INDENT_TAB);
    let align = match pp.align {
        Align::TowardsSpine => {
            if left_page {
                Align::Right
            } else {
                Align::Left
            }
        }
        Align::AwayFromSpine => {
            if left_page {
                Align::Left
            } else {
                Align::Right
            }
        }
        a => a,
    };
    let justify_this = align.is_justified() && (!last || align == Align::FullyJustified || forced_mid) && !has_tab;
    let mut per_space = 0.0;
    let mut offset = 0.0;
    let mut ratio = 1.0;
    if justify_this && !spaces.is_empty() {
        per_space = extra / spaces.len() as f64;
        let space_w: f64 = spaces.iter().map(|&i| line[i].adv).sum::<f64>() / spaces.len() as f64;
        ratio = if space_w > 0.0 { (space_w + per_space) / space_w * pp.word_space_desired } else { 1.0 };
    } else if justify_this && spaces.is_empty() && line.len() > 1 && !last {
        // Single word: Single Word Justification.
        match pp.single_word_justify {
            Align::FullyJustified => {
                let gaps = (line.len() - 1) as f64;
                let per = extra / gaps;
                let mut x = x0;
                let mut out = Vec::with_capacity(line.len());
                for (i, g) in line.iter().enumerate() {
                    out.push(place(g, x));
                    x += g.adv + if i + 1 < line.len() { per } else { 0.0 };
                }
                return (out, x, 1.0);
            }
            Align::Center => offset = extra / 2.0,
            Align::Right => offset = extra,
            _ => {}
        }
    } else {
        let last_align = match align {
            Align::LeftJustified => Align::Left,
            Align::CenterJustified => Align::Center,
            Align::RightJustified => Align::Right,
            Align::FullyJustified => Align::Left,
            a => a,
        };
        offset = match last_align {
            Align::Center => extra / 2.0,
            Align::Right => extra,
            _ => 0.0,
        };
        // Overfull ragged lines are not shifted left of the slot.
        if offset < 0.0 {
            offset = 0.0;
        }
    }
    let mut out = Vec::with_capacity(line.len());
    let mut x = x0 + offset;
    for (i, g) in line.iter().enumerate() {
        out.push(place(g, x));
        x += g.adv;
        if per_space != 0.0 && spaces.binary_search(&i).is_ok() {
            x += per_space;
            if let Some(p) = out.last_mut() {
                p.adv += per_space;
            }
        }
    }
    (out, x, ratio)
}

fn place(g: &Glyph, x: f64) -> PlacedGlyph {
    let visible = !(g.ch == '\t' || g.ch == '\n' || breaker::is_forced(g.ch) || g.ch == story::INDENT_HERE || g.ch == story::RIGHT_INDENT_TAB || g.ch == shape::SOFT_HYPHEN);
    PlacedGlyph { face: g.face.clone(), gid: g.gid, x: x + g.dx, y: -g.shift + g.dy, adv: g.adv, sx: g.sx, sy: g.sy, style: g.style, byte: g.byte, len: g.len, visible }
}

fn prepend_label(db: &FontDb, glyphs: &mut Vec<Glyph>, label: &str, at: usize, base: &designcraft_doc::CharProps, pp: &ParaProps, table: &mut StyleTable<'_>) {
    // Shape the label as a tiny standalone story so it uses the paragraph's base character style.
    let mut tmp = Story::new(StoryId(0));
    tmp.insert(0, label);
    let styles = designcraft_doc::Styles::default();
    let shaped = shape::shape_para(db, &styles, &tmp, 0, 0..label.len(), base, pp.auto_leading, &SubstCtx::default(), table);
    let mut pre: Vec<Glyph> = shaped
        .glyphs
        .into_iter()
        .map(|mut g| {
            g.byte = at;
            g.len = 0;
            g
        })
        .collect();
    pre.append(glyphs);
    *glyphs = pre;
}

/// Mark glyphs after which a hyphen may be inserted.
fn hyphenation_points(text: &str, glyphs: &[Glyph], pp: &ParaProps) -> Vec<bool> {
    let mut out = vec![false; glyphs.len()];
    if !pp.hyphenate {
        return out;
    }
    let lim = hyphen::Limits {
        min_word: pp.hyph_min_word as usize,
        after_first: pp.hyph_after_first as usize,
        before_last: pp.hyph_before_last as usize,
        capitalized: pp.hyph_capitalized,
    };
    let mut i = 0;
    while i < glyphs.len() {
        if !glyphs[i].is_letter() || glyphs[i].len == 0 {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < glyphs.len() && (glyphs[j].is_letter() || glyphs[j].len == 0) && !glyphs[j].no_break {
            j += 1;
        }
        let (a, b) = (glyphs[i].byte, glyphs[j - 1].byte + glyphs[j - 1].len);
        if b > a && b <= text.len() && text.is_char_boundary(a) && text.is_char_boundary(b) {
            let word = &text[a..b];
            // Do not hyphenate the paragraph's last word unless allowed.
            let is_last_word = !pp.hyph_last_word && glyphs[j..].iter().all(|g| !g.is_letter());
            if !is_last_word {
                let pts = hyphen::hyphen_points(word, &lim);
                for p in pts {
                    // char index p → byte → glyph whose cluster ends at that byte.
                    let byte = a + word.char_indices().nth(p).map(|(bi, _)| bi).unwrap_or(word.len());
                    if let Some(k) = (i..j).find(|&k| glyphs[k].byte + glyphs[k].len == byte && glyphs[k].len > 0) {
                        out[k] = true;
                    }
                }
            }
        }
        i = j.max(i + 1);
    }
    out
}

fn vertical_justify(ft: &mut FrameText, f: &FrameSpec) {
    if ft.lines.is_empty() || f.opts.vertical_justification == VerticalJustification::Top {
        return;
    }
    // Per column.
    let ncols = ft.columns.len().max(1) as u32;
    for c in 0..ncols {
        let idx: Vec<usize> = ft.lines.iter().enumerate().filter(|(_, l)| l.column == c).map(|(i, _)| i).collect();
        let (Some(&a), Some(&z)) = (idx.first(), idx.last()) else { continue };
        let bottom = ft.lines[z].baseline + ft.lines[z].descent;
        let space = f.area.y1 - bottom;
        if space <= 0.0 {
            continue;
        }
        let _ = a;
        match f.opts.vertical_justification {
            VerticalJustification::Center => idx.iter().for_each(|&i| ft.lines[i].baseline += space / 2.0),
            VerticalJustification::Bottom => idx.iter().for_each(|&i| ft.lines[i].baseline += space),
            VerticalJustification::Justify if idx.len() > 1 => {
                let per = space / (idx.len() - 1) as f64;
                for (k, &i) in idx.iter().enumerate() {
                    ft.lines[i].baseline += per * k as f64;
                }
            }
            _ => {}
        }
    }
}

// ---------- caret and hit testing (Type tool) ----------

/// Caret geometry for story byte `pos`: (frame index, x, baseline, ascent, descent).
pub fn caret(cs: &ComposedStory, pos: usize) -> Option<(usize, f64, f64, f64, f64)> {
    let mut best: Option<(usize, &Line)> = None;
    for (fi, ft) in cs.frames.iter().enumerate() {
        for l in &ft.lines {
            if pos >= l.range.start && pos <= l.range.end {
                // Prefer the line where pos is not at its end (except the last line of a paragraph).
                let at_end = pos == l.range.end && !l.last_in_para;
                if best.is_none() || !at_end {
                    best = Some((fi, l));
                    if !at_end {
                        break;
                    }
                }
            }
        }
        if best.is_some_and(|(_, l)| pos < l.range.end || l.last_in_para) {
            break;
        }
    }
    let (fi, l) = best?;
    Some((fi, caret_x(l, pos), l.baseline, l.ascent, l.descent))
}

fn caret_x(l: &Line, pos: usize) -> f64 {
    for g in &l.glyphs {
        if g.len > 0 && pos >= g.byte && pos < g.byte + g.len {
            // Inside a multi-char cluster (ligature): interpolate.
            let t = (pos - g.byte) as f64 / g.len as f64;
            return g.x + g.adv * t;
        }
        if g.len > 0 && g.byte >= pos {
            return g.x;
        }
    }
    l.end_x
}

/// Story byte nearest to point `p` (frame inner space) in frame `fi`.
pub fn hit(cs: &ComposedStory, fi: usize, p: Point) -> Option<usize> {
    let ft = cs.frames.get(fi)?;
    if ft.lines.is_empty() {
        return Some(ft.range.start);
    }
    // Closest line vertically (within its column).
    let l = ft
        .lines
        .iter()
        .filter(|l| p.x >= l.x0 - 20.0 && p.x <= l.x1 + 20.0 || ft.columns.len() <= 1)
        .min_by(|a, b| line_dist(a, p.y).total_cmp(&line_dist(b, p.y)))
        .or_else(|| ft.lines.first())?;
    let mut best = l.range.start;
    let mut prev_mid = f64::NEG_INFINITY;
    for g in l.glyphs.iter().filter(|g| g.len > 0) {
        let mid = g.x + g.adv / 2.0;
        if p.x < mid && p.x >= prev_mid {
            return Some(g.byte);
        }
        prev_mid = mid;
        best = g.byte + g.len;
    }
    // After the last glyph: line end (before the paragraph separator / break).
    Some(if l.last_in_para { l.range.end } else { best.min(l.range.end) })
}

fn line_dist(l: &Line, y: f64) -> f64 {
    let top = l.baseline - l.ascent;
    let bot = l.baseline + l.descent;
    if y < top {
        top - y
    } else if y > bot {
        y - bot
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
