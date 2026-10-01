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
    let mut it = designcraft_doc::Item::new(id, lid, designcraft_doc::Shape::Rectangle, designcraft_geom::shapes::rectangle(Rect::new(0.0, 0.0, 120.0, 60.0)));
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
        let (d, sid, _) = doc_with(&[LOREM; 2].join(" "), Rect::new(0.0, 0.0, measure, 4000.0), ParaAttrs { composer: Some(c), hyphenate: Some(false), ..Default::default() });
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
    let mut pa = ParaAttrs::default();
    pa.tabs = Some(vec![designcraft_doc::TabStop { position: 100.0, align: TabAlign::Right, leader: String::new(), align_on: String::new() }]);
    let (d, sid, _) = doc_with("Name\t42", Rect::new(0.0, 0.0, 300.0, 100.0), pa);
    let cs = compose_story(&d, sid, &ComposeOptions::default());
    let l = &cs.frames[0].lines[0];
    assert!((l.end_x - 100.0).abs() < 0.5, "{}", l.end_x);
}
