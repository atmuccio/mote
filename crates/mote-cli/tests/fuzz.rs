//! Robustness fuzzing of both language plugins, every rule and the engine's per-file pipeline.
//!
//! Two harnesses, both deterministic (seeded) and `#[ignore]`d — they take minutes and read seed
//! files from outside the repository:
//!
//! * [`fuzz_mutations`] mutates real source files (truncation, byte flips / insertions /
//!   deletions, token splicing between files, invalid UTF-8, BOMs and UTF-16, NUL bytes, mixed
//!   line endings, Unicode, 1 MB single lines, line shuffles, stacked mutations) and runs each one
//!   through the engine's map step ([`analyze_bytes`]: decode, skip checks, plugin with every rule
//!   enabled, lexer, generic rules, suppressions, fingerprints). It flags panics (also
//!   `mote/internal-error` findings), process aborts, hangs, per-file time blowups, invalid
//!   ranges, and nondeterminism (the same input analysed twice; a sample of every chunk scanned
//!   as a directory on 1 and on N threads, and cold vs warm cache). Work is split into chunks,
//!   each run in a child process (this test binary re-executed), so an abort (stack overflow) or
//!   a hang is attributed to the inputs in flight and the rest of the run continues.
//! * [`fuzz_deep_nesting`] generates deeply nested Java and JavaScript/TypeScript (parentheses,
//!   brackets, braces, arrays, lambdas, ternaries, generics, JSX, operator chains, `else if`
//!   chains, regular expressions, …) at increasing depths up to 4 MB files and scans each with
//!   the real `mote` binary (a stack overflow aborts the process, which `catch_unwind` cannot
//!   catch). It flags crashes, internal errors, files slower than the budget and superlinear
//!   growth.
//!
//! Run in a release build (rules are much slower in debug builds), and the deep-nesting test in
//! a debug build too (its stack frames are larger):
//!
//! ```text
//! MOTE_FUZZ_SEEDS="/path/to/repos;/path/to/more" MOTE_FUZZ_ITERS=200000 \
//!   cargo test -p mote-cli --release --test fuzz -- --ignored --nocapture fuzz_mutations
//! cargo test -p mote-cli [--release] --test fuzz -- --ignored --nocapture fuzz_deep_nesting
//! ```
//!
//! Environment (all optional):
//! * `MOTE_FUZZ_SEEDS` — `;`-separated directories of seed files (read only; default: the
//!   plugins' fixtures only);
//! * `MOTE_FUZZ_SEED` — RNG seed (default 1); `MOTE_FUZZ_ITERS` — mutated files (default
//!   20 000); `MOTE_FUZZ_START` — first iteration (default 0);
//! * `MOTE_FUZZ_CHUNK` — iterations per child process (default 10 000);
//!   `MOTE_FUZZ_THREADS` — analysis threads per child (default: all cores);
//! * `MOTE_FUZZ_SEEDS_PER_LANG` — seed files sampled per language (default 3 000);
//! * `MOTE_FUZZ_SLOW_MS` / `MOTE_FUZZ_SLOW_US_PER_KB` — a file is a blowup when its
//!   analysis takes longer than both (defaults 1 000 ms and 15 000 µs per KiB; 8 times as long
//!   in a debug build, as is the superlinear-growth floor);
//!   `MOTE_FUZZ_HANG_SECS` — in flight longer than this is a hang (default 120; deep
//!   nesting: 60); `MOTE_FUZZ_DET_EVERY` — analyse every n-th input twice (default 3);
//!   `MOTE_FUZZ_SCAN_SAMPLE` — inputs of each chunk scanned as a directory (default 300);
//! * `MOTE_FUZZ_WORKDIR` — where scan directories go (default: the system temp directory);
//! * `MOTE_FUZZ_DUMP=<i>` — write mutated file `i` to the workdir and stop (to reproduce);
//! * `MOTE_FUZZ_SHAPE` — only deep-nesting shapes whose name contains this text;
//!   `MOTE_FUZZ_DEEP_MAX` — largest deep-nesting file in bytes (default 4 MiB);
//!   `MOTE_FUZZ_PROCS` — scans run at once (default 4); `MOTE_FUZZ_MAX_MB` — a scan
//!   using more memory is killed and reported (default 4 096); `MOTE_FUZZ_FLOOR_MS` —
//!   superlinear growth is only reported above this time (default 200).
//!
//! [`fuzz_profile`] prints per-rule timings for one file (`MOTE_FUZZ_PROFILE=<path>`, or
//! `shape:<deep-nesting shape>:<n>`).

#![cfg(all(feature = "java", feature = "js"))]

use mote_core::Finding;
use mote_engine::analyze::{analyze_bytes, AnalysisSettings, FileJob, FileOutcome};
use mote_engine::config::CompiledConfig;
use mote_engine::scan::{ScanOptions, Scanner, SourceSpec};
use mote_engine::{Config, Registry};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------------

/// SplitMix64: small, fast, and stable across platforms and Rust versions.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut r = Rng(seed ^ 0x9E37_79B9_7F4A_7C15);
        r.next();
        r
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `0..n` (`0` when `n == 0`).
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next() % den < num
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().replace('_', "").parse().ok())
        .unwrap_or(default)
}

fn workdir() -> PathBuf {
    let dir = std::env::var_os("MOTE_FUZZ_WORKDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mote-fuzz"));
    std::fs::create_dir_all(&dir).expect("create workdir");
    dir
}

const INTERNAL_ERROR: &str = "mote/internal-error";

/// The configuration every harness scans with: all rules, nothing skipped by size.
fn fuzz_yaml(skip_generated: bool) -> String {
    format!(
        "extends: all\nmax_file_size: 16777216\nskip_generated: {skip_generated}\n\
         suppressions:\n  compat: [checkstyle, eslint, intellij, nosonar, pmd, spotbugs, suppress-warnings-all]\n  require_reason: {skip_generated}\n\
         cpd:\n  ignore_literals: {skip_generated}\n  min_tokens: 30\n"
    )
}

/// Whether analysing `bytes` bytes in `elapsed` is a blowup: over `MOTE_FUZZ_SLOW_MS`
/// (default 1 s) and over `MOTE_FUZZ_SLOW_US_PER_KB` per KiB (default 15 000 µs: a megabyte
/// of garbage Java takes tree-sitter's error recovery 3–9 s on a loaded machine, linearly; a
/// quadratic rule takes minutes). Both are 8 times as long in a debug build.
fn is_slow(elapsed: Duration, bytes: usize) -> bool {
    let scale = if cfg!(debug_assertions) { 8 } else { 1 };
    let floor = Duration::from_millis(scale * env_u64("MOTE_FUZZ_SLOW_MS", 1_000));
    let per_kb = scale * env_u64("MOTE_FUZZ_SLOW_US_PER_KB", 15_000);
    elapsed > floor && elapsed > Duration::from_micros(per_kb * (bytes as u64 / 1024 + 1))
}

// ---------------------------------------------------------------------------------------------
// Mutation fuzzing
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Lang {
    Java,
    Js,
}

impl Lang {
    fn name(self) -> &'static str {
        match self {
            Lang::Java => "java",
            Lang::Js => "js",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Class {
    Original,
    Truncate,
    ByteFlip,
    Insert,
    Delete,
    Splice,
    InvalidUtf8,
    Bom,
    Nul,
    LineEndings,
    Unicode,
    Shuffle,
    LongLine,
    Stacked,
}

/// Mutation classes with their weights (a long line is a megabyte of code: rarer).
const CLASSES: &[(Class, u32)] = &[
    (Class::Original, 2),
    (Class::Truncate, 10),
    (Class::ByteFlip, 10),
    (Class::Insert, 10),
    (Class::Delete, 8),
    (Class::Splice, 10),
    (Class::InvalidUtf8, 6),
    (Class::Bom, 6),
    (Class::Nul, 6),
    (Class::LineEndings, 8),
    (Class::Unicode, 8),
    (Class::Shuffle, 6),
    (Class::LongLine, 1),
    (Class::Stacked, 12),
];

fn class_name(c: Class) -> String {
    format!("{c:?}")
}

struct Seed {
    /// Where it came from (for reports).
    origin: String,
    lang: Lang,
    /// Extension (`java`, `ts`, …).
    ext: String,
    bytes: Vec<u8>,
}

const JS_EXTS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

fn lang_of(ext: &str) -> Option<Lang> {
    if ext == "java" {
        Some(Lang::Java)
    } else if JS_EXTS.contains(&ext) {
        Some(Lang::Js)
    } else {
        None
    }
}

/// Every source file under `dir` (sorted, skipping `.git`, `node_modules` and `target`).
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if !matches!(name.as_ref(), ".git" | "node_modules" | "target") {
                walk(&p, out);
            }
        } else if ft.is_file() {
            let ext = p
                .extension()
                .map(|x| x.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if lang_of(&ext).is_some() {
                out.push(p);
            }
        }
    }
}

/// Seed files: a deterministic sample per language from `MOTE_FUZZ_SEEDS`, plus the plugins'
/// own fixtures.
fn load_seeds(seed: u64) -> (Vec<Seed>, Vec<Seed>) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = [
        manifest.join("../mote-lang-java/tests/fixtures"),
        manifest.join("../mote-lang-js/tests/fixtures"),
    ];
    let per_lang = env_u64("MOTE_FUZZ_SEEDS_PER_LANG", 3_000) as usize;
    let mut external = Vec::new();
    if let Ok(dirs) = std::env::var("MOTE_FUZZ_SEEDS") {
        for d in dirs.split(';').filter(|d| !d.trim().is_empty()) {
            walk(Path::new(d.trim()), &mut external);
        }
    }
    let mut fixture_files = Vec::new();
    for d in &fixtures {
        walk(d, &mut fixture_files);
    }
    let mut rng = Rng::new(seed ^ 0x5EED);
    let mut by_lang: BTreeMap<Lang, Vec<PathBuf>> = BTreeMap::new();
    for p in external {
        let ext = p
            .extension()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        by_lang.entry(lang_of(&ext).unwrap()).or_default().push(p);
    }
    let mut out: BTreeMap<Lang, Vec<Seed>> = BTreeMap::new();
    let read = |p: &Path, lang: Lang, out: &mut BTreeMap<Lang, Vec<Seed>>| {
        let Ok(bytes) = std::fs::read(p) else { return };
        if bytes.is_empty() || bytes.len() > 256 * 1024 {
            return;
        }
        let ext = p
            .extension()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        out.entry(lang).or_default().push(Seed {
            origin: p.to_string_lossy().replace('\\', "/"),
            lang,
            ext,
            bytes,
        });
    };
    for (lang, mut paths) in by_lang {
        // Fisher-Yates with the seeded RNG: the same sample on every run and machine.
        for i in (1..paths.len()).rev() {
            let j = rng.below(i + 1);
            paths.swap(i, j);
        }
        for p in paths.iter().take(per_lang) {
            read(p, lang, &mut out);
        }
    }
    for p in fixture_files {
        let ext = p
            .extension()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        read(&p, lang_of(&ext).unwrap(), &mut out);
    }
    (
        out.remove(&Lang::Java).unwrap_or_default(),
        out.remove(&Lang::Js).unwrap_or_default(),
    )
}

/// Tokens that change the lexical structure.
const TOKENS: &[&str] = &[
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "<",
    ">",
    "\"",
    "'",
    "`",
    "${",
    "/*",
    "*/",
    "//",
    "\\",
    "\n",
    "\r",
    "\r\n",
    "@",
    "#",
    "=>",
    "->",
    ";",
    ",",
    "?.",
    "...",
    "::",
    "\"\"\"",
    "/",
    "<div>",
    "</div>",
    "/>",
    "<>",
    "</>",
    "?",
    ":",
    "=",
    "!",
    "&&",
    "||",
    "??",
    "+",
    "-",
    "*",
    "%",
    "^",
    "~",
    "class ",
    "function ",
    "return ",
    "if (",
    "else ",
    "try {",
    "catch (e) {",
    "finally {",
    "new ",
    "this.",
    "super(",
    "async ",
    "await ",
    "yield ",
    "static ",
    "enum ",
    "record ",
    "interface ",
    "@interface ",
    "import ",
    "export ",
    "default ",
    "case ",
    "switch (",
    "=>{",
    "/*/",
    "/**/",
    "/**",
    "\\u",
    "\\u{",
    "0x",
    "1e",
    ".5",
    "1_000",
    "0b",
    "n",
    "#!",
    "<!--",
    "-->",
    "@SuppressWarnings(\"all\")",
    "@SuppressWarnings(\"unchecked\")",
    "// mote-ignore[java/empty-catch-block, js/no-empty]: fuzz\n",
    "// mote-ignore: fuzz\n",
    "// mote-ignore\n",
    "/* mote-ignore-file */",
    "// mote-ignore-file[all]\n",
    "// eslint-disable-next-line\n",
    "/* eslint-disable */",
    "// eslint-disable-line no-console\n",
    "// CHECKSTYLE:OFF\n",
    "// CHECKSTYLE:ON\n",
    "// NOPMD\n",
    "//noinspection ALL\n",
    "@SuppressFBWarnings",
    "// @ts-ignore\n",
    "/* @generated */",
    "// Code generated by fuzz. DO NOT EDIT.\n",
    "/*! jQuery v3.7.1 */",
    "sourceMappingURL=",
    "'use strict';",
    "\u{feff}",
    "\u{2028}",
    "\0",
];

const UNICODE: &[&str] = &[
    "é",
    "ß",
    "中",
    "😀",
    "\u{200B}",
    "\u{200D}",
    "\u{202E}",
    "\u{0301}",
    "\u{FFFD}",
    "\u{10FFFF}",
    "𝑥",
    "\u{3000}",
    "\u{00A0}",
    "\u{FEFF}",
    "\u{2028}",
    "\u{2029}",
    "\u{0085}",
    "\u{000B}",
    "\u{000C}",
    "Ω",
    "ǅ",
    "ﬀ",
    "İ",
    "ı",
    "\u{E000}",
    "👨‍👩‍👧",
    "🏳️‍🌈",
];

const INVALID_UTF8: &[&[u8]] = &[
    &[0x80],
    &[0xBF],
    &[0xC0, 0x80],
    &[0xC1, 0xBF],
    &[0xE0, 0x80, 0x80],
    &[0xED, 0xA0, 0x80],
    &[0xED, 0xBF, 0xBF],
    &[0xF4, 0x90, 0x80, 0x80],
    &[0xF8, 0x88, 0x80, 0x80, 0x80],
    &[0xFE],
    &[0xFF],
    &[0xE2, 0x82],
    &[0xC3],
    &[0xF0, 0x9F, 0x98],
];

fn insert_at(bytes: &mut Vec<u8>, pos: usize, what: &[u8]) {
    let pos = pos.min(bytes.len());
    bytes.splice(pos..pos, what.iter().copied());
}

/// Offsets of the line starts of `bytes` (by `\n`).
fn line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut v = vec![0];
    v.extend(
        bytes
            .iter()
            .enumerate()
            .filter(|(_, &b)| b == b'\n')
            .map(|(i, _)| i + 1),
    );
    v
}

fn mutate_once(class: Class, bytes: &mut Vec<u8>, rng: &mut Rng, seeds: &[&[Seed]]) {
    let len = bytes.len();
    match class {
        Class::Original | Class::Stacked => {}
        Class::Truncate => {
            let at = rng.below(len + 1);
            bytes.truncate(at);
        }
        Class::ByteFlip => {
            for _ in 0..rng.range(1, 8) {
                if bytes.is_empty() {
                    break;
                }
                let i = rng.below(bytes.len());
                if rng.chance(1, 2) {
                    bytes[i] ^= 1 << rng.below(8);
                } else {
                    bytes[i] = rng.next() as u8;
                }
            }
        }
        Class::Insert => {
            for _ in 0..rng.range(1, 8) {
                let pos = rng.below(bytes.len() + 1);
                if rng.chance(1, 4) {
                    let b = rng.next() as u8;
                    insert_at(bytes, pos, &[b]);
                } else {
                    let t = *rng.pick(TOKENS);
                    let reps = if rng.chance(1, 8) {
                        rng.range(2, 64)
                    } else {
                        1
                    };
                    insert_at(bytes, pos, t.repeat(reps).as_bytes());
                }
            }
        }
        Class::Delete => {
            for _ in 0..rng.range(1, 4) {
                if bytes.is_empty() {
                    break;
                }
                let start = rng.below(bytes.len());
                let max = if rng.chance(1, 4) {
                    bytes.len() / 4 + 1
                } else {
                    64
                };
                let n = rng.range(1, max).min(bytes.len() - start);
                bytes.drain(start..start + n);
            }
        }
        Class::Splice => {
            let pool = seeds[rng.below(seeds.len())];
            if pool.is_empty() {
                return;
            }
            let donor = &pool[rng.below(pool.len())].bytes;
            let (from, to) = if rng.chance(1, 2) {
                // Whole lines.
                let starts = line_starts(donor);
                let a = rng.below(starts.len());
                let b = (a + rng.range(1, 40)).min(starts.len() - 1);
                (starts[a], starts[b].max(starts[a]))
            } else {
                let a = rng.below(donor.len());
                let n = rng.range(1, 400).min(donor.len() - a);
                (a, a + n)
            };
            let chunk = donor[from..to].to_vec();
            let pos = if rng.chance(1, 2) {
                let starts = line_starts(bytes);
                starts[rng.below(starts.len())]
            } else {
                rng.below(bytes.len() + 1)
            };
            if rng.chance(1, 3) && pos < bytes.len() {
                // Replace instead of insert.
                let end = (pos + chunk.len()).min(bytes.len());
                bytes.splice(pos..end, chunk);
            } else {
                insert_at(bytes, pos, &chunk);
            }
        }
        Class::InvalidUtf8 => {
            for _ in 0..rng.range(1, 6) {
                let seq = *rng.pick(INVALID_UTF8);
                let pos = if rng.chance(1, 8) {
                    bytes.len()
                } else {
                    rng.below(bytes.len() + 1)
                };
                insert_at(bytes, pos, seq);
            }
        }
        Class::Bom => match rng.below(7) {
            0 => insert_at(bytes, 0, &[0xEF, 0xBB, 0xBF]),
            1 => insert_at(bytes, 0, &[0xEF, 0xBB, 0xBF, 0xEF, 0xBB, 0xBF]),
            2 => {
                let pos = rng.below(bytes.len() + 1);
                insert_at(bytes, pos, &[0xEF, 0xBB, 0xBF]);
            }
            3 | 4 => {
                // Re-encode as UTF-16 with a BOM (little or big endian), sometimes odd-sized.
                let text = String::from_utf8_lossy(bytes).into_owned();
                let le = rng.chance(1, 2);
                let mut out = if le {
                    vec![0xFF, 0xFE]
                } else {
                    vec![0xFE, 0xFF]
                };
                for u in text.encode_utf16() {
                    out.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
                }
                if rng.chance(1, 4) {
                    out.pop();
                }
                if rng.chance(1, 4) && out.len() > 4 {
                    // A lone surrogate.
                    let at = 2 + 2 * rng.below((out.len() - 2) / 2);
                    let s: u16 = 0xD800 + rng.below(0x800) as u16;
                    let b = if le { s.to_le_bytes() } else { s.to_be_bytes() };
                    if at + 1 < out.len() {
                        out[at] = b[0];
                        out[at + 1] = b[1];
                    }
                }
                *bytes = out;
            }
            5 => insert_at(bytes, 0, &[0xFF, 0xFE]),
            _ => insert_at(bytes, 0, &[0xFE, 0xFF]),
        },
        Class::Nul => {
            if rng.chance(1, 2) {
                // Past the binary check's window, so the NULs reach the plugin.
                let pad = "// padding padding padding padding padding padding padding padding\n"
                    .repeat(130);
                insert_at(bytes, 0, pad.as_bytes());
                let lo = pad.len();
                for _ in 0..rng.range(1, 8) {
                    let pos = lo + rng.below(bytes.len() - lo + 1);
                    insert_at(bytes, pos, &[0]);
                }
            } else {
                for _ in 0..rng.range(1, 4) {
                    let pos = rng.below(bytes.len() + 1);
                    insert_at(bytes, pos, &[0]);
                }
            }
        }
        Class::LineEndings => match rng.below(6) {
            0 => *bytes = replace_newlines(bytes, |_| b"\r\n"),
            1 => *bytes = replace_newlines(bytes, |_| b"\r"),
            2 => {
                let mut r = Rng::new(rng.next());
                let choices: [&[u8]; 5] = [b"\n", b"\r\n", b"\r", b"\n\r", b"\r\r\n"];
                *bytes = replace_newlines(bytes, |_| choices[r.below(choices.len())]);
            }
            3 => {
                let seps = [
                    "\u{2028}", "\u{2029}", "\u{0085}", "\u{000B}", "\u{000C}", "\r",
                ];
                for _ in 0..rng.range(1, 10) {
                    let pos = rng.below(bytes.len() + 1);
                    insert_at(bytes, pos, rng.pick(&seps).as_bytes());
                }
            }
            4 => *bytes = replace_newlines(bytes, |_| "\u{2028}".as_bytes()),
            _ => *bytes = replace_newlines(bytes, |_| b" "),
        },
        Class::Unicode => {
            for _ in 0..rng.range(1, 10) {
                let pos = rng.below(bytes.len() + 1);
                let u = *rng.pick(UNICODE);
                let reps = if rng.chance(1, 10) {
                    rng.range(2, 200)
                } else {
                    1
                };
                insert_at(bytes, pos, u.repeat(reps).as_bytes());
            }
        }
        Class::Shuffle => {
            let starts = line_starts(bytes);
            if starts.len() < 3 {
                return;
            }
            let a = rng.below(starts.len() - 1);
            let b = (a + rng.range(1, 30)).min(starts.len() - 1);
            let block = bytes[starts[a]..starts[b]].to_vec();
            match rng.below(3) {
                0 => {
                    // Duplicate the block.
                    let reps = rng.range(1, 20);
                    let pos = starts[b];
                    insert_at(bytes, pos, &block.repeat(reps));
                }
                1 => {
                    // Move it elsewhere.
                    bytes.drain(starts[a]..starts[b]);
                    let starts = line_starts(bytes);
                    let pos = starts[rng.below(starts.len())];
                    insert_at(bytes, pos, &block);
                }
                _ => {
                    // Reverse its lines.
                    let text = String::from_utf8_lossy(&block).into_owned();
                    let rev: Vec<&str> = text.split_inclusive('\n').rev().collect();
                    bytes.splice(starts[a]..starts[b], rev.concat().into_bytes());
                }
            }
        }
        Class::LongLine => {
            const MB: usize = 1 << 20;
            match rng.below(7) {
                0 => {
                    // The whole file on one line, repeated to a megabyte.
                    let one = replace_newlines(bytes, |_| b" ");
                    if one.is_empty() {
                        *bytes = vec![b' '; MB];
                    } else {
                        let mut out = Vec::with_capacity(MB + one.len());
                        while out.len() < MB {
                            out.extend_from_slice(&one);
                            out.push(b' ');
                        }
                        *bytes = out;
                    }
                }
                k => {
                    let filler: Vec<u8> = match k {
                        1 => [b"\"".as_slice(), &vec![b'a'; MB], b"\""].concat(),
                        2 => vec![b'x'; MB],
                        3 => [b"// ".as_slice(), &vec![b'c'; MB], b"\n"].concat(),
                        4 => [b"/* ".as_slice(), &vec![b'c'; MB], b" */"].concat(),
                        5 => "a;".repeat(MB / 2).into_bytes(),
                        _ => vec![b' '; MB],
                    };
                    let starts = line_starts(bytes);
                    let pos = if rng.chance(1, 2) {
                        starts[rng.below(starts.len())]
                    } else {
                        rng.below(bytes.len() + 1)
                    };
                    insert_at(bytes, pos, &filler);
                }
            }
        }
    }
}

fn replace_newlines<'a>(bytes: &[u8], mut with: impl FnMut(usize) -> &'a [u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 16);
    let mut i = 0;
    let mut n = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            out.extend_from_slice(with(n));
            n += 1;
            i += 2;
        } else if bytes[i] == b'\n' || bytes[i] == b'\r' {
            out.extend_from_slice(with(n));
            n += 1;
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// One generated input.
struct Case {
    class: Class,
    lang: Lang,
    origin: String,
    /// Repo-relative path the file is analysed under.
    path: String,
    bytes: Vec<u8>,
    is_test: bool,
    /// Which fuzz configuration (`fuzz_yaml(variant == 1)`).
    variant: usize,
    java_release: Option<u32>,
}

struct Fuzz {
    seed: u64,
    java: Vec<Seed>,
    js: Vec<Seed>,
    registry: Arc<Registry>,
    java_plugin: usize,
    js_plugin: usize,
    configs: [CompiledConfig; 2],
    settings: [AnalysisSettings; 2],
    class_table: Vec<Class>,
}

impl Fuzz {
    fn load(seed: u64) -> Fuzz {
        let (java, js) = load_seeds(seed);
        assert!(!java.is_empty() && !js.is_empty(), "no seed files");
        let registry = mote_cli::plugins::registry().expect("registry");
        let java_plugin = registry.plugin_index("java").expect("java plugin");
        let js_plugin = registry.plugin_index("js").expect("js plugin");
        let compile = |skip: bool| {
            let config = Config::from_yaml_str(&fuzz_yaml(skip)).expect("fuzz config");
            CompiledConfig::new(config, &registry).expect("compile config")
        };
        let configs = [compile(false), compile(true)];
        let settings = [
            AnalysisSettings::from_config(configs[0].config()),
            AnalysisSettings::from_config(configs[1].config()),
        ];
        let class_table = CLASSES
            .iter()
            .flat_map(|&(c, w)| std::iter::repeat_n(c, w as usize))
            .collect();
        Fuzz {
            seed,
            java,
            js,
            registry,
            java_plugin,
            js_plugin,
            configs,
            settings,
            class_table,
        }
    }

    fn case(&self, iter: u64) -> Case {
        let mut rng = Rng::new(self.seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ iter);
        let lang = if iter % 2 == 0 { Lang::Java } else { Lang::Js };
        let class = self.class_table[((iter / 2) as usize) % self.class_table.len()];
        let pool = match lang {
            Lang::Java => &self.java,
            Lang::Js => &self.js,
        };
        let seed = &pool[rng.below(pool.len())];
        let mut bytes = seed.bytes.clone();
        let both: [&[Seed]; 2] = [&self.java, &self.js];
        let same: [&[Seed]; 2] = [pool, pool];
        let donors: &[&[Seed]] = if rng.chance(1, 4) { &both } else { &same };
        if class == Class::Stacked {
            const CHEAP: &[Class] = &[
                Class::Truncate,
                Class::ByteFlip,
                Class::Insert,
                Class::Delete,
                Class::Splice,
                Class::InvalidUtf8,
                Class::LineEndings,
                Class::Unicode,
                Class::Shuffle,
                Class::Insert,
                Class::Splice,
            ];
            for _ in 0..rng.range(2, 5) {
                let c = *rng.pick(CHEAP);
                mutate_once(c, &mut bytes, &mut rng, donors);
            }
        } else {
            mutate_once(class, &mut bytes, &mut rng, donors);
        }
        let is_test = rng.chance(1, 4);
        let variant = rng.below(2);
        let (path, java_release) = match lang {
            Lang::Java => {
                let dir = if is_test {
                    "src/test/java"
                } else {
                    "src/main/java"
                };
                let release = *rng.pick(&[None, Some(8), Some(11), Some(17), Some(21), Some(25)]);
                (format!("m/{dir}/p/F{iter}.java"), release)
            }
            Lang::Js => {
                // Usually the seed's own extension; sometimes another dialect.
                let ext = if rng.chance(1, 5) {
                    rng.pick(JS_EXTS).to_string()
                } else {
                    seed.ext.clone()
                };
                let dir = if is_test { "src/__tests__" } else { "src" };
                (format!("w/{dir}/f{iter}.{ext}"), None)
            }
        };
        Case {
            class,
            lang: seed.lang,
            origin: seed.origin.clone(),
            path,
            bytes,
            is_test,
            variant,
            java_release,
        }
    }

    fn analyze(&self, case: &Case) -> FileOutcome {
        let plugin = match case.lang {
            Lang::Java => self.java_plugin,
            Lang::Js => self.js_plugin,
        };
        let cc = &self.configs[case.variant];
        let rules = cc.resolve_with_release(
            &self.registry,
            plugin,
            case.is_test,
            &case.path,
            case.java_release,
        );
        let settings = &self.settings[case.variant];
        let job = FileJob {
            registry: &self.registry,
            plugin,
            path: &case.path,
            is_test: case.is_test,
            rules: &rules,
            settings,
            skip_generated: cc.skips_generated(&case.path),
            packages: &[],
            keep_tokens: true,
        };
        analyze_bytes(&job, &case.bytes)
    }
}

thread_local! {
    /// The last panic on this thread (message and location), recorded by the fuzz panic hook.
    static LAST_PANIC: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn install_panic_recorder() {
    std::panic::set_hook(Box::new(|info| {
        let text = info.to_string().replace('\n', " ");
        LAST_PANIC.with(|p| *p.borrow_mut() = Some(text));
    }));
}

fn take_panic() -> Option<String> {
    LAST_PANIC.with(|p| p.borrow_mut().take())
}

/// Problems with one result: internal errors and invalid ranges.
fn check_findings(findings: &[Finding], lines: u32, issues: &mut Vec<(String, String)>) {
    for f in findings {
        if f.rule_id == INTERNAL_ERROR {
            issues.push(("internal-error".into(), f.message.clone()));
        }
        let r = &f.range;
        let bad = r.start_line == 0
            || r.start_col == 0
            || r.end_line < r.start_line
            || (r.end_line == r.start_line && r.end_col < r.start_col)
            || r.start_line > lines.max(1)
            || r.end_line > lines.max(1) + 1;
        if bad {
            issues.push((
                "bad-range".into(),
                format!(
                    "{} {}:{}-{}:{} in a {lines}-line file",
                    f.rule_id, r.start_line, r.start_col, r.end_line, r.end_col
                ),
            ));
        }
    }
}

const WORKER_ENV: &str = "MOTE_FUZZ_WORKER";
const SKIP_FILE_ENV: &str = "MOTE_FUZZ_SKIP_FILE";
/// Iterations that crashed or hung (also in the skip file): kept out of the scan sample too.
const BAD_FILE_ENV: &str = "MOTE_FUZZ_BAD_FILE";

fn read_ids(env: &str) -> BTreeSet<u64> {
    std::env::var(env)
        .ok()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| {
            s.split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Run `f` on every item on `threads` threads with the stack of the engine's analysis threads.
fn par_for_each<T: Sync>(items: &[T], threads: usize, f: impl Fn(&T) + Sync) {
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            std::thread::Builder::new()
                .stack_size(mote_engine::ANALYSIS_STACK_BYTES)
                .spawn_scoped(s, || loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(item) = items.get(k) else { break };
                    f(item);
                })
                .expect("spawn");
        }
    });
}

/// Child process: analyse iterations `start..end` (minus the skip list) and report on stdout:
/// `B <i>` when an input starts, `E <i> <class> <lang> <micros> <bytes>` when it ends, `I <kind>
/// <i> <detail>` per problem, `S <check> <detail>` per scan check, and `DONE` at the end.
#[test]
#[ignore = "child process of fuzz_mutations"]
fn fuzz_worker() {
    let Ok(range) = std::env::var(WORKER_ENV) else {
        return;
    };
    let (start, end) = range.split_once("..").expect("start..end");
    let (start, end): (u64, u64) = (start.parse().unwrap(), end.parse().unwrap());
    let skip = read_ids(SKIP_FILE_ENV);
    let seed = env_u64("MOTE_FUZZ_SEED", 1);
    let threads = env_u64(
        "MOTE_FUZZ_THREADS",
        std::thread::available_parallelism().map_or(4, |n| n.get() as u64),
    ) as usize;
    let det_every = env_u64("MOTE_FUZZ_DET_EVERY", 3).max(1);
    let fuzz = Fuzz::load(seed);
    install_panic_recorder();
    let out = std::sync::Mutex::new(std::io::stdout());
    let say = |line: String| {
        let mut o = out.lock().unwrap_or_else(|p| p.into_inner());
        let _ = writeln!(o, "{line}");
        let _ = o.flush();
    };
    // libtest prints `test fuzz_worker ... ` without a line break before the test's output:
    // give it a line of its own.
    say("START".into());
    let iters: Vec<u64> = (start..end).filter(|i| !skip.contains(i)).collect();
    par_for_each(&iters, threads, |&i| {
        say(format!("B {i}"));
        let case = fuzz.case(i);
        take_panic();
        let t = Instant::now();
        let outcome = fuzz.analyze(&case);
        let elapsed = t.elapsed();
        let mut issues: Vec<(String, String)> = Vec::new();
        if let FileOutcome::Analyzed(r) = &outcome {
            let lines = r.measures.lines;
            check_findings(&r.findings, lines, &mut issues);
            if r.internal_error && !issues.iter().any(|(k, _)| k == "internal-error") {
                issues.push(("internal-error".into(), "internal_error flag".into()));
            }
        }
        if let Some(p) = take_panic() {
            issues.push(("panic".into(), p));
        }
        if is_slow(elapsed, case.bytes.len()) {
            issues.push((
                "slow".into(),
                format!("{} ms for {} bytes", elapsed.as_millis(), case.bytes.len()),
            ));
        }
        if i % det_every == 0 {
            let again = fuzz.analyze(&case);
            if mote_engine::cache::encode(&[0; 16], &outcome)
                != mote_engine::cache::encode(&[0; 16], &again)
            {
                issues.push(("nondeterministic".into(), "second analysis differs".into()));
            }
        }
        for (kind, detail) in issues {
            say(format!(
                "I {kind} {i} {} {} {} <- {} :: {detail}",
                class_name(case.class),
                case.lang.name(),
                case.path,
                case.origin
            ));
        }
        say(format!(
            "E {i} {} {} {} {}",
            class_name(case.class),
            case.lang.name(),
            elapsed.as_micros(),
            case.bytes.len()
        ));
    });
    let sample = env_u64("MOTE_FUZZ_SCAN_SAMPLE", 300);
    if sample > 0 {
        // The first inputs of the chunk, also when this run only finishes it after a crash.
        let bad = read_ids(BAD_FILE_ENV);
        let ids: Vec<u64> = (start..end)
            .filter(|i| !bad.contains(i))
            .take(sample as usize)
            .collect();
        for line in scan_checks(&fuzz, &ids, threads) {
            say(line);
        }
    }
    say("DONE".into());
}

/// A hash of the parts of a result that must not depend on threads or the cache (everything
/// but the run statistics; a cached file counts as analyzed), with the file and finding counts.
fn digest(mut r: mote_engine::ScanResult) -> (u64, usize, usize) {
    use mote_engine::result::FileStatus;
    use std::hash::{Hash, Hasher};
    for f in &mut r.files {
        if f.status == FileStatus::Cached {
            f.status = FileStatus::Analyzed;
        }
    }
    let bytes = serde_json::to_vec(&(
        &r.findings,
        &r.files,
        &r.duplications,
        &r.measures,
        &r.grades,
        &r.gate,
        &r.diagnostics,
        &r.new_code,
        &r.source,
    ))
    .expect("json");
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    (h.finish(), r.files.len(), r.findings.len())
}

/// Write `ids`' inputs into a directory and scan it on 1 and on N threads, without a cache and
/// with a cold and a warm cache: all four results must agree.
fn scan_checks(fuzz: &Fuzz, ids: &[u64], threads: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let Some(&first) = ids.first() else {
        return lines;
    };
    let dir = workdir().join(format!("scan-{}-{first}", fuzz.seed));
    let _ = std::fs::remove_dir_all(&dir);
    for &i in ids {
        let case = fuzz.case(i);
        let p = dir.join(&case.path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, &case.bytes).unwrap();
    }
    std::fs::write(dir.join(".mote.yaml"), fuzz_yaml(false)).unwrap();
    let config = Config::from_yaml_str(&fuzz_yaml(false)).unwrap();
    let scanner = Scanner::new(fuzz.registry.clone(), config, &dir).expect("scanner");
    let cache_dir = dir.join(".cache");
    let run = |threads: usize, cache: bool| {
        let opts = ScanOptions {
            source: SourceSpec::Directory,
            threads: Some(threads),
            cache: Some(cache),
            cache_dir: Some(cache_dir.clone()),
            ..ScanOptions::default()
        };
        let t = Instant::now();
        let r = scanner.scan(&opts).expect("scan");
        let internal = r
            .findings
            .iter()
            .filter(|f| f.rule_id == INTERNAL_ERROR)
            .count();
        let (hash, files, findings) = digest(r);
        (hash, (files, findings, internal), t.elapsed())
    };
    let (one, (files, findings, internal), t1) = run(1, false);
    let (many, _, tn) = run(threads, false);
    let (cold, _, _) = run(threads, true);
    let (warm, _, _) = run(1, true);
    lines.push(format!(
        "S scan {first} files={files} findings={findings} internal={internal} t1={}ms tN={}ms",
        t1.as_millis(),
        tn.as_millis()
    ));
    for (name, other) in [
        ("threads", many),
        ("cold-cache", cold),
        ("warm-cache", warm),
    ] {
        if other != one {
            lines.push(format!(
                "I scan-{name} {first} - - {} <- batch :: results differ from the 1-thread scan",
                dir.display()
            ));
        }
    }
    if internal > 0 {
        lines.push(format!(
            "I scan-internal-error {first} - - {} <- batch :: {internal} internal errors",
            dir.display()
        ));
    }
    let failed = lines.iter().any(|l| l.starts_with("I "));
    if !failed {
        let _ = std::fs::remove_dir_all(&dir);
    }
    lines
}

#[derive(Default)]
struct Totals {
    files: BTreeMap<(String, String), (u64, u64, u64)>, // (class, lang) -> (count, micros, bytes)
    slowest: Vec<(u64, String)>,                        // (micros, description)
    issues: Vec<String>,
    scans: Vec<String>,
    done: BTreeSet<u64>,
}

/// Run one chunk in a child process; returns the iterations that crashed or hung.
fn run_chunk(
    start: u64,
    end: u64,
    skip: &BTreeSet<u64>,
    bad: &BTreeSet<u64>,
    totals: &mut Totals,
) -> Vec<u64> {
    let work = workdir();
    let skip_file = work.join(format!("skip-{start}.txt"));
    let bad_file = work.join(format!("bad-{start}.txt"));
    let list = |s: &BTreeSet<u64>| s.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
    std::fs::write(&skip_file, list(skip)).unwrap();
    std::fs::write(&bad_file, list(bad)).unwrap();
    let stderr = std::fs::File::create(work.join(format!("worker-{start}.stderr"))).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["fuzz_worker", "--exact", "--ignored", "--nocapture"])
        .args(["--test-threads", "1"])
        .env(WORKER_ENV, format!("{start}..{end}"))
        .env(SKIP_FILE_ENV, &skip_file)
        .env(BAD_FILE_ENV, &bad_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn worker");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let hang = Duration::from_secs(env_u64("MOTE_FUZZ_HANG_SECS", 120));
    let mut in_flight: BTreeMap<u64, Instant> = BTreeMap::new();
    let mut hung = Vec::new();
    let mut done_marker = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                let line = line
                    .strip_prefix("test fuzz_worker ... ")
                    .unwrap_or(&line)
                    .to_string();
                let mut parts = line.splitn(2, ' ');
                let tag = parts.next().unwrap_or("");
                let rest = parts.next().unwrap_or("");
                match tag {
                    "B" => {
                        in_flight.insert(rest.parse().unwrap(), Instant::now());
                    }
                    "E" => {
                        let f: Vec<&str> = rest.split(' ').collect();
                        let i: u64 = f[0].parse().unwrap();
                        in_flight.remove(&i);
                        totals.done.insert(i);
                        let micros: u64 = f[3].parse().unwrap();
                        let bytes: u64 = f[4].parse().unwrap();
                        let e = totals
                            .files
                            .entry((f[1].to_string(), f[2].to_string()))
                            .or_default();
                        e.0 += 1;
                        e.1 += micros;
                        e.2 += bytes;
                        totals
                            .slowest
                            .push((micros, format!("#{i} {} {} {bytes} B", f[1], f[2])));
                        if totals.slowest.len() > 64 {
                            totals.slowest.sort_by_key(|a| std::cmp::Reverse(a.0));
                            totals.slowest.truncate(16);
                        }
                    }
                    "I" => {
                        println!("ISSUE {rest}");
                        totals.issues.push(rest.to_string());
                    }
                    "S" => totals.scans.push(rest.to_string()),
                    "DONE" => done_marker = true,
                    _ => {}
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let stuck: Vec<u64> = in_flight
                    .iter()
                    .filter(|(_, t)| t.elapsed() > hang)
                    .map(|(&i, _)| i)
                    .collect();
                if !stuck.is_empty() {
                    for &i in &stuck {
                        let msg = format!("hang {i} - - - <- - :: still running after {hang:?}");
                        println!("ISSUE {msg}");
                        totals.issues.push(msg);
                    }
                    hung = stuck;
                    let _ = child.kill();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = reader.join();
    let status = child.wait().expect("wait worker");
    let _ = std::fs::remove_file(&skip_file);
    let _ = std::fs::remove_file(&bad_file);
    if !hung.is_empty() {
        return hung;
    }
    if status.success() && done_marker {
        return Vec::new();
    }
    // Aborted: the inputs in flight are suspects. Confirm each on its own.
    let suspects: Vec<u64> = in_flight.keys().copied().collect();
    println!("worker {start}..{end} died ({status}); suspects {suspects:?}");
    let mut crashed = Vec::new();
    for &i in &suspects {
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["fuzz_worker", "--exact", "--ignored", "--nocapture"])
            .env(WORKER_ENV, format!("{i}..{}", i + 1))
            .env("MOTE_FUZZ_SCAN_SAMPLE", "0")
            .env("MOTE_FUZZ_THREADS", "1")
            .output()
            .expect("spawn confirm");
        let ok = out.status.success() && String::from_utf8_lossy(&out.stdout).contains("DONE");
        if !ok {
            let msg = format!("crash {i} - - - <- - :: process died: {}", out.status);
            println!("ISSUE {msg}");
            totals.issues.push(msg);
            crashed.push(i);
        }
    }
    if crashed.is_empty() {
        let msg = format!(
            "crash {start} - - - <- - :: worker died ({status}) but no suspect reproduces alone: {suspects:?}"
        );
        println!("ISSUE {msg}");
        totals.issues.push(msg);
        crashed = suspects;
    }
    crashed
}

#[test]
#[ignore = "long-running fuzzing; see the module docs"]
fn fuzz_mutations() {
    let seed = env_u64("MOTE_FUZZ_SEED", 1);
    if let Ok(i) = std::env::var("MOTE_FUZZ_DUMP") {
        let i: u64 = i.parse().expect("iteration");
        let fuzz = Fuzz::load(seed);
        let case = fuzz.case(i);
        let p = workdir().join(format!("dump-{seed}-{i}")).join(&case.path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, &case.bytes).unwrap();
        println!(
            "{} ({:?}, {} bytes, test {}, variant {}, release {:?}, from {})",
            p.display(),
            case.class,
            case.bytes.len(),
            case.is_test,
            case.variant,
            case.java_release,
            case.origin
        );
        return;
    }
    let start = env_u64("MOTE_FUZZ_START", 0);
    let iters = env_u64("MOTE_FUZZ_ITERS", 20_000);
    let chunk = env_u64("MOTE_FUZZ_CHUNK", 10_000).max(1);
    let t0 = Instant::now();
    let mut totals = Totals::default();
    let mut bad: BTreeSet<u64> = BTreeSet::new();
    let mut lo = start;
    while lo < start + iters {
        let hi = (lo + chunk).min(start + iters);
        loop {
            let mut skip: BTreeSet<u64> = bad.clone();
            skip.extend(totals.done.range(lo..hi).copied());
            let failed = run_chunk(lo, hi, &skip, &bad, &mut totals);
            if failed.is_empty() {
                break;
            }
            bad.extend(failed);
        }
        println!(
            "chunk {lo}..{hi} done: {} files, {} issues, {:.0?}",
            totals.done.len(),
            totals.issues.len(),
            t0.elapsed()
        );
        lo = hi;
    }
    println!(
        "\n== mutation fuzzing: seed {seed}, iterations {start}..{} ==",
        start + iters
    );
    println!(
        "{:<12} {:<5} {:>8} {:>12} {:>10}",
        "class", "lang", "files", "MB", "avg ms"
    );
    let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
    for ((class, lang), (n, micros, bytes)) in &totals.files {
        *by_class.entry(class.clone()).or_default() += n;
        println!(
            "{class:<12} {lang:<5} {n:>8} {:>12.1} {:>10.2}",
            *bytes as f64 / 1e6,
            *micros as f64 / 1e3 / (*n).max(1) as f64
        );
    }
    println!("per class: {by_class:?}");
    println!("total files: {}", totals.done.len());
    totals.slowest.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (micros, d) in totals.slowest.iter().take(10) {
        println!("slowest: {:.1} ms {d}", *micros as f64 / 1e3);
    }
    for s in &totals.scans {
        println!("scan check: {s}");
    }
    println!("wall: {:.1?}", t0.elapsed());
    for i in &totals.issues {
        println!("ISSUE {i}");
    }
    assert!(
        totals.issues.is_empty(),
        "{} issues (see above)",
        totals.issues.len()
    );
}

// ---------------------------------------------------------------------------------------------
// Deep nesting through the real binary
// ---------------------------------------------------------------------------------------------

/// A nesting shape: `make(n)` returns a file nested `n` levels (or with `n` links).
struct Shape {
    name: &'static str,
    ext: &'static str,
    make: fn(usize) -> String,
}

fn rep(s: &str, n: usize) -> String {
    s.repeat(n)
}

fn java(body: String) -> String {
    format!("package p;\n\nclass A {{\n{body}\n}}\n")
}

fn jm(stmts: String) -> String {
    java(format!(
        "  int m(boolean b, int a, Object o, String s) {{\n{stmts}\n    return 0;\n  }}"
    ))
}

fn js(body: String) -> String {
    format!("function f(a, b, c) {{\n{body}\n}}\nexport {{ f }};\n")
}

fn shapes() -> Vec<Shape> {
    macro_rules! s {
        ($name:expr, $ext:expr, $f:expr) => {
            Shape {
                name: $name,
                ext: $ext,
                make: $f,
            }
        };
    }
    vec![
        // ---- Java ----
        s!("java-parens", "java", |n| java(format!(
            "  int x = {}1{};",
            rep("(", n),
            rep(")", n)
        ))),
        s!("java-unary", "java", |n| java(format!(
            "  int x = {}1;",
            rep("- ~", n)
        ))),
        s!("java-not", "java", |n| java(format!(
            "  boolean x = {}b;",
            rep("!", n)
        ))),
        s!("java-casts", "java", |n| java(format!(
            "  Object o = {}x;",
            rep("(A) ", n)
        ))),
        s!("java-ternary-right", "java", |n| java(format!(
            "  int x = {}0;",
            rep("a ? b : ", n)
        ))),
        s!("java-ternary-left", "java", |n| java(format!(
            "  int x = {}a{};",
            rep("(", n),
            rep(" ? b : c)", n)
        ))),
        s!("java-plus-chain", "java", |n| java(format!(
            "  int x = a{};",
            rep(" + a", n)
        ))),
        s!("java-mul-chain", "java", |n| java(format!(
            "  long x = a{};",
            rep(" * 1000", n)
        ))),
        s!("java-const-chain", "java", |n| java(format!(
            "  int x = 1{};",
            rep(" + 1", n)
        ))),
        s!("java-string-concat", "java", |n| java(format!(
            "  String s = \"a\"{};",
            rep(" + \"a\"", n)
        ))),
        s!("java-string-var-concat", "java", |n| jm(format!(
            "    String t = s{};",
            rep(" + s + \"x\"", n)
        ))),
        s!("java-log-concat", "java", |n| jm(format!(
            "    log.debug(\"a\"{});",
            rep(" + s", n)
        ))),
        s!("java-and-chain", "java", |n| java(format!(
            "  boolean x = b{};",
            rep(" && o instanceof A", n)
        ))),
        s!("java-or-eq-chain", "java", |n| java(format!(
            "  boolean x = a == 1{};",
            rep(" || a == 1", n)
        ))),
        s!("java-right-plus", "java", |n| java(format!(
            "  int x = {}a{};",
            rep("(a + ", n),
            rep(")", n)
        ))),
        s!("java-assign-chain", "java", |n| jm(format!(
            "    a = {}1;",
            rep("a = ", n)
        ))),
        s!("java-method-chain", "java", |n| java(format!(
            "  Object o = a{};",
            rep(".b()", n)
        ))),
        s!("java-field-chain", "java", |n| java(format!(
            "  Object o = a{};",
            rep(".b", n)
        ))),
        s!("java-stream-chain", "java", |n| jm(format!(
            "    list.stream(){}.count();",
            rep(".map(x -> x).filter(x -> true)", n)
        ))),
        s!("java-builder-chain", "java", |n| jm(format!(
            "    new StringBuilder(){}.toString();",
            rep(".append(s)", n)
        ))),
        s!("java-calls", "java", |n| java(format!(
            "  int x = {}1{};",
            rep("f(", n),
            rep(")", n)
        ))),
        s!("java-new", "java", |n| java(format!(
            "  Object o = {}null{};",
            rep("new A(", n),
            rep(")", n)
        ))),
        s!("java-array-init", "java", |n| java(format!(
            "  int[] x = {}1{};",
            rep("{", n),
            rep("}", n)
        ))),
        s!("java-array-access", "java", |n| java(format!(
            "  int x = a{};",
            rep("[0]", n)
        ))),
        s!("java-array-dims", "java", |n| java(format!(
            "  int{} x;",
            rep("[]", n)
        ))),
        s!("java-lambdas", "java", |n| java(format!(
            "  Object o = {}null;",
            rep("x -> ", n)
        ))),
        s!("java-lambda-blocks", "java", |n| java(format!(
            "  Runnable r = {}{};",
            rep("() -> { Runnable r = ", n),
            rep("null; }", n)
        ))),
        s!("java-anon-classes", "java", |n| java(format!(
            "  Object o = {}null{};",
            rep("new Object() { Object f() { return ", n),
            rep("; } }", n)
        ))),
        s!("java-inner-classes", "java", |n| java(format!(
            "{}{}",
            rep("class B { ", n),
            rep("}", n)
        ))),
        s!("java-local-classes", "java", |n| jm(format!(
            "{}{}",
            rep("class L { void g() { ", n),
            rep("} }", n)
        ))),
        s!("java-blocks", "java", |n| jm(format!(
            "{}{}",
            rep("{", n),
            rep("}", n)
        ))),
        s!("java-if-braced", "java", |n| jm(format!(
            "{} a++; {}",
            rep("if (b) {", n),
            rep("}", n)
        ))),
        s!("java-if-bare", "java", |n| jm(format!(
            "{} a++;",
            rep("if (b) ", n)
        ))),
        s!("java-if-else-nested", "java", |n| jm(format!(
            "{} a++; {}",
            rep("if (b) { a++; } else {", n),
            rep("}", n)
        ))),
        s!("java-else-if-chain", "java", |n| jm(format!(
            "    if (a == 0) {{ a++; }}{}",
            (1..=n)
                .map(|i| format!(" else if (a == {i}) {{ a--; }}"))
                .collect::<String>()
        ))),
        s!("java-else-if-return", "java", |n| jm(format!(
            "    if (a == 0) {{ return 0; }}{} else {{ return 1; }}",
            (1..=n)
                .map(|i| format!(" else if (a == {i}) {{ return {i}; }}"))
                .collect::<String>()
        ))),
        s!("java-for", "java", |n| jm(format!(
            "{} a++; {}",
            rep("for (int i = 0; i < a; i++) {", n),
            rep("}", n)
        ))),
        s!("java-while", "java", |n| jm(format!(
            "{} a++;",
            rep("while (b) ", n)
        ))),
        s!("java-do", "java", |n| jm(format!(
            "{} a++; {}",
            rep("do {", n),
            rep("} while (b);", n)
        ))),
        s!("java-try", "java", |n| jm(format!(
            "{} a++; {}",
            rep("try {", n),
            rep("} catch (Exception e) { }", n)
        ))),
        s!("java-try-finally", "java", |n| jm(format!(
            "{} a++; {}",
            rep("try {", n),
            rep("} finally { a--; }", n)
        ))),
        s!("java-try-resources", "java", |n| jm(format!(
            "{} a++; {}",
            rep(
                "try (java.io.InputStream in = new java.io.FileInputStream(s)) {",
                n
            ),
            rep(
                "} catch (Exception e) { throw new RuntimeException(e); }",
                n
            )
        ))),
        s!("java-sync", "java", |n| jm(format!(
            "{} a++; {}",
            rep("synchronized (o) {", n),
            rep("}", n)
        ))),
        s!("java-switch", "java", |n| jm(format!(
            "{} a++; {}",
            rep("switch (a) { case 1: ", n),
            rep("}", n)
        ))),
        s!("java-switch-expr", "java", |n| java(format!(
            "  int y = {}1{};",
            rep("switch (x) { default -> ", n),
            rep("; }", n)
        ))),
        s!("java-labels", "java", |n| jm(format!(
            "{} a++;",
            rep("l: ", n)
        ))),
        s!("java-generics", "java", |n| java(format!(
            "  {}X{} x;",
            rep("List<", n),
            rep(">", n)
        ))),
        s!("java-generic-bounds", "java", |n| java(format!(
            "  <T extends {}T{}> void g() {{}}",
            rep("Comparable<", n),
            rep(">", n)
        ))),
        s!("java-wildcards", "java", |n| java(format!(
            "  {}X{} x;",
            rep("List<? extends ", n),
            rep(">", n)
        ))),
        s!("java-annotations", "java", |n| format!(
            "{}1{} class A {{}}",
            rep("@A(", n),
            rep(")", n)
        )),
        s!("java-annotation-arrays", "java", |n| format!(
            "@SuppressWarnings({}\"x\"{}) class A {{}}",
            rep("{", n),
            rep("}", n)
        )),
        s!("java-record-patterns", "java", |n| jm(format!(
            "    if (o instanceof {}var x{}) {{ a++; }}",
            rep("R(", n),
            rep(")", n)
        ))),
        s!("java-regex-groups", "java", |n| jm(format!(
            "    java.util.regex.Pattern.compile(\"{}a{}\");",
            rep("(", n),
            rep(")", n)
        ))),
        s!("java-regex-noncapture", "java", |n| jm(format!(
            "    s.matches(\"{}a{}\");",
            rep("(?:", n),
            rep(")*", n)
        ))),
        s!("java-regex-classes", "java", |n| jm(format!(
            "    s.split(\"{}a{}\");",
            rep("[", n),
            rep("]", n)
        ))),
        s!("java-regex-lookahead", "java", |n| jm(format!(
            "    s.replaceAll(\"{}a{}\", \"\");",
            rep("(?=", n),
            rep(")", n)
        ))),
        s!("java-regex-alternation", "java", |n| jm(format!(
            "    s.matches(\"a{}\");",
            rep("|a", n)
        ))),
        s!("java-regex-quantifiers", "java", |n| jm(format!(
            "    s.matches(\"(a{})\");",
            rep("+?", n)
        ))),
        s!("java-regex-backrefs", "java", |n| jm(format!(
            "    s.matches(\"(a){}\");",
            rep("\\\\1", n)
        ))),
        s!("java-regex-escapes", "java", |n| jm(format!(
            "    s.matches(\"{}\");",
            rep("\\\\Q(\\\\E", n)
        ))),
        s!("java-javadoc-tags", "java", |n| format!(
            "import java.util.List;\n/** {}x{} */\nclass A {{}}\n",
            rep("{@link ", n),
            rep("}", n)
        )),
        s!("java-javadoc-code", "java", |n| format!(
            "import java.util.List;\n/** {}x{} */\nclass A {{}}\n",
            rep("{@code {", n),
            rep("}}", n)
        )),
        s!("java-comment-braces", "java", |n| jm(format!(
            "    // {}\n    /* {} */",
            rep("{(", n),
            rep("[<", n)
        ))),
        s!("java-text-block", "java", |n| java(format!(
            "  String t = \"\"\"\n{}\"\"\";",
            rep("  ((([{\\n\n", n)
        ))),
        s!("java-string-escapes", "java", |n| java(format!(
            "  String t = \"{}\";",
            rep("\\\\\\\"\\u0041", n)
        ))),
        s!("java-many-methods", "java", |n| java(
            (0..n)
                .map(|i| format!("  void m{i}() {{ try {{ f(); }} catch (Exception e) {{ }} }}\n"))
                .collect()
        )),
        s!("java-many-fields", "java", |n| java(
            (0..n).map(|i| format!("  private int f{i};\n")).collect()
        )),
        s!("java-field-uses", "java", |n| java(format!(
            "{}  void m() {{\n{}  }}\n",
            (0..n)
                .map(|i| format!("  private int f{i};\n"))
                .collect::<String>(),
            (0..n)
                .map(|i| format!("    f{i} = f{i} + 1;\n"))
                .collect::<String>()
        ))),
        s!("java-many-locals", "java", |n| jm(format!(
            "{}{}",
            (0..n)
                .map(|i| format!("    int v{i} = a;\n"))
                .collect::<String>(),
            (0..n)
                .map(|i| format!("    a += v{i};\n"))
                .collect::<String>()
        ))),
        s!("java-many-classes", "java", |n| (0..n)
            .map(|i| format!("class C{i} {{ void m() {{ int x = {i}; }} }}\n"))
            .collect()),
        s!("java-long-method", "java", |n| jm(rep(
            "    if (a > 1) { s = s + \"x\"; } else { o = null; }\n",
            n
        ))),
        s!("java-wide-array", "java", |n| java(format!(
            "  int[] x = {{{}}};",
            rep("1, ", n)
        ))),
        s!("java-wide-switch", "java", |n| jm(format!(
            "    switch (a) {{ {} default: break; }}",
            (0..n)
                .map(|i| format!("case {i}: a++; break; "))
                .collect::<String>()
        ))),
        s!("java-wide-args", "java", |n| jm(format!(
            "    f(a{});",
            rep(", a", n)
        ))),
        s!("java-wide-params", "java", |n| java(format!(
            "  void g(int a0{}) {{}}",
            (1..=n).map(|i| format!(", int a{i}")).collect::<String>()
        ))),
        s!("java-enum-constants", "java", |n| format!(
            "enum E {{ {} }}",
            (0..n)
                .map(|i| format!("C{i} {{ void f() {{}} }}, "))
                .collect::<String>()
        )),
        s!("java-unclosed", "java", |n| format!(
            "class A {{ void m() {{ {}",
            rep("if (b) { f((", n)
        )),
        s!("java-closers", "java", |n| format!(
            "class A {{ {} }}",
            rep(")}]>", n)
        )),
        // ---- JavaScript / TypeScript ----
        s!("js-parens", "js", |n| js(format!(
            "  return {}1{};",
            rep("(", n),
            rep(")", n)
        ))),
        s!("js-arrays", "js", |n| js(format!(
            "  return {}1{};",
            rep("[", n),
            rep("]", n)
        ))),
        s!("js-objects", "js", |n| js(format!(
            "  return {}1{};",
            rep("{a: ", n),
            rep("}", n)
        ))),
        s!("js-calls", "js", |n| js(format!(
            "  return {}1{};",
            rep("g(", n),
            rep(")", n)
        ))),
        s!("js-member-chain", "js", |n| js(format!(
            "  return a{};",
            rep(".b", n)
        ))),
        s!("js-call-chain", "js", |n| js(format!(
            "  return a{};",
            rep(".b()", n)
        ))),
        s!("js-promise-chain", "js", |n| js(format!(
            "  return a{};",
            rep(".then(x => x)", n)
        ))),
        s!("js-optional-chain", "js", |n| js(format!(
            "  return a{};",
            rep("?.b", n)
        ))),
        s!("js-index-chain", "js", |n| js(format!(
            "  return a{};",
            rep("[0]", n)
        ))),
        s!("js-plus-chain", "js", |n| js(format!(
            "  return a{};",
            rep(" + a", n)
        ))),
        s!("js-string-concat", "js", |n| js(format!(
            "  return 'a'{};",
            rep(" + 'a'", n)
        ))),
        s!("js-const-chain", "js", |n| js(format!(
            "  return 1{};",
            rep(" + 1", n)
        ))),
        s!("js-and-chain", "js", |n| js(format!(
            "  return a{};",
            rep(" && b", n)
        ))),
        s!("js-or-chain", "js", |n| js(format!(
            "  if (a === 1{}) {{ g(); }}",
            rep(" || a === 1", n)
        ))),
        s!("js-nullish-chain", "js", |n| js(format!(
            "  return a{};",
            rep(" ?? b", n)
        ))),
        s!("js-compare-chain", "js", |n| js(format!(
            "  return a{};",
            rep(" < b", n)
        ))),
        s!("js-in-chain", "js", |n| js(format!(
            "  return a{};",
            rep(" instanceof B", n)
        ))),
        s!("js-right-plus", "js", |n| js(format!(
            "  return {}a{};",
            rep("(a + ", n),
            rep(")", n)
        ))),
        s!("js-exponent", "js", |n| js(format!(
            "  return a{};",
            rep(" ** a", n)
        ))),
        s!("js-comma", "js", |n| js(format!(
            "  return (a{});",
            rep(", a", n)
        ))),
        s!("js-ternary-right", "js", |n| js(format!(
            "  return {}0;",
            rep("a ? b : ", n)
        ))),
        s!("js-ternary-left", "js", |n| js(format!(
            "  return {}a{};",
            rep("(", n),
            rep(" ? b : c)", n)
        ))),
        s!("js-ternary-middle", "js", |n| js(format!(
            "  return {}0{};",
            rep("a ? ", n),
            rep(" : c", n)
        ))),
        s!("js-assign-chain", "js", |n| js(format!(
            "  a = {}1;",
            rep("a = ", n)
        ))),
        s!("js-compound-assign", "js", |n| js(format!(
            "  a += {}1;",
            rep("a += ", n)
        ))),
        s!("js-not", "js", |n| js(format!(
            "  return {}a;",
            rep("!", n)
        ))),
        s!("js-unary", "js", |n| js(format!(
            "  return {}a;",
            rep("- ", n)
        ))),
        s!("js-typeof", "js", |n| js(format!(
            "  return {}a;",
            rep("typeof ", n)
        ))),
        s!("js-void", "js", |n| js(format!(
            "  return {}a;",
            rep("void ", n)
        ))),
        s!("js-delete", "js", |n| js(format!(
            "  return {}a.b;",
            rep("delete ", n)
        ))),
        s!("js-await", "js", |n| format!(
            "async function f(a) {{ return {}a; }}\n",
            rep("await ", n)
        )),
        s!("js-yield", "js", |n| format!(
            "function* f(a) {{ return {}a; }}\n",
            rep("yield ", n)
        )),
        s!("js-new", "js", |n| js(format!(
            "  return {}A;",
            rep("new ", n)
        ))),
        s!("js-new-args", "js", |n| js(format!(
            "  return {}null{};",
            rep("new A(", n),
            rep(")", n)
        ))),
        s!("js-spread", "js", |n| js(format!(
            "  return {}a{};",
            rep("[...", n),
            rep("]", n)
        ))),
        s!("js-object-spread", "js", |n| js(format!(
            "  return {}a{};",
            rep("{...", n),
            rep("}", n)
        ))),
        s!("js-arrows", "js", |n| js(format!(
            "  return {}1;",
            rep("() => ", n)
        ))),
        s!("js-arrow-blocks", "js", |n| js(format!(
            "  return {}{};",
            rep("() => { return ", n),
            rep("1; }", n)
        ))),
        s!("js-callbacks", "js", |n| format!(
            "{}g();{}",
            rep("f(() => {", n),
            rep("});", n)
        )),
        s!("js-functions", "js", |n| format!(
            "{}g();{}",
            rep("function f() {", n),
            rep("}", n)
        )),
        s!("js-iife", "js", |n| js(format!(
            "  return {}1{};",
            rep("(function () { return ", n),
            rep("; })()", n)
        ))),
        s!("js-classes", "js", |n| format!(
            "{}{}",
            rep("class A { m() {", n),
            rep("}}", n)
        )),
        s!("js-class-extends", "js", |n| js(format!(
            "  return {}Object{};",
            rep("class extends (", n),
            rep(") {}", n)
        ))),
        s!("js-object-methods", "js", |n| js(format!(
            "  return {}1{};",
            rep("{ m() { return ", n),
            rep("; } }", n)
        ))),
        s!("js-getters", "js", |n| js(format!(
            "  return {}1{};",
            rep("{ get a() { return ", n),
            rep("; } }", n)
        ))),
        s!("js-templates", "js", |n| js(format!(
            "  return {}1{};",
            rep("`${", n),
            rep("}`", n)
        ))),
        s!("js-tagged-templates", "js", |n| js(format!(
            "  return f{};",
            rep("``", n)
        ))),
        s!("js-tagged-templates-sub", "js", |n| js(format!(
            "  return f{};",
            rep("`a${b}c`", n)
        ))),
        s!("js-template-wide", "js", |n| js(format!(
            "  return `{}`;",
            rep("${a}b", n)
        ))),
        s!("js-regex-groups", "js", |n| js(format!(
            "  return /{}a{}/.test(a);",
            rep("(", n),
            rep(")", n)
        ))),
        s!("js-regex-noncapture", "js", |n| js(format!(
            "  return /{}a{}/.test(a);",
            rep("(?:", n),
            rep(")*", n)
        ))),
        s!("js-regex-named", "js", |n| js(format!(
            "  return /{}/.test(a);",
            (0..n).map(|i| format!("(?<g{i}>a)")).collect::<String>()
        ))),
        s!("js-regex-lookaround", "js", |n| js(format!(
            "  return /{}a{}/.test(a);",
            rep("(?<=", n),
            rep(")", n)
        ))),
        s!("js-regex-class-v", "js", |n| js(format!(
            "  return /{}a{}/v.test(a);",
            rep("[", n),
            rep("]", n)
        ))),
        s!("js-regex-class-ops", "js", |n| js(format!(
            "  return /[a{}]/v.test(a);",
            rep("--[b&&c]", n)
        ))),
        s!("js-regex-alternation", "js", |n| js(format!(
            "  return /a{}/.test(a);",
            rep("|a", n)
        ))),
        s!("js-regex-quantifiers", "js", |n| js(format!(
            "  return /(a{})/.test(a);",
            rep("+?", n)
        ))),
        s!("js-regex-backrefs", "js", |n| js(format!(
            "  return /(a){}/.test(a);",
            rep("\\1", n)
        ))),
        s!("js-regex-ctor", "js", |n| js(format!(
            "  return new RegExp('{}a{}');",
            rep("(", n),
            rep(")", n)
        ))),
        s!("js-if-bare", "js", |n| js(format!(
            "{} g();",
            rep("if (a) ", n)
        ))),
        s!("js-if-braced", "js", |n| js(format!(
            "{} g(); {}",
            rep("if (a) {", n),
            rep("}", n)
        ))),
        s!("js-else-if-chain", "js", |n| js(format!(
            "  if (a === 0) {{ g(); }}{}",
            (1..=n)
                .map(|i| format!(" else if (a === {i}) {{ g(); }}"))
                .collect::<String>()
        ))),
        s!("js-else-if-return", "js", |n| js(format!(
            "  if (a === 0) {{ return 0; }}{} else {{ return 1; }}",
            (1..=n)
                .map(|i| format!(" else if (a === {i}) {{ return {i}; }}"))
                .collect::<String>()
        ))),
        s!("js-for", "js", |n| js(format!(
            "{} g();",
            rep("for (;;) ", n)
        ))),
        s!("js-for-of", "js", |n| js(format!(
            "{} g(); {}",
            rep("for (const x of a) {", n),
            rep("}", n)
        ))),
        s!("js-while", "js", |n| js(format!(
            "{} g();",
            rep("while (a) ", n)
        ))),
        s!("js-do", "js", |n| js(format!(
            "{} g(); {}",
            rep("do {", n),
            rep("} while (a);", n)
        ))),
        s!("js-try", "js", |n| js(format!(
            "{} g(); {}",
            rep("try {", n),
            rep("} catch (e) { }", n)
        ))),
        s!("js-try-finally", "js", |n| js(format!(
            "{} g(); {}",
            rep("try {", n),
            rep("} finally { h(); }", n)
        ))),
        s!("js-switch", "js", |n| js(format!(
            "{} g(); {}",
            rep("switch (a) { case 1: ", n),
            rep("}", n)
        ))),
        s!("js-labels", "js", |n| js(format!("{} g();", rep("l: ", n)))),
        s!("js-blocks", "js", |n| js(format!(
            "{}{}",
            rep("{", n),
            rep("}", n)
        ))),
        s!("js-with", "js", |n| format!("{} g();", rep("with (a) ", n))),
        s!("js-destructure-object", "js", |n| js(format!(
            "  const {}x{} = a;",
            rep("{a: ", n),
            rep("}", n)
        ))),
        s!("js-destructure-array", "js", |n| js(format!(
            "  const {}x{} = a;",
            rep("[", n),
            rep("]", n)
        ))),
        s!("js-default-params", "js", |n| format!(
            "function f(a = {}1{}) {{ return a; }}\n",
            rep("(b = ", n),
            rep(")", n)
        )),
        s!("js-jsx", "jsx", |n| js(format!(
            "  return {}{};",
            rep("<a>", n),
            rep("</a>", n)
        ))),
        s!("js-jsx-siblings", "jsx", |n| js(format!(
            "  return {}{};",
            rep("<a><br/>", n),
            rep("</a>", n)
        ))),
        s!("js-jsx-containers", "jsx", |n| js(format!(
            "  return {}{};",
            rep("<a>{", n),
            rep("}</a>", n)
        ))),
        s!("js-jsx-attrs", "jsx", |n| js(format!(
            "  return {}{};",
            rep("<a b={", n),
            rep("} />", n)
        ))),
        s!("js-jsx-cond", "tsx", |n| js(format!(
            "  return <a>{}<b/>{}</a>;",
            rep("{a && (", n),
            rep(")}", n)
        ))),
        s!("js-jsx-map", "tsx", |n| js(format!(
            "  return {}{};",
            rep("<ul>{a.map((x) => ", n),
            rep("<li/>)}</ul>", n)
        ))),
        s!("js-jsx-wide", "tsx", |n| js(format!(
            "  return <a>{}</a>;",
            rep("<b key={a} />", n)
        ))),
        s!("ts-generics", "ts", |n| format!(
            "let x: {}X{};\nfunction f() {{}}\n",
            rep("Array<", n),
            rep(">", n)
        )),
        s!("ts-conditional", "ts", |n| format!(
            "type T = {}X;\n",
            rep("A extends B ? C : ", n)
        )),
        s!("ts-conditional-check", "ts", |n| format!(
            "type T = {}X{};\n",
            rep("(A extends ", n),
            rep(" ? C : D)", n)
        )),
        s!("ts-union", "ts", |n| format!(
            "type T = A{};\nlet x: T;\n",
            rep(" | A", n)
        )),
        s!("ts-union-nested", "ts", |n| format!(
            "type T = {}A{};\n",
            rep("(A | ", n),
            rep(")", n)
        )),
        s!("ts-intersection", "ts", |n| format!(
            "type T = A{};\n",
            rep(" & B", n)
        )),
        s!("ts-object-types", "ts", |n| format!(
            "let x: {}X{};\n",
            rep("{a: ", n),
            rep("}", n)
        )),
        s!("ts-tuples", "ts", |n| format!(
            "let x: {}X{};\n",
            rep("[", n),
            rep("]", n)
        )),
        s!("ts-array-types", "ts", |n| format!(
            "let x: X{};\n",
            rep("[]", n)
        )),
        s!("ts-function-types", "ts", |n| format!(
            "let x: {}X;\n",
            rep("() => ", n)
        )),
        s!("ts-indexed-access", "ts", |n| format!(
            "type T = X{};\n",
            rep("['a']", n)
        )),
        s!("ts-keyof", "ts", |n| format!(
            "type T = {}X;\n",
            rep("keyof ", n)
        )),
        s!("ts-typeof-type", "ts", |n| format!(
            "type T = typeof a{};\n",
            rep(".b", n)
        )),
        s!("ts-mapped", "ts", |n| format!(
            "type T = {}X{};\n",
            rep("{ [K in keyof A]: ", n),
            rep(" }", n)
        )),
        s!("ts-as-chain", "ts", |n| js(format!(
            "  return a{};",
            rep(" as A", n)
        ))),
        s!("ts-satisfies-chain", "ts", |n| js(format!(
            "  return a{};",
            rep(" satisfies A", n)
        ))),
        s!("ts-non-null", "ts", |n| js(format!(
            "  return a{};",
            rep("!", n)
        ))),
        s!("ts-non-null-member", "ts", |n| js(format!(
            "  return a{};",
            rep("!.b", n)
        ))),
        s!("ts-type-assertions", "ts", |n| js(format!(
            "  return {}a;",
            rep("<A>", n)
        ))),
        s!("ts-type-args-calls", "ts", |n| js(format!(
            "  return {}1{};",
            rep("g<A>(", n),
            rep(")", n)
        ))),
        s!("ts-namespaces", "ts", |n| format!(
            "{}{}",
            rep("namespace a { ", n),
            rep("}", n)
        )),
        s!("ts-enum-wide", "ts", |n| format!(
            "enum E {{ {} }}\n",
            (0..n).map(|i| format!("A{i} = {i}, ")).collect::<String>()
        )),
        s!("ts-decorators", "ts", |n| format!(
            "{}class A {{}}\n",
            rep("@d ", n)
        )),
        s!("ts-interface-extends", "ts", |n| format!(
            "interface I extends A{} {{}}\n",
            rep(", A", n)
        )),
        s!("ts-overloads", "ts", |n| format!(
            "{}function f(a) {{ return a; }}\n",
            rep("function f(a: string): string;\n", n)
        )),
        s!("js-wide-array", "js", |n| js(format!(
            "  return [{}];",
            rep("1, ", n)
        ))),
        s!("js-wide-object", "js", |n| js(format!(
            "  return {{{}}};",
            (0..n).map(|i| format!("k{i}: {i}, ")).collect::<String>()
        ))),
        s!("js-wide-switch", "js", |n| js(format!(
            "  switch (a) {{ {} }}",
            (0..n)
                .map(|i| format!("case {i}: g(); break; "))
                .collect::<String>()
        ))),
        s!("js-many-functions", "js", |n| (0..n)
            .map(|i| format!("function f{i}() {{ try {{ g(); }} catch (e) {{}} }}\n"))
            .collect()),
        s!("js-many-locals-used", "js", |n| js(format!(
            "{}{}",
            (0..n)
                .map(|i| format!("  let v{i} = a;\n"))
                .collect::<String>(),
            (0..n)
                .map(|i| format!("  b += v{i};\n"))
                .collect::<String>()
        ))),
        s!("js-many-classes", "ts", |n| (0..n)
            .map(|i| format!("export class C{i} {{ m(): number {{ return {i}; }} }}\n"))
            .collect()),
        s!("js-many-vars", "js", |n| js((0..n)
            .map(|i| format!("  let v{i} = {i};\n"))
            .collect())),
        s!("js-many-statements", "js", |n| js(rep("  a = a + 1;\n", n))),
        s!("js-unclosed", "ts", |n| format!(
            "function f() {{ {}",
            rep("if (a) { g((", n)
        )),
        s!("js-closers", "ts", |n| rep(")}]>", n)),
        s!("js-comment-braces", "js", |n| js(format!(
            "  // {}\n  /* {} */",
            rep("{(", n),
            rep("[<", n)
        ))),
        s!("js-string-escapes", "js", |n| js(format!(
            "  return '{}';",
            rep("\\\\\\'\\u0041", n)
        ))),
        s!("js-eslint-directives", "js", |n| js(rep(
            "  // eslint-disable-next-line no-console, no-alert\n  g();\n",
            n
        ))),
        s!("js-html-comments", "js", |n| rep("<!-- a\n--> b\n", n)),
        // Many findings, many suppressions, many findings on one long line.
        s!("java-suppressed-findings", "java", |n| {
            jm(rep(
            "    try { f(); } catch (Exception e) { } // mote-ignore[java/empty-catch-block]: r\n",
            n
        ))
        }),
        s!("java-nopmd-findings", "java", |n| jm(rep(
            "    try { f(); } catch (Exception e) { } // NOPMD\n",
            n
        ))),
        s!("java-findings-one-line", "java", |n| jm(format!(
            "    {}",
            rep("try { f(); } catch (Exception e) { } ", n)
        ))),
        s!("js-suppressed-findings", "js", |n| {
            js(rep(
            "  try { g(); } catch (e) { } // eslint-disable-line no-empty\n  debugger; // mote-ignore[js/no-debugger]: r\n",
            n
        ))
        }),
        s!("js-findings-one-line", "js", |n| js(format!(
            "  {}",
            rep("try { g(); } catch (e) { } debugger; ", n)
        ))),
    ]
}

/// Nesting counts for `shape`: fixed depths, then counts that make 1 MB and `max` files.
fn depths(shape: &Shape, max: usize) -> Vec<usize> {
    let unit = ((shape.make)(1001)
        .len()
        .saturating_sub((shape.make)(1).len())
        / 1000)
        .max(1);
    let mut v = vec![10, 100, 1_000, 5_000, 20_000];
    for bytes in [1 << 20, max] {
        let n = bytes / unit;
        if n > *v.last().unwrap() {
            v.push(n);
        }
    }
    v.retain(|&n| n * unit <= max + 1024);
    v
}

/// Resident memory of process `pid` in MB (polled to stop runaway children).
fn rss_mb(pid: u32) -> Option<u64> {
    if cfg!(windows) {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let last = text.trim().rsplit("\",\"").next()?;
        let kb: u64 = last
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok()?;
        Some(kb / 1024)
    } else {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / 1024)
    }
}

struct DeepRun {
    shape: &'static str,
    n: usize,
    bytes: usize,
    /// Process wall time (a hang guard).
    elapsed: Duration,
    /// Analysis time reported by the scan (`stats.timings.map_ms`).
    map: Duration,
    problem: Option<String>,
    syntax_error: bool,
    findings: usize,
}

fn deep_run(dir: &Path, shape: &Shape, n: usize) -> DeepRun {
    let text = (shape.make)(n);
    let case_dir = dir.join(format!("{}-{n}", shape.name));
    let _ = std::fs::remove_dir_all(&case_dir);
    std::fs::create_dir_all(&case_dir).unwrap();
    std::fs::write(case_dir.join(".mote.yaml"), fuzz_yaml(false)).unwrap();
    let file = format!(
        "{}.{}",
        if shape.ext == "java" { "A" } else { "f" },
        shape.ext
    );
    std::fs::write(case_dir.join(&file), &text).unwrap();
    let t = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mote"))
        .current_dir(&case_dir)
        .args([
            "scan",
            "--no-cache",
            "--format",
            "json",
            "--threads",
            "1",
            "--no-gate",
        ])
        .env("GIT_CEILING_DIRECTORIES", dir)
        .env_remove("MOTE_CACHE_DIR")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run mote");
    let limit = Duration::from_secs(env_u64("MOTE_FUZZ_HANG_SECS", 60));
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out_t = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut s);
        s
    });
    let err_t = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut s);
        s
    });
    let max_mb = env_u64("MOTE_FUZZ_MAX_MB", 4096);
    let mut peak_mb = 0;
    let mut last_poll = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("wait") {
            break Some(s);
        }
        if last_poll.elapsed() > Duration::from_millis(250) {
            last_poll = Instant::now();
            peak_mb = peak_mb.max(rss_mb(child.id()).unwrap_or(0));
        }
        if t.elapsed() > limit || peak_mb > max_mb {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let elapsed = t.elapsed();
    let stdout = out_t.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).into_owned();
    let mut run = DeepRun {
        shape: shape.name,
        n,
        bytes: text.len(),
        elapsed,
        map: Duration::ZERO,
        problem: None,
        syntax_error: false,
        findings: 0,
    };
    let Some(status) = status else {
        run.problem = Some(if peak_mb > max_mb {
            format!("memory: killed above {max_mb} MB after {elapsed:?}")
        } else {
            format!("hang: killed after {limit:?} ({peak_mb} MB)")
        });
        return run;
    };
    if !status.success() {
        let tail: String = stderr
            .chars()
            .rev()
            .take(400)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        run.problem = Some(format!("exit {status}: {tail}"));
        return run;
    }
    match serde_json::from_slice::<serde_json::Value>(&stdout) {
        Ok(v) => {
            let findings = v["findings"].as_array().cloned().unwrap_or_default();
            run.findings = findings.len();
            run.map = Duration::from_secs_f64(
                v["stats"]["timings"]["map_ms"].as_f64().unwrap_or(0.0) / 1e3,
            );
            run.syntax_error = findings.iter().any(|f| f["rule_id"] == "mote/parse-error");
            if let Some(f) = findings.iter().find(|f| f["rule_id"] == INTERNAL_ERROR) {
                run.problem = Some(format!("internal error: {}", f["message"]));
            } else if v["files"]
                .as_array()
                .is_none_or(|f| f.iter().any(|f| f["status"] != "analyzed"))
            {
                run.problem = Some(format!("file not analyzed: {}", v["files"]));
            }
        }
        Err(e) => run.problem = Some(format!("bad JSON ({e}): {stderr}")),
    }
    if run.problem.is_none() {
        let _ = std::fs::remove_dir_all(&case_dir);
    }
    run
}

#[test]
#[ignore = "long-running; see the module docs"]
fn fuzz_deep_nesting() {
    let filter = std::env::var("MOTE_FUZZ_SHAPE").unwrap_or_default();
    let max = env_u64("MOTE_FUZZ_DEEP_MAX", 4 << 20) as usize;
    let dir = workdir().join("deep");
    std::fs::create_dir_all(&dir).unwrap();
    let all = shapes();
    let jobs: Vec<(&Shape, usize)> = all
        .iter()
        .filter(|s| s.name.contains(filter.as_str()))
        .flat_map(|s| depths(s, max).into_iter().map(move |n| (s, n)))
        .collect();
    let t0 = Instant::now();
    let parallel = env_u64("MOTE_FUZZ_PROCS", 4) as usize;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..parallel {
            s.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&(shape, n)) = jobs.get(k) else {
                    break;
                };
                let r = deep_run(&dir, shape, n);
                if let Some(p) = &r.problem {
                    println!("PROBLEM {} n={} ({} B): {p}", r.shape, r.n, r.bytes);
                }
                results.lock().unwrap().push(r);
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| (a.shape, a.n).cmp(&(b.shape, b.n)));
    let mut problems = Vec::new();
    let mut by_shape: BTreeMap<&str, Vec<&DeepRun>> = BTreeMap::new();
    for r in &results {
        by_shape.entry(r.shape).or_default().push(r);
        if let Some(p) = &r.problem {
            problems.push(format!("{} n={} ({} B): {p}", r.shape, r.n, r.bytes));
        } else if is_slow(r.map, r.bytes) {
            problems.push(format!(
                "{} n={} ({} B): slow, {} ms",
                r.shape,
                r.n,
                r.bytes,
                r.map.as_millis()
            ));
        }
    }
    println!(
        "\n== deep nesting: {} runs, {:.1?} ==",
        results.len(),
        t0.elapsed()
    );
    for (shape, runs) in &by_shape {
        let cells: Vec<String> = runs
            .iter()
            .map(|r| {
                format!(
                    "{}:{}ms({}){}{}",
                    r.n,
                    r.map.as_millis(),
                    r.elapsed.as_millis(),
                    if r.syntax_error { "/E" } else { "" },
                    if r.problem.is_some() { "/!" } else { "" }
                )
            })
            .collect();
        println!("{shape:<28} {}", cells.join(" "));
        // Superlinear growth between adjacent depths: time grows over 3x faster than the input,
        // above a noise floor.
        let ok: Vec<&&DeepRun> = runs.iter().filter(|r| r.problem.is_none()).collect();
        for w in ok.windows(2) {
            let (a, b) = (w[0], w[1]);
            let scale = if cfg!(debug_assertions) { 8 } else { 1 };
            let floor = Duration::from_millis(scale * env_u64("MOTE_FUZZ_FLOOR_MS", 200));
            let growth = b.map.as_secs_f64() / a.map.as_secs_f64().max(1e-3);
            let size = b.bytes as f64 / a.bytes as f64;
            // (The smaller run must be measurable too: a few milliseconds are mostly noise.)
            if b.map > floor && a.map > floor / 4 && growth > 3.0 * size {
                problems.push(format!(
                    "{shape}: superlinear, n={} ({} B) in {} ms vs n={} ({} B) in {} ms",
                    b.n,
                    b.bytes,
                    b.map.as_millis(),
                    a.n,
                    a.bytes,
                    a.map.as_millis()
                ));
            }
        }
    }
    for p in &problems {
        println!("PROBLEM {p}");
    }
    assert!(
        problems.is_empty(),
        "{} problems (see above)",
        problems.len()
    );
}

/// Developer helper: analyse one file (`MOTE_FUZZ_PROFILE=<path>`) with every rule of its
/// plugin and print the per-rule timings, to find the rule behind a slow input.
#[test]
#[ignore = "developer helper"]
fn fuzz_profile() {
    let Ok(spec) = std::env::var("MOTE_FUZZ_PROFILE") else {
        return;
    };
    // `shape:<name>:<n>` profiles a generated deep-nesting input.
    let (path, text) = match spec.strip_prefix("shape:") {
        Some(rest) => {
            let (name, n) = rest.rsplit_once(':').expect("shape:<name>:<n>");
            let shape = shapes()
                .into_iter()
                .find(|s| s.name == name)
                .expect("unknown shape");
            let file = if shape.ext == "java" { "A" } else { "f" };
            (
                format!("{file}.{}", shape.ext),
                (shape.make)(n.parse().expect("n")),
            )
        }
        None => (
            spec.clone(),
            std::fs::read_to_string(&spec).expect("read file"),
        ),
    };
    let file = mote_core::SourceFile {
        path: &path,
        text: &text,
        is_test: false,
    };
    let t = Instant::now();
    let findings = if path.ends_with(".java") {
        mote_lang_java::timing::set_enabled(true);
        let plugin = mote_lang_java::JavaPlugin::new();
        let rules = mote_core::testing::ruleset_all(&plugin);
        let a = mote_core::LanguagePlugin::analyze(&plugin, &file, &rules);
        println!("{}", mote_lang_java::timing::report(15));
        a.findings.len()
    } else {
        mote_lang_js::timing::set_enabled(true);
        let plugin =
            mote_lang_js::JsPlugin::new().with_analysis_stack(mote_engine::ANALYSIS_STACK_BYTES);
        let rules = mote_core::testing::ruleset_all(&plugin);
        let a = std::thread::scope(|s| {
            std::thread::Builder::new()
                .stack_size(mote_engine::ANALYSIS_STACK_BYTES)
                .spawn_scoped(s, || {
                    mote_core::LanguagePlugin::analyze(&plugin, &file, &rules)
                })
                .expect("spawn")
                .join()
                .expect("analysis panicked")
        });
        println!("{}", mote_lang_js::timing::report(15));
        a.findings.len()
    };
    println!("{path}: {findings} findings in {:.1?}", t.elapsed());
}
