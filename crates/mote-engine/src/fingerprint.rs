//! Finding fingerprints: stable identities that survive line shifts and file renames.
//!
//! `fingerprint = hex(xxh3_128(rule_id, symbol, anchor, occurrence))` where `anchor` is the
//! whitespace-collapsed text of the finding's start line (for duplicated blocks, a hash of the
//! duplicated token sequence) and `occurrence` counts earlier findings in the same file that share
//! the first three components. The path is deliberately *not* part of the fingerprint, so a
//! renamed file keeps its fingerprints. The short `id` does include the path.

use rustc_hash::FxHashMap;
use xxhash_rust::xxh3::{xxh3_128, xxh3_64, Xxh3};

/// Collapse all whitespace runs to single spaces and trim the ends.
pub fn normalize_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Longest fingerprint anchor kept verbatim, in bytes. Every finding carries the anchor of its
/// start line, so an unbounded anchor makes memory and time grow with (findings on a line) ×
/// (line length): a generated one-line `switch` of 20 000 cases took gigabytes.
pub const MAX_ANCHOR_BYTES: usize = 1024;

/// The fingerprint anchor of a line: its [`normalize_line`] text, or for a longer line than
/// [`MAX_ANCHOR_BYTES`] its first bytes (cut at a character boundary) followed by `…` and a hash of
/// the whole normalized line.
pub fn anchor_text(line_text: &str) -> String {
    let norm = normalize_line(line_text);
    if norm.len() <= MAX_ANCHOR_BYTES {
        return norm;
    }
    let mut cut = MAX_ANCHOR_BYTES;
    while !norm.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…{:016x}", &norm[..cut], xxh3_64(norm.as_bytes()))
}

/// Hash of a line's whitespace-normalized text (see [`normalize_line`]): identifies the code a
/// finding sits on, independently of its line number (new-code matching).
pub fn anchor_hash(line_text: &str) -> u64 {
    xxh3_64(normalize_line(line_text).as_bytes())
}

/// Hash of a line together with the lines just before and after it, each whitespace-normalized
/// (`""` for a line outside the file): identifies code that moved, for new-code matching.
pub fn context_hash(before: &str, line: &str, after: &str) -> u64 {
    let mut buf = normalize_line(before);
    buf.push('\n');
    buf.push_str(&normalize_line(line));
    buf.push('\n');
    buf.push_str(&normalize_line(after));
    xxh3_64(buf.as_bytes())
}

/// Compute a fingerprint (32 lowercase hex characters).
pub fn fingerprint(rule_id: &str, symbol: Option<&str>, anchor: &str, occurrence: u32) -> String {
    let mut buf = Vec::with_capacity(rule_id.len() + anchor.len() + 32);
    buf.extend_from_slice(rule_id.as_bytes());
    buf.push(0x1F);
    if let Some(s) = symbol {
        buf.push(1);
        buf.extend_from_slice(s.as_bytes());
    } else {
        buf.push(0);
    }
    buf.push(0x1F);
    buf.extend_from_slice(anchor.as_bytes());
    buf.push(0x1F);
    buf.extend_from_slice(&occurrence.to_le_bytes());
    format!("{:032x}", xxh3_128(&buf))
}

/// Short finding id: the first 12 hex characters of `xxh3(path, fingerprint)`.
pub fn finding_id(path: &str, fingerprint: &str) -> String {
    let mut buf = Vec::with_capacity(path.len() + fingerprint.len() + 1);
    buf.extend_from_slice(path.as_bytes());
    buf.push(0);
    buf.extend_from_slice(fingerprint.as_bytes());
    let h = format!("{:016x}", xxh3_64(&buf));
    h[..12].to_string()
}

/// Assigns occurrence indices for fingerprints within one file. Feed findings in their final
/// deterministic order.
#[derive(Default)]
pub struct OccurrenceCounter {
    /// Occurrences so far, by the 128-bit hash of `(rule id, symbol, anchor)`: the hash state
    /// [`fingerprint`] reaches before the occurrence, so no copy of the components is made per
    /// finding (a line with hundreds of thousands of findings made gigabytes of copies).
    seen: FxHashMap<u128, u32>,
}

impl OccurrenceCounter {
    /// Returns the fingerprint for the next finding with these components (the same as
    /// [`fingerprint`] with the occurrence counted so far).
    pub fn next(&mut self, rule_id: &str, symbol: Option<&str>, anchor: &str) -> String {
        let mut h = Xxh3::new();
        h.update(rule_id.as_bytes());
        h.update(&[0x1F]);
        if let Some(s) = symbol {
            h.update(&[1]);
            h.update(s.as_bytes());
        } else {
            h.update(&[0]);
        }
        h.update(&[0x1F]);
        h.update(anchor.as_bytes());
        h.update(&[0x1F]);
        let n = self.seen.entry(h.digest128()).or_insert(0);
        h.update(&n.to_le_bytes());
        *n += 1;
        format!("{:032x}", h.digest128())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_whitespace() {
        assert_eq!(normalize_line("  int   x =\t1;  \r"), "int x = 1;");
        assert_eq!(normalize_line(""), "");
        assert_eq!(anchor_hash("  a  =  b; "), anchor_hash("a = b;"));
        assert_ne!(anchor_hash("a = b;"), anchor_hash("a = c;"));
        assert_eq!(
            context_hash("  try {", "x();", "} catch (E e) {"),
            context_hash("try {", "    x();", "}  catch (E e) {")
        );
        assert_ne!(
            context_hash("a();", "x();", "b();"),
            context_hash("c();", "x();", "b();")
        );
        // The separators keep line boundaries: lines are not concatenated.
        assert_ne!(context_hash("a", "b", ""), context_hash("", "a", "b"));
    }

    #[test]
    fn fingerprint_is_stable_and_sensitive() {
        let a = fingerprint("java/x", Some("A#b"), "int x = 1;", 0);
        assert_eq!(a.len(), 32);
        assert_eq!(a, fingerprint("java/x", Some("A#b"), "int x = 1;", 0));
        assert_ne!(a, fingerprint("java/x", Some("A#b"), "int x = 1;", 1));
        assert_ne!(a, fingerprint("java/x", None, "int x = 1;", 0));
        assert_ne!(a, fingerprint("java/y", Some("A#b"), "int x = 1;", 0));
        // symbol "" and no symbol differ
        assert_ne!(
            fingerprint("r", Some(""), "t", 0),
            fingerprint("r", None, "t", 0)
        );
    }

    #[test]
    fn line_shift_keeps_fingerprint_rename_keeps_fingerprint_not_id() {
        // Same line text at a different line number → same fingerprint (line isn't an input).
        let text_before = "\n\nfoo();\n";
        let text_after = "\n\n\n\n\nfoo();\n";
        let l1 = normalize_line(text_before.lines().nth(2).unwrap());
        let l2 = normalize_line(text_after.lines().nth(5).unwrap());
        let f1 = fingerprint("js/r", None, &l1, 0);
        let f2 = fingerprint("js/r", None, &l2, 0);
        assert_eq!(f1, f2);
        assert_ne!(finding_id("a.js", &f1), finding_id("b.js", &f1));
        assert_eq!(finding_id("a.js", &f1).len(), 12);
    }

    #[test]
    fn long_anchors_are_capped() {
        assert_eq!(anchor_text("  a  =  b; "), "a = b;");
        let line = "é".repeat(MAX_ANCHOR_BYTES); // 2 bytes per character
        let a = anchor_text(&line);
        assert!(a.len() < MAX_ANCHOR_BYTES + 32, "{}", a.len());
        assert!(a.starts_with(&"é".repeat(MAX_ANCHOR_BYTES / 2)));
        // The hash keeps lines that differ after the cut apart.
        let other = format!("{}x", &line[..line.len() - 2]);
        assert_ne!(a, anchor_text(&other));
        assert_eq!(a, anchor_text(&format!("  {line} ")));
        let exact = "x".repeat(MAX_ANCHOR_BYTES);
        assert_eq!(anchor_text(&exact), exact);
    }

    /// The counter's streamed hash is [`fingerprint`] exactly, for every shape of component.
    #[test]
    fn counter_fingerprints_equal_the_one_shot_hash() {
        let mut c = OccurrenceCounter::default();
        for (rule, symbol, anchor) in [
            ("java/x", Some("A#m"), "int x = 1;"),
            ("java/x", None, "int x = 1;"),
            ("java/x", Some(""), ""),
            ("js/y", Some("f"), &*"é".repeat(600)),
            ("java/x", Some("A#m"), "int x = 1;"),
        ] {
            let got = c.next(rule, symbol, anchor);
            let n = (0..4)
                .find(|&n| fingerprint(rule, symbol, anchor, n) == got)
                .expect("a fingerprint of the same components");
            assert_eq!(got, fingerprint(rule, symbol, anchor, n));
        }
        // Occurrences count per components.
        let mut c = OccurrenceCounter::default();
        assert_eq!(c.next("r", None, "x"), fingerprint("r", None, "x", 0));
        assert_eq!(
            c.next("r", Some("s"), "x"),
            fingerprint("r", Some("s"), "x", 0)
        );
        assert_eq!(c.next("r", None, "x"), fingerprint("r", None, "x", 1));
    }

    #[test]
    fn occurrence_counter() {
        let mut c = OccurrenceCounter::default();
        let a = c.next("r", None, "x");
        let b = c.next("r", None, "x");
        let d = c.next("r", None, "y");
        assert_ne!(a, b);
        assert_eq!(a, fingerprint("r", None, "x", 0));
        assert_eq!(b, fingerprint("r", None, "x", 1));
        assert_eq!(d, fingerprint("r", None, "y", 0));
    }
}
