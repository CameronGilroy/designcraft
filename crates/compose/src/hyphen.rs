//! Dictionary-free English hyphenation (ours): breaks between syllables at vowel–consonant
//! boundaries (V-CV "ty-po", VC-CV "hap-pen"), respecting onset clusters ("gra-phy").
//!
//! A Liang pattern engine with loadable pattern sets is planned (see plan/adr); patterns are only
//! bundled once their licence is confirmed to fit the asset policy.

/// Hyphenation limits from the paragraph's Hyphenation settings.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub min_word: usize,
    pub after_first: usize,
    pub before_last: usize,
    pub capitalized: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { min_word: 5, after_first: 2, before_last: 2, capitalized: true }
    }
}

fn is_vowel(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y') || (c.is_alphabetic() && !c.is_ascii())
}

fn onset_pair(a: char, b: char) -> bool {
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    matches!(
        (a, b),
        ('c' | 's' | 't' | 'p' | 'w' | 'g', 'h')
            | ('b' | 'c' | 'd' | 'f' | 'g' | 'k' | 'p' | 't', 'r')
            | ('b' | 'c' | 'f' | 'g' | 'k' | 'p' | 's', 'l')
            | ('q', 'u')
            | ('s', 't' | 'p' | 'c' | 'k')
    )
}

/// Common English suffixes that form their own syllable (break before them).
/// Common English prefixes (break after them).
const PREFIXES: &[&str] = &["re", "de", "pre", "un", "dis", "con", "com", "inter", "over", "under", "trans", "sub", "super", "anti", "auto"];

const SUFFIXES: &[&str] = &["tion", "sion", "ment", "ness", "less", "ful", "able", "ible", "ing", "ly"];

/// Allowed hyphenation points in `word` as char indices (a hyphen goes before that char).
pub fn hyphen_points(word: &str, lim: &Limits) -> Vec<usize> {
    let c: Vec<char> = word.chars().collect();
    let n = c.len();
    let min_before = lim.after_first.max(1);
    let min_after = lim.before_last.max(1);
    if n < lim.min_word.max(2) || n < min_before + min_after || !c.iter().all(|c| c.is_alphabetic() || matches!(c, '\'' | '’')) {
        return vec![];
    }
    if c.iter().all(|c| c.is_uppercase()) || (!lim.capitalized && c[0].is_uppercase()) {
        return vec![];
    }
    let v: Vec<bool> = c.iter().map(|&x| is_vowel(x)).collect();
    let lower: String = word.to_lowercase();
    let mut out = vec![];
    for i in min_before..=n - min_after {
        if !c[i].is_alphabetic() || !c[i - 1].is_alphabetic() {
            continue;
        }
        let suffix_at = SUFFIXES.iter().any(|s| {
            let sl = s.chars().count();
            n - i == sl && lower.ends_with(s) && v[..i].iter().any(|x| *x)
        });
        let prefix_at = PREFIXES.iter().any(|p| p.chars().count() == i && lower.starts_with(p) && n - i >= 4 && c[i..].iter().any(|x| is_vowel(*x)));
        let ok = suffix_at
            || prefix_at
            || if v[i - 1] && !v[i] {
                (i + 1 < n && v[i + 1]) || (i + 2 < n && onset_pair(c[i], c[i + 1]) && v[i + 2])
            } else if !v[i - 1] && !v[i] {
                i >= 2 && v[i - 2] && i + 1 < n && v[i + 1] && !onset_pair(c[i - 1], c[i]) && c[i - 1] != c[i]
                    || (c[i - 1] == c[i] && i >= 2 && v[i - 2])
            } else {
                false
            };
        // Both parts need a sounded vowel: a trailing silent `e`, `ed` or `es` doesn't count
        // ("strai-ned" is wrong, "re-strained" is right).
        let right: String = c[i..].iter().collect::<String>().to_lowercase();
        let right_core = right.strip_suffix("ed").or_else(|| right.strip_suffix("es")).or_else(|| right.strip_suffix('e')).unwrap_or(&right);
        let sounded = right_core.chars().any(is_vowel) && right_core.len() > 1;
        let ok = ok && sounded && v[..i].iter().any(|x| *x);
        if ok && !out.contains(&i) {
            out.push(i);
        }
    }
    out.sort();
    out
}

/// `word` with `-` at every hyphenation point (tests and diagnostics).
pub fn hyphenate_word(word: &str, lim: &Limits) -> String {
    let pts = hyphen_points(word, lim);
    let mut s = String::with_capacity(word.len() + pts.len());
    for (i, ch) in word.chars().enumerate() {
        if pts.contains(&i) {
            s.push('-');
        }
        s.push(ch);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(w: &str) -> String {
        hyphenate_word(w, &Limits::default())
    }

    #[test]
    fn common_words() {
        assert_eq!(h("happen"), "hap-pen");
        assert_eq!(h("typography"), "ty-po-gra-phy");
        assert!(h("composition").contains("com-"));
        assert_eq!(h("cat"), "cat");
        assert!(!h("restrained").contains("-ned"), "{}", h("restrained"));
        assert!(!h("changed").contains('-'));
        assert!(h("restrained").starts_with("re-"), "{}", h("restrained"));
        assert_eq!(h("NASA"), "NASA");
    }

    #[test]
    fn limits_are_respected() {
        let lim = Limits { min_word: 5, after_first: 3, before_last: 3, capitalized: false };
        for w in ["typography", "hyphenation", "justification", "Paragraph", "beautiful"] {
            for p in hyphen_points(w, &lim) {
                assert!(p >= 3 && p <= w.chars().count() - 3, "{w} {p}");
            }
        }
        assert!(hyphen_points("Paragraph", &lim).is_empty());
    }
}
