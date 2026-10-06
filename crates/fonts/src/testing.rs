//! Synthetic fonts for tests (here and in dependent crates, through the `testing` feature).

use skrifa::MetadataProvider;

/// A font named `family` (style Regular) that maps exactly `chars`, all to one glyph: the bundled
/// Source Sans 3 Regular with its `name` and `cmap` tables replaced. Stands in for a font that
/// isn't installed (a system CJK font on a machine without it). `None` only if the bundled font
/// can't be read.
pub fn font_with(family: &str, chars: &[char]) -> Option<Vec<u8>> {
    let base = *crate::bundled().first()?;
    let gid = skrifa::FontRef::new(base).ok()?.charmap().map('X')?.to_u32();
    let be16 = |b: &[u8], at: usize| b.get(at..at + 2).and_then(|s| s.try_into().ok()).map(u16::from_be_bytes);
    let be32 = |b: &[u8], at: usize| b.get(at..at + 4).and_then(|s| s.try_into().ok()).map(u32::from_be_bytes);

    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    for r in 0..usize::from(be16(base, 4)?) {
        let rec = 12 + r * 16;
        let tag: [u8; 4] = base.get(rec..rec + 4)?.try_into().ok()?;
        let (offset, len) = (be32(base, rec + 8)? as usize, be32(base, rec + 12)? as usize);
        if &tag != b"name" && &tag != b"cmap" {
            tables.push((tag, base.get(offset..offset.checked_add(len)?)?.to_vec()));
        }
    }

    // `name`: format 0, family (1) and subfamily (2), Windows Unicode English.
    let strings: Vec<Vec<u8>> = [family, "Regular"].iter().map(|s| s.encode_utf16().flat_map(u16::to_be_bytes).collect()).collect();
    let mut name = Vec::new();
    for v in [0u16, 2, 6 + 2 * 12] {
        name.extend_from_slice(&v.to_be_bytes());
    }
    let mut at = 0usize;
    for (id, s) in [1u16, 2].iter().zip(&strings) {
        for v in [3u16, 1, 0x409, *id, u16::try_from(s.len()).ok()?, u16::try_from(at).ok()?] {
            name.extend_from_slice(&v.to_be_bytes());
        }
        at += s.len();
    }
    strings.iter().for_each(|s| name.extend_from_slice(s));
    tables.push((*b"name", name));

    // `cmap`: one format 12 subtable (Windows, full Unicode), one group per character.
    let mut cps: Vec<u32> = chars.iter().map(|c| *c as u32).collect();
    cps.sort_unstable();
    cps.dedup();
    let sub_len = u32::try_from(16 + 12 * cps.len()).ok()?;
    let mut cmap = Vec::new();
    for v in [0u16, 1, 3, 10] {
        cmap.extend_from_slice(&v.to_be_bytes());
    }
    cmap.extend_from_slice(&12u32.to_be_bytes());
    cmap.extend_from_slice(&12u16.to_be_bytes());
    cmap.extend_from_slice(&0u16.to_be_bytes());
    for v in [sub_len, 0, u32::try_from(cps.len()).ok()?] {
        cmap.extend_from_slice(&v.to_be_bytes());
    }
    for cp in cps {
        for v in [cp, cp, gid] {
            cmap.extend_from_slice(&v.to_be_bytes());
        }
    }
    tables.push((*b"cmap", cmap));

    // The file: table directory (sorted by tag), then the tables, each 4-byte aligned.
    tables.sort_by_key(|t| t.0);
    let n = u16::try_from(tables.len()).ok()?;
    let pow = 1u16 << (15 - n.leading_zeros().min(15));
    let mut out = base.get(0..4)?.to_vec();
    for v in [n, pow * 16, pow.trailing_zeros() as u16, n * 16 - pow * 16] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&u32::try_from(offset).ok()?.to_be_bytes());
        out.extend_from_slice(&u32::try_from(data.len()).ok()?.to_be_bytes());
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    Some(out)
}
