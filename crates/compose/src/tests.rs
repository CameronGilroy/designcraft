use designcraft_doc::build::NewDocument;
use designcraft_doc::{Align, Document, ParaAttrs, ParaFormat, SpreadRef};
use designcraft_geom::Rect;

use super::*;

const LOREM: &str = "Typography is the art and technique of arranging type to make written language legible, readable and appealing when displayed. \
The arrangement of type involves selecting typefaces, point sizes, line lengths, line spacing, and letter spacing, and adjusting the space between pairs of letters.";

fn doc_with(text: &str, rect: Rect, para: ParaAttrs) -> (Document, StoryId, ItemId) {
    let mut d = Document::new(&NewDocument::default());
    let lid = d.default_layer();
    let (fid, sid) = d.add_text_frame(SpreadRef::Doc(0), rect, lid, text, ParaFormat { para, ..Default::default() }).unwrap();
    (d, sid, fid)
}

fn all_lines(cs: &ComposedStory) -> Vec<&Line> {
    cs.frames.iter().flat_map(|f| f.lines.iter()).collect()
}

#[test]
fn composes_simple_paragraph() {
    let (d, sid, _) = doc_with(LOREM, Rect::new(36.0, 36.0, 300.0, 700.0), ParaAttrs::default());
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert!(!cs.is_overset());
    let lines = all_lines(&cs);
    assert!(lines.len() >= 5, "{}", lines.len());
    // Baselines increase by the auto leading (12 × 120% = 14.4).
    for w in lines.windows(2) {
        assert!((w[1].baseline - w[0].baseline - 14.4).abs() < 1e-6);
    }
    // Lines fit their measure.
    for l in &lines {
        assert!(l.end_x <= l.x1 + 0.5, "line overflows: {} > {}", l.end_x, l.x1);
    }
}

#[test]
fn line_ranges_partition_the_story() {
    let text = format!("{LOREM}\n\nSecond paragraph here.\n{LOREM}");
    let (d, sid, _) = doc_with(&text, Rect::new(0.0, 0.0, 200.0, 2000.0), ParaAttrs::default());
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let lines = all_lines(&cs);
    let mut pos = 0;
    for l in &lines {
        // Each line starts where the previous ended (or after a paragraph separator).
        assert!(l.range.start == pos || l.range.start == pos + 1, "gap at {pos}: {:?}", l.range);
        pos = l.range.end;
    }
    assert_eq!(pos, text.len());
    assert_eq!(lines.iter().filter(|l| l.first_in_para).count(), 4);
}

#[test]
fn justified_lines_fill_the_measure() {
    let (d, sid, _) = doc_with(LOREM, Rect::new(0.0, 0.0, 220.0, 2000.0), ParaAttrs { align: Some(Align::LeftJustified), ..Default::default() });
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let lines = all_lines(&cs);
    for l in &lines[..lines.len() - 1] {
        assert!((l.end_x - l.x1).abs() < 0.6, "justified line ends at {} not {}", l.end_x, l.x1);
    }
    let last = lines.last().unwrap();
    assert!(last.end_x < last.x1 - 1.0, "last line is ragged");
}

#[test]
fn small_frame_oversets() {
    let (d, sid, _) = doc_with(LOREM, Rect::new(0.0, 0.0, 200.0, 40.0), ParaAttrs::default());
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert!(cs.is_overset());
    let shown = cs.frames[0].range.clone();
    assert_eq!(cs.overset_at.unwrap(), shown.end.max(cs.overset_at.unwrap()).min(cs.overset_at.unwrap()));
    assert!(cs.frames[0].lines.len() <= 3);
}

#[test]
fn text_flows_through_threaded_frames() {
    let mut d = Document::new(&NewDocument::default());
    let lid = d.default_layer();
    let long = [LOREM; 4].join("\n");
    let (a, sid) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(0.0, 0.0, 200.0, 120.0), lid, &long, ParaFormat::default()).unwrap();
    let (b, _) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(250.0, 0.0, 450.0, 2000.0), lid, "", ParaFormat::default()).unwrap();
    d.thread(a, b).unwrap();
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert!(!cs.is_overset());
    assert!(!cs.frames[0].lines.is_empty() && !cs.frames[1].lines.is_empty());
    assert_eq!(cs.frames[0].range.end, cs.frames[1].range.start.min(cs.frames[0].range.end).max(cs.frames[0].range.end));
    // The second frame's lines start at its top.
    assert!(cs.frames[1].lines[0].baseline < 20.0);
}

#[test]
fn columns_fill_left_to_right() {
    let (mut d, sid, fid) = doc_with(&[LOREM; 3].join(" "), Rect::new(0.0, 0.0, 400.0, 150.0), ParaAttrs::default());
    d.item_mut(fid).unwrap().text_frame_mut().unwrap().options.columns = 2;
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let lines = all_lines(&cs);
    assert!(lines.iter().any(|l| l.column == 1));
    let c1 = lines.iter().find(|l| l.column == 1).unwrap();
    assert!(c1.x0 > 200.0);
}

#[test]
fn centered_text_is_centered() {
    let (d, sid, _) = doc_with("Hello", Rect::new(0.0, 0.0, 300.0, 100.0), ParaAttrs { align: Some(Align::Center), ..Default::default() });
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let l = &cs.frames[0].lines[0];
    let left = l.glyphs[0].x - l.x0;
    let right = l.x1 - l.end_x;
    assert!((left - right).abs() < 0.5, "{left} vs {right}");
}

#[test]
fn caret_and_hit_roundtrip() {
    let (d, sid, _) = doc_with(LOREM, Rect::new(0.0, 0.0, 200.0, 1000.0), ParaAttrs::default());
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    for pos in [0, 5, 40, 100, LOREM.len()] {
        let (fi, x, b, _, _) = caret(&cs, pos).unwrap();
        let back = hit(&cs, fi, Point::new(x + 0.1, b - 2.0)).unwrap();
        assert!((back as i64 - pos as i64).abs() <= 1, "{pos} -> {back}");
    }
}

#[test]
fn empty_story_has_one_line() {
    let (d, sid, _) = doc_with("", Rect::new(0.0, 0.0, 200.0, 100.0), ParaAttrs::default());
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert_eq!(cs.line_count(), 1);
    assert!(caret(&cs, 0).is_some());
}

#[test]
fn wrap_pushes_text_aside() {
    let (mut d, sid, _) = doc_with(&[LOREM; 2].join(" "), Rect::new(0.0, 0.0, 300.0, 800.0), ParaAttrs::default());
    let lid = d.default_layer();
    let id = ItemId(d.alloc());
    let mut it =
        designcraft_doc::Item::new(id, lid, designcraft_doc::Shape::Rectangle, designcraft_geom::shapes::rectangle(Rect::new(0.0, 0.0, 120.0, 60.0)));
    it.wrap.mode = WrapMode::BoundingBox;
    it.wrap.offsets = [0.0, 0.0, 6.0, 6.0];
    d.insert_item(SpreadRef::Doc(0), it, None).unwrap();
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let first = &cs.frames[0].lines[0];
    assert!(first.x0 >= 126.0 - 1e-6, "{}", first.x0);
    let below = cs.frames[0].lines.iter().find(|l| l.baseline - l.ascent > 66.0).unwrap();
    assert!(below.x0 < 1.0);
}

#[test]
fn paragraph_composer_is_no_worse_than_greedy() {
    // Sum of squared slack over lines (excluding last) should not exceed the greedy result.
    let measure = 180.0;
    let mk = |c: designcraft_doc::Composer| {
        let (d, sid, _) = doc_with(
            &[LOREM; 2].join(" "),
            Rect::new(0.0, 0.0, measure, 4000.0),
            ParaAttrs { composer: Some(c), hyphenate: Some(false), ..Default::default() },
        );
        let cs = compose_story(&d, sid, &ComposeOptions::default());
        let lines = all_lines(&cs);
        let n = lines.len();
        lines[..n - 1].iter().map(|l| (l.x1 - l.end_x).powi(2)).sum::<f64>()
    };
    let kp = mk(designcraft_doc::Composer::Paragraph);
    let gr = mk(designcraft_doc::Composer::SingleLine);
    assert!(kp <= gr * 1.05 + 1.0, "kp {kp} greedy {gr}");
}

#[test]
fn tabs_align_to_stops() {
    let pa = ParaAttrs {
        tabs: Some(vec![designcraft_doc::TabStop { position: 100.0, align: TabAlign::Right, leader: String::new(), align_on: String::new() }]),
        ..Default::default()
    };
    let (d, sid, _) = doc_with("Name\t42", Rect::new(0.0, 0.0, 300.0, 100.0), pa);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let l = &cs.frames[0].lines[0];
    assert!((l.end_x - 100.0).abs() < 0.5, "{}", l.end_x);
}

const SWATCH_TEXT: &str = "Color holds it all together. A restrained palette of two or three swatches, applied consistently to headlines, rules and backgrounds, gives a publication its voice. Spot inks, tints and gradients are all just named swatches, so a single change ripples through every page.";

/// Worst word-space ratio (excluding last lines) of `text` in a narrow justified column.
fn narrow_worst(para: ParaAttrs) -> f64 {
    let para = ParaAttrs { align: Some(Align::LeftJustified), first_line_indent: Some(12.0), hyph_min_word: Some(6), ..para };
    let mut d = Document::new(&NewDocument::default());
    let lid = d.default_layer();
    let (_, sid) = d
        .add_text_frame(SpreadRef::Doc(0), Rect::new(0.0, 0.0, 170.67, 2000.0), lid, SWATCH_TEXT, ParaFormat { para, ..Default::default() })
        .unwrap();
    d.story_mut(sid).unwrap().format_chars(0..SWATCH_TEXT.len(), |f| f.over.size = Some(9.75));
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let mut worst = 0.0f64;
    for l in all_lines(&cs) {
        eprintln!("{:5.2} {}", l.spacing, &SWATCH_TEXT[l.range.clone()]);
        assert!(l.end_x <= l.x1 + 0.5, "overfull line");
        if !l.last_in_para {
            assert!((l.end_x - l.x1).abs() < 0.6, "justified line ends at {} not {}", l.end_x, l.x1);
            worst = worst.max(l.spacing);
        }
    }
    worst
}

#[test]
fn justified_narrow_column_has_no_extreme_lines() {
    // Word spacing only (InDesign's defaults: letter spacing 0%, glyph scaling 100%). The first
    // line can't do better than ~2.7: "…together. A re-" is the longest first line that fits.
    let words_only = narrow_worst(ParaAttrs::default());
    assert!(words_only < 4.5, "loose line ratio {words_only}");
    // A typical magazine body setup: letter spacing −5…10% and glyph scaling 97…103% absorb the rest.
    let full = narrow_worst(ParaAttrs {
        letter_space_min: Some(-0.05),
        letter_space_max: Some(0.10),
        glyph_scale_min: Some(0.97),
        glyph_scale_max: Some(1.03),
        ..Default::default()
    });
    assert!(full < 2.0, "loose line ratio {full}");
}

#[test]
fn glyph_scaling_and_letter_spacing_are_applied() {
    let para = ParaAttrs {
        align: Some(Align::LeftJustified),
        hyphenate: Some(false),
        letter_space_min: Some(-0.05),
        letter_space_max: Some(0.2),
        glyph_scale_min: Some(0.95),
        glyph_scale_max: Some(1.05),
        ..Default::default()
    };
    let text = CORPUS.join(" ");
    let (d, sid, _) = doc_with(&text, Rect::new(0.0, 0.0, 150.0, 20000.0), para.clone());
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let lines = all_lines(&cs);
    let mut scaled = false;
    let mut sum = 0.0;
    for l in &lines[..lines.len() - 1] {
        assert!((l.end_x - l.x1).abs() < 0.6, "justified line ends at {} not {}", l.end_x, l.x1);
        sum += l.spacing;
        let k = 12.0 / l.glyphs[0].face.upem;
        scaled |= l.glyphs.iter().any(|g| g.visible && (g.sx / k - 1.0).abs() > 1e-3);
    }
    assert!(scaled, "some line uses glyph scaling");
    // Word spaces stay closer to their desired width than with word spacing alone.
    let plain = ParaAttrs { align: Some(Align::LeftJustified), hyphenate: Some(false), ..Default::default() };
    let (d, sid, _) = doc_with(&text, Rect::new(0.0, 0.0, 150.0, 20000.0), plain);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let pl = all_lines(&cs);
    let plain_avg = pl[..pl.len() - 1].iter().map(|l| l.spacing).sum::<f64>() / (pl.len() - 1) as f64;
    let avg = sum / (lines.len() - 1) as f64;
    assert!(avg < plain_avg - 0.1, "with tiers {avg:.3}, words only {plain_avg:.3}");
    // Desired values apply to every line, including ragged text.
    let wide = |ls: f64| {
        let (d, sid, _) =
            doc_with("Hello world", Rect::new(0.0, 0.0, 300.0, 100.0), ParaAttrs { letter_space_desired: Some(ls), ..Default::default() });
        compose_story(&d, sid, &ComposeOptions::default()).frames[0].lines[0].end_x
    };
    assert!(wide(0.5) > wide(0.0) + 10.0 * 0.5 * 2.0);
}

#[test]
fn hyphen_limit_is_a_hard_constraint() {
    let text = CORPUS.join(" ");
    for limit in [1u32, 2] {
        let para = ParaAttrs { align: Some(Align::LeftJustified), hyph_limit: Some(limit), hyph_weight: Some(0.0), ..Default::default() };
        let (d, sid, _) = doc_with(&text, Rect::new(0.0, 0.0, 130.0, 100_000.0), para);
        let cs = compose_story(&d, sid, &ComposeOptions::default());
        let mut run = 0;
        let mut any = false;
        for l in all_lines(&cs) {
            run = if l.hyphenated { run + 1 } else { 0 };
            any |= l.hyphenated;
            assert!(run <= limit, "{run} consecutive hyphens with limit {limit}");
        }
        assert!(any);
    }
}

#[test]
fn hyphenation_zone_limits_ragged_hyphens() {
    let text = CORPUS.join(" ");
    let count = |zone: f64, composer: designcraft_doc::Composer| {
        let para =
            ParaAttrs { align: Some(Align::Left), hyph_zone: Some(zone), composer: Some(composer), hyph_weight: Some(0.0), ..Default::default() };
        let (d, sid, _) = doc_with(&text, Rect::new(0.0, 0.0, 110.0, 100_000.0), para);
        let cs = compose_story(&d, sid, &ComposeOptions::default());
        all_lines(&cs).iter().filter(|l| l.hyphenated).count()
    };
    for c in [designcraft_doc::Composer::Paragraph, designcraft_doc::Composer::SingleLine] {
        let (free, zoned, huge) = (count(0.0, c), count(36.0, c), count(1000.0, c));
        // A huge zone leaves only words that start a line (no space to break at) hyphenable.
        assert!(free > 0 && zoned <= free && huge * 4 <= free, "{c:?}: {free} {zoned} {huge}");
    }
}

#[test]
fn discretionary_hyphen_replaces_automatic_points() {
    let text = "aaaa bbbb cccc extraordi\u{AD}narily dddd eeee";
    let para = ParaAttrs { align: Some(Align::Left), ..Default::default() };
    let (d, sid, _) = doc_with(text, Rect::new(0.0, 0.0, 120.0, 1000.0), para);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let soft = text.find('\u{AD}').unwrap();
    for l in all_lines(&cs) {
        if l.hyphenated {
            let last = l.glyphs.iter().rfind(|g| g.len > 0).unwrap();
            assert!(last.byte + last.len >= soft && last.byte <= soft + 2, "{:?}", &text[l.range.clone()]);
        }
    }
    // Hyphenation points of the word itself: none besides the discretionary one.
    let lines = all_lines(&cs);
    assert!(lines.iter().filter(|l| l.hyphenated).count() <= 1);
}

/// Story of `n` one-line filler paragraphs followed by `extra` paragraphs, in a 2-column frame.
fn keep_doc(fillers: usize, extra: &[&str], height: f64) -> (Document, StoryId, Vec<usize>) {
    let mut parts: Vec<String> = (0..fillers).map(|k| format!("Filler {k}")).collect();
    parts.extend(extra.iter().map(|s| s.to_string()));
    let text = parts.join("\n");
    let (mut d, sid, fid) = doc_with(&text, Rect::new(0.0, 0.0, 400.0, height), ParaAttrs::default());
    d.item_mut(fid).unwrap().text_frame_mut().unwrap().options.columns = 2;
    (d, sid, (0..parts.len()).collect())
}

fn column_of_para(cs: &ComposedStory, pi: usize) -> Vec<u32> {
    all_lines(cs).iter().filter(|l| l.para == pi).map(|l| l.column).collect()
}

/// Lines that fit in one column of the keep test frame.
fn lines_per_column(height: f64) -> usize {
    let (d, sid, _) = keep_doc(60, &[], height);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    all_lines(&cs).iter().filter(|l| l.column == 0).count()
}

#[test]
fn keep_with_next_moves_a_heading() {
    let h = 200.0;
    let n = lines_per_column(h);
    let body = "Body text that follows the heading.";
    let (mut d, sid, _) = keep_doc(n - 1, &["Heading", body], h);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert_eq!(column_of_para(&cs, n - 1), vec![0], "without keeps the heading ends column 0");
    d.story_mut(sid).unwrap().paras[n - 1].para.keep_with_next = Some(1);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert_eq!(column_of_para(&cs, n - 1), vec![1]);
    assert_eq!(column_of_para(&cs, n), vec![1]);
    assert_eq!(column_of_para(&cs, n - 2), vec![0]);
}

#[test]
fn keep_lines_together_moves_the_paragraph() {
    let h = 200.0;
    let n = lines_per_column(h);
    let long = [LOREM, LOREM].join(" ");
    let (mut d, sid, _) = keep_doc(n - 3, &[&long], h);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert!(column_of_para(&cs, n - 3).contains(&0));
    d.story_mut(sid).unwrap().paras[n - 3].para.keep_lines_together = Some(true);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert!(column_of_para(&cs, n - 3).iter().all(|&c| c == 1));
}

#[test]
fn keep_first_and_last_lines() {
    let h = 200.0;
    let n = lines_per_column(h);
    // Orphan: one line at the bottom of column 0 → the paragraph moves.
    let (mut d, sid, _) = keep_doc(n - 1, &[LOREM], h);
    {
        let p = &mut d.story_mut(sid).unwrap().paras[n - 1].para;
        p.keep_lines_together = Some(true);
        p.keep_all_lines = Some(false);
    }
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let cols = column_of_para(&cs, n - 1);
    assert!(cols.iter().all(|&c| c == 1), "{cols:?}");
    // Widow: a paragraph whose last line would sit alone in column 1 gives it a companion.
    let para_lines = {
        let (d, sid, _) = keep_doc(0, &[LOREM], 2000.0);
        compose_story(&d, sid, &ComposeOptions::default()).line_count()
    };
    assert!(para_lines >= 4);
    let start = n - (para_lines - 1);
    let (mut d, sid, _) = keep_doc(start, &[LOREM], h);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert_eq!(column_of_para(&cs, start).iter().filter(|&&c| c == 1).count(), 1, "one widowed line without keeps");
    {
        let p = &mut d.story_mut(sid).unwrap().paras[start].para;
        p.keep_lines_together = Some(true);
        p.keep_all_lines = Some(false);
    }
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let cols = column_of_para(&cs, start);
    assert_eq!(cols.iter().filter(|&&c| c == 1).count(), 2, "{cols:?}");
    assert!(cols.iter().filter(|&&c| c == 0).count() >= 2);
}

#[test]
fn balance_ragged_lines_evens_a_headline() {
    let text = "Balanced headlines read better than a stub";
    let widths = |balance: bool| {
        let para = ParaAttrs { balance_ragged: Some(balance), hyphenate: Some(false), ..Default::default() };
        let (mut d, sid, _) = doc_with(text, Rect::new(0.0, 0.0, 300.0, 500.0), para);
        d.story_mut(sid).unwrap().format_chars(0..text.len(), |f| f.over.size = Some(18.0));
        let cs = compose_story(&d, sid, &ComposeOptions::default());
        all_lines(&cs).iter().map(|l| l.end_x - l.x0).collect::<Vec<_>>()
    };
    let plain = widths(false);
    let bal = widths(true);
    assert_eq!(plain.len(), bal.len());
    assert!(plain.len() >= 2);
    let spread = |w: &[f64]| w.iter().cloned().fold(f64::MIN, f64::max) - w.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread(&bal) < spread(&plain), "{bal:?} vs {plain:?}");
}

#[test]
fn optical_margin_hangs_punctuation() {
    let text = "Hanging punctuation, quietly. “Quotes” sit outside the measure, as do commas, periods and hyphens.";
    let run = |optical: bool| {
        let para = ParaAttrs { align: Some(Align::LeftJustified), optical_margin: Some(optical), ..Default::default() };
        let (d, sid, _) = doc_with(text, Rect::new(0.0, 0.0, 120.0, 1000.0), para);
        compose_story(&d, sid, &ComposeOptions::default())
    };
    let plain = run(false);
    let hung = run(true);
    for l in all_lines(&plain) {
        assert!(l.end_x <= l.x1 + 0.5);
    }
    // Some justified line ends with punctuation that now protrudes past the right edge.
    let lines = all_lines(&hung);
    assert!(lines[..lines.len() - 1].iter().any(|l| l.end_x > l.x1 + 0.5), "no line hangs");
    assert!(lines.iter().all(|l| l.end_x <= l.x1 + 12.0));
}

// ---------- golden paragraph corpus ----------

/// Original prose (written for this corpus) with a realistic mix of short and long words.
const CORPUS: &[&str] = &[
    "Every printed page is a negotiation between the designer and the reader. The designer proposes an order: a column of type, a picture that interrupts it, a caption that explains the picture. The reader accepts that order only when nothing on the page calls attention to itself for the wrong reasons.",
    "Justified text is especially unforgiving. When the measure is narrow, the composer must choose between loose lines with rivers of white running down the column, tight lines in which words collide, and hyphenated lines that interrupt the rhythm of reading. Good composition balances all three considerations across the whole paragraph instead of one line at a time.",
    "Hyphenation dictionaries record where conventional syllable divisions fall in thousands of ordinary words: international, responsibility, characteristically, misunderstanding, photographer, extraordinarily, administration, comprehensive, establishment, unquestionably and their many relatives.",
    "Magazines often set body copy in two or three columns on a page, with generous gutters and a baseline grid that keeps neighbouring lines aligned. Headlines span the columns, pull quotes break into them, and captions sit beside photographs in a smaller, contrasting typeface.",
    "The typesetter of the nineteenth century worked with metal sorts, composing sticks and wedges of spacing material. Many of the conventions we take for granted today, such as the preference for consistent word spacing over consistent line endings, were established by those craftsmen long before computers automated their decisions.",
    "Readers rarely notice good typography, but they immediately feel the discomfort of bad typography: uneven spacing, awkward breaks, widowed lines stranded at the top of a column, and orphaned headings left at the bottom of a page without the paragraph that follows them.",
];

#[derive(Debug, Default)]
struct CorpusStats {
    lines: usize,
    overfull: usize,
    hyphens: usize,
    sum_spacing: f64,
    worst: f64,
    loose: usize,
}

fn corpus_stats(align: Align, measure: f64, size: f64) -> CorpusStats {
    let text = CORPUS.join("\n");
    let para = ParaAttrs { align: Some(align), ..Default::default() };
    let mut d = Document::new(&NewDocument::default());
    let lid = d.default_layer();
    let (_, sid) =
        d.add_text_frame(SpreadRef::Doc(0), Rect::new(0.0, 0.0, measure, 100_000.0), lid, &text, ParaFormat { para, ..Default::default() }).unwrap();
    d.story_mut(sid).unwrap().format_chars(0..text.len(), |f| f.over.size = Some(size));
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    assert!(!cs.is_overset());
    let mut st = CorpusStats::default();
    for l in all_lines(&cs) {
        if l.end_x > l.x1 + 0.5 {
            st.overfull += 1;
        }
        if l.hyphenated {
            st.hyphens += 1;
        }
        if !l.last_in_para {
            st.lines += 1;
            st.sum_spacing += l.spacing;
            st.worst = st.worst.max(l.spacing);
            if l.spacing > 1.33 + 1e-6 {
                st.loose += 1;
            }
        }
    }
    st
}

#[test]
fn golden_corpus_spacing() {
    let mut total = CorpusStats::default();
    for &(measure, size) in &[(120.0, 9.0), (170.67, 9.75), (240.0, 10.0), (340.0, 11.0)] {
        for align in [Align::LeftJustified, Align::Left] {
            let st = corpus_stats(align, measure, size);
            let avg = st.sum_spacing / st.lines.max(1) as f64;
            eprintln!(
                "{align:?} measure {measure:6.2} size {size:5.2}: lines {:3} hyphens {:2} avg {avg:.3} worst {:.3} loose {:2} overfull {}",
                st.lines, st.hyphens, st.worst, st.loose, st.overfull
            );
            assert_eq!(st.overfull, 0, "overfull lines at {measure}");
            if align == Align::LeftJustified && measure >= 170.0 {
                assert!(st.worst < 3.5, "worst justified line {} at {measure}", st.worst);
                assert!(avg < 1.65, "average justified word-space ratio {avg} at {measure}");
            }
            if align == Align::LeftJustified {
                total.lines += st.lines;
                total.sum_spacing += st.sum_spacing;
                total.worst = total.worst.max(st.worst);
                total.loose += st.loose;
            }
        }
    }
    let avg = total.sum_spacing / total.lines.max(1) as f64;
    eprintln!("justified total: lines {} avg {avg:.3} worst {:.3} loose {}", total.lines, total.worst, total.loose);
    assert!(avg < 1.9, "average justified word-space ratio {avg}");
}

#[test]
#[ignore = "perf: run with --release -- --ignored --nocapture"]
fn perf_compose_300k_story() {
    let mut text = String::new();
    let mut k = 0;
    while text.len() < 300_000 {
        text.push_str(CORPUS[k % CORPUS.len()]);
        text.push('\n');
        k += 1;
    }
    let para = ParaAttrs { align: Some(Align::LeftJustified), ..Default::default() };
    let mut d = Document::new(&NewDocument::default());
    let lid = d.default_layer();
    let (_, sid) =
        d.add_text_frame(SpreadRef::Doc(0), Rect::new(0.0, 0.0, 240.0, 3_000_000.0), lid, &text, ParaFormat { para, ..Default::default() }).unwrap();
    // Cold run (loads hyphenation data, fills the word caches), then the best of a few warm runs.
    let t = std::time::Instant::now();
    let _ = compose_story(&d, sid, &ComposeOptions::default());
    let cold = t.elapsed().as_secs_f64() * 1000.0;
    let mut ms = f64::INFINITY;
    let mut cs = ComposedStory::default();
    for _ in 0..5 {
        let t = std::time::Instant::now();
        cs = compose_story(&d, sid, &ComposeOptions::default());
        ms = ms.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    eprintln!("composed {} chars, {} lines: cold {cold:.1} ms, warm {ms:.1} ms", text.len(), cs.line_count());
    assert!(!cs.is_overset());
    if !cfg!(debug_assertions) {
        assert!(ms < 300.0, "composition took {ms:.1} ms");
    }
}
