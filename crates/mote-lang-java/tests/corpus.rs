//! Real-world corpus checks (skipped when the corpus is absent).
//!
//! * `corpus_no_panics` — every rule over every `.java` file of the corpus; asserts nothing
//!   panics and prints finding counts per rule. `MOTE_SHOW=<rule-id-substring>` prints the
//!   matching findings for false-positive review.
//! * `corpus_throughput` (ignored; run in release) — single-thread and rayon throughput. With
//!   `MOTE_RULE_TIMINGS=1` it adds one profiled single-thread pass and prints the framework
//!   phases and the most expensive rules (`MOTE_JAVA_TOP=<n>` of them; default 60).
//!
//! The corpus is the directory in `MOTE_JAVA_CORPUS` (skipped when unset). The
//! plugin runs without an engine, so no Java release is known (rules gated on one stay silent)
//! unless `MOTE_JAVA_RELEASE=<n>` gives one for every file (SymmetricDS declares 21 in
//! `symmetric-assemble/common.gradle`).
//!
//! ```text
//! cargo test -p mote-lang-java --test corpus -- --nocapture
//! cargo test --release -p mote-lang-java --test corpus -- --ignored --nocapture
//! ```

use mote_core::testing::{ruleset_all, ruleset_for};
use mote_core::{FileAnalysis, LanguagePlugin, ParamValue, RuleSet, SourceFile};
use mote_lang_java::{timing, JavaPlugin, JAVA_RELEASE_PARAM};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Every rule enabled, with the release of `MOTE_JAVA_RELEASE` when set.
fn all_rules(plugin: &JavaPlugin) -> RuleSet {
    let Some(release) = std::env::var("MOTE_JAVA_RELEASE")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
    else {
        return ruleset_all(plugin);
    };
    let metas = plugin.rules();
    let ids: Vec<&str> = metas.iter().map(|m| m.id).collect();
    let params = BTreeMap::from([(JAVA_RELEASE_PARAM.to_string(), ParamValue::Int(release))]);
    ruleset_for(&metas, &ids, &params)
}

fn corpus_root() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("MOTE_JAVA_CORPUS").ok()?);
    p.is_dir().then_some(p)
}

fn java_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != ".git") {
                    stack.push(p);
                }
            } else if p.extension().is_some_and(|x| x == "java") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

struct Loaded {
    rel: String,
    text: String,
    is_test: bool,
}

fn load(root: &Path) -> Vec<Loaded> {
    java_files(root)
        .into_par_iter()
        .filter_map(|p| {
            let bytes = std::fs::read(&p).ok()?;
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let name = rel.rsplit('/').next().unwrap_or("");
            let is_test = rel.contains("/src/test/")
                || rel.starts_with("test/")
                || rel.contains("/test/")
                || name.ends_with("Test.java")
                || name.ends_with("Tests.java")
                || name.ends_with("IT.java");
            Some(Loaded { rel, text, is_test })
        })
        .collect()
}

fn analyze(plugin: &JavaPlugin, f: &Loaded, rules: &RuleSet) -> FileAnalysis {
    plugin.analyze(
        &SourceFile {
            path: &f.rel,
            text: &f.text,
            is_test: f.is_test,
        },
        rules,
    )
}

#[test]
fn corpus_no_panics() {
    let Some(root) = corpus_root() else {
        eprintln!("corpus not found; skipping");
        return;
    };
    let plugin = JavaPlugin::new();
    let rules = all_rules(&plugin);
    let files = load(&root);
    let show = std::env::var("MOTE_SHOW").ok();
    let results: Vec<(usize, Result<FileAnalysis, String>)> = files
        .par_iter()
        .enumerate()
        .map(|(i, f)| {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                analyze(&plugin, f, &rules)
            }))
            .map_err(|e| {
                e.downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            });
            (i, r)
        })
        .collect();

    let mut per_rule: BTreeMap<String, usize> = BTreeMap::new();
    for m in plugin.rules() {
        per_rule.insert(m.id.to_string(), 0);
    }
    let mut panics = Vec::new();
    let (mut syntax_errors, mut functions, mut comments, mut suppressions) =
        (0u64, 0usize, 0usize, 0usize);
    for (i, r) in &results {
        match r {
            Err(msg) => panics.push(format!("{}: {msg}", files[*i].rel)),
            Ok(a) => {
                syntax_errors += a.syntax_errors as u64;
                functions += a.functions.len();
                comments += a.comments.len();
                suppressions += a.suppressions.len();
                for f in &a.findings {
                    *per_rule.entry(f.rule_id.clone()).or_default() += 1;
                    if show.as_deref().is_some_and(|s| f.rule_id.contains(s)) {
                        println!(
                            "{}:{}:{}: {} [{}]",
                            files[*i].rel,
                            f.range.start_line,
                            f.range.start_col,
                            f.message,
                            f.symbol.as_deref().unwrap_or("-")
                        );
                    }
                }
            }
        }
    }
    println!(
        "corpus: {} files, {} functions, {} comments, {} suppressions, {} syntax errors",
        files.len(),
        functions,
        comments,
        suppressions,
        syntax_errors
    );
    for (rule, n) in &per_rule {
        println!("  {rule:40} {n}");
    }
    assert!(panics.is_empty(), "panics:\n  {}", panics.join("\n  "));
}

#[test]
#[ignore = "benchmark; run with --release -- --ignored --nocapture"]
fn corpus_throughput() {
    let Some(root) = corpus_root() else {
        eprintln!("corpus not found; skipping");
        return;
    };
    // Measure throughput without instrumentation; profile afterwards when asked to.
    let profile = timing::enabled();
    timing::set_enabled(false);
    let plugin = JavaPlugin::new();
    let all_rules = all_rules(&plugin);
    let no_rules = RuleSet::default();
    let files = load(&root);
    let lines: usize = files.iter().map(|f| f.text.lines().count()).sum();
    // Warm up (thread-local parsers, lazy tables).
    for f in files.iter().take(20) {
        let _ = analyze(&plugin, f, &all_rules);
    }

    let run_single = |rules: &RuleSet| {
        let t = Instant::now();
        let mut findings = 0usize;
        for f in &files {
            findings += analyze(&plugin, f, rules).findings.len();
        }
        (t.elapsed().as_secs_f64(), findings)
    };
    let run_par = |rules: &RuleSet| {
        let t = Instant::now();
        let findings: usize = files
            .par_iter()
            .map(|f| analyze(&plugin, f, rules).findings.len())
            .sum();
        (t.elapsed().as_secs_f64(), findings)
    };
    // Parse-only baseline through the same tree-sitter build.
    let t = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .unwrap();
    for f in &files {
        let _ = parser.parse(f.text.as_bytes(), None);
    }
    let parse_only = t.elapsed().as_secs_f64();

    let (base_s, _) = run_single(&no_rules);
    let (single_s, n1) = run_single(&all_rules);
    let (par_s, n2) = run_par(&all_rules);
    assert_eq!(n1, n2, "finding count must not depend on threading");
    let rate = |s: f64| lines as f64 / s / 1000.0;
    println!(
        "{} files, {} lines, {} findings, {} threads",
        files.len(),
        lines,
        n1,
        rayon::current_num_threads()
    );
    println!(
        "  parse only (1 thread):          {parse_only:7.3} s  {:8.0}k lines/s",
        rate(parse_only)
    );
    println!(
        "  analyze, no rules (1 thread):   {base_s:7.3} s  {:8.0}k lines/s",
        rate(base_s)
    );
    println!(
        "  analyze, all rules (1 thread):  {single_s:7.3} s  {:8.0}k lines/s  ({:.2}x parse)",
        rate(single_s),
        single_s / parse_only
    );
    println!(
        "  analyze, all rules (rayon):     {par_s:7.3} s  {:8.0}k lines/s",
        rate(par_s)
    );
    if profile {
        // One more single-thread pass with per-rule timing (adds a clock read per `check`).
        timing::set_enabled(true);
        timing::reset();
        let (s, _) = run_single(&all_rules);
        timing::set_enabled(false);
        println!("  profiled pass (1 thread):       {s:7.3} s");
        let top = std::env::var("MOTE_JAVA_TOP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        print!("{}", timing::report(top));
    }
}
