//! Glyph grids for the Glyphs panel: a font's characters drawn into square cells.

use designcraft_fonts::FontDb;
use vello_cpu::{RenderContext, Resources, kurbo, peniko};

use crate::Rendered;

/// Draw `chars` of `family`/`style` into a grid of `cols` columns of `cell`-pixel cells, in
/// `color` (straight RGBA) on transparency. Glyphs are scaled to the font's em and centred.
pub fn glyph_grid(family: &str, style: &str, chars: &[char], cols: u32, cell: u32, color: [u8; 4]) -> Rendered {
    let db = FontDb::global();
    let face = db.face(family, style);
    let cols = cols.max(1);
    let rows = (chars.len() as u32).div_ceil(cols).max(1);
    let (w, h) = ((cols * cell).clamp(1, u16::MAX as u32) as u16, (rows * cell).clamp(1, u16::MAX as u32) as u16);
    let mut ctx = RenderContext::new(w, h);
    ctx.set_paint(peniko::Color::from_rgba8(color[0], color[1], color[2], color[3]));
    let upem = face.units_per_em().max(1.0);
    let (asc, desc) = face.vertical_metrics();
    let k = cell as f64 * 0.62 / upem;
    for (i, c) in chars.iter().enumerate() {
        let gid = face.glyph_for(*c);
        if gid == 0 {
            continue;
        }
        let outline = db.outline(&face, gid);
        if outline.elements().is_empty() {
            continue;
        }
        let (col, row) = (i as u32 % cols, i as u32 / cols);
        let adv = face.advance(gid);
        // Horizontally centred by advance; the baseline puts the em box in the middle.
        let x = (col * cell) as f64 + (cell as f64 - adv * k) / 2.0;
        let mid = (asc - desc.abs()) / 2.0;
        let y = (row * cell) as f64 + cell as f64 / 2.0 + mid * k;
        ctx.set_transform(kurbo::Affine::translate((x, y)) * kurbo::Affine::scale(k));
        ctx.fill_path(&outline);
    }
    ctx.flush();
    let mut pixels = vec![0u8; w as usize * h as usize * 4];
    let mut res = Resources::new();
    ctx.render(vello_cpu::PixmapMut::new(w, h, &mut pixels).expect("buffer size"), &mut res);
    Rendered { width: w as u32, height: h as u32, pixels }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_draws_each_cell() {
        let img = glyph_grid(designcraft_fonts::DEFAULT_FAMILY, "Regular", &['A', 'B', 'C', ' '], 2, 40, [0, 0, 0, 255]);
        assert_eq!((img.width, img.height), (80, 80));
        let ink = |cx: u32, cy: u32| {
            (cx * 40..cx * 40 + 40).flat_map(|x| (cy * 40..cy * 40 + 40).map(move |y| (x, y))).filter(|&(x, y)| img.pixel(x, y)[3] > 128).count()
        };
        assert!(ink(0, 0) > 30 && ink(1, 0) > 30 && ink(0, 1) > 30, "letters drawn");
        assert_eq!(ink(1, 1), 0, "a space is blank");
    }
}
