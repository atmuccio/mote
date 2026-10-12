//! Copy/paste detection over the token streams produced by the lexer (reduce phase).
//!
//! 1. Each file's token hashes are turned into k-gram hashes (k = 25, or `min_tokens` if smaller)
//!    with a rolling hash and *winnowed* (Schleimer, Wilkerson & Aiken, 2003) with window
//!    `w = min_tokens − k + 1`: any shared run of at least `min_tokens` tokens is guaranteed to
//!    share a selected fingerprint, while only about `2/(w+1)` of positions are kept.
//! 2. Fingerprints are hash-joined across files (sorting, not hashing, so the result is
//!    independent of thread count). Buckets with more than [`MAX_BUCKET`] entries are ignored:
//!    such sequences are boilerplate and would make the join quadratic.
//! 3. Each candidate pair is extended backwards and forwards over the token arrays to a maximal
//!    match; matches shorter than `min_tokens` tokens or `min_lines` lines are dropped. Positions
//!    already inside a found match of the same pair and offset are skipped, so long duplicates
//!    are extended once.
//! 4. Matches with identical content are merged into blocks listing all their occurrences.

use rayon::prelude::*;
use rustc_hash::FxHashMap;
use xxhash_rust::xxh3::xxh3_64;

/// Buckets larger than this are ignored.
pub const MAX_BUCKET: usize = 64;
/// Default k-gram size.
pub const K: usize = 25;

/// One file's tokens.
pub struct CpdFile<'a> {
    pub hashes: &'a [u32],
    /// `(line, first token index)` per line with tokens (see [`CpdTokens`](crate::analyze::CpdTokens)).
    pub runs: &'a [(u32, u32)],
    /// Precomputed [`fingerprints`] for this `min_tokens`, if any (`None`: computed here).
    pub fingerprints: Option<&'a [(u64, u32)]>,
}

impl CpdFile<'_> {
    /// Line of token `i`.
    fn line(&self, i: usize) -> u32 {
        crate::analyze::line_in_runs(self.runs, i as u32)
    }
}

/// One occurrence of a duplicated block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DupOccurrence {
    /// Index into the input file slice.
    pub file: usize,
    pub start_token: u32,
    pub start_line: u32,
    pub end_line: u32,
}

/// A duplicated block with all its occurrences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DupBlock {
    /// Length in tokens.
    pub tokens: u32,
    /// Hash of the duplicated token sequence (stable across line shifts).
    pub content_hash: u64,
    /// Sorted by (file, start_token).
    pub occurrences: Vec<DupOccurrence>,
}

#[inline]
fn mix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

fn kgram_hashes(tokens: &[u32], k: usize) -> Vec<u64> {
    const B: u64 = 1_000_003;
    if tokens.len() < k || k == 0 {
        return Vec::new();
    }
    let mut pw = 1u64;
    for _ in 0..k - 1 {
        pw = pw.wrapping_mul(B);
    }
    let mut out = Vec::with_capacity(tokens.len() - k + 1);
    let mut h = 0u64;
    for (i, &t) in tokens.iter().enumerate() {
        if i >= k {
            h = h.wrapping_sub(u64::from(tokens[i - k]).wrapping_mul(pw));
        }
        h = h.wrapping_mul(B).wrapping_add(u64::from(t));
        if i + 1 >= k {
            out.push(mix(h));
        }
    }
    out
}

/// Robust winnowing: the rightmost minimum of every window of `w` hashes, recorded when it
/// changes. Returns `(hash, k-gram position)`.
pub fn winnow(tokens: &[u32], k: usize, w: usize) -> Vec<(u64, u32)> {
    let hs = kgram_hashes(tokens, k);
    let w = w.max(1);
    if hs.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(hs.len() * 2 / (w + 1) + 2);
    // The candidates for the window minimum, oldest first, each with a smaller hash than every
    // later one: `(hash, position)` in a ring buffer (never more than `w + 1` entries), live
    // between the ever-increasing `head` and `tail`. Runs for every token of every file.
    let mask = (w + 1).next_power_of_two() - 1;
    let mut ring: Vec<(u64, u32)> = vec![(0, 0); mask + 1];
    let (mut head, mut tail) = (0usize, 0usize);
    let mut last = u32::MAX;
    for (i, &h) in hs.iter().enumerate() {
        // An equal hash replaces the older one: the rightmost minimum wins.
        while tail > head && ring[(tail - 1) & mask].0 >= h {
            tail -= 1;
        }
        ring[tail & mask] = (h, i as u32);
        tail += 1;
        if ring[head & mask].1 as usize + w <= i {
            head += 1;
        }
        if i + 1 >= w || i + 1 == hs.len() {
            let (min, m) = ring[head & mask];
            if m != last {
                out.push((min, m));
                last = m;
            }
        }
    }
    out
}

/// k-gram size and winnowing window for `min_tokens`.
fn k_and_w(min_tokens: u32) -> (usize, usize) {
    let min_tokens = min_tokens.max(1) as usize;
    let k = K.min(min_tokens);
    (k, min_tokens - k + 1)
}

/// A file's winnowed fingerprints for `min_tokens` (empty for files with fewer tokens): what
/// [`find_duplicates`] joins across files. Computed once per file in the map phase and cached.
pub fn fingerprints(hashes: &[u32], min_tokens: u32) -> Vec<(u64, u32)> {
    let (k, w) = k_and_w(min_tokens);
    if hashes.len() >= (min_tokens.max(1) as usize) {
        winnow(hashes, k, w)
    } else {
        Vec::new()
    }
}

/// A maximal match between two positions.
#[derive(Clone, Copy, Debug)]
struct Match {
    fa: u32,
    sa: u32,
    fb: u32,
    sb: u32,
    len: u32,
}

/// Find duplicated blocks among `files` (one language).
pub fn find_duplicates(files: &[CpdFile<'_>], min_tokens: u32, min_lines: u32) -> Vec<DupBlock> {
    // 1. winnow (parallel, per file) unless the map phase did
    let computed: Vec<Option<Vec<(u64, u32)>>> = files
        .par_iter()
        .map(|f| match f.fingerprints {
            Some(_) => None,
            None => Some(fingerprints(f.hashes, min_tokens)),
        })
        .collect();
    let per_file: Vec<&[(u64, u32)]> = files
        .iter()
        .zip(&computed)
        .map(|(f, c)| c.as_deref().or(f.fingerprints).unwrap_or(&[]))
        .collect();
    let min_tokens = min_tokens.max(1) as usize;
    let mut all: Vec<(u64, u32, u32)> = Vec::with_capacity(per_file.iter().map(|f| f.len()).sum());
    for (fi, fps) in per_file.iter().enumerate() {
        // Positions come from the cache: ignore any that do not fit the token array.
        let n = files[fi].hashes.len();
        all.extend(
            fps.iter()
                .filter(|&&(_, p)| (p as usize) < n)
                .map(|&(h, p)| (h, fi as u32, p)),
        );
    }
    all.par_sort_unstable();

    // 2. hash-join into candidate pairs
    let mut candidates: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let mut j = i + 1;
        while j < all.len() && all[j].0 == all[i].0 {
            j += 1;
        }
        let n = j - i;
        if (2..=MAX_BUCKET).contains(&n) {
            for a in i..j {
                for b in a + 1..j {
                    let (_, fa, pa) = all[a];
                    let (_, fb, pb) = all[b];
                    candidates.push((fa, fb, pa, pb));
                }
            }
        }
        i = j;
    }
    candidates.sort_unstable();

    // 3. extend, grouped by file pair (independent groups run in parallel)
    let mut groups: Vec<&[(u32, u32, u32, u32)]> = Vec::new();
    let mut s = 0;
    for e in 1..=candidates.len() {
        if e == candidates.len()
            || (candidates[e].0, candidates[e].1) != (candidates[s].0, candidates[s].1)
        {
            groups.push(&candidates[s..e]);
            s = e;
        }
    }
    let matches: Vec<Match> = groups
        .par_iter()
        .flat_map_iter(|group| extend_group(files, group, min_tokens, min_lines))
        .collect();

    // 4. merge matches with identical content into blocks
    let mut by_content: FxHashMap<(u64, u32), Vec<(u32, u32)>> = FxHashMap::default();
    for m in &matches {
        let fa = &files[m.fa as usize];
        let slice = &fa.hashes[m.sa as usize..(m.sa + m.len) as usize];
        let bytes: Vec<u8> = slice.iter().flat_map(|t| t.to_le_bytes()).collect();
        let key = (xxh3_64(&bytes), m.len);
        let occ = by_content.entry(key).or_default();
        occ.push((m.fa, m.sa));
        occ.push((m.fb, m.sb));
    }
    let mut blocks: Vec<DupBlock> = by_content
        .into_iter()
        .map(|((content_hash, len), mut occ)| {
            occ.sort_unstable();
            occ.dedup();
            let occurrences = occ
                .into_iter()
                .map(|(f, s)| {
                    let file = &files[f as usize];
                    DupOccurrence {
                        file: f as usize,
                        start_token: s,
                        start_line: file.line(s as usize),
                        end_line: file.line((s + len - 1) as usize),
                    }
                })
                .collect();
            DupBlock {
                tokens: len,
                content_hash,
                occurrences,
            }
        })
        .collect();
    blocks.sort_by(|a, b| {
        let ka = (
            a.occurrences[0].file,
            a.occurrences[0].start_token,
            std::cmp::Reverse(a.tokens),
            a.content_hash,
        );
        let kb = (
            b.occurrences[0].file,
            b.occurrences[0].start_token,
            std::cmp::Reverse(b.tokens),
            b.content_hash,
        );
        ka.cmp(&kb)
    });
    blocks
}

fn extend_group(
    files: &[CpdFile<'_>],
    group: &[(u32, u32, u32, u32)],
    min_tokens: usize,
    min_lines: u32,
) -> Vec<Match> {
    let mut covered: FxHashMap<i64, Vec<(u32, u32)>> = FxHashMap::default();
    let mut out = Vec::new();
    for &(fa, fb, pa, pb) in group {
        let delta = i64::from(pb) - i64::from(pa);
        if covered
            .get(&delta)
            .is_some_and(|iv| iv.iter().any(|&(s, e)| pa >= s && pa < e))
        {
            continue;
        }
        let same = fa == fb;
        if same && pa == pb {
            continue;
        }
        let (ta, tb) = (files[fa as usize].hashes, files[fb as usize].hashes);
        let (mut s1, mut s2) = (pa as usize, pb as usize);
        if ta.get(s1) != tb.get(s2) {
            continue;
        }
        while s1 > 0 && s2 > 0 && ta[s1 - 1] == tb[s2 - 1] {
            s1 -= 1;
            s2 -= 1;
        }
        let mut len = 0usize;
        while s1 + len < ta.len() && s2 + len < tb.len() && ta[s1 + len] == tb[s2 + len] {
            if same && s1 + len >= s2 {
                break; // copies within one file must not overlap
            }
            len += 1;
        }
        covered
            .entry(delta)
            .or_default()
            .push((s1 as u32, (s1 + len) as u32));
        if len < min_tokens {
            continue;
        }
        let (fa_, fb_) = (&files[fa as usize], &files[fb as usize]);
        let span_a = fa_.line(s1 + len - 1) - fa_.line(s1) + 1;
        let span_b = fb_.line(s2 + len - 1) - fb_.line(s2) + 1;
        if span_a.min(span_b) < min_lines {
            continue;
        }
        out.push(Match {
            fa,
            sa: s1 as u32,
            fb,
            sb: s2 as u32,
            len: len as u32,
        });
    }
    out
}

/// Merged duplicated line ranges per file (`nfiles` entries), from all block occurrences.
pub fn duplicated_ranges(blocks: &[DupBlock], nfiles: usize) -> Vec<Vec<(u32, u32)>> {
    let mut per: Vec<Vec<(u32, u32)>> = vec![Vec::new(); nfiles];
    for b in blocks {
        for o in &b.occurrences {
            per[o.file].push((o.start_line, o.end_line));
        }
    }
    for ranges in &mut per {
        *ranges = merge_ranges(std::mem::take(ranges));
    }
    per
}

/// Merge overlapping or adjacent inclusive line ranges.
pub fn merge_ranges(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut out: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (s, e) in ranges {
        match out.last_mut() {
            Some(last) if s <= last.1.saturating_add(1) => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// Number of lines covered by merged inclusive ranges.
pub fn count_lines(ranges: &[(u32, u32)]) -> u32 {
    ranges.iter().map(|(s, e)| e - s + 1).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{lex, LexOptions};
    use mote_core::LexerSpec;

    fn spec() -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: false,
            template_literals: false,
            cpd_skip_line_prefixes: &["import "],
        }
    }

    fn body(seed: &str, lines: usize) -> String {
        (0..lines)
            .map(|i| {
                format!("  total_{seed} = compute(value{i}, \"text\", {i}) + other.call(x, y);\n")
            })
            .collect()
    }

    fn run(texts: &[String], min_tokens: u32, min_lines: u32) -> Vec<DupBlock> {
        let lexed: Vec<_> = texts
            .iter()
            .map(|t| {
                let l = lex(t, &spec(), LexOptions::default());
                crate::analyze::CpdTokens::new(l.token_hashes, &l.token_lines)
            })
            .collect();
        let files: Vec<CpdFile> = lexed
            .iter()
            .map(|l| CpdFile {
                hashes: &l.hashes,
                runs: &l.runs,
                fingerprints: None,
            })
            .collect();
        find_duplicates(&files, min_tokens, min_lines)
    }

    /// The original formulation of [`winnow`], with a `VecDeque` of positions.
    fn winnow_reference(tokens: &[u32], k: usize, w: usize) -> Vec<(u64, u32)> {
        let hs = kgram_hashes(tokens, k);
        let w = w.max(1);
        if hs.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut dq: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        let mut last = usize::MAX;
        for i in 0..hs.len() {
            while dq.back().is_some_and(|&j| hs[j] >= hs[i]) {
                dq.pop_back();
            }
            dq.push_back(i);
            if dq.front().is_some_and(|&j| j + w <= i) {
                dq.pop_front();
            }
            if i + 1 >= w || i + 1 == hs.len() {
                if let Some(&m) = dq.front() {
                    if m != last {
                        out.push((hs[m], m as u32));
                        last = m;
                    }
                }
            }
        }
        out
    }

    #[test]
    fn winnowing_matches_the_reference() {
        // Few distinct tokens (many equal k-gram hashes, so ties matter), many, and runs.
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut next = |m: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % m) as u32
        };
        let mut inputs: Vec<Vec<u32>> = vec![Vec::new(), vec![7], vec![1; 300]];
        for m in [2, 3, 50, 1_000_000] {
            for len in [1, 2, 24, 25, 26, 100, 101, 5_000] {
                inputs.push((0..len).map(|_| next(m)).collect());
            }
        }
        inputs.push((0..3_000u32).map(|i| (i / 40) % 3).collect());
        for tokens in &inputs {
            for (k, w) in [
                (1, 1),
                (2, 3),
                (25, 1),
                (25, 76),
                (25, 128),
                (10, 7),
                (25, 200),
            ] {
                assert_eq!(
                    winnow(tokens, k, w),
                    winnow_reference(tokens, k, w),
                    "k={k} w={w} len={}",
                    tokens.len()
                );
            }
        }
    }

    #[test]
    fn winnowing_guarantee_and_density() {
        let tokens: Vec<u32> = (0..10_000u32)
            .map(|i| i.wrapping_mul(2_654_435_761) % 1000)
            .collect();
        let fps = winnow(&tokens, 25, 76);
        let density = fps.len() as f64 / tokens.len() as f64;
        assert!(density > 0.01 && density < 0.05, "density {density}");
        // every window of 76 k-grams contains a selected position
        let mut positions: Vec<u32> = fps.iter().map(|f| f.1).collect();
        positions.sort_unstable();
        for win in positions.windows(2) {
            assert!(win[1] - win[0] <= 76);
        }
    }

    #[test]
    fn finds_cross_file_duplicate_with_shifted_lines_and_different_literals() {
        let a = format!("class A {{\n{}}}\n", body("a", 15));
        let b = format!(
            "// header\n\n\nclass B {{\n{}}}\n",
            body("a", 15).replace("\"text\"", "\"other\"")
        );
        let blocks = run(&[a, b], 100, 10);
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        let blk = &blocks[0];
        assert_eq!(blk.occurrences.len(), 2);
        assert_eq!(blk.occurrences[0].file, 0);
        assert_eq!(blk.occurrences[1].file, 1);
        assert_eq!(
            blk.occurrences[1].start_line - blk.occurrences[0].start_line,
            3
        );
        assert!(blk.tokens >= 15 * 20);
    }

    #[test]
    fn three_copies_form_one_block() {
        let t = |n: &str| format!("fn {n}() {{\n{}}}\n", body("z", 12));
        let blocks = run(&[t("a"), t("b"), t("c")], 100, 10);
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].occurrences.len(), 3);
    }

    #[test]
    fn below_thresholds_is_not_reported() {
        // 4 lines × 20 tokens = 80 tokens < 100
        let a = format!("class A {{\n{}}}\n", body("a", 4));
        let b = format!("class B {{\n{}}}\n", body("a", 4));
        assert!(run(&[a, b], 100, 10).is_empty());
        // enough tokens, but on too few lines
        let one_line = |n: &str| format!("x{n}; {}\n", body("q", 12).replace('\n', " "));
        assert!(run(&[one_line("1"), one_line("2")], 100, 10).is_empty());
        // near-duplicate: identifiers differ every few lines → no long common run
        let b2: String = (0..15)
            .map(|i| {
                format!("  total_a = compute(value{i}, \"text\", {i}) + other.call{i}(x, y);\n")
            })
            .collect();
        let a2 = format!("class A {{\n{}}}\n", body("a", 15));
        assert!(run(&[a2, format!("class B {{\n{b2}}}\n")], 100, 10).is_empty());
    }

    #[test]
    fn same_file_copies_do_not_overlap() {
        let text = format!("class A {{\n{}\n{}}}\n", body("m", 12), body("m", 12));
        let blocks = run(&[text], 100, 10);
        assert!(!blocks.is_empty());
        for b in &blocks {
            let o = &b.occurrences;
            assert_eq!(o.len(), 2);
            assert!(o[0].start_token + b.tokens <= o[1].start_token);
        }
    }

    #[test]
    fn boilerplate_buckets_are_capped() {
        // 100 identical files: every fingerprint bucket has 100 entries → ignored
        let t = format!("class A {{\n{}}}\n", body("b", 12));
        let texts: Vec<String> = (0..100).map(|_| t.clone()).collect();
        assert!(run(&texts, 100, 10).is_empty());
    }

    #[test]
    fn range_merging() {
        assert_eq!(
            merge_ranges(vec![(5, 9), (1, 3), (4, 4), (20, 22), (21, 30)]),
            vec![(1, 9), (20, 30)]
        );
        assert_eq!(count_lines(&[(1, 9), (20, 30)]), 20);
    }
}
