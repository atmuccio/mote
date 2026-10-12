//! The per-file map step: decode → skip checks → plugin → lexer → generic rules → suppressions →
//! fingerprints. The output ([`FileOutcome`]) is what the cache stores.

use crate::config::{Config, ResolvedRules};
use crate::fingerprint::{anchor_hash, anchor_text, context_hash, finding_id, OccurrenceCounter};
use crate::generic::{self, GenericInput, MetaRule};
use crate::lexer::{self, LexOptions};
use crate::measures::FileMeasures;
use crate::registry::Registry;
use crate::suppress::{self, Suppression};
use mote_core::{
    FileAnalysis, Finding, FindingStatus, LineIndex, Quality, RawFinding, RuleSet, RuleStatus,
    SourceFile, SuppressionInfo, TextRange,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Why a file was not analyzed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SkipReason {
    /// Matched an `exclude` glob.
    Excluded,
    /// Larger than `max_file_size`.
    TooLarge,
    /// Carries a generated-code marker in its header.
    Generated,
    /// Looks minified.
    Minified,
    /// Contains NUL bytes.
    Binary,
    /// A vendored library or bundler output (JavaScript; see [`crate::vendored`]).
    Vendored,
}

impl SkipReason {
    /// Kebab-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::Excluded => "excluded",
            SkipReason::TooLarge => "too-large",
            SkipReason::Generated => "generated",
            SkipReason::Minified => "minified",
            SkipReason::Binary => "binary",
            SkipReason::Vendored => "vendored",
        }
    }
}

/// Settings (besides the rule set) that influence per-file output. Part of the cache key.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisSettings {
    pub ignore_literals: bool,
    pub require_reason: bool,
    /// Compatibility suppression sources honored (`suppressions.compat`).
    pub compat: crate::suppress::CompatSet,
    pub skip_generated: bool,
    pub max_file_size: u64,
    /// `cpd.min_tokens` (fingerprints are computed in the map phase).
    pub cpd_min_tokens: u32,
}

impl AnalysisSettings {
    /// Extract from a config.
    pub fn from_config(c: &Config) -> Self {
        AnalysisSettings {
            ignore_literals: c.cpd.ignore_literals,
            require_reason: c.suppressions.require_reason,
            compat: c.suppressions.compat.sources(),
            skip_generated: c.skip_generated,
            max_file_size: c.max_file_size,
            cpd_min_tokens: c.cpd.min_tokens,
        }
    }
}

/// Duplication tokens of a file: token hashes, and for every line with tokens the line number
/// and the index of its first token (lines are only needed for the few tokens that start or end
/// a duplicate, so they are looked up by binary search instead of stored per token).
///
/// Serialized compactly for the cache (see [`CpdTokens::encode`]): a per-file dictionary of the
/// distinct hashes, most frequent first, and one LEB128 index per token (typically one byte),
/// plus `(line delta, token count)` pairs per line — about 1.5 bytes per token instead of ~6.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CpdTokens {
    pub hashes: Vec<u32>,
    /// `(line, first token index)` per line that has tokens, both strictly increasing; the first
    /// run starts at token 0.
    pub runs: Vec<(u32, u32)>,
    /// Winnowed k-gram fingerprints `(hash, token position)` for `min_tokens` =
    /// [`fp_min_tokens`](Self::fp_min_tokens), computed in the map phase so warm scans skip
    /// winnowing (see [`cpd::fingerprints`](crate::cpd::fingerprints)).
    pub fingerprints: Vec<(u64, u32)>,
    /// `min_tokens` the fingerprints were computed for (0 = not computed).
    pub fp_min_tokens: u32,
    /// Exclusive end column (characters + 1) of each line in [`runs`](Self::runs), so duplicated
    /// blocks can end at the end of their last line. Empty when unknown.
    pub line_ends: Vec<u32>,
}

fn put_varint(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(bytes: &[u8], pos: &mut usize) -> Option<u32> {
    let mut v: u32 = 0;
    for shift in (0..35).step_by(7) {
        let b = *bytes.get(*pos)?;
        *pos += 1;
        v |= u32::from(b & 0x7f).checked_shl(shift)?;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

impl CpdTokens {
    /// From token hashes and the (non-decreasing) line of each token.
    pub fn new(hashes: Vec<u32>, lines: &[u32]) -> Self {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for (i, &l) in lines.iter().enumerate().take(hashes.len()) {
            if runs.last().is_none_or(|r| l > r.0) {
                runs.push((l, i as u32));
            }
        }
        if runs.is_empty() && !hashes.is_empty() {
            runs.push((1, 0));
        }
        CpdTokens {
            hashes,
            runs,
            ..Default::default()
        }
    }

    /// Record the exclusive end column of every line that has tokens.
    pub fn with_line_ends(mut self, end_col: impl Fn(u32) -> u32) -> Self {
        self.line_ends = self.runs.iter().map(|&(line, _)| end_col(line)).collect();
        self
    }

    /// Exclusive end column of `line` (a line with tokens), when known.
    pub fn line_end(&self, line: u32) -> Option<u32> {
        let k = self.runs.binary_search_by_key(&line, |r| r.0).ok()?;
        self.line_ends.get(k).copied()
    }

    /// Compute the fingerprints for `min_tokens`.
    pub fn with_fingerprints(mut self, min_tokens: u32) -> Self {
        self.fingerprints = crate::cpd::fingerprints(&self.hashes, min_tokens);
        self.fp_min_tokens = min_tokens;
        self
    }

    /// Line of token `i` (0 when out of range).
    pub fn line_of(&self, i: u32) -> u32 {
        line_in_runs(&self.runs, i)
    }

    /// The line of every token.
    pub fn lines(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.hashes.len());
        for (k, &(line, start)) in self.runs.iter().enumerate() {
            let end = self
                .runs
                .get(k + 1)
                .map_or(self.hashes.len() as u32, |r| r.1);
            out.extend((start..end).map(|_| line));
        }
        out
    }

    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }

    /// Append the compact encoding to `out`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let n = self.hashes.len() as u32;
        put_varint(out, n);
        if n == 0 {
            return;
        }
        // Dictionary, most frequent first (ties by value, for a deterministic encoding).
        let mut freq: rustc_hash::FxHashMap<u32, u32> = rustc_hash::FxHashMap::default();
        for &h in &self.hashes {
            *freq.entry(h).or_default() += 1;
        }
        let mut dict: Vec<(u32, u32)> = freq.into_iter().collect();
        dict.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        put_varint(out, dict.len() as u32);
        let mut index: rustc_hash::FxHashMap<u32, u32> = rustc_hash::FxHashMap::default();
        index.reserve(dict.len());
        for (i, &(h, _)) in dict.iter().enumerate() {
            out.extend_from_slice(&h.to_le_bytes());
            index.insert(h, i as u32);
        }
        for h in &self.hashes {
            put_varint(out, index[h]);
        }
        put_varint(out, self.runs.len() as u32);
        let mut prev_line = 0;
        for (k, &(line, start)) in self.runs.iter().enumerate() {
            let end = self.runs.get(k + 1).map_or(n, |r| r.1);
            put_varint(out, line.saturating_sub(prev_line));
            put_varint(out, end - start);
            prev_line = line;
        }
        put_varint(out, self.line_ends.len() as u32);
        for &e in &self.line_ends {
            put_varint(out, e);
        }
        put_varint(out, self.fp_min_tokens);
        put_varint(out, self.fingerprints.len() as u32);
        let mut prev_pos = 0;
        for &(h, p) in &self.fingerprints {
            out.extend_from_slice(&h.to_le_bytes());
            put_varint(out, p.saturating_sub(prev_pos));
            prev_pos = p;
        }
    }

    /// Decode [`encode`](Self::encode)d bytes; `None` if they are malformed.
    pub fn decode(bytes: &[u8]) -> Option<CpdTokens> {
        let mut pos = 0;
        let n = get_varint(bytes, &mut pos)? as usize;
        if n == 0 {
            return (pos == bytes.len()).then(CpdTokens::default);
        }
        let u = get_varint(bytes, &mut pos)? as usize;
        let table_end = pos.checked_add(u.checked_mul(4)?)?;
        // Every token takes at least one byte: reject lengths the input cannot hold.
        if u == 0 || table_end > bytes.len() || n > bytes.len() - table_end {
            return None;
        }
        let dict: Vec<u32> = bytes[pos..table_end]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        pos = table_end;
        let mut hashes = Vec::with_capacity(n);
        for _ in 0..n {
            // Fast path: one-byte indices.
            let b = *bytes.get(pos)?;
            let i = if b < 0x80 {
                pos += 1;
                u32::from(b)
            } else {
                get_varint(bytes, &mut pos)?
            };
            hashes.push(*dict.get(i as usize)?);
        }
        let r = get_varint(bytes, &mut pos)? as usize;
        if r > n {
            return None;
        }
        let mut runs = Vec::with_capacity(r);
        let (mut line, mut start) = (0u32, 0u32);
        for k in 0..r {
            let delta = get_varint(bytes, &mut pos)?;
            let count = get_varint(bytes, &mut pos)?;
            if count == 0 || (k > 0 && delta == 0) {
                return None;
            }
            line = line.checked_add(delta)?;
            runs.push((line, start));
            start = start.checked_add(count)?;
        }
        if start as usize != n {
            return None;
        }
        let e = get_varint(bytes, &mut pos)? as usize;
        if e != 0 && e != r {
            return None;
        }
        let mut line_ends = Vec::with_capacity(e);
        for _ in 0..e {
            line_ends.push(get_varint(bytes, &mut pos)?);
        }
        let fp_min_tokens = get_varint(bytes, &mut pos)?;
        let f = get_varint(bytes, &mut pos)? as usize;
        if f > n || f.checked_mul(9)? > bytes.len() - pos {
            return None;
        }
        let mut fingerprints = Vec::with_capacity(f);
        let mut p = 0u32;
        for k in 0..f {
            let h = u64::from_le_bytes(bytes.get(pos..pos + 8)?.try_into().ok()?);
            pos += 8;
            let delta = get_varint(bytes, &mut pos)?;
            if k > 0 && delta == 0 {
                return None;
            }
            p = p.checked_add(delta)?;
            if p as usize >= n {
                return None;
            }
            fingerprints.push((h, p));
        }
        (pos == bytes.len()).then_some(CpdTokens {
            hashes,
            runs,
            fingerprints,
            fp_min_tokens,
            line_ends,
        })
    }
}

/// Line of token `i` given `(line, first token)` runs.
pub fn line_in_runs(runs: &[(u32, u32)], i: u32) -> u32 {
    let k = runs.partition_point(|r| r.1 <= i);
    if k == 0 {
        0
    } else {
        runs[k - 1].0
    }
}

impl Serialize for CpdTokens {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = Vec::with_capacity(self.hashes.len() * 2 + 16);
        self.encode(&mut out);
        s.serialize_bytes(&out)
    }
}

impl<'de> Deserialize<'de> for CpdTokens {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = CpdTokens;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("encoded duplication tokens")
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<CpdTokens, E> {
                CpdTokens::decode(v).ok_or_else(|| E::custom("malformed duplication tokens"))
            }
        }
        d.deserialize_bytes(V)
    }
}

/// Everything the map phase knows about an analyzed file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileResult {
    /// File-local findings: fingerprinted, suppression-applied, sorted.
    pub findings: Vec<Finding>,
    pub measures: FileMeasures,
    pub tokens: CpdTokens,
    /// Parsed suppressions (with `used` flags from the file-local findings).
    pub suppressions: Vec<Suppression>,
    /// The plugin or engine failed on this file; such results are not cached.
    pub internal_error: bool,
    /// Every line a finding or a suppression directive starts on, sorted by line, with hashes
    /// that identify its code for new-code matching.
    pub line_anchors: Vec<LineAnchor>,
}

/// Hashes of one line's code (see [`FileResult::line_anchors`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineAnchor {
    pub line: u32,
    /// The line's text ([`anchor_hash`]).
    pub text: u64,
    /// The line with its neighbors ([`context_hash`]): recognizes code that moved.
    pub context: u64,
}

impl FileResult {
    fn line_anchor(&self, line: u32) -> Option<&LineAnchor> {
        let k = self
            .line_anchors
            .binary_search_by_key(&line, |a| a.line)
            .ok()?;
        self.line_anchors.get(k)
    }

    /// Anchor hash of `line`, when it hosts a finding or a suppression directive.
    pub fn anchor_of(&self, line: u32) -> Option<u64> {
        self.line_anchor(line).map(|a| a.text)
    }

    /// Context hash of `line` (the line and its neighbors), when it hosts a finding or a
    /// suppression directive.
    pub fn context_of(&self, line: u32) -> Option<u64> {
        self.line_anchor(line).map(|a| a.context)
    }
}

/// Map-phase outcome for a file (the cached value).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FileOutcome {
    Analyzed(Box<FileResult>),
    Skipped(SkipReason),
}

/// Identifies the file being analyzed.
pub struct FileJob<'a> {
    pub registry: &'a Registry,
    pub plugin: usize,
    /// Path the plugin sees and findings carry (repo-relative, `/`).
    pub path: &'a str,
    pub is_test: bool,
    pub rules: &'a ResolvedRules,
    pub settings: &'a AnalysisSettings,
    /// Whether the file is skipped when it looks generated, minified or vendored
    /// (`skip_generated`, unless `skip_generated_except` matches the path).
    pub skip_generated: bool,
    /// The npm packages the file belongs to ([`crate::packages`]; JavaScript files the vendored
    /// check applies to, else empty): a library banner naming one of them is the project's own.
    pub packages: &'a [String],
    /// Keep duplication tokens (false when the file is excluded from duplication detection).
    pub keep_tokens: bool,
}

/// Decode file bytes: strips a UTF-8 BOM, decodes UTF-16 with a BOM, falls back to Latin-1 for
/// invalid UTF-8 (`lossy = true`). Files with NUL bytes in the first 8 KiB are binary.
pub fn decode_source(bytes: &[u8]) -> Result<(Cow<'_, str>, bool), SkipReason> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return decode_utf8(rest);
    }
    if bytes.len() >= 2 && (bytes[..2] == [0xFF, 0xFE] || bytes[..2] == [0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units = bytes[2..].chunks_exact(2).map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        });
        let s: String = char::decode_utf16(units)
            .map(|r| r.unwrap_or('\u{FFFD}'))
            .collect();
        if s.contains('\0') {
            return Err(SkipReason::Binary);
        }
        return Ok((Cow::Owned(s), false));
    }
    decode_utf8(bytes)
}

fn decode_utf8(bytes: &[u8]) -> Result<(Cow<'_, str>, bool), SkipReason> {
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err(SkipReason::Binary);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok((Cow::Borrowed(s), false)),
        Err(_) => Ok((Cow::Owned(bytes.iter().map(|&b| b as char).collect()), true)),
    }
}

const JS_LIKE: &[&str] = &["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];

/// Most lines of a file's leading comments inspected for a generated-code marker.
const HEADER_LINES: usize = 60;

/// The comment lines at the top of a file, before its first code token: blank lines and a `#!`
/// line are skipped; `//` and `/* … */` comments are collected (at most [`HEADER_LINES`] lines).
pub fn leading_comments(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_block = false;
    for (i, line) in text.lines().take(HEADER_LINES).enumerate() {
        let t = line.trim();
        if in_block {
            match t.find("*/") {
                None => out.push(t),
                Some(end) => {
                    out.push(&t[..end]);
                    in_block = false;
                    // Code after the comment ends the header.
                    if !t[end + 2..].trim().is_empty() {
                        break;
                    }
                }
            }
            continue;
        }
        if t.is_empty() || (i == 0 && t.starts_with("#!")) {
            continue;
        }
        if t.starts_with("//") {
            out.push(t);
        } else if let Some(rest) = t.strip_prefix("/*") {
            match rest.find("*/") {
                None => {
                    out.push(t);
                    in_block = true;
                }
                Some(end) => {
                    out.push(&t[..end + 2]);
                    if !rest[end + 2..].trim().is_empty() {
                        break;
                    }
                }
            }
        } else {
            break;
        }
    }
    out
}

/// Whether leading comment lines declare the file generated. Only explicit markers count:
/// `@generated`, Go's `Code generated … DO NOT EDIT`, "generated by … do not edit/modify", and
/// full sentences such as "This file was automatically generated". Loose words ("auto-generated
/// id", "do not edit" alone) do not.
pub fn is_generated_header(comment_lines: &[&str]) -> bool {
    let norm = normalized_comment(comment_lines);
    if has_word(&norm, "@generated") {
        return true;
    }
    if says_do_not_edit(&norm)
        && (norm.contains("code generated")
            || norm.contains("generated code")
            || norm.contains("generated by"))
    {
        return true;
    }
    says_file_is_generated(&norm)
}

/// Whether the doc comment of the first type declares the file generated. Stricter than the
/// header test, as such comments also describe what the class does: it takes a full sentence
/// ("This file is auto generated", "This class was generated") together with a do-not-edit
/// warning.
pub fn is_generated_type_doc(comment_lines: &[&str]) -> bool {
    if comment_lines.is_empty() {
        return false;
    }
    let norm = normalized_comment(comment_lines);
    says_do_not_edit(&norm) && says_file_is_generated(&norm)
}

/// Comment words in lower case, separated by one space, without comment punctuation.
fn normalized_comment(comment_lines: &[&str]) -> String {
    let mut norm = String::new();
    for w in comment_lines.iter().flat_map(|l| l.split_whitespace()) {
        let w = w.trim_start_matches(['/', '*', '!']);
        if w.is_empty() {
            continue;
        }
        if !norm.is_empty() {
            norm.push(' ');
        }
        let at = norm.len();
        norm.push_str(w);
        norm[at..].make_ascii_lowercase();
    }
    norm
}

/// `needle` occurs in `norm` and is not followed by more of a word.
fn has_word(norm: &str, needle: &str) -> bool {
    norm.match_indices(needle)
        .any(|(i, _)| ends_word(&norm[i + needle.len()..]))
}

/// `rest` (the text after a match) does not continue a word.
fn ends_word(rest: &str) -> bool {
    !rest
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn says_do_not_edit(norm: &str) -> bool {
    ["do not edit", "do not modify", "don't edit", "don't modify"]
        .iter()
        .any(|p| norm.contains(p))
}

/// "This file / code / class / source file is / was / has been (auto-)generated".
fn says_file_is_generated(norm: &str) -> bool {
    const SUBJECTS: &[&str] = &["this file", "this code", "this class", "this source file"];
    const VERBS: &[&str] = &["is", "was", "has been"];
    const HOW: &[&str] = &[
        "generated",
        "auto-generated",
        "auto generated",
        "autogenerated",
        "automatically generated",
        "machine-generated",
        "machine generated",
    ];
    // `has_word(norm, "{subject} {verb} {how}")` for any combination, without building the 84
    // phrases and searching for each (that was most of the time of a scan's generated-code
    // check): every phrase starts with "this " — which cannot overlap itself, so its matches are
    // all its occurrences — and is checked word by word where it occurs.
    norm.match_indices("this ").any(|(i, _)| {
        let rest = &norm[i..];
        SUBJECTS.iter().any(|s| {
            let Some(rest) = rest.strip_prefix(s).and_then(|r| r.strip_prefix(' ')) else {
                return false;
            };
            VERBS.iter().any(|v| {
                let Some(rest) = rest.strip_prefix(v).and_then(|r| r.strip_prefix(' ')) else {
                    return false;
                };
                HOW.iter()
                    .any(|h| rest.strip_prefix(h).is_some_and(ends_word))
            })
        })
    })
}

/// Most lines inspected for the doc comment of a file's first type declaration.
const TYPE_DOC_LINES: usize = 200;

/// The block comment right in front of the file's first type declaration, when only a
/// `package` line, imports, annotations, line comments and blank lines come before it
/// (`/** … */` then `@Tag public class X`); empty otherwise. Read from at most
/// [`TYPE_DOC_LINES`] non-blank lines; old Mac line ends (`\r` alone) count as line ends.
pub fn first_type_doc_comment(text: &str) -> Vec<&str> {
    let mut last: Vec<&str> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut in_block = false;
    let lines = text
        .split(['\n', '\r'])
        .filter(|l| !l.trim().is_empty())
        .take(TYPE_DOC_LINES);
    for (i, line) in lines.enumerate() {
        let mut t = line.trim();
        if in_block {
            match t.find("*/") {
                None => {
                    current.push(t);
                    continue;
                }
                Some(end) => {
                    current.push(&t[..end]);
                    in_block = false;
                    last = std::mem::take(&mut current);
                    t = t[end + 2..].trim();
                }
            }
        } else if let Some(rest) = t.strip_prefix("/*") {
            match rest.find("*/") {
                None => {
                    current = vec![t];
                    in_block = true;
                    continue;
                }
                Some(end) => {
                    last = vec![&t[..end + 4]];
                    t = rest[end + 2..].trim();
                }
            }
        }
        if t.is_empty() || t.starts_with("//") || (i == 0 && t.starts_with("#!")) {
            continue;
        }
        if t.starts_with("package ") || t.starts_with("import ") {
            last.clear();
            continue;
        }
        if declares_type(t) {
            return last;
        }
        if t.starts_with('@') {
            continue;
        }
        break;
    }
    Vec::new()
}

/// Whether a code line starts a class, interface, enum, record or annotation type
/// declaration (after modifiers and simple annotations).
fn declares_type(line: &str) -> bool {
    let mut rest = line.trim_start();
    // Leading annotations with their arguments: `@Tag`, `@SuppressWarnings({"a", "b"})`.
    while let Some(after_at) = rest.strip_prefix('@') {
        let name_end = after_at
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
            .unwrap_or(after_at.len());
        if &after_at[..name_end] == "interface" {
            return true;
        }
        let mut r = after_at[name_end..].trim_start();
        if r.starts_with('(') {
            let mut depth = 0usize;
            let mut close = None;
            for (i, c) in r.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else {
                return false;
            };
            r = r[close + 1..].trim_start();
        }
        rest = r;
    }
    for w in rest.split_whitespace() {
        match w {
            "class" | "interface" | "enum" | "record" => return true,
            "public" | "protected" | "private" | "abstract" | "final" | "static" | "sealed"
            | "non-sealed" | "strictfp" => {}
            _ => return false,
        }
    }
    false
}

/// Whether `path` is a file a tool writes and the repository only carries: Yarn Plug'n'Play's
/// install state and bundled tooling (`.yarn/**` — releases, plugins, editor SDKs, unplugged
/// packages — and the `.pnp.cjs` / `.pnp.js` / `.pnp.loader.mjs` loaders). Skipped as generated
/// with `skip_generated`, before the file is read.
pub fn is_generated_path(path: &str) -> bool {
    let mut parts = path.split('/');
    let name = parts.next_back().unwrap_or("");
    parts.any(|p| p == ".yarn") || matches!(name, ".pnp.cjs" | ".pnp.js" | ".pnp.loader.mjs")
}

/// A generated-code marker in the file's leading comments or in the doc comment of its first
/// type, minified content, or vendored / bundled JavaScript ([`crate::vendored`]), checked in
/// that order.
pub fn generated_or_minified(path: &str, text: &str) -> Option<SkipReason> {
    generated_or_minified_in(path, text, &LineIndex::new(text), &[])
}

/// [`generated_or_minified`] for a file of the npm packages `packages` (a library banner naming
/// one of them is not a vendored signal, see [`crate::vendored`]).
pub fn generated_or_minified_for(
    path: &str,
    text: &str,
    packages: &[String],
) -> Option<SkipReason> {
    generated_or_minified_in(path, text, &LineIndex::new(text), packages)
}

/// [`generated_or_minified_for`] with the text's line index (built once per file for this check
/// and for the analysis).
fn generated_or_minified_in(
    path: &str,
    text: &str,
    index: &LineIndex<'_>,
    packages: &[String],
) -> Option<SkipReason> {
    if is_generated_header(&leading_comments(text))
        || is_generated_type_doc(&first_type_doc_comment(text))
    {
        return Some(SkipReason::Generated);
    }
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    // The pieces of `text.split('\n')`: their number, and the longest (in bytes, a `\r` before
    // the newline included).
    let starts = index.line_starts();
    let lines = starts.len();
    let ends = starts
        .iter()
        .skip(1)
        .map(|&s| s as usize - 1)
        .chain([text.len()]);
    let longest = starts
        .iter()
        .zip(ends)
        .map(|(&start, end)| end - start as usize)
        .max()
        .unwrap_or(0);
    if JS_LIKE.contains(&ext.as_str()) && longest > 5000 {
        return Some(SkipReason::Minified);
    }
    if text.len() > 1024 && text.len() / lines.max(1) > 300 {
        return Some(SkipReason::Minified);
    }
    if crate::vendored::vendored_signal_for(path, text, packages).is_some() {
        return Some(SkipReason::Vendored);
    }
    None
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// Decode and analyze raw bytes (skip rules included).
pub fn analyze_bytes(job: &FileJob<'_>, bytes: &[u8]) -> FileOutcome {
    if bytes.len() as u64 > job.settings.max_file_size {
        return FileOutcome::Skipped(SkipReason::TooLarge);
    }
    let (text, _lossy) = match decode_source(bytes) {
        Ok(t) => t,
        Err(reason) => return FileOutcome::Skipped(reason),
    };
    let index = LineIndex::new(&text);
    if job.skip_generated {
        if let Some(reason) = generated_or_minified_in(job.path, &text, &index, job.packages) {
            return FileOutcome::Skipped(reason);
        }
    }
    FileOutcome::Analyzed(Box::new(analyze_indexed(job, &text, &index)))
}

/// Result for a file that could not be analyzed at all (e.g. unreadable).
pub fn internal_error_result(job: &FileJob<'_>, message: &str) -> FileResult {
    let mut findings = Vec::new();
    if job.rules.rules.is_enabled(MetaRule::InternalError.id()) {
        let raw = RawFinding {
            rule_id: MetaRule::InternalError.id().to_string(),
            message: message.to_string(),
            range: TextRange::line(1),
            symbol: None,
            value: None,
            secondary: Vec::new(),
        };
        findings = finalize(
            job.registry,
            &job.rules.rules,
            job.path,
            vec![Pending {
                raw,
                anchor: "".into(),
                suppressed: None,
            }],
        );
    }
    FileResult {
        findings,
        internal_error: true,
        ..Default::default()
    }
}

/// Analyze decoded text.
pub fn analyze_text(job: &FileJob<'_>, text: &str) -> FileResult {
    analyze_indexed(job, text, &LineIndex::new(text))
}

/// [`analyze_text`] with the text's line index.
fn analyze_indexed(job: &FileJob<'_>, text: &str, index: &LineIndex<'_>) -> FileResult {
    let plugin = job.registry.plugin(job.plugin);
    let src = SourceFile {
        path: job.path,
        text,
        is_test: job.is_test,
    };
    let (analysis, plugin_error) =
        match catch_unwind(AssertUnwindSafe(|| plugin.analyze(&src, &job.rules.rules))) {
            Ok(a) => (a, None),
            Err(p) => (FileAnalysis::default(), Some(panic_message(p.as_ref()))),
        };
    match catch_unwind(AssertUnwindSafe(|| {
        engine_pass(job, text, index, &analysis, plugin_error.as_deref())
    })) {
        Ok(r) => r,
        Err(p) => internal_error_result(
            job,
            &format!(
                "Mote could not analyze this file (engine failure: {})",
                panic_message(p.as_ref())
            ),
        ),
    }
}

/// A finding before fingerprinting.
pub(crate) struct Pending {
    pub raw: RawFinding,
    /// Fingerprint anchor: shared by the findings of one line (a line can carry hundreds of
    /// thousands of findings, so a copy each was gigabytes).
    pub anchor: std::sync::Arc<str>,
    pub suppressed: Option<SuppressionInfo>,
}

fn engine_pass(
    job: &FileJob<'_>,
    text: &str,
    index: &LineIndex<'_>,
    analysis: &FileAnalysis,
    plugin_error: Option<&str>,
) -> FileResult {
    let registry = job.registry;
    let plugin = registry.plugin(job.plugin);
    let rules = &job.rules.rules;
    let spec = plugin.lexer();
    let lex = lexer::lex(
        text,
        &spec,
        LexOptions {
            ignore_literals: job.settings.ignore_literals,
        },
    );
    let comments = if analysis.comments.is_empty() {
        &lex.comments
    } else {
        &analysis.comments
    };

    let mut raw: Vec<RawFinding> = analysis
        .findings
        .iter()
        .filter(|f| rules.is_enabled(&f.rule_id))
        .cloned()
        .collect();
    raw.extend(generic::evaluate(&GenericInput {
        set: registry.generic_set(job.plugin),
        overridden: registry.overridden_generics(job.plugin),
        rules,
        analysis,
        comments,
        lex: &lex,
        spec: &spec,
    }));

    let mut sups = suppress::collect(
        comments,
        &analysis.suppressions,
        plugin.id(),
        job.settings.compat,
        &lex,
        index,
    );
    let line_count = index.line_count().max(1);
    let sup_index = suppress::SuppressionIndex::new(&sups);
    // Anchors per line: computed once, however many findings share a line.
    let mut anchors: rustc_hash::FxHashMap<u32, std::sync::Arc<str>> = Default::default();
    let mut anchor_of = |line: u32| {
        anchors
            .entry(line)
            .or_insert_with(|| anchor_text(index.line_text(line)).into())
            .clone()
    };
    let mut pending: Vec<Pending> = raw
        .into_iter()
        .map(|mut f| {
            sanitize_range(&mut f.range, line_count);
            let suppressed = sup_index.suppress(&f.rule_id, f.range.start_line, &mut sups);
            let anchor = anchor_of(f.range.start_line);
            Pending {
                raw: f,
                anchor,
                suppressed,
            }
        })
        .collect();

    let meta = |rule: MetaRule, message: String| Pending {
        raw: RawFinding {
            rule_id: rule.id().to_string(),
            message,
            range: TextRange::line(1),
            symbol: None,
            value: None,
            secondary: Vec::new(),
        },
        anchor: "".into(),
        suppressed: None,
    };
    if analysis.syntax_errors > 0 && rules.is_enabled(MetaRule::ParseError.id()) {
        let n = analysis.syntax_errors;
        let s = if n == 1 { "" } else { "s" };
        pending.push(meta(
            MetaRule::ParseError,
            format!("Fix the {n} syntax error{s} in this file; results for it may be incomplete"),
        ));
    }
    if let Some(msg) = plugin_error {
        if rules.is_enabled(MetaRule::InternalError.id()) {
            pending.push(meta(
                MetaRule::InternalError,
                format!(
                    "The {} analyzer failed on this file: {msg}",
                    plugin.display_name()
                ),
            ));
        }
    }
    for f in suppress::directive_findings(&sups, rules, job.settings.require_reason) {
        let anchor = anchor_of(f.range.start_line);
        pending.push(Pending {
            raw: f,
            anchor,
            suppressed: None,
        });
    }

    let findings = finalize(registry, rules, job.path, pending);
    let mut anchor_lines: Vec<u32> = findings
        .iter()
        .map(|f| f.range.start_line)
        .chain(sups.iter().filter(|s| s.checks_unused()).map(|s| s.line))
        .collect();
    anchor_lines.sort_unstable();
    anchor_lines.dedup();
    let line_anchors = anchor_lines
        .into_iter()
        .map(|l| LineAnchor {
            line: l,
            text: anchor_hash(index.line_text(l)),
            context: context_hash(
                index.line_text(l.saturating_sub(1)),
                index.line_text(l),
                index.line_text(l.saturating_add(1)),
            ),
        })
        .collect();
    let measures = FileMeasures {
        lines: lex.lines,
        ncloc: lex.ncloc(),
        comment_lines: lex.comment_lines,
        functions: analysis.functions.len() as u32,
        classes: analysis.classes,
        statements: analysis.statements,
        cognitive_complexity: analysis.functions.iter().map(|f| f.cognitive).sum(),
        cyclomatic_complexity: analysis.functions.iter().map(|f| f.cyclomatic).sum(),
        duplicated_lines: 0,
        duplicated_blocks: 0,
        syntax_errors: analysis.syntax_errors,
    };
    FileResult {
        findings,
        measures,
        tokens: if job.keep_tokens {
            CpdTokens::new(lex.token_hashes, &lex.token_lines)
                .with_line_ends(|line| index.line_text(line).chars().count() as u32 + 1)
                .with_fingerprints(job.settings.cpd_min_tokens)
        } else {
            CpdTokens::default()
        },
        suppressions: sups,
        internal_error: plugin_error.is_some(),
        line_anchors,
    }
}

fn sanitize_range(r: &mut TextRange, line_count: u32) {
    r.start_line = r.start_line.clamp(1, line_count);
    r.start_col = r.start_col.max(1);
    if r.end_line < r.start_line {
        r.end_line = r.start_line;
        r.end_col = r.end_col.max(r.start_col);
    }
}

/// Convert pending findings into sorted, fingerprinted [`Finding`]s. Findings for rules not
/// enabled in `rules` are dropped.
pub(crate) fn finalize(
    registry: &Registry,
    rules: &RuleSet,
    path: &str,
    mut pending: Vec<Pending>,
) -> Vec<Finding> {
    pending.retain(|p| rules.is_enabled(&p.raw.rule_id));
    pending.sort_by(|a, b| {
        let (x, y) = (&a.raw, &b.raw);
        (
            x.range.start_line,
            x.range.start_col,
            &x.rule_id,
            &x.message,
            x.range.end_line,
            x.range.end_col,
            &x.symbol,
            &a.anchor,
        )
            .cmp(&(
                y.range.start_line,
                y.range.start_col,
                &y.rule_id,
                &y.message,
                y.range.end_line,
                y.range.end_col,
                &y.symbol,
                &b.anchor,
            ))
    });
    let mut counter = OccurrenceCounter::default();
    pending
        .into_iter()
        .map(|p| {
            let raw = p.raw;
            let meta = registry.rule(&raw.rule_id).map(|e| e.meta);
            let fingerprint = counter.next(&raw.rule_id, raw.symbol.as_deref(), &p.anchor);
            let severity = rules
                .get(&raw.rule_id)
                .map(|r| r.severity)
                .or(meta.map(|m| m.severity))
                .unwrap_or(mote_core::Severity::Low);
            Finding {
                id: finding_id(path, &fingerprint),
                rule_id: raw.rule_id,
                message: raw.message,
                path: path.to_string(),
                range: raw.range,
                severity,
                quality: meta.map(|m| m.quality).unwrap_or(Quality::Maintainability),
                symbol: raw.symbol,
                value: raw.value,
                fingerprint,
                status: FindingStatus::Unknown,
                incubating: meta.is_some_and(|m| m.status == RuleStatus::Incubating),
                suppressed: p.suppressed,
                secondary: raw.secondary,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CompiledConfig;
    use crate::testutil::{registry, FakePlugin};

    fn run(text: &str, yaml: &str, path: &str) -> FileResult {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let cc = CompiledConfig::new(Config::from_yaml_str(yaml).unwrap(), &reg).unwrap();
        let is_test = cc.is_test(0, path);
        let rules = cc.resolve(&reg, 0, is_test, path);
        let settings = AnalysisSettings::from_config(cc.config());
        let job = FileJob {
            registry: &reg,
            plugin: 0,
            path,
            is_test,
            rules: &rules,
            settings: &settings,
            skip_generated: settings.skip_generated,
            packages: &[],
            keep_tokens: true,
        };
        match analyze_bytes(&job, text.as_bytes()) {
            FileOutcome::Analyzed(r) => *r,
            FileOutcome::Skipped(s) => panic!("skipped: {s:?}"),
        }
    }

    /// Regression: every finding carried its line's whole whitespace-normalized text as its
    /// fingerprint anchor, so 120 000 findings on a 1 MB line needed over 100 GB (a one-line
    /// `switch` of 20 000 cases took 25 GB). Anchors are capped and computed once per line.
    #[test]
    fn many_findings_on_one_long_line_stay_cheap() {
        let line = format!("MANY_LOUD {}", "x LOUD y ".repeat(120_000));
        let t = std::time::Instant::now();
        let r = run(
            &format!("{line}\nLOUD\n"),
            "max_file_size: 4194304\nskip_generated: false\n",
            "a.fk",
        );
        let loud: Vec<&Finding> = r
            .findings
            .iter()
            .filter(|f| f.rule_id == "fk/loud")
            .collect();
        // Every `LOUD` of the line (the marker's own included) and the one of line 2.
        assert_eq!(loud.len(), 120_002);
        let distinct: std::collections::HashSet<&str> =
            loud.iter().map(|f| f.fingerprint.as_str()).collect();
        assert_eq!(distinct.len(), loud.len());
        assert!(
            t.elapsed() < std::time::Duration::from_secs(60),
            "{:?}",
            t.elapsed()
        );
    }

    /// Regression: every finding was matched against every suppression of its file, quadratic
    /// in a file with tens of thousands of suppressed lines.
    #[test]
    fn many_suppressions_stay_cheap() {
        let text = "LOUD // mote-ignore[fk/loud]: generated\n".repeat(40_000);
        let t = std::time::Instant::now();
        let r = run(
            &text,
            "max_file_size: 16777216\nskip_generated: false\n",
            "a.fk",
        );
        let suppressed = r
            .findings
            .iter()
            .filter(|f| f.rule_id == "fk/loud" && f.suppressed.is_some())
            .count();
        assert_eq!(suppressed, 40_000);
        assert!(r.suppressions.iter().all(|s| s.used));
        assert!(
            t.elapsed() < std::time::Duration::from_secs(30),
            "{:?}",
            t.elapsed()
        );
    }

    #[test]
    fn plugin_generic_and_meta_findings() {
        let text = "\
fn big cog=20 params=9
  LOUD
  // TODO: tidy
  x = 1; // mote-ignore[loud]: unused here
end
LOUD // mote-ignore[loud]: demo
SYNTAX_ERROR
";
        let r = run(text, "", "src/a.fk");
        let got: Vec<(&str, u32, bool)> = r
            .findings
            .iter()
            .map(|f| {
                (
                    f.rule_id.as_str(),
                    f.range.start_line,
                    f.suppressed.is_some(),
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                ("mote/parse-error", 1, false),
                ("fk/cognitive-complexity", 1, false),
                ("fk/too-many-parameters", 1, false),
                ("fk/loud", 2, false),
                ("fk/todo-comment", 3, false),
                ("fk/loud", 6, true),
            ]
        );
        let cog = &r.findings[1];
        assert_eq!(cog.symbol.as_deref(), Some("big"));
        assert_eq!(cog.value, Some(20.0));
        assert_eq!(cog.severity, mote_core::Severity::Medium);
        assert_eq!(r.findings[3].symbol.as_deref(), Some("big"));
        assert_eq!(r.measures.functions, 1);
        assert_eq!(r.measures.syntax_errors, 1);
        // the unused suppression on line 4 is known but reported after the reduce phase
        assert_eq!(r.suppressions.iter().filter(|s| !s.used).count(), 1);
        assert!(!r.tokens.is_empty());
    }

    #[test]
    fn plugin_panic_becomes_internal_error() {
        let r = run("a;\nPANIC_NOW\n", "", "a.fk");
        assert!(r.internal_error);
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].rule_id, "mote/internal-error");
        assert!(r.findings[0].message.contains("fake plugin asked to panic"));
        assert_eq!(r.measures.lines, 2, "lexer measures still computed");
    }

    #[test]
    fn severity_override_and_incubating_flag() {
        let r = run("LOUD INCUB\n", "rules: { fk/loud: HIGH }", "a.fk");
        let loud = r.findings.iter().find(|f| f.rule_id == "fk/loud").unwrap();
        assert_eq!(loud.severity, mote_core::Severity::High);
        let inc = r
            .findings
            .iter()
            .find(|f| f.rule_id == "fk/incubating")
            .unwrap();
        assert!(inc.incubating);
    }

    #[test]
    fn fingerprints_survive_line_shifts() {
        let a = run("LOUD\nLOUD\n", "", "a.fk");
        let b = run("\n\n\nLOUD\n\nLOUD\n", "", "a.fk");
        let fa: Vec<&str> = a.findings.iter().map(|f| f.fingerprint.as_str()).collect();
        let fb: Vec<&str> = b.findings.iter().map(|f| f.fingerprint.as_str()).collect();
        assert_eq!(fa, fb);
        assert_ne!(
            fa[0], fa[1],
            "occurrence index separates identical findings"
        );
    }

    #[test]
    fn decoding_and_skips() {
        assert_eq!(decode_source(b"\xEF\xBB\xBFabc").unwrap().0, "abc");
        assert_eq!(decode_source(b"a\0b").unwrap_err(), SkipReason::Binary);
        let (t, lossy) = decode_source(b"caf\xE9").unwrap();
        assert_eq!(t, "café");
        assert!(lossy);
        let utf16: Vec<u8> = [0xFF, 0xFE, b'h', 0, b'i', 0].to_vec();
        assert_eq!(decode_source(&utf16).unwrap().0, "hi");
        assert_eq!(
            generated_or_minified("a.java", "// Code generated by x. DO NOT EDIT.\nclass A {}"),
            Some(SkipReason::Generated)
        );
        assert_eq!(
            generated_or_minified("a.js", &format!("var a={};", "x".repeat(6000))),
            Some(SkipReason::Minified)
        );
        assert_eq!(generated_or_minified("a.java", "class A {}\n"), None);
        // Line count and longest line (in bytes, `\r` included) come from the line index; the
        // thresholds are exclusive.
        let line = |n: usize| "x".repeat(n);
        // 40 short lines keep the average line length low.
        let short = "a\n".repeat(40);
        for (path, text, minified) in [
            ("a.js", format!("{}\r\n{short}", line(5000)), true),
            ("a.js", format!("{}\n{short}", line(5000)), false),
            ("a.js", format!("{short}{}", line(5001)), true),
            ("a.ts", format!("{short}{}\n\n", line(4999)), false),
            ("a.java", format!("{}\n{short}", line(6000)), false),
            ("a.java", line(9000), true),
            (
                "a.java",
                [line(300), line(300), line(300), line(300)].join("\n"),
                false,
            ),
            (
                "a.java",
                [line(301), line(301), line(301), line(301)].join("\n"),
                true,
            ),
            ("a.java", format!("{}\n", line(1000)), false),
            ("a.java", String::new(), false),
        ] {
            let got = generated_or_minified(path, &text) == Some(SkipReason::Minified);
            assert_eq!(got, minified, "{path} {} bytes", text.len());
        }
    }

    #[test]
    fn generated_sentence_matches_the_phrase_search() {
        // The reference definition: any "{subject} {verb} {how}" phrase as a word.
        fn reference(norm: &str) -> bool {
            let subjects = ["this file", "this code", "this class", "this source file"];
            let verbs = ["is", "was", "has been"];
            let how = [
                "generated",
                "auto-generated",
                "auto generated",
                "autogenerated",
                "automatically generated",
                "machine-generated",
                "machine generated",
            ];
            subjects.iter().any(|s| {
                verbs
                    .iter()
                    .any(|v| how.iter().any(|h| has_word(norm, &format!("{s} {v} {h}"))))
            })
        }
        for norm in [
            "",
            "this",
            "this file is",
            "this file is generated",
            "this file is generated.",
            "this file is generatedx",
            "this file is generated_",
            "this file is generated-",
            "this file is generated2",
            "xthis file was auto generated by a tool",
            "this this file has been machine-generated",
            "this source file is automatically generated, do not edit",
            "this class was autogenerated",
            "this code is auto-generated",
            "this code is auto",
            "this file is not generated",
            "this file  is generated",
            "this file was machine generatedé",
            "notes: this class has been generated by hand. this file is generated",
        ] {
            assert_eq!(says_file_is_generated(norm), reference(norm), "{norm:?}");
        }
        assert_eq!(
            normalized_comment(&["/** This File IS", " * GENERATED — Ünïcode */"]),
            "this file is generated — Ünïcode"
        );
    }

    #[test]
    fn generated_detection_only_reads_explicit_markers_in_leading_comments() {
        let generated = [
            "// @generated\nclass A {}\n",
            "#!/usr/bin/env node\n\n// @generated by relay\nlet a;\n",
            "// Code generated by protoc-gen-go. DO NOT EDIT.\npackage a;\n",
            "/* Copyright 2020 Someone */\n\n// Generated by the protocol buffer compiler.  DO NOT EDIT!\npackage a;\n",
            "/*\n * This file was automatically generated by xjc.\n * Changes will be lost.\n */\npackage a;\n",
            "/**\n * THIS FILE IS AUTO-GENERATED. Do not change it by hand.\n */\nexport {};\n",
            "/* This code was generated by a tool. */ \nclass A {}\n",
            // grpc-tools and base64-embedding scripts
            "// GENERATED CODE -- DO NOT EDIT!\n/* eslint-disable */\nexport {};\n",
        ];
        for text in generated {
            assert_eq!(
                generated_or_minified("a.java", text),
                Some(SkipReason::Generated),
                "{text}"
            );
        }
        let ordinary = [
            // loose words in a leading comment
            "/** Entity with an auto-generated id. */\nclass E { long id; }\n",
            "// Do not edit without talking to the team\nclass A {}\n",
            "/* autogenerated ids are used below */\nclass A {}\n",
            // markers in code or after the first code line
            "class A {\n  String s = \"Do not edit\";\n}\n",
            "String s = \"@generated\"; // x\n",
            "package a;\n// @generated\nclass A {}\n",
            "/* header */ class A {} // @generated\n",
            "class A { /** @generatedAt */ }\n",
            "// @generatedAt 2020\nclass A {}\n",
            "// The generated code below is reviewed by hand\nclass A {}\n",
        ];
        for text in ordinary {
            assert_eq!(generated_or_minified("a.java", text), None, "{text}");
        }
    }

    #[test]
    fn generated_marker_in_the_doc_comment_of_the_first_type() {
        let license = "/*\n * Licensed under the Apache License.\n */\n";
        let generated = [
            // the marker sits in the class Javadoc, after package and imports
            "package a.b;\n\nimport a.C;\n\n/**\n * Marshalling code for C\n *\n * NOTE!: This file is auto generated - do not modify!\n *        regenerate it with the scripts.\n */\npublic class CMarshaller extends Base {\n}\n",
            // annotations and a line comment between the comment and the class
            "package a;\n/**\n * Test case for C.\n * NOTE!: This file is auto generated - do not modify! if you need a\n * change, edit the generator.\n */\n// tests\n@Category(Fast.class)\n@Deprecated public final class CTest {\n}\n",
            "package a;\n/** This class was generated by the builder; do not edit. */ public class A {}\n",
            "package a;\n/**\n * This file is autogenerated. Do not edit.\n */\n@SuppressWarnings({\"a\", \"b\"}) public class A {}\n",
            // CRLF and old Mac (CR only) line ends
            "package a;\r\n\r\n/**\r\n * This file is auto generated - do not modify!\r\n */\r\npublic class A {}\r\n",
            "package a;\r\rimport a.B;\r/**\r * This file is auto generated - do not modify!\r */\rpublic class A {}\r",
        ];
        for body in generated {
            for text in [body.to_string(), format!("{license}{body}")] {
                assert_eq!(
                    generated_or_minified("a.java", &text),
                    Some(SkipReason::Generated),
                    "{text}"
                );
            }
        }
        let ordinary = [
            // the sentence without a do-not-edit warning describes the class
            "package a;\n/**\n * This class is generated at runtime as a proxy for each entity.\n */\npublic class Proxy {}\n",
            // a warning without the sentence
            "package a;\n/** Do not modify: the generated code depends on this layout. */\nclass Layout {}\n",
            // a line comment is not a doc comment
            "package a;\n// This file is auto generated - do not modify!\nclass A {}\n",
            // the comment of a later type, after code
            "package a;\nclass A {}\n/** This file is auto generated - do not modify! */\nclass B {}\n",
            // the comment of a member
            "package a;\nclass A {\n  /** This file is auto generated - do not modify! */\n  void f() {}\n}\n",
            // a comment before the imports belongs to the header, which has no marker here
            "package a;\n/** This file is auto generated - do not modify! */\nimport a.B;\nclass A {}\n",
            // code before the comment
            "package a;\nstatic { }\n/** This file is auto generated - do not modify! */\nclass A {}\n",
        ];
        for text in ordinary {
            assert_eq!(generated_or_minified("a.java", text), None, "{text}");
        }
        assert!(first_type_doc_comment("class A {}\n").is_empty());
    }

    #[test]
    fn yarn_plug_n_play_files_are_generated_paths() {
        for p in [
            ".yarn/releases/yarn-4.1.0.cjs",
            "web/.yarn/sdks/typescript/lib/tsserver.js",
            ".yarn/unplugged/pkg/node_modules/pkg/index.js",
            ".pnp.cjs",
            "frontend/.pnp.loader.mjs",
            ".pnp.js",
        ] {
            assert!(is_generated_path(p), "{p}");
        }
        for p in [
            "src/yarn.js",
            "src/.yarnrc.js",
            "a/.yarn-cache.ts",
            "pnp.cjs",
            "src/.pnp.cjs.ts",
        ] {
            assert!(!is_generated_path(p), "{p}");
        }
    }

    #[test]
    fn cpd_tokens_roundtrip() {
        let t = CpdTokens::new(vec![1, 2, 3, 4], &[1, 1, 3, 7]);
        assert_eq!(t.runs, vec![(1, 0), (3, 2), (7, 3)]);
        assert_eq!(t.lines(), vec![1, 1, 3, 7]);
        assert_eq!(
            (0..5).map(|i| t.line_of(i)).collect::<Vec<_>>(),
            vec![1, 1, 3, 7, 7]
        );
        let with_ends = t.clone().with_line_ends(|l| l * 10 + 1);
        assert_eq!(with_ends.line_end(3), Some(31));
        assert_eq!(with_ends.line_end(4), None);
        for t in [
            t,
            with_ends,
            CpdTokens::default(),
            CpdTokens::new(
                (0..5000u32)
                    .map(|i| i.wrapping_mul(2_654_435_761) % 700)
                    .collect(),
                &(0..5000u32).map(|i| 3 + i / 9).collect::<Vec<_>>(),
            ),
        ] {
            let mut bytes = Vec::new();
            t.encode(&mut bytes);
            assert_eq!(CpdTokens::decode(&bytes).as_ref(), Some(&t));
            let via_postcard: CpdTokens =
                postcard::from_bytes(&postcard::to_allocvec(&t).unwrap()).unwrap();
            assert_eq!(via_postcard, t);
            // truncations and bit flips never panic and never decode into garbage silently
            for cut in 0..bytes.len() {
                assert!(CpdTokens::decode(&bytes[..cut]).is_none() || cut == bytes.len());
            }
            for i in 0..bytes.len().min(64) {
                let mut b = bytes.clone();
                b[i] ^= 0x5a;
                let _ = CpdTokens::decode(&b);
            }
        }
    }
}
