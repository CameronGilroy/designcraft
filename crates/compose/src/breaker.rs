//! Line breaking: the paragraph composer (Knuth–Plass total fit) and the single-line composer
//! (greedy first fit). Both work on a paragraph's glyphs and a per-line measure function.

use crate::shape::{Glyph, SOFT_HYPHEN};

/// One line chosen by a breaker: glyphs `start..end` are visible; `next` is where the next line starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Break {
    pub start: usize,
    pub end: usize,
    pub next: usize,
    /// Broken at a hyphenation point (a hyphen glyph must be added).
    pub hyphen: bool,
    /// Ended by a forced break (paragraph end, forced line break, column/frame/page break).
    pub forced: bool,
}

/// Spacing parameters (from the paragraph's Justification settings), as fractions.
#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub justify: bool,
    pub word_min: f64,
    pub word_desired: f64,
    pub word_max: f64,
    /// Penalty for a hyphenated break (0 = free, larger = fewer hyphens).
    pub hyphen_penalty: f64,
    /// Maximum consecutive hyphenated lines (0 = unlimited).
    pub hyphen_limit: u32,
    /// Ragged-right stretchability (points) for unjustified paragraphs.
    pub ragged_stretch: f64,
}

#[derive(Clone, Copy, Debug)]
enum Item {
    /// Unbreakable width (a glyph).
    Box { w: f64 },
    /// Space glyph `g`.
    Glue { w: f64, st: f64, sh: f64 },
    Penalty { w: f64, p: f64, flagged: bool },
}

const INF: f64 = 10000.0;

/// Does the glyph force a line end after it?
pub fn is_forced(c: char) -> bool {
    use designcraft_doc::story::*;
    matches!(c, FORCED_LINE_BREAK | COLUMN_BREAK | FRAME_BREAK | PAGE_BREAK)
}

/// Build Knuth items. `item_glyph[k]` = glyph index of item k (penalties: the glyph *before* which the break happens).
fn items(glyphs: &[Glyph], hyph_after: &[bool], sp: &Spacing) -> (Vec<Item>, Vec<usize>) {
    let mut it = Vec::with_capacity(glyphs.len() * 2 + 2);
    let mut ig = Vec::with_capacity(glyphs.len() * 2 + 2);
    let n = glyphs.len();
    for (i, g) in glyphs.iter().enumerate() {
        if is_forced(g.ch) {
            if !sp.justify || true {
                it.push(Item::Glue { w: 0.0, st: INF, sh: 0.0 });
                ig.push(i);
            }
            it.push(Item::Penalty { w: 0.0, p: -INF, flagged: false });
            ig.push(i + 1);
            continue;
        }
        if g.is_space() && !g.no_break {
            let w = g.adv;
            if sp.justify {
                let base = w / sp.word_desired.max(1e-6);
                it.push(Item::Glue { w, st: base * (sp.word_max - sp.word_desired).max(0.0), sh: base * (sp.word_desired - sp.word_min).max(0.0) });
                ig.push(i);
            } else {
                // Ragged right (Knuth): glue(0, s, 0) penalty(0) glue(w, -s, 0).
                it.push(Item::Glue { w: 0.0, st: sp.ragged_stretch, sh: 0.0 });
                ig.push(i);
                it.push(Item::Penalty { w: 0.0, p: 0.0, flagged: false });
                ig.push(i);
                it.push(Item::Glue { w, st: -sp.ragged_stretch, sh: 0.0 });
                ig.push(i);
            }
            continue;
        }
        if g.ch == SOFT_HYPHEN {
            let hw = g.size * 0.33;
            it.push(Item::Penalty { w: hw, p: sp.hyphen_penalty, flagged: true });
            ig.push(i + 1);
            continue;
        }
        it.push(Item::Box { w: g.adv });
        ig.push(i);
        let next_is_space = glyphs.get(i + 1).is_none_or(|n| n.is_space());
        if i + 1 < n && !next_is_space && !g.no_break {
            if matches!(g.ch, '-' | '\u{2010}') {
                it.push(Item::Penalty { w: 0.0, p: 50.0, flagged: true });
                ig.push(i + 1);
            } else if matches!(g.ch, '\u{2013}' | '\u{2014}' | '/') {
                it.push(Item::Penalty { w: 0.0, p: 0.0, flagged: false });
                ig.push(i + 1);
            } else if hyph_after[i] {
                let hw = g.size * 0.33;
                it.push(Item::Penalty { w: hw, p: sp.hyphen_penalty, flagged: true });
                ig.push(i + 1);
            }
        }
    }
    // Paragraph end: fill glue + forced break.
    it.push(Item::Glue { w: 0.0, st: INF, sh: 0.0 });
    ig.push(n);
    it.push(Item::Penalty { w: 0.0, p: -INF, flagged: false });
    ig.push(n);
    (it, ig)
}

#[derive(Clone, Copy, Debug)]
struct Node {
    pos: usize,
    line: usize,
    fitness: u8,
    tw: f64,
    ty: f64,
    tz: f64,
    demerits: f64,
    prev: Option<usize>,
    hyphens: u32,
    flagged: bool,
}

/// Knuth–Plass total-fit line breaking. `width(line)` gives each line's measure.
pub fn knuth_plass(glyphs: &[Glyph], hyph_after: &[bool], sp: &Spacing, width: &dyn Fn(usize) -> f64) -> Vec<Break> {
    if glyphs.is_empty() {
        return vec![Break { start: 0, end: 0, next: 0, hyphen: false, forced: true }];
    }
    let (items, ig) = items(glyphs, hyph_after, sp);
    for tolerance in [3.0, 30.0, f64::INFINITY] {
        if let Some(b) = kp_pass(&items, &ig, glyphs, sp, width, tolerance, tolerance.is_infinite()) {
            return b;
        }
    }
    greedy(glyphs, hyph_after, sp, width)
}

#[allow(clippy::too_many_arguments)]
fn kp_pass(items: &[Item], ig: &[usize], glyphs: &[Glyph], sp: &Spacing, width: &dyn Fn(usize) -> f64, tol: f64, emergency: bool) -> Option<Vec<Break>> {
    let m = items.len();
    // Prefix sums (before item i).
    let mut sw = vec![0.0; m + 1];
    let mut sy = vec![0.0; m + 1];
    let mut sz = vec![0.0; m + 1];
    for (i, it) in items.iter().enumerate() {
        let (w, y, z) = match *it {
            Item::Box { w } => (w, 0.0, 0.0),
            Item::Glue { w, st, sh } => (w, st, sh),
            Item::Penalty { .. } => (0.0, 0.0, 0.0),
        };
        sw[i + 1] = sw[i] + w;
        sy[i + 1] = sy[i] + y;
        sz[i + 1] = sz[i] + z;
    }
    let mut arena: Vec<Node> = vec![Node { pos: 0, line: 0, fitness: 1, tw: 0.0, ty: 0.0, tz: 0.0, demerits: 0.0, prev: None, hyphens: 0, flagged: false }];
    let mut active: Vec<usize> = vec![0];
    for b in 0..m {
        let (is_break, pw, pp, flagged) = match items[b] {
            Item::Penalty { w, p, flagged } if p < INF => (true, w, p, flagged),
            Item::Glue { .. } if b > 0 && matches!(items[b - 1], Item::Box { .. }) => (true, 0.0, 0.0, false),
            _ => (false, 0.0, 0.0, false),
        };
        if !is_break {
            continue;
        }
        let forced = pp <= -INF;
        let mut best: [Option<(f64, usize, f64)>; 4] = [None; 4]; // (demerits, active node, ratio)
        let mut keep = Vec::with_capacity(active.len());
        let mut last_removed: Option<(usize, f64)> = None;
        for &a in &active {
            let n = arena[a];
            let l = sw[b] - n.tw + pw;
            let target = width(n.line).max(1.0);
            let (y, z) = (sy[b] - n.ty, sz[b] - n.tz);
            let r = if l < target {
                if y > 1e-9 { (target - l) / y } else { INF }
            } else if l > target {
                if z > 1e-9 { (target - l) / z } else { -INF }
            } else {
                0.0
            };
            let deactivate = r < -1.0 || forced;
            if !deactivate {
                keep.push(a);
            } else {
                last_removed = Some((a, r));
            }
            if r >= -1.0 && r <= tol || (forced && r >= -1.0) {
                let bad = (100.0 * r.abs().powi(3)).min(INF);
                let lp = 10.0 + bad;
                let mut d = if pp >= 0.0 { lp * lp + pp * pp } else if pp > -INF { lp * lp - pp * pp } else { lp * lp };
                if flagged && n.flagged {
                    d += 3000.0;
                }
                if flagged && sp.hyphen_limit > 0 && n.hyphens >= sp.hyphen_limit {
                    d += 1e7;
                }
                let fit = if r < -0.5 {
                    0
                } else if r <= 0.5 {
                    1
                } else if r <= 1.0 {
                    2
                } else {
                    3
                };
                if (fit as i32 - n.fitness as i32).abs() > 1 {
                    d += 3000.0;
                }
                let total = n.demerits + d;
                if best[fit as usize].is_none_or(|(bd, _, _)| total < bd) {
                    best[fit as usize] = Some((total, a, r));
                }
            }
        }
        // Emergency: nothing can reach this break and nothing remains active → accept an overfull line.
        if emergency
            && keep.is_empty()
            && best.iter().all(Option::is_none)
            && let Some((a, _)) = last_removed
        {
            best[0] = Some((arena[a].demerits + 1e8, a, -1.0));
        }
        active = keep;
        // Totals after the break: skip glue and penalties up to the next box.
        let (mut tw, mut ty, mut tz) = (sw[b], sy[b], sz[b]);
        let mut i = b;
        while i < m {
            match items[i] {
                Item::Box { .. } => break,
                Item::Glue { w, st, sh } => {
                    tw += w;
                    ty += st;
                    tz += sh;
                }
                Item::Penalty { p, .. } if p <= -INF && i > b => break,
                Item::Penalty { .. } => {}
            }
            i += 1;
        }
        for (fit, c) in best.iter().enumerate() {
            if let Some((d, a, _)) = *c {
                let prev = arena[a];
                arena.push(Node {
                    pos: b,
                    line: prev.line + 1,
                    fitness: fit as u8,
                    tw,
                    ty,
                    tz,
                    demerits: d,
                    prev: Some(a),
                    hyphens: if flagged { prev.hyphens + 1 } else { 0 },
                    flagged,
                });
                active.push(arena.len() - 1);
            }
        }
        if active.is_empty() {
            return None;
        }
    }
    // The final forced break: best node at position m-1.
    let end = active.iter().copied().filter(|&a| arena[a].pos == m - 1).min_by(|&a, &b| arena[a].demerits.total_cmp(&arena[b].demerits))?;
    let mut chain = Vec::new();
    let mut cur = Some(end);
    while let Some(c) = cur {
        if arena[c].prev.is_some() {
            chain.push(arena[c].pos);
        }
        cur = arena[c].prev;
    }
    chain.reverse();
    Some(breaks_from_positions(items, ig, glyphs, &chain))
}

fn breaks_from_positions(items: &[Item], ig: &[usize], glyphs: &[Glyph], chain: &[usize]) -> Vec<Break> {
    let n = glyphs.len();
    let mut out = Vec::with_capacity(chain.len());
    let mut start = 0;
    for &b in chain {
        let (end, hyphen, forced) = match items[b] {
            Item::Penalty { p, flagged, w } => {
                let e = ig[b].min(n);
                // A flagged penalty with width = an inserted hyphen (not an explicit '-').
                let explicit = e > 0 && matches!(glyphs[e - 1].ch, '-' | '\u{2010}');
                (e, flagged && w > 0.0 && !explicit || (flagged && e > 0 && glyphs[e - 1].ch == SOFT_HYPHEN), p <= -INF)
            }
            Item::Glue { .. } => (ig[b], false, false),
            Item::Box { .. } => (ig[b], false, false),
        };
        // Trim trailing spaces from the visible range; skip leading spaces of the next line.
        let mut vis_end = end;
        while vis_end > start && glyphs[vis_end - 1].is_space() {
            vis_end -= 1;
        }
        let mut next = end;
        while next < n && glyphs[next].is_space() {
            next += 1;
        }
        let forced_glyph = end > 0 && end <= n && is_forced(glyphs[end - 1].ch);
        out.push(Break { start, end: vis_end, next, hyphen, forced: forced || forced_glyph });
        start = next;
    }
    if out.is_empty() {
        out.push(Break { start: 0, end: n, next: n, hyphen: false, forced: true });
    }
    // Lines produced past the paragraph end (e.g. a trailing forced break) are dropped.
    out.retain(|b| b.start <= n);
    out
}

/// Greedy first-fit breaking (single-line composer; also used for paragraphs with tabs).
pub fn greedy(glyphs: &[Glyph], hyph_after: &[bool], sp: &Spacing, width: &dyn Fn(usize) -> f64) -> Vec<Break> {
    let n = glyphs.len();
    let mut out = Vec::new();
    let mut start = 0;
    let mut line = 0;
    if n == 0 {
        return vec![Break { start: 0, end: 0, next: 0, hyphen: false, forced: true }];
    }
    while start < n {
        let w = width(line).max(1.0);
        let mut x = 0.0;
        let mut spaces_shrink = 0.0;
        let mut last_ok: Option<(usize, bool)> = None; // (break position = end glyph, hyphen)
        let mut i = start;
        let mut brk: Option<(usize, bool, bool)> = None;
        while i < n {
            let g = &glyphs[i];
            if is_forced(g.ch) {
                brk = Some((i + 1, false, true));
                break;
            }
            if g.is_space() && !g.no_break {
                if sp.justify {
                    spaces_shrink += g.adv * (sp.word_desired - sp.word_min).max(0.0) / sp.word_desired.max(1e-6);
                }
                last_ok = Some((i, false));
                x += g.adv;
                i += 1;
                continue;
            }
            let hy = if hyph_after[i] { g.size * 0.33 } else { 0.0 };
            if x + g.adv > w + spaces_shrink && i > start {
                // Overflow: break at the last opportunity, else before this glyph.
                brk = Some(match last_ok {
                    Some((p, h)) => (p, h, false),
                    None => (i, false, false),
                });
                break;
            }
            x += g.adv;
            if i + 1 < n && !glyphs[i + 1].is_space() && !g.no_break {
                if matches!(g.ch, '-' | '\u{2010}' | '\u{2013}' | '\u{2014}' | '/') {
                    last_ok = Some((i + 1, false));
                } else if hyph_after[i] && x + hy <= w + spaces_shrink {
                    last_ok = Some((i + 1, true));
                }
            }
            i += 1;
        }
        let (end, hyphen, forced) = brk.unwrap_or((n, false, true));
        let mut vis_end = end;
        while vis_end > start && glyphs[vis_end - 1].is_space() {
            vis_end -= 1;
        }
        let mut next = end;
        while next < n && glyphs[next].is_space() {
            next += 1;
        }
        let next = next.max(start + 1).min(n.max(start + 1));
        out.push(Break { start, end: vis_end, next: next.min(n), hyphen, forced: forced || end == n });
        if next >= n {
            break;
        }
        start = next;
        line += 1;
    }
    // A forced break as the very last glyph leaves an empty final line (InDesign shows it).
    if let Some(last) = glyphs.last()
        && is_forced(last.ch)
    {
        out.push(Break { start: n, end: n, next: n, hyphen: false, forced: true });
    }
    out
}
