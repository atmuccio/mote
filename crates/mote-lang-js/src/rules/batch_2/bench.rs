//! Per-rule cost of this batch over a real corpus (ignored by default):
//!
//! ```text
//! cargo test -p mote-lang-js --release --lib batch_2::bench -- --ignored --nocapture
//! ```

use crate::JsPlugin;
use mote_core::testing::ruleset_all;
use mote_core::{LanguagePlugin, RuleSet, SourceFile};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name != "node_modules" && name != ".git" {
                collect(&path, out);
            }
        } else if ["ts", "tsx", "js", "mjs", "cjs", "jsx"]
            .iter()
            .any(|e| name.ends_with(&format!(".{e}")))
            && !name.ends_with(".d.ts")
        {
            out.push(path);
        }
    }
}

static NOOP: mote_core::RuleMeta = mote_core::RuleMeta::new("js/noop", "js", "Noop");

struct Noop;

impl crate::JsRule for Noop {
    fn meta(&self) -> &'static mote_core::RuleMeta {
        &NOOP
    }
}

#[test]
#[ignore = "slow: needs a corpus"]
fn batch_2_rule_costs() {
    let Ok(root) = std::env::var("MOTE_JS_CORPUS") else {
        eprintln!("MOTE_JS_CORPUS not set; skipping");
        return;
    };
    let mut paths = Vec::new();
    collect(Path::new(&root), &mut paths);
    let files: Vec<(String, String)> = paths
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            (text.len() < 1_000_000).then(|| (p.to_string_lossy().replace('\\', "/"), text))
        })
        .collect();
    let run = |plugin: &JsPlugin, rules: &RuleSet| {
        let t = Instant::now();
        for (path, text) in &files {
            let file = SourceFile {
                path,
                text,
                is_test: false,
            };
            std::hint::black_box(plugin.analyze(&file, rules));
        }
        t.elapsed().as_secs_f64()
    };
    let noop = JsPlugin::with_rules(vec![Box::new(Noop)]);
    let noop_rules = ruleset_all(&noop);
    // Interleave baseline and measured runs so machine load affects both alike.
    let cost = |plugin: &JsPlugin| {
        let rules = ruleset_all(plugin);
        let (mut base, mut with) = (f64::MAX, f64::MAX);
        for _ in 0..3 {
            base = base.min(run(&noop, &noop_rules));
            with = with.min(run(plugin, &rules));
        }
        (with - base, base)
    };
    let all = JsPlugin::with_rules(super::rules());
    let (total, base) = cost(&all);
    eprintln!("baseline (walk, no rule work) {base:.3}s");
    eprintln!("{:<32} {total:>7.3}s", "all of batch_2");
    let Ok(filter) = std::env::var("MOTE_B2_PER_RULE") else {
        return;
    };
    let names: Vec<&'static str> = super::rules().iter().map(|r| r.meta().id).collect();
    for (i, name) in names.iter().enumerate() {
        if !filter.split(',').any(|f| name.contains(f)) {
            continue;
        }
        let plugin = JsPlugin::with_rules(vec![super::rules().swap_remove(i)]);
        eprintln!("{name:<32} {:>7.3}s", cost(&plugin).0);
    }
}
