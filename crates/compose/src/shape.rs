//! Paragraph → styled, positioned glyphs (before line breaking).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use designcraft_doc::{Capitalization, CharProps, Kerning, Leading, Position, Story, Styles, story};
use designcraft_fonts::{FaceRef, Feature, FontDb, FontFace, ShapedGlyph, feature};

use crate::RunStyle;

/// One glyph with every character attribute applied, in points.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub face: FaceRef,
    pub gid: u32,
    /// Story byte offset of the cluster and its byte length (0 for generated glyphs).
    pub byte: usize,
    pub len: usize,
    /// First source character of the cluster (special characters keep their code).
    pub ch: char,
    /// Advance in points (tracking and horizontal scale applied).
    pub adv: f64,
    pub dx: f64,
    pub dy: f64,
    /// Font units → points (incl. horizontal / vertical scale).
    pub sx: f64,
    pub sy: f64,
    /// Baseline shift (positive = up), including super/subscript.
    pub shift: f64,
    pub ascent: f64,
    pub descent: f64,
    /// The leading this character asks for (absolute, or auto = size × auto %).
    pub leading: f64,
    pub cap: f64,
    pub xh: f64,
    pub size: f64,
    /// Index into the composed story's [`RunStyle`] table.
    pub style: u32,
    /// Unbreakable (No Break).
    pub no_break: bool,
    /// Width of a word space (U+0020) in this glyph's font, size and horizontal scale: the unit of
    /// word and letter spacing in justification.
    pub space: f64,
}

impl Glyph {
    pub fn is_space(&self) -> bool {
        matches!(self.ch, ' ' | '\u{2002}'..='\u{200A}' | '\u{3000}')
    }
    pub fn is_letter(&self) -> bool {
        self.ch.is_alphabetic() || matches!(self.ch, '\'' | '’')
    }
}

pub const SOFT_HYPHEN: char = '\u{AD}';

/// Shaped paragraph.
pub struct ShapedPara {
    pub glyphs: Vec<Glyph>,
    /// Byte range of the paragraph text (without its `\n`).
    pub range: std::ops::Range<usize>,
}

/// Context for substitutions (page number markers).
#[derive(Clone, Debug, Default)]
pub struct SubstCtx {
    pub page_name: Option<String>,
    pub section_marker: Option<String>,
    /// Text variable values by index (on the page being composed).
    pub vars: Arc<Vec<String>>,
}

pub(crate) struct StyleTable<'a> {
    pub styles: &'a mut Vec<RunStyle>,
}

impl StyleTable<'_> {
    fn intern(&mut self, p: &CharProps) -> u32 {
        let rs = RunStyle {
            fill: p.fill.clone(),
            fill_tint: p.fill_tint,
            stroke: p.stroke.clone(),
            stroke_tint: p.stroke_tint,
            stroke_weight: p.stroke_weight,
            underline: p.underline,
            strikethrough: p.strikethrough,
            skew: p.skew,
            size: p.size,
        };
        if let Some(i) = self.styles.iter().rposition(|s| *s == rs) {
            return i as u32;
        }
        self.styles.push(rs);
        (self.styles.len() - 1) as u32
    }
}

/// Resolve and shape one paragraph.
pub(crate) fn shape_para(
    db: &FontDb,
    styles: &Styles,
    story: &Story,
    pi: usize,
    range: std::ops::Range<usize>,
    para_chars: &CharProps,
    auto_leading: f64,
    sub: &SubstCtx,
    table: &mut StyleTable<'_>,
) -> ShapedPara {
    let _ = pi;
    let mut glyphs = Vec::with_capacity(range.len());
    for (rr, fmt) in story.runs() {
        let a = rr.start.max(range.start);
        let b = rr.end.min(range.end);
        if a >= b {
            continue;
        }
        let props = styles.resolve_char(para_chars, fmt);
        let style = table.intern(&props);
        shape_run(db, &story.text, a..b, &props, auto_leading, style, sub, &mut glyphs);
    }
    ShapedPara { glyphs, range }
}

fn features_for(p: &CharProps) -> Vec<Feature> {
    let mut v = Vec::new();
    if !matches!(p.kerning, Kerning::Metrics | Kerning::Optical) {
        v.extend(feature("-kern"));
    }
    if !p.ligatures || p.tracking.abs() > 1e-9 {
        v.extend(feature("-liga"));
        v.extend(feature("-clig"));
    }
    if p.capitalization == Capitalization::AllCaps {
        v.extend(feature("case"));
    }
    if matches!(p.capitalization, Capitalization::SmallCaps | Capitalization::OpenTypeAllSmallCaps) {
        v.extend(feature("smcp"));
    }
    if p.capitalization == Capitalization::OpenTypeAllSmallCaps {
        v.extend(feature("c2sc"));
    }
    match p.position {
        Position::OtSuperscript => v.extend(feature("sups")),
        Position::OtSubscript => v.extend(feature("subs")),
        Position::OtNumerator => v.extend(feature("numr")),
        Position::OtDenominator => v.extend(feature("dnom")),
        _ => {}
    }
    for f in &p.otf_features {
        v.extend(feature(f));
    }
    v
}

fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F | 0x200D | 0xFE00..=0xFE0F)
}

#[allow(clippy::too_many_arguments)]
fn shape_run(
    db: &FontDb,
    text: &str,
    range: std::ops::Range<usize>,
    p: &CharProps,
    auto_leading: f64,
    style: u32,
    sub: &SubstCtx,
    out: &mut Vec<Glyph>,
) {
    let primary = db.face(&p.font_family, &p.font_style);
    // Split into segments: font coverage changes and special characters (markers, tabs, breaks).
    let mut seg_start = range.start;
    let mut seg_face = primary.clone();
    let flush = |a: usize, b: usize, face: &Arc<FontFace>, out: &mut Vec<Glyph>| {
        if a < b {
            shape_segment(db, text, a..b, None, p, face, auto_leading, style, out);
        }
    };
    for (i, c) in text[range.clone()].char_indices() {
        let i = range.start + i;
        let special = matches!(
            c,
            '\t' | story::FORCED_LINE_BREAK
                | story::PAGE_NUMBER
                | story::NEXT_PAGE_NUMBER
                | story::PREV_PAGE_NUMBER
                | story::SECTION_MARKER
                | story::COLUMN_BREAK
                | story::FRAME_BREAK
                | story::PAGE_BREAK
                | story::INDENT_HERE
                | story::RIGHT_INDENT_TAB
                | story::TABLE_ANCHOR
        ) || designcraft_doc::vars::var_index(c).is_some();
        if special {
            flush(seg_start, i, &seg_face, out);
            seg_start = i + c.len_utf8();
            if let Some(vi) = designcraft_doc::vars::var_index(c) {
                match sub.vars.get(vi).filter(|v| !v.is_empty()) {
                    Some(v) => shape_segment(db, text, i..i + c.len_utf8(), Some(v), p, &primary, auto_leading, style, out),
                    None => {
                        let mut g = control_glyph(&primary, p, auto_leading, style, i, c);
                        g.adv = 0.0;
                        out.push(g);
                    }
                }
                continue;
            }
            match c {
                story::PAGE_NUMBER | story::NEXT_PAGE_NUMBER | story::PREV_PAGE_NUMBER | story::SECTION_MARKER => {
                    let s = if c == story::SECTION_MARKER {
                        sub.section_marker.clone().unwrap_or_else(|| "Section".into())
                    } else {
                        sub.page_name.clone().unwrap_or_else(|| "#".into())
                    };
                    shape_segment(db, text, i..i + c.len_utf8(), Some(&s), p, &primary, auto_leading, style, out);
                }
                _ => {
                    // Zero-width control glyph carrying metrics (tabs get their width at line layout).
                    let mut g = control_glyph(&primary, p, auto_leading, style, i, c);
                    if c == '\t' || c == story::RIGHT_INDENT_TAB || c == story::TABLE_ANCHOR {
                        g.adv = 0.0;
                    }
                    out.push(g);
                }
            }
            continue;
        }
        let covered = c.is_whitespace() || c.is_control() || c == SOFT_HYPHEN || primary.covers(c);
        let face = if covered || is_mark(c) {
            if is_mark(c) { seg_face.clone() } else { primary.clone() }
        } else {
            db.fallback_for(c, primary.id()).unwrap_or_else(|| primary.clone())
        };
        if face.id() != seg_face.id() {
            flush(seg_start, i, &seg_face, out);
            seg_start = i;
            seg_face = face;
        }
    }
    flush(seg_start, range.end, &seg_face, out);
}

type WordMap = HashMap<Box<str>, Arc<[ShapedGlyph]>>;
type WordKey = (u32, bool, String);

/// Shaped words (font units) by (face, caps, features), then text. Lookups take read locks and
/// misses are shaped outside any lock, so parallel composition doesn't serialize here.
static WORD_CACHE: RwLock<Vec<(WordKey, Arc<RwLock<WordMap>>)>> = RwLock::new(Vec::new());
const WORD_CACHE_MAX: usize = 50_000;

fn word_map(key: WordKey) -> Arc<RwLock<WordMap>> {
    if let Some((_, m)) = WORD_CACHE.read().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.0 == key) {
        return m.clone();
    }
    let mut w = WORD_CACHE.write().unwrap_or_else(|e| e.into_inner());
    if let Some((_, m)) = w.iter().find(|e| e.0 == key) {
        return m.clone();
    }
    if w.len() > 64 {
        w.clear();
    }
    let m = Arc::new(RwLock::new(HashMap::new()));
    w.push((key, m.clone()));
    m
}

/// Shape `src` word by word through a cache (like a browser's word cache): the text is split
/// after each U+0020 so repeated words are shaped once. Shaping does not cross word spaces in the
/// scripts we lay out, and clusters stay byte offsets into `src`.
fn shape_cached(face: &FontFace, src: &str, feats: &[Feature], caps: bool) -> Vec<ShapedGlyph> {
    let map = |c: char| if caps { c.to_uppercase().next().unwrap_or(c) } else { c };
    if src.len() < 2 || !src.contains(' ') {
        return designcraft_fonts::shape(face, src, feats, map);
    }
    let words = word_map((face.id(), caps, format!("{feats:?}")));
    let pieces: Vec<&str> = src.split_inclusive(' ').collect();
    let mut found: Vec<Option<Arc<[ShapedGlyph]>>> = {
        let r = words.read().unwrap_or_else(|e| e.into_inner());
        pieces.iter().map(|p| r.get(*p).cloned()).collect()
    };
    let mut fresh: Vec<(Box<str>, Arc<[ShapedGlyph]>)> = Vec::new();
    for (k, f) in found.iter_mut().enumerate() {
        if f.is_none() {
            let g: Arc<[ShapedGlyph]> = designcraft_fonts::shape(face, pieces[k], feats, map).into();
            fresh.push((pieces[k].into(), g.clone()));
            *f = Some(g);
        }
    }
    if !fresh.is_empty() {
        let mut w = words.write().unwrap_or_else(|e| e.into_inner());
        if w.len() + fresh.len() > WORD_CACHE_MAX {
            w.clear();
        }
        w.extend(fresh);
    }
    let mut out = Vec::with_capacity(src.len());
    let mut start = 0;
    for (piece, glyphs) in pieces.iter().zip(found) {
        let glyphs = glyphs.unwrap_or_else(|| Arc::from(Vec::new()));
        out.extend(glyphs.iter().map(|g| ShapedGlyph { cluster: g.cluster + start, ..*g }));
        start += piece.len();
    }
    out
}

fn metrics(face: &FontFace, p: &CharProps, auto_leading: f64) -> (f64, f64, f64, f64, f64, f64, f64, f64) {
    let (size, shift) = effective_size(p);
    let k = size / face.upem;
    let vs = p.v_scale;
    let leading = match p.leading {
        Leading::Auto => p.size * auto_leading,
        Leading::Points(v) => v,
    };
    (size, k, face.ascent * k * vs, face.descent * k * vs, leading, face.cap_height * k * vs, face.x_height * k * vs, shift)
}

/// Super/subscript use 58.3% size and ±33.3% / −10% shift (InDesign's defaults).
fn effective_size(p: &CharProps) -> (f64, f64) {
    match p.position {
        Position::Superscript => (p.size * 0.583, p.baseline_shift + p.size * 0.333),
        Position::Subscript => (p.size * 0.583, p.baseline_shift - p.size * 0.333 * 0.3),
        _ => (p.size, p.baseline_shift),
    }
}

fn control_glyph(face: &Arc<FontFace>, p: &CharProps, auto_leading: f64, style: u32, byte: usize, ch: char) -> Glyph {
    let (size, k, ascent, descent, leading, cap, xh, shift) = metrics(face, p, auto_leading);
    Glyph {
        face: FaceRef::of(face),
        gid: face.glyph_for(' '),
        byte,
        len: ch.len_utf8(),
        ch,
        adv: 0.0,
        dx: 0.0,
        dy: 0.0,
        sx: k * p.h_scale,
        sy: k * p.v_scale,
        shift,
        ascent,
        descent,
        leading,
        cap,
        xh,
        size,
        style,
        no_break: p.no_break,
        space: face.advance(face.glyph_for(' ')) * k * p.h_scale,
    }
}

#[allow(clippy::too_many_arguments)]
fn shape_segment(
    db: &FontDb,
    text: &str,
    range: std::ops::Range<usize>,
    replacement: Option<&str>,
    p: &CharProps,
    face: &Arc<FontFace>,
    auto_leading: f64,
    style: u32,
    out: &mut Vec<Glyph>,
) {
    let _ = db;
    let (size, k, ascent, descent, leading, cap, xh, shift) = metrics(face, p, auto_leading);
    let hs = p.h_scale;
    let tracking = p.tracking / 1000.0 * p.size;
    let manual = if let Kerning::Manual(v) = p.kerning { v / 1000.0 * p.size } else { 0.0 };
    let src = replacement.unwrap_or(&text[range.clone()]);
    let caps = p.capitalization == Capitalization::AllCaps;
    let feats = features_for(p);
    let space = face.advance(face.glyph_for(' ')) * k * hs;
    let shaped: Vec<ShapedGlyph> = shape_cached(face, src, &feats, caps);
    let n = shaped.len();
    let fref = FaceRef::of(face);
    for (gi, sg) in shaped.iter().enumerate() {
        let (byte, len, ch) = if replacement.is_some() {
            (range.start, if gi == 0 { range.len() } else { 0 }, text[range.start..].chars().next().unwrap_or(' '))
        } else {
            let cl = range.start + sg.cluster;
            let end = shaped[gi + 1..].iter().map(|r| range.start + r.cluster).find(|&c| c > cl).unwrap_or(range.end);
            (cl, end.saturating_sub(cl), text[cl..].chars().next().unwrap_or(' '))
        };
        let last_in_cluster = gi + 1 == n || shaped[gi + 1].cluster != sg.cluster;
        let mut adv = sg.x_advance as f64 * k * hs;
        if ch == SOFT_HYPHEN {
            adv = 0.0;
        } else if last_in_cluster {
            adv += tracking + manual;
        }
        // Only the first glyph of a cluster owns the bytes (so ranges partition the text).
        let first_in_cluster = gi == 0 || shaped[gi - 1].cluster != sg.cluster;
        out.push(Glyph {
            face: fref,
            gid: sg.gid,
            byte,
            len: if first_in_cluster || replacement.is_some() { len } else { 0 },
            ch,
            adv,
            dx: sg.x_offset as f64 * k * hs,
            dy: -(sg.y_offset as f64) * k * p.v_scale,
            sx: k * hs,
            sy: k * p.v_scale,
            shift,
            ascent,
            descent,
            leading,
            cap,
            xh,
            size,
            style,
            no_break: p.no_break,
            space,
        });
    }
}

/// A hyphen glyph in the face/size of `g`, placed after it (zero source length).
pub(crate) fn hyphen_after(g: &Glyph) -> Glyph {
    let gid = designcraft_fonts::first_glyph(&g.face, &['-', '\u{2010}']);
    let mut h = g.clone();
    h.gid = gid;
    h.adv = g.face.advance(gid) * g.sx;
    h.dx = 0.0;
    h.dy = 0.0;
    h.byte = g.byte + g.len;
    h.len = 0;
    h.ch = '-';
    h
}
