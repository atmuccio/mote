//! Robustness and throughput over a real-world corpus (any large JS/TS repository).
//!
//! ```text
//! cargo test -p mote-lang-js --release --test corpus -- --ignored --nocapture
//! MOTE_JS_CORPUS=/path/to/repo      (skipped when unset or absent)
//! MOTE_JS_SAMPLES=20                (sample findings printed per rule; default 8)
//! MOTE_JS_RULE=js/no-unused-vars (print samples for this rule only)
//! MOTE_JS_REPEAT=3                  (single-thread passes timed N times, fastest kept; default 1)
//! MOTE_RULE_TIMINGS=1               (one more profiled single-thread pass: phases, per-rule time)
//! MOTE_JS_DUMP=findings.txt         (write every finding, all fields, for before/after diffs)
//! ```
//!
//! Asserts that no file panics with every rule enabled, and prints finding counts per rule with
//! sample locations (to eyeball false positives) plus single-threaded and parallel throughput.
//! Everything runs on a pool with the engine's analysis stack, declared to the plugin as the CLI
//! does, so the numbers match a real scan.

use mote_core::testing::ruleset_all;
use mote_core::{FileAnalysis, LanguagePlugin, RuleSet, SourceFile};
use mote_lang_js::{timing, JsPlugin};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
const MAX_BYTES: usize = 1_000_000;

struct Src {
    path: String,
    text: String,
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if name != "node_modules" && name != ".git" {
                collect(&path, out);
            }
        } else if kind.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_dts =
                name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts");
            if EXTENSIONS.contains(&ext) && !is_dts {
                out.push(path);
            }
        }
    }
}

fn load(root: &Path) -> Vec<Src> {
    let mut paths = Vec::new();
    collect(root, &mut paths);
    paths
        .par_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            if text.len() > MAX_BYTES {
                return None;
            }
            let rel = p
                .strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            Some(Src { path: rel, text })
        })
        .collect()
}

fn is_test_path(path: &str) -> bool {
    let p = format!("/{path}");
    p.contains(".test.")
        || p.contains(".spec.")
        || p.contains("/test/")
        || p.contains("/tests/")
        || p.contains("/__tests__/")
        || p.contains("/e2e/")
}

fn analyze(plugin: &JsPlugin, rules: &RuleSet, src: &Src) -> Result<FileAnalysis, String> {
    let file = SourceFile {
        path: &src.path,
        text: &src.text,
        is_test: is_test_path(&src.path),
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        plugin.analyze(&file, rules)
    }))
    .map_err(|_| src.path.clone())
}

/// The engine's analysis stack (`mote_engine::ANALYSIS_STACK_BYTES`; this crate cannot depend
/// on the engine).
const ANALYSIS_STACK: usize = 256 << 20;

#[test]
#[ignore = "slow: run in release with --ignored --nocapture"]
fn corpus_robustness_and_throughput() {
    rayon::ThreadPoolBuilder::new()
        .stack_size(ANALYSIS_STACK)
        .build()
        .expect("analysis pool")
        .install(run_corpus);
}

fn run_corpus() {
    let Ok(root) = std::env::var("MOTE_JS_CORPUS") else {
        eprintln!("MOTE_JS_CORPUS not set; skipping");
        return;
    };
    let root = PathBuf::from(root);
    if !root.is_dir() {
        eprintln!("corpus {} not found; skipping", root.display());
        return;
    }
    let samples: usize = std::env::var("MOTE_JS_SAMPLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let only_rule = std::env::var("MOTE_JS_RULE").ok();
    let repeat: usize = std::env::var("MOTE_JS_REPEAT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1);
    // Measure throughput without instrumentation; profile afterwards when asked to.
    let profile = timing::enabled();
    timing::set_enabled(false);

    let srcs = load(&root);
    let lines: usize = srcs.iter().map(|s| s.text.lines().count()).sum();
    let bytes: usize = srcs.iter().map(|s| s.text.len()).sum();
    eprintln!(
        "corpus: {} files, {} lines, {:.1} MB",
        srcs.len(),
        lines,
        bytes as f64 / 1e6
    );

    let plugin = JsPlugin::new().with_analysis_stack(ANALYSIS_STACK);
    let rules = ruleset_all(&plugin);
    let empty = RuleSet::default();

    // Warm-up (page cache, allocator arenas).
    let _ = srcs.par_iter().map(|s| analyze(&plugin, &rules, s)).count();

    // Single-threaded: raw Oxc parse + semantic (the floor), the plugin with rules off (adds
    // metrics, comments, suppressions), and the plugin with all rules on. Each timed `repeat`
    // times, fastest kept (shared machines are noisy).
    let fastest = |f: &dyn Fn()| {
        (0..repeat)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed()
            })
            .min()
            .unwrap_or_default()
    };
    let oxc = fastest(&|| {
        let mut arena = oxc_allocator::Allocator::default();
        for s in &srcs {
            arena.reset();
            let st = oxc_span::SourceType::from_path(&s.path).unwrap_or_default();
            let parsed = oxc_parser::Parser::new(&arena, &s.text, st).parse();
            let semantic = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
            std::hint::black_box(semantic.semantic.nodes().len());
        }
    });
    let base = fastest(&|| {
        for s in &srcs {
            let _ = analyze(&plugin, &empty, s);
        }
    });
    // All rules on; the results of the last pass are checked below.
    let mut results: Vec<Result<FileAnalysis, String>> = Vec::new();
    let mut single = std::time::Duration::MAX;
    for _ in 0..repeat {
        let t = Instant::now();
        results = srcs.iter().map(|s| analyze(&plugin, &rules, s)).collect();
        single = single.min(t.elapsed());
    }
    // Parallel, all rules on.
    let t = Instant::now();
    let par_results: Vec<Result<FileAnalysis, String>> = srcs
        .par_iter()
        .map(|s| analyze(&plugin, &rules, s))
        .collect();
    let parallel = t.elapsed();

    let rate = |d: std::time::Duration| lines as f64 / d.as_secs_f64() / 1e6;
    eprintln!(
        "throughput: oxc parse+semantic 1 thread {:.2}s ({:.2}M lines/s) | plugin, no rules 1 thread {:.2}s ({:.2}M lines/s) | all rules 1 thread {:.2}s ({:.2}M lines/s) | all rules {} threads {:.2}s ({:.2}M lines/s)",
        oxc.as_secs_f64(),
        rate(oxc),
        base.as_secs_f64(),
        rate(base),
        single.as_secs_f64(),
        rate(single),
        rayon::current_num_threads(),
        parallel.as_secs_f64(),
        rate(parallel),
    );
    eprintln!(
        "ratio: all rules 1 thread / oxc parse+semantic = {:.2}x (no rules {:.2}x)",
        single.as_secs_f64() / oxc.as_secs_f64(),
        base.as_secs_f64() / oxc.as_secs_f64(),
    );
    if profile {
        // One more single-thread pass with per-rule timing (adds a clock read per `run`).
        timing::set_enabled(true);
        timing::reset();
        let t = Instant::now();
        for s in &srcs {
            let _ = analyze(&plugin, &rules, s);
        }
        timing::set_enabled(false);
        eprintln!(
            "profiled pass (1 thread): {:.2}s",
            t.elapsed().as_secs_f64()
        );
        eprint!(
            "{}",
            timing::report(
                std::env::var("MOTE_JS_TOP")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(60)
            )
        );
    }
    if let Ok(dump) = std::env::var("MOTE_JS_DUMP") {
        use std::fmt::Write;
        let mut out = String::new();
        for (src, result) in srcs.iter().zip(&results) {
            match result {
                Ok(a) => {
                    for f in &a.findings {
                        let _ = writeln!(out, "{} {f:?}", src.path);
                    }
                }
                Err(p) => {
                    let _ = writeln!(out, "{p} PANIC");
                }
            }
        }
        std::fs::write(&dump, out).expect("write MOTE_JS_DUMP");
        eprintln!("findings written to {dump}");
    }

    // Determinism: parallel and sequential runs agree.
    let fingerprint = |r: &Result<FileAnalysis, String>| match r {
        Ok(a) => format!("{:?}{:?}{:?}", a.findings, a.functions, a.suppressions),
        Err(p) => p.clone(),
    };
    let mismatches = results
        .iter()
        .zip(&par_results)
        .filter(|(a, b)| fingerprint(a) != fingerprint(b))
        .count();

    let mut panics = Vec::new();
    let mut per_rule: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut syntax_error_files, mut functions, mut comments, mut suppressions) =
        (0usize, 0usize, 0usize, 0usize);
    for (src, result) in srcs.iter().zip(&results) {
        match result {
            Err(path) => panics.push(path.clone()),
            Ok(a) => {
                syntax_error_files += usize::from(a.syntax_errors > 0);
                functions += a.functions.len();
                comments += a.comments.len();
                suppressions += a.suppressions.len();
                for f in &a.findings {
                    let line = src
                        .text
                        .lines()
                        .nth(f.range.start_line as usize - 1)
                        .unwrap_or("")
                        .trim();
                    let line: String = line.chars().take(100).collect();
                    per_rule.entry(f.rule_id.clone()).or_default().push(format!(
                        "{}:{}:{} {} | {}",
                        src.path, f.range.start_line, f.range.start_col, f.message, line
                    ));
                }
            }
        }
    }
    eprintln!(
        "files with syntax errors: {syntax_error_files}; functions: {functions}; comments: {comments}; suppressions: {suppressions}"
    );
    eprintln!("findings per rule:");
    for (rule, found) in &per_rule {
        eprintln!("  {rule:<24} {:>7}", found.len());
    }
    for (rule, found) in &per_rule {
        if only_rule.as_deref().is_some_and(|r| r != rule) {
            continue;
        }
        eprintln!("--- samples: {rule}");
        let step = (found.len() / samples.max(1)).max(1);
        for f in found.iter().step_by(step).take(samples) {
            eprintln!("    {f}");
        }
    }
    assert!(
        panics.is_empty(),
        "{} file(s) panicked: {:?}",
        panics.len(),
        &panics[..panics.len().min(20)]
    );
    assert_eq!(mismatches, 0, "parallel and sequential results differ");
}
