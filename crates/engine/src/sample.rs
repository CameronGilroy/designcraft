//! A sample magazine document (original text; imagery generated in code) used for the welcome
//! screen, screenshots and tests.

use std::sync::Arc;

use designcraft_color::{Color, Gradient, GradientKind, GradientStop, Swatch, SwatchValue};
use designcraft_doc::build::NewDocument;
use designcraft_doc::{
    Align, Asset, AssetId, CharAttrs, CharFormat, CharacterStyle, Content, Document, Fill, FirstBaseline, Graphic, Item, ItemId, Leading, Margins,
    ParaAttrs, ParaFormat, ParagraphStyle, Rule, Shape, SpreadRef, Stroke, VerticalJustification, WrapMode, story,
};
use designcraft_geom::{Affine, Rect, shapes};

const BODY: &str = "Every page begins as an empty field of possibility. The grid arrives first: margins that frame the reading area, columns that set the rhythm, and a baseline grid that keeps every line in step from one column to the next. Good layout is quiet. It lets the reader move through a story without noticing the machinery that carries them.\n\
Typography does the heavy lifting. A paragraph composer weighs every possible line break at once, trading a slightly loose line here for a much better one three lines later. The result is an even texture of type, free of rivers and ragged gaps, where hyphens appear only when they truly help.\n\
Images give a spread its pulse. A strong photograph bleeds off the edge of the page, while smaller pictures sit inside the grid and let the text wrap around them. Captions, pull quotes and running heads add layers that a careful reader can explore at their own pace.\n\
Color holds it all together. A restrained palette of two or three swatches, applied consistently to headlines, rules and backgrounds, gives a publication its voice. Spot inks, tints and gradients are all just named swatches, so a single change ripples through every page.\n\
Behind the scenes, styles are the secret to working fast. Paragraph styles define the voice of each element, character styles pick out emphasis, and object styles keep frames consistent. Change a style once and the whole document follows, from the cover to the very last page.\n\
And when the layout is done, the document must travel: to a printer as a press-ready PDF with bleed and crop marks, to a colleague as an open interchange file, or to a screen as a crisp image. A layout tool earns its keep by making every one of those journeys effortless.";

/// Procedural "photograph": a dusk sky with a sun and layered hills.
fn art_png(w: u32, h: u32, hue: f32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let sun = (w as f32 * 0.68, h as f32 * 0.42, h as f32 * 0.16);
    for y in 0..h {
        for x in 0..w {
            let fy = y as f32 / h as f32;
            let fx = x as f32 / w as f32;
            // Sky gradient.
            let top = [0.10 + hue * 0.2, 0.12, 0.32 + hue * 0.1];
            let mid = [0.95, 0.45 + hue * 0.2, 0.30];
            let t = (fy / 0.62).min(1.0);
            let mut c = [top[0] + (mid[0] - top[0]) * t, top[1] + (mid[1] - top[1]) * t, top[2] + (mid[2] - top[2]) * t];
            // Sun glow.
            let d = ((x as f32 - sun.0).powi(2) + (y as f32 - sun.1).powi(2)).sqrt();
            if d < sun.2 {
                c = [1.0, 0.86, 0.55];
            } else {
                let g = (1.0 - (d - sun.2) / (h as f32 * 0.5)).max(0.0).powi(2) * 0.5;
                c = [c[0] + g, c[1] + g * 0.7, c[2] + g * 0.3];
            }
            // Hills.
            for (k, (amp, base, col)) in
                [(0.05, 0.62, [0.32, 0.16, 0.30]), (0.07, 0.72, [0.20, 0.10, 0.22]), (0.04, 0.84, [0.10, 0.06, 0.14])].iter().enumerate()
            {
                let hill = base + amp * ((fx * (5.0 + k as f32 * 3.0) + k as f32).sin() + 0.5 * (fx * 13.0 + hue * 4.0).sin());
                if fy > hill {
                    c = *col;
                }
            }
            for v in c {
                px.push((v.clamp(0.0, 1.0) * 255.0) as u8);
            }
            px.push(255);
        }
    }
    let r = designcraft_render::Rendered { width: w, height: h, pixels: px };
    r.to_png()
}

fn para_style(name: &str, based: Option<&str>, para: ParaAttrs, chars: CharAttrs) -> ParagraphStyle {
    ParagraphStyle { name: name.into(), based_on: based.map(Into::into), next_style: None, para, chars, shortcut: String::new() }
}

fn image(d: &mut Document, name: &str, w: u32, h: u32, hue: f32) -> AssetId {
    let id = AssetId(d.alloc());
    let data = art_png(w, h, hue);
    d.assets.insert(id, Arc::new(Asset { id, name: name.into(), mime: "image/png".into(), link: None, data: Arc::new(data), pixels: Some((w, h)) }));
    id
}

fn graphic_frame(d: &mut Document, sr: SpreadRef, r: Rect, asset: AssetId, px: (u32, u32)) -> ItemId {
    let id = ItemId(d.alloc());
    let lid = d.default_layer();
    let mut it = Item::new(id, lid, Shape::Rectangle, shapes::rectangle(r));
    let (nw, nh) = (px.0 as f64, px.1 as f64);
    let k = (r.width() / nw).max(r.height() / nh);
    it.content = Content::Graphic(Graphic {
        asset,
        size: (nw, nh),
        xf: Affine::translate((r.x0 + (r.width() - nw * k) / 2.0, r.y0 + (r.height() - nh * k) / 2.0)) * Affine::scale(k),
        auto_fit: designcraft_doc::Fitting::FillProportionally,
    });
    it.object_style = designcraft_doc::BASIC_GRAPHICS_FRAME.into();
    d.insert_item(sr, it, None).expect("spread");
    id
}

fn text(d: &mut Document, sr: SpreadRef, r: Rect, t: &str, style: &str) -> (ItemId, designcraft_doc::StoryId) {
    let lid = d.default_layer();
    d.add_text_frame(sr, r, lid, t, ParaFormat { style: style.into(), ..Default::default() }).expect("spread")
}

pub fn magazine() -> Document {
    let mut d = Document::new(&NewDocument {
        title: "Quarterly — Spring Issue".into(),
        pages: 4,
        margins: Margins { top: 54.0, bottom: 54.0, inside: 54.0, outside: 42.0 },
        columns: 3,
        gutter: 14.0,
        bleed: [9.0; 4],
        ..Default::default()
    });
    d.title = "Quarterly — Spring Issue".into();
    // Swatches.
    d.swatches.push(Swatch::color("Ink Plum", Color::cmyk(0.62, 0.95, 0.30, 0.25)));
    d.swatches.push(Swatch::color("Sunset", Color::cmyk(0.0, 0.62, 0.78, 0.0)));
    d.swatches.push(Swatch::color("Paper Warm", Color::cmyk(0.0, 0.03, 0.08, 0.0)));
    d.swatches.push(Swatch {
        name: "Dusk Gradient".into(),
        value: SwatchValue::Gradient {
            gradient: Gradient {
                kind: GradientKind::Linear,
                stops: vec![
                    GradientStop { offset: 0.0, color: Color::cmyk(0.62, 0.95, 0.30, 0.25), opacity: 1.0, midpoint: 0.5 },
                    GradientStop { offset: 1.0, color: Color::cmyk(0.0, 0.62, 0.78, 0.0), opacity: 1.0, midpoint: 0.5 },
                ],
            },
        },
        locked: false,
        named: true,
    });
    // Styles.
    let serif = |style: &str, size: f64| CharAttrs {
        font_family: Some("Source Serif 4".into()),
        font_style: Some(style.into()),
        size: Some(size),
        ..Default::default()
    };
    let sans = |style: &str, size: f64| CharAttrs {
        font_family: Some("Source Sans 3".into()),
        font_style: Some(style.into()),
        size: Some(size),
        ..Default::default()
    };
    {
        let st = d.styles_mut();
        st.paragraph.push(para_style(
            "Body",
            Some(story::BASIC_PARAGRAPH),
            ParaAttrs { align: Some(Align::LeftJustified), first_line_indent: Some(12.0), hyph_min_word: Some(6), ..Default::default() },
            CharAttrs { leading: Some(Leading::Points(13.5)), ..serif("Regular", 9.75) },
        ));
        st.paragraph.push(para_style(
            "Body First",
            Some("Body"),
            ParaAttrs { first_line_indent: Some(0.0), ..Default::default() },
            CharAttrs::default(),
        ));
        st.paragraph.push(para_style(
            "Headline",
            None,
            ParaAttrs { align: Some(Align::Left), space_after: Some(10.0), hyphenate: Some(false), ..Default::default() },
            CharAttrs { leading: Some(Leading::Points(46.0)), tracking: Some(-20.0), fill: Some("Ink Plum".into()), ..serif("Bold", 46.0) },
        ));
        st.paragraph.push(para_style(
            "Deck",
            None,
            ParaAttrs { space_after: Some(14.0), hyphenate: Some(false), ..Default::default() },
            CharAttrs { leading: Some(Leading::Points(19.0)), fill: Some("[Black]".into()), fill_tint: Some(0.75), ..serif("Italic", 14.0) },
        ));
        st.paragraph.push(para_style(
            "Kicker",
            None,
            ParaAttrs {
                space_after: Some(6.0),
                rule_below: Some(Rule { on: true, weight: 1.5, color: "Sunset".into(), offset: 5.0, column_width: false, ..Default::default() }),
                ..Default::default()
            },
            CharAttrs {
                tracking: Some(160.0),
                capitalization: Some(designcraft_doc::Capitalization::AllCaps),
                fill: Some("Sunset".into()),
                ..sans("Semibold", 9.0)
            },
        ));
        st.paragraph.push(para_style(
            "Pull Quote",
            None,
            ParaAttrs { align: Some(Align::Left), hyphenate: Some(false), ..Default::default() },
            CharAttrs { leading: Some(Leading::Points(24.0)), fill: Some("Ink Plum".into()), ..serif("Italic", 19.0) },
        ));
        st.paragraph.push(para_style(
            "Caption",
            None,
            ParaAttrs::default(),
            CharAttrs { leading: Some(Leading::Points(10.5)), fill_tint: Some(0.7), ..sans("Regular", 7.5) },
        ));
        st.paragraph.push(para_style("Folio", None, ParaAttrs::default(), CharAttrs { tracking: Some(60.0), ..sans("Semibold", 7.5) }));
        st.paragraph.push(para_style(
            "Cover Title",
            None,
            ParaAttrs { align: Some(Align::Left), hyphenate: Some(false), ..Default::default() },
            CharAttrs { leading: Some(Leading::Points(88.0)), tracking: Some(-30.0), fill: Some("[Paper]".into()), ..serif("Bold", 96.0) },
        ));
        st.paragraph.push(para_style(
            "Cover Deck",
            None,
            ParaAttrs { hyphenate: Some(false), ..Default::default() },
            CharAttrs { leading: Some(Leading::Points(22.0)), fill: Some("[Paper]".into()), ..serif("Italic", 17.0) },
        ));
        st.character.push(CharacterStyle {
            name: "Emphasis".into(),
            based_on: None,
            chars: CharAttrs { font_style: Some("Italic".into()), ..Default::default() },
            shortcut: String::new(),
        });
        st.character.push(CharacterStyle {
            name: "Accent".into(),
            based_on: None,
            chars: CharAttrs { fill: Some("Sunset".into()), font_style: Some("Semibold".into()), ..Default::default() },
            shortcut: String::new(),
        });
    }
    let (w, h) = (612.0, 792.0);
    // Parent A: folio and running head.
    let pa = SpreadRef::Parent(0);
    {
        let (_, s) = text(&mut d, pa, Rect::new(42.0, 750.0, 300.0, 762.0), &format!("{}  ·  QUARTERLY", story::PAGE_NUMBER), "Folio");
        let _ = s;
        let (_, _) = text(&mut d, pa, Rect::new(w + 312.0, 750.0, w + w - 42.0, 762.0), &format!("SPRING ISSUE  ·  {}", story::PAGE_NUMBER), "Folio");
        if let Some(st) = d.story_mut(designcraft_doc::StoryId(d.next_id - 1)) {
            st.format_paras(0..0, |p| p.para.align = Some(Align::Right));
        }
        let lid = d.default_layer();
        for x in [42.0, w + 42.0] {
            let id = ItemId(d.alloc());
            let mut l = Item::new(id, lid, Shape::GraphicLine, shapes::line((x, 742.0).into(), (x + w - 96.0, 742.0).into()));
            l.stroke = Stroke { weight: 0.5, swatch: "[Black]".into(), tint: 0.4, ..Stroke::default() };
            d.insert_item(pa, l, None).expect("parent");
        }
    }
    // Page 1: cover (spread 0, single right page, no parent).
    let _ = d.apply_parent(&[0], None);
    let cover_art = image(&mut d, "dusk-cover.png", 1224, 1584, 0.0);
    graphic_frame(&mut d, SpreadRef::Doc(0), Rect::new(-9.0, -9.0, w + 9.0, h + 9.0), cover_art, (1224, 1584));
    let (_, _) = text(&mut d, SpreadRef::Doc(0), Rect::new(42.0, 60.0, 570.0, 120.0), "THE SPRING ISSUE  ·  NO. 01", "Kicker");
    if let Some(st) = d.stories.values().last().map(|s| s.id)
        && let Some(st) = d.story_mut(st)
    {
        st.format_chars(0..st.len(), |f| f.over.fill = Some("[Paper]".into()));
    }
    let (_, cs) = text(&mut d, SpreadRef::Doc(0), Rect::new(42.0, 450.0, 570.0, 690.0), "The Quiet\nArt of Layout", "Cover Title");
    let _ = cs;
    text(
        &mut d,
        SpreadRef::Doc(0),
        Rect::new(42.0, 700.0, 420.0, 760.0),
        "Grids, type and color: how great pages are made — and the tools that make them.",
        "Cover Deck",
    );

    // Spread 1 (pages 2–3).
    let s1 = SpreadRef::Doc(1);
    let lx = 42.0; // left page outside margin
    text(&mut d, s1, Rect::new(lx, 54.0, w - 54.0, 70.0), "Feature  ·  Design", "Kicker");
    let (_, hs) = text(&mut d, s1, Rect::new(lx, 80.0, w - 54.0, 190.0), "Notes on the Grid", "Headline");
    let _ = hs;
    text(
        &mut d,
        s1,
        Rect::new(lx, 182.0, w - 120.0, 240.0),
        "A good layout is invisible. Here is what is going on underneath the page — from the baseline grid to the paragraph composer.",
        "Deck",
    );
    let art2 = image(&mut d, "hills.png", 1200, 760, 0.6);
    let pic = graphic_frame(&mut d, s1, Rect::new(lx, 252.0, w - 54.0, 252.0 + 300.0), art2, (1200, 760));
    let _ = pic;
    text(
        &mut d,
        s1,
        Rect::new(lx, 558.0, w - 54.0, 572.0),
        "Evening light over layered hills — generated entirely in code for this sample.",
        "Caption",
    );
    // Body: threaded through 2 columns on the left page and 3 columns on the right page.
    let (f1, body) = text(&mut d, s1, Rect::new(lx, 584.0, w - 54.0, 738.0), BODY, "Body");
    if let Some(it) = d.item_mut(f1).and_then(Item::text_frame_mut) {
        it.options.columns = 2;
        it.options.gutter = 14.0;
    }
    let (f2, _) = text(&mut d, s1, Rect::new(w + 54.0, 54.0, 2.0 * w - 42.0, 708.0), "", "Body");
    if let Some(it) = d.item_mut(f2).and_then(Item::text_frame_mut) {
        it.options.columns = 3;
        it.options.gutter = 14.0;
    }
    d.thread(f1, f2).expect("thread");
    // First paragraph: Body First + accent lead-in.
    if let Some(st) = d.story_mut(body) {
        st.paras[0].style = "Body First".into();
        st.format_chars(0..20, |f| *f = CharFormat { style: "Accent".into(), over: Default::default() });
        // Repeat the body text so it flows across the spread.
        let more = format!("\n{BODY}\n{BODY}");
        let end = st.len();
        st.insert(end, &more);
    }
    // Pull quote with shading, wrapped.
    let (pq, pqs) = text(
        &mut d,
        s1,
        Rect::new(w + 54.0 + 178.0, 300.0, 2.0 * w - 42.0, 420.0),
        "“The best layouts disappear. What remains is the story.”",
        "Pull Quote",
    );
    if let Some(it) = d.item_mut(pq) {
        it.fill = Fill::swatch("Paper Warm");
        it.wrap.mode = WrapMode::BoundingBox;
        it.wrap.offsets = [10.0, 10.0, 10.0, 10.0];
        if let Some(tf) = it.text_frame_mut() {
            tf.options.inset = [14.0, 14.0, 14.0, 14.0];
            tf.options.vertical_justification = VerticalJustification::Center;
            tf.options.first_baseline = FirstBaseline::Ascent;
        }
    }
    let _ = pqs;
    // A sunset-gradient accent bar.
    let lid = d.default_layer();
    let id = ItemId(d.alloc());
    let mut bar = Item::new(id, lid, Shape::Rectangle, shapes::rectangle(Rect::new(w + 54.0, 600.0 + 120.0, 2.0 * w - 42.0, 600.0 + 126.0)));
    bar.fill = Fill { swatch: "Dusk Gradient".into(), ..Fill::none() };
    d.insert_item(s1, bar, None).expect("spread");

    // Page 4: color page.
    let s2 = SpreadRef::Doc(2);
    let id = ItemId(d.alloc());
    let mut bg = Item::new(id, lid, Shape::Rectangle, shapes::rectangle(Rect::new(-9.0, -9.0, w + 9.0, 396.0)));
    bg.fill = Fill::swatch("Ink Plum");
    d.insert_item(s2, bg, None).expect("spread");
    let (_, k) = text(&mut d, s2, Rect::new(42.0, 80.0, 400.0, 100.0), "Coming next", "Kicker");
    let _ = k;
    let (t4, _) = text(&mut d, s2, Rect::new(42.0, 110.0, 560.0, 330.0), "Color, Ink & Paper", "Cover Title");
    if let Some(sid) = d.item(t4).and_then(|i| i.text_frame()).map(|t| t.story)
        && let Some(st) = d.story_mut(sid)
    {
        st.format_chars(0..st.len(), |f| f.over.size = Some(64.0));
        st.format_chars(0..st.len(), |f| f.over.leading = Some(Leading::Points(64.0)));
    }
    for (i, sw) in ["Ink Plum", "Sunset", "C=100 M=0 Y=0 K=0", "Paper Warm"].iter().enumerate() {
        let id = ItemId(d.alloc());
        let x = 42.0 + i as f64 * 132.0;
        let mut c = Item::new(id, lid, Shape::Oval, shapes::ellipse(Rect::new(x, 440.0, x + 116.0, 556.0)));
        c.fill = Fill::swatch(sw);
        if *sw == "Paper Warm" {
            c.stroke = Stroke { weight: 0.5, ..Stroke::default() };
        }
        d.insert_item(s2, c, None).expect("spread");
        text(&mut d, s2, Rect::new(x, 566.0, x + 116.0, 580.0), sw, "Caption");
    }
    let (f4, _) = text(&mut d, s2, Rect::new(42.0, 610.0, 570.0, 730.0), &BODY[..BODY.find('\n').unwrap_or(BODY.len())], "Body First");
    if let Some(it) = d.item_mut(f4).and_then(Item::text_frame_mut) {
        it.options.columns = 2;
    }
    debug_assert!(d.check().is_ok(), "{:?}", d.check());
    d
}
