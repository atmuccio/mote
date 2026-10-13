//! Fixes: the text edits a rule can attach to a finding, and how to apply them.
//!
//! A [`Fix`] is one or more [`Edit`]s — a byte range of the analyzed text and its replacement —
//! that remove the finding without changing what the program does. Rules attach one only where
//! the rewrite is mechanical and safe by construction; `mote fix` applies them, and the result
//! formats (JSON, SARIF, MCP) carry them for other tools.
//!
//! Offsets are byte offsets into the text the plugin analyzed: the file's UTF-8 content after a
//! byte-order mark, which is not part of the text.

use crate::lines::LineIndex;
use crate::model::TextRange;
use serde::{Deserialize, Serialize};

/// Passes `mote fix` makes at most over one file: each applies every fix that does not conflict
/// with another, then the file is analyzed again for the fixes left.
pub const MAX_PASSES: usize = 10;

/// A mechanical, behaviour-preserving rewrite that removes a finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Fix {
    /// What applying the fix does, e.g. "Remove the unused import".
    pub description: String,
    /// The edits, sorted by position and not overlapping. Applied together or not at all.
    pub edits: Vec<Edit>,
}

/// Replace the text between two byte offsets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Edit {
    /// Byte offset where the replaced text starts (in the UTF-8 text, after a byte-order mark).
    pub start: u32,
    /// Byte offset one past the replaced text (`start` for an insertion).
    pub end: u32,
    /// The replaced text as lines and columns, like a finding's range (columns count Unicode
    /// scalar values; an insertion has an empty range). Filled in by the engine.
    #[serde(default)]
    pub range: TextRange,
    /// The new text.
    pub replacement: String,
}

impl Edit {
    /// Replace `[start, end)` with `replacement`.
    pub fn replace(start: usize, end: usize, replacement: impl Into<String>) -> Edit {
        Edit {
            start: start as u32,
            end: end.max(start) as u32,
            range: TextRange::default(),
            replacement: replacement.into(),
        }
    }

    /// Delete `[start, end)`.
    pub fn delete(start: usize, end: usize) -> Edit {
        Edit::replace(start, end, "")
    }

    /// Insert `text` at `at`.
    pub fn insert(at: usize, text: impl Into<String>) -> Edit {
        Edit::replace(at, at, text)
    }

    fn is_insertion(&self) -> bool {
        self.start == self.end
    }

    /// Whether `self` and `other` cannot both be applied: they overlap, or they insert at the
    /// same place (or one inserts where the other replaces), where the order would be ambiguous.
    pub fn conflicts_with(&self, other: &Edit) -> bool {
        if self.start < other.end && other.start < self.end {
            return true;
        }
        (self.is_insertion() || other.is_insertion()) && self.start == other.start
    }
}

impl Fix {
    /// A fix made of `edits` (sorted here).
    pub fn new(description: impl Into<String>, mut edits: Vec<Edit>) -> Fix {
        edits.sort_by_key(|e| (e.start, e.end));
        Fix {
            description: description.into(),
            edits,
        }
    }

    /// Whether the fix can be applied to `text`: at least one edit, every offset inside the text
    /// and on a character boundary, edits sorted and not in conflict with each other, and the
    /// result differs from `text`.
    pub fn is_valid_for(&self, text: &str) -> bool {
        if self.edits.is_empty() {
            return false;
        }
        let len = text.len() as u32;
        let mut changes = false;
        for (i, e) in self.edits.iter().enumerate() {
            if e.start > e.end
                || e.end > len
                || !text.is_char_boundary(e.start as usize)
                || !text.is_char_boundary(e.end as usize)
            {
                return false;
            }
            if let Some(prev) = i.checked_sub(1).map(|p| &self.edits[p]) {
                if prev.start > e.start || prev.conflicts_with(e) {
                    return false;
                }
            }
            changes |= text[e.start as usize..e.end as usize] != *e.replacement;
        }
        changes
    }

    /// Fill in every edit's line/column [`Edit::range`] from `index`.
    pub fn locate(&mut self, index: &LineIndex<'_>) {
        for e in &mut self.edits {
            e.range = index.range(e.start, e.end);
        }
    }

    /// Whether any edit of `self` conflicts with an edit of `other`.
    pub fn conflicts_with(&self, other: &Fix) -> bool {
        self.edits
            .iter()
            .any(|a| other.edits.iter().any(|b| a.conflicts_with(b)))
    }
}

/// Outcome of [`apply`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// The text with the accepted fixes applied.
    pub text: String,
    /// Indices (into the fixes given) of the fixes applied, ascending.
    pub applied: Vec<usize>,
    /// Indices of the fixes left out because an edit conflicted with an applied fix, ascending.
    pub conflicts: Vec<usize>,
    /// Indices of the fixes left out because they are not valid for the text, ascending.
    pub invalid: Vec<usize>,
}

/// Apply as many of `fixes` to `text` as can be applied together: in order of their first edit
/// (ties in the order given), a fix whose edits conflict with an already accepted edit is left out
/// for a later pass, invalid fixes are left out. Deterministic.
pub fn apply(text: &str, fixes: &[&Fix]) -> Applied {
    let mut order: Vec<usize> = (0..fixes.len()).collect();
    order.sort_by_key(|&i| (fixes[i].edits.first().map(|e| (e.start, e.end)), i));
    let mut accepted: Vec<&Edit> = Vec::new();
    let (mut applied, mut conflicts, mut invalid) = (Vec::new(), Vec::new(), Vec::new());
    for i in order {
        let fix = fixes[i];
        if !fix.is_valid_for(text) {
            invalid.push(i);
            continue;
        }
        let clash = fix.edits.iter().any(|e| {
            // `accepted` is sorted by start: only neighbours of `e`'s position can conflict.
            let at = accepted.partition_point(|a| a.end < e.start);
            accepted[at..]
                .iter()
                .take_while(|a| a.start <= e.end)
                .any(|a| a.conflicts_with(e))
        });
        if clash {
            conflicts.push(i);
            continue;
        }
        for e in &fix.edits {
            let at = accepted.partition_point(|a| (a.start, a.end) <= (e.start, e.end));
            accepted.insert(at, e);
        }
        applied.push(i);
    }
    let mut out = String::with_capacity(text.len());
    let mut pos = 0usize;
    for e in &accepted {
        out.push_str(&text[pos..e.start as usize]);
        out.push_str(&e.replacement);
        pos = e.end as usize;
    }
    out.push_str(&text[pos..]);
    applied.sort_unstable();
    conflicts.sort_unstable();
    invalid.sort_unstable();
    Applied {
        text: out,
        applied,
        conflicts,
        invalid,
    }
}

/// The line terminator of the line holding byte `offset` of `text` (`"\r\n"` or `"\n"`), or of
/// the file's first line when that line has none: what an inserted line break should use.
pub fn line_break_at(text: &str, offset: usize) -> &'static str {
    let offset = offset.min(text.len());
    let end = text[offset..]
        .find('\n')
        .map(|i| offset + i)
        .or_else(|| text.find('\n'));
    match end {
        Some(i) if i > 0 && text.as_bytes()[i - 1] == b'\r' => "\r\n",
        _ => "\n",
    }
}

/// Byte range of the whole line holding `[start, end)` — from the start of the line of `start`
/// to past the line terminator of the line of `end` — when nothing but whitespace surrounds the
/// span on those lines; else `None`. For deleting a statement or declaration together with its
/// line.
pub fn whole_line(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    if !text[line_start..start]
        .bytes()
        .all(|b| b == b' ' || b == b'\t')
    {
        return None;
    }
    let rest = &text[end..];
    let eol = rest.find('\n').map_or(text.len(), |i| end + i + 1);
    let tail = &text[end..eol];
    if !tail
        .bytes()
        .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
    {
        return None;
    }
    // A file's last line without a terminator: take the preceding line break instead, so that no
    // empty line is left at the end.
    if eol == text.len() && !text.ends_with('\n') && line_start > 0 {
        let prev = if line_start >= 2 && bytes[line_start - 2] == b'\r' {
            line_start - 2
        } else {
            line_start - 1
        };
        return Some((prev, eol));
    }
    Some((line_start, eol))
}

/// The deletion of `[start, end)` with the blanks that would be left over: its whole line(s)
/// when nothing else is on them (see [`whole_line`]); the blanks on both sides when only the line
/// end follows (`foo(); ;` → `foo();`); the blanks after it when it starts its line or follows a
/// blank (`a; b; c;` → `a; c;`); else the span alone, so that `x;; // note` keeps the blank
/// before its comment.
pub fn delete_span(text: &str, start: usize, end: usize) -> Edit {
    if let Some((s, e)) = whole_line(text, start, end) {
        return Edit::delete(s, e);
    }
    let blank = |b: &u8| *b == b' ' || *b == b'\t';
    let after = text[end..].bytes().take_while(blank).count();
    let before = text[..start].bytes().rev().take_while(blank).count();
    let line_end = matches!(text.as_bytes().get(end + after), None | Some(b'\r' | b'\n'));
    if line_end {
        return Edit::delete(start - before, end + after);
    }
    let starts_line = text[..start - before]
        .bytes()
        .next_back()
        .is_none_or(|b| b == b'\n');
    if before > 0 || starts_line {
        return Edit::delete(start, end + after);
    }
    Edit::delete(start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(edits: Vec<Edit>) -> Fix {
        Fix::new("test", edits)
    }

    #[test]
    fn applies_non_conflicting_fixes_in_order() {
        let text = "abcdef";
        let a = fix(vec![Edit::replace(0, 1, "A")]);
        let b = fix(vec![Edit::replace(0, 2, "XY")]); // overlaps a
        let c = fix(vec![Edit::insert(3, "-"), Edit::delete(5, 6)]);
        let d = fix(vec![Edit::insert(3, "+")]); // same insertion point as c
        let e = fix(vec![Edit::replace(1, 2, "B")]); // touches a: fine
        let r = apply(text, &[&a, &b, &c, &d, &e]);
        assert_eq!(r.text, "ABc-de");
        assert_eq!(r.applied, vec![0, 2, 4]);
        assert_eq!(r.conflicts, vec![1, 3]);
        assert!(r.invalid.is_empty());
    }

    #[test]
    fn invalid_fixes_are_left_out() {
        let text = "héllo";
        let past_end = fix(vec![Edit::delete(3, 99)]);
        let mid_char = fix(vec![Edit::delete(2, 3)]);
        let no_op = fix(vec![Edit::replace(0, 1, "h")]);
        let empty = Fix::new("nothing", vec![]);
        let overlapping = Fix {
            description: "x".into(),
            edits: vec![Edit::delete(0, 3), Edit::delete(1, 2)],
        };
        let r = apply(text, &[&past_end, &mid_char, &no_op, &empty, &overlapping]);
        assert_eq!(r.text, text);
        assert_eq!(r.invalid, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn line_breaks_and_whole_lines() {
        assert_eq!(line_break_at("a\r\nb\r\n", 0), "\r\n");
        assert_eq!(line_break_at("a\nb", 2), "\n");
        assert_eq!(line_break_at("ab\r\nlast", 5), "\r\n");
        let t = "x;\n  import a;\ny;\n";
        let s = t.find("import").unwrap();
        assert_eq!(whole_line(t, s, s + 9), Some((3, 15)));
        assert_eq!(whole_line("a; b;\n", 3, 5), None);
        // Last line without a terminator: the line break before it goes.
        let t = "x;\nimport a;";
        assert_eq!(whole_line(t, 3, t.len()), Some((2, t.len())));
        let t = "x;\r\nimport a;";
        assert_eq!(whole_line(t, 4, t.len()), Some((2, t.len())));
        // Not alone on the line.
        let del = |t: &str, s: usize, e: usize| {
            let d = delete_span(t, s, e);
            format!("{}{}", &t[..d.start as usize], &t[d.end as usize..])
        };
        assert_eq!(del("a; b; c;\n", 3, 5), "a; c;\n");
        assert_eq!(del("a; b;\n", 3, 5), "a;\n");
        assert_eq!(del("int x;; // c\n", 6, 7), "int x; // c\n");
        assert_eq!(del("  import a; // c\n", 2, 11), "  // c\n");
        assert_eq!(del("{ return; }", 2, 9), "{ }");
        assert_eq!(del("foo();;", 6, 7), "foo();");
    }
}
