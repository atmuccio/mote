//! Measures engine overhead (discovery, lexer, generic rules, CPD, cache) with a trivial Java
//! plugin that parses nothing.
//!
//! ```text
//! cargo run --release -p mote-engine --example bench -- <repo> [threads]
//! ```
//! The cache is written to a temporary directory, never into the scanned repository.

use mote_core::{FileAnalysis, LanguagePlugin, LexerSpec, RuleMeta, RuleSet, SourceFile};
use mote_engine::result::ScanResult;
use mote_engine::scan::{ScanOptions, Scanner, SourceSpec};
use mote_engine::{Config, Registry};
use std::sync::Arc;
use std::time::Instant;

struct TrivialJava;

impl LanguagePlugin for TrivialJava {
    fn id(&self) -> &'static str {
        "java"
    }
    fn display_name(&self) -> &'static str {
        "Java (trivial)"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }
    fn version(&self) -> u32 {
        1
    }
    fn rules(&self) -> Vec<&'static RuleMeta> {
        Vec::new()
    }
    fn lexer(&self) -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: true,
            template_literals: false,
            cpd_skip_line_prefixes: &["import ", "package "],
        }
    }
    fn default_test_globs(&self) -> &'static [&'static str] {
        &["**/src/test/**"]
    }
    fn analyze(&self, _file: &SourceFile<'_>, _rules: &RuleSet) -> FileAnalysis {
        FileAnalysis::default()
    }
}

fn report(label: &str, wall: f64, r: &ScanResult) {
    let t = &r.stats.timings;
    println!(
        "{label:<28} wall {wall:>7.1} ms | discovery {:>6.1} | map {:>6.1} | cpd {:>5.1} | cache write {:>6.1} | analyzed {:>5} | hits {:>5} | findings {:>5} | dup blocks {:>4}",
        t.discovery_ms,
        t.map_ms,
        t.cpd_ms,
        t.cache_write_ms,
        r.stats.files_analyzed,
        r.stats.cache_hits,
        r.findings.len(),
        r.duplications.len(),
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: bench <repo> [threads]");
        std::process::exit(2);
    };
    let threads: Option<usize> = args.next().and_then(|t| t.parse().ok());
    let registry = Arc::new(Registry::new(vec![Box::new(TrivialJava)]).expect("registry"));
    let scanner = Scanner::new(registry, Config::default(), &root).expect("scanner");
    let cache_dir = std::env::temp_dir().join("mote-bench-cache");
    let _ = std::fs::remove_dir_all(&cache_dir);

    let base = ScanOptions {
        source: SourceSpec::Auto,
        threads,
        ..Default::default()
    };
    let no_cache = ScanOptions {
        cache: Some(false),
        ..base.clone()
    };
    let with_cache = ScanOptions {
        cache: Some(true),
        cache_dir: Some(cache_dir.clone()),
        ..base
    };

    // warm the OS file cache and the process once
    let _ = scanner.scan(&no_cache).expect("scan");

    for (label, opts) in [
        ("cold, cache off", &no_cache),
        ("cold, cache off (again)", &no_cache),
        ("cold, writing cache", &with_cache),
        ("warm, cache hits", &with_cache),
        ("warm, cache hits (again)", &with_cache),
    ] {
        let t = Instant::now();
        let r = scanner.scan(opts).expect("scan");
        report(label, t.elapsed().as_secs_f64() * 1e3, &r);
    }
    let r = scanner.scan(&with_cache).expect("scan");
    let m = &r.measures.total;
    println!(
        "files {} | lines {} | ncloc {} | duplicated lines {} ({:.1}%) | threads {} | cache {}",
        m.files,
        m.lines,
        m.ncloc,
        m.duplicated_lines,
        m.duplicated_lines_density,
        r.stats.threads,
        r.stats.cache_path.as_deref().unwrap_or("-")
    );
    for (lang, s) in &r.stats.languages {
        println!(
            "  {lang}: {} files analyzed this run, {:.1} ms thread time",
            s.files, s.analyze_ms
        );
    }
    let mut by_rule = std::collections::BTreeMap::<&str, usize>::new();
    for f in &r.findings {
        *by_rule.entry(f.rule_id.as_str()).or_default() += 1;
    }
    println!("findings by rule: {by_rule:?}");
    if std::env::var_os("MOTE_BENCH_SAMPLES").is_some() {
        for rule in by_rule.keys() {
            for f in r.findings.iter().filter(|f| f.rule_id == *rule).take(4) {
                println!("  {rule} {}:{} {}", f.path, f.range.start_line, f.message);
            }
        }
    }
    drop(scanner);
    let _ = std::fs::remove_dir_all(&cache_dir);
}
