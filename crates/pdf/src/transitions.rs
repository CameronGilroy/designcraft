//! Page transitions (`/Trans`) for interactive PDF. krilla doesn't write them, so they're added
//! as an incremental update: each page with a transition is re-stated with a `/Trans` entry, with
//! a new cross-reference section pointing back at the original.

use designcraft_doc::{PageTransition, TransitionKind};

/// The `/Trans` dictionary for a transition.
fn trans_dict(t: &PageTransition) -> String {
    use TransitionKind as K;
    let dm = if t.horizontal { "/H" } else { "/V" };
    let m = if t.horizontal { "/I" } else { "/O" };
    let body = match t.kind {
        K::Blinds => format!("/S/Blinds/Dm{dm}"),
        K::Box => format!("/S/Box/M{m}"),
        K::Comb => "/S/Glitter/Di 0".into(),
        K::Cover => "/S/Cover/Di 0".into(),
        K::Dissolve => "/S/Dissolve".into(),
        K::Fade => "/S/Fade".into(),
        K::Push => "/S/Push/Di 0".into(),
        K::Split => format!("/S/Split/Dm{dm}/M{m}"),
        K::Uncover => "/S/Uncover/Di 0".into(),
        K::Wipe => "/S/Wipe/Di 0".into(),
        K::ZoomIn => "/S/Fly/M/I/SS 0.01".into(),
        K::ZoomOut => "/S/Fly/M/O/SS 0.01".into(),
    };
    format!("/Trans<</Type/Trans{body}/D {:.2}>>", t.duration.clamp(0.0, 60.0))
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// The integer after `key` in `s` (e.g. `/Size 14`, `/Root 13 0 R`).
fn int_after(s: &str, key: &str) -> Option<usize> {
    let i = s.find(key)? + key.len();
    let rest = s[i..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Byte range of object `id`'s body (between `obj` and `endobj`).
fn object(pdf: &[u8], id: usize) -> Option<std::ops::Range<usize>> {
    let head = format!("\n{id} 0 obj");
    let start = find(pdf, head.as_bytes(), 0)? + head.len();
    let end = find(pdf, b"endobj", start)?;
    Some(start..end)
}

/// Add transitions to the pages of `pdf` (index = PDF page). `None` when the file's structure
/// isn't the simple kind this understands (classic xref table, a flat page tree).
pub fn add_transitions(pdf: &[u8], trans: &[Option<PageTransition>]) -> Option<Vec<u8>> {
    if trans.iter().all(Option::is_none) {
        return Some(pdf.to_vec());
    }
    let t_at = rfind(pdf, b"trailer")?;
    let trailer = String::from_utf8_lossy(&pdf[t_at..]).to_string();
    let size = int_after(&trailer, "/Size")?;
    let root = int_after(&trailer, "/Root")?;
    let prev = int_after(&trailer, "startxref")?;
    let info = int_after(&trailer, "/Info");
    let id = trailer.find("/ID").and_then(|i| trailer[i..].find(']').map(|j| trailer[i..i + j + 1].to_string()));
    let cat = String::from_utf8_lossy(&pdf[object(pdf, root)?]).to_string();
    let pages_id = int_after(&cat, "/Pages")?;
    let pages = String::from_utf8_lossy(&pdf[object(pdf, pages_id)?]).to_string();
    let kids_at = pages.find("/Kids")?;
    let kids_str = &pages[kids_at + 5..];
    let kids_str = &kids_str[kids_str.find('[')? + 1..kids_str.find(']')?];
    let nums: Vec<usize> = kids_str.split_whitespace().filter_map(|t| t.parse().ok()).collect();
    let kids: Vec<usize> = nums.chunks(2).map(|c| c[0]).collect();
    let mut out = pdf.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut entries: Vec<(usize, usize)> = Vec::new();
    for (i, t) in trans.iter().enumerate() {
        let (Some(t), Some(&kid)) = (t, kids.get(i)) else { continue };
        let body = String::from_utf8_lossy(&pdf[object(pdf, kid)?]).to_string();
        if !body.contains("/Type/Page") && !body.contains("/Type /Page") {
            return None;
        }
        let close = body.rfind(">>")?;
        let new_body = format!("{}{}{}", &body[..close], trans_dict(t), &body[close..]);
        entries.push((kid, out.len()));
        out.extend_from_slice(format!("{kid} 0 obj{new_body}endobj\n").as_bytes());
    }
    let xref_at = out.len();
    let mut x = String::from("xref\n");
    for (kid, off) in &entries {
        x.push_str(&format!("{kid} 1\n{off:010} 00000 n\r\n"));
    }
    let info = info.map(|i| format!("/Info {i} 0 R")).unwrap_or_default();
    let id = id.unwrap_or_default();
    x.push_str(&format!("trailer\n<</Size {size}/Root {root} 0 R{info}{id}/Prev {prev}>>\nstartxref\n{xref_at}\n%%EOF\n"));
    out.extend_from_slice(x.as_bytes());
    Some(out)
}
