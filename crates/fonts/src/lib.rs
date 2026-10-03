//! DesignCraft fonts: the font database (bundled OFL families + user/system fonts), vertical
//! metrics, glyph outlines and OpenType shaping.
//!
//! Shaping here is style-agnostic: [`shape`] turns a string in one face into glyph ids, clusters
//! and advances in font units. `designcraft-compose` applies sizes, tracking, scaling and
//! justification on top.
#![forbid(unsafe_code)]

mod fontdb;

pub use fontdb::{FALLBACK_FAMILY, FaceRef, FontDb, FontFace, bundled};
pub use harfrust::Feature;
use harfrust::{Direction, ShapeOptions, Tag, UnicodeBuffer};
pub use kurbo::BezPath;
use skrifa::MetadataProvider;
use skrifa::instance::Size;

/// InDesign's default text font is a serif; ours is Source Serif 4.
pub const DEFAULT_FAMILY: &str = "Source Serif 4";

/// One shaped glyph, in font units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub gid: u32,
    /// Byte offset (in the shaped string) of the cluster this glyph belongs to.
    pub cluster: usize,
    pub x_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

/// An OpenType feature setting: `"liga"`, `"-kern"`, `"ss01"`.
pub fn feature(tag: &str) -> Option<Feature> {
    let (on, t) = match tag.strip_prefix('-') {
        Some(r) => (false, r),
        None => (true, tag.strip_prefix('+').unwrap_or(tag)),
    };
    let b = t.as_bytes();
    if b.len() != 4 {
        return None;
    }
    Some(Feature::new(Tag::new(&[b[0], b[1], b[2], b[3]]), on as u32, ..))
}

/// Shape `text` with `face`. `chars` lets callers substitute characters (e.g. uppercase for All
/// Caps) while keeping clusters pointing into the original string.
pub fn shape(face: &FontFace, text: &str, features: &[Feature], map: impl Fn(char) -> char) -> Vec<ShapedGlyph> {
    let mut out = Vec::with_capacity(text.len());
    let shaped = face.hb().map(|hb| {
        let shaper = face.shaper.shaper(&hb).instance(face.instance.as_ref()).build();
        let mut buf = UnicodeBuffer::new();
        for (i, c) in text.char_indices() {
            buf.add(map(c), i as u32);
        }
        buf.set_direction(Direction::LeftToRight);
        buf.guess_segment_properties();
        let gb = shaper.shape(buf, ShapeOptions::new().features(features));
        for (info, pos) in gb.glyph_infos().iter().zip(gb.glyph_positions()) {
            out.push(ShapedGlyph {
                gid: info.glyph_id,
                cluster: info.cluster as usize,
                x_advance: pos.x_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            });
        }
    });
    if shaped.is_none()
        && let Some(f) = face.skrifa()
    {
        let cmap = f.charmap();
        let gm = f.glyph_metrics(Size::unscaled(), face.location());
        for (i, c) in text.char_indices() {
            let g = cmap.map(map(c)).unwrap_or_default();
            let adv = gm.advance_width(g).unwrap_or(face.upem as f32 * 0.5);
            out.push(ShapedGlyph { gid: g.to_u32(), cluster: i, x_advance: adv.round() as i32, x_offset: 0, y_offset: 0 });
        }
    }
    out
}

/// Glyph id of the first of `chars` the face has (0 = .notdef).
pub fn first_glyph(face: &FontFace, chars: &[char]) -> u32 {
    chars.iter().map(|c| face.glyph_for(*c)).find(|g| *g != 0).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_families_load() {
        let db = FontDb::global();
        let fams = db.families();
        assert!(fams.iter().any(|f| f == DEFAULT_FAMILY), "{fams:?}");
        assert!(fams.iter().any(|f| f == "Source Sans 3"));
        let styles = db.styles(DEFAULT_FAMILY);
        for s in ["Regular", "Italic", "Bold", "Semibold"] {
            assert!(styles.iter().any(|x| x == s), "{s} in {styles:?}");
        }
    }

    #[test]
    fn shaping_produces_clusters_and_advances() {
        let face = FontDb::global().face(DEFAULT_FAMILY, "Regular");
        let g = shape(&face, "Hello", &[], |c| c);
        assert_eq!(g.len(), 5);
        assert_eq!(g.iter().map(|g| g.cluster).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        assert!(g.iter().all(|g| g.x_advance > 0 && g.gid != 0));
    }

    #[test]
    fn ligatures_can_be_disabled() {
        let face = FontDb::global().face(DEFAULT_FAMILY, "Regular");
        let on = shape(&face, "office", &[], |c| c);
        let off = shape(&face, "office", &[feature("-liga").unwrap()], |c| c);
        assert!(on.len() <= off.len());
        assert_eq!(off.len(), 6);
    }

    #[test]
    fn mapping_keeps_clusters() {
        let face = FontDb::global().face(DEFAULT_FAMILY, "Regular");
        let g = shape(&face, "ab", &[], |c| c.to_ascii_uppercase());
        assert_eq!(g[0].gid, face.glyph_for('A'));
        assert_eq!(g[1].cluster, 1);
    }

    #[test]
    fn variable_font_named_instances_are_styles() {
        // Uses a variable system font when one is installed (macOS ships several).
        let Some(data) =
            ["/System/Library/Fonts/Supplemental/Skia.ttf", "/System/Library/Fonts/NewYork.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"]
                .iter()
                .find_map(|p| std::fs::read(p).ok())
        else {
            return;
        };
        let db = FontDb::global();
        db.add_font(data.clone());
        let Some(fam) = skrifa::FontRef::new(&data).ok().and_then(|f| {
            (f.named_instances().len() > 1)
                .then(|| f.localized_strings(skrifa::string::StringId::FAMILY_NAME).english_or_first().map(|s| s.to_string()))?
        }) else {
            return;
        };
        let styles = db.styles(&fam);
        assert!(styles.len() > 1, "{styles:?}");
        let faces: Vec<_> = styles.iter().map(|s| db.face(&fam, s)).filter(|f| f.is_variable()).collect();
        let light = faces.iter().min_by(|a, b| a.weight.total_cmp(&b.weight)).unwrap();
        let heavy = faces.iter().max_by(|a, b| a.weight.total_cmp(&b.weight)).unwrap();
        assert!(heavy.weight > light.weight, "{light:?} {heavy:?}");
        let ink = |f: &FontFace| {
            let g = shape(f, "H", &[], |c| c);
            let b = kurbo::Shape::bounding_box(&*db.outline(f, g[0].gid));
            ((b.area() * 100.0).round() as i64, g[0].x_advance)
        };
        assert_ne!(ink(light), ink(heavy), "instances draw differently");
    }

    #[test]
    fn outlines_and_metrics() {
        let db = FontDb::global();
        let face = db.face(DEFAULT_FAMILY, "Bold");
        assert_eq!(face.style, "Bold");
        let o = db.outline(&face, face.glyph_for('O'));
        assert!(!o.elements().is_empty());
        assert!(face.ascent > 0.0 && face.descent > 0.0 && face.cap_height > face.x_height);
        assert!(feature("abc").is_none());
    }
}
