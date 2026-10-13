//! `mote fix`: apply the fixes of fixable findings to the working tree (a dry run printing a
//! unified diff by default).

use crate::cli::FixArgs;
use crate::commands::scan::{load_config, scanner_error};
use crate::util::use_color;
use crate::{exit, plugins, CliError, CliResult, Io};
use mote_engine::fix::{self, FileFix, FixOptions};
use mote_engine::{ScanOptions, Scanner, SourceSpec};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

/// Run `mote fix`. Exit codes: 0 nothing to fix or everything written, 1 a dry run found fixes to
/// apply, 2 usage or configuration error, 3 the scan failed or a file could not be read or
/// written.
pub fn run(args: &FixArgs, quiet: bool, io: &mut Io<'_>) -> CliResult {
    if args.rev.is_some() || args.staged {
        return Err(CliError::usage(
            "`mote fix` works on the working tree only: it cannot fix a revision (--rev) or the \
             staged content (--staged)",
        ));
    }
    let registry = plugins::registry()?;
    let root = args.path.clone().unwrap_or_else(|| PathBuf::from("."));
    if !root.is_dir() {
        return Err(CliError::usage(format!(
            "{}: not a directory",
            root.display()
        )));
    }
    let (config, origin) = load_config(args.config.as_deref(), &root, None)?;
    let scanner =
        Scanner::new(registry, config, &root).map_err(|e| scanner_error(e, origin.as_deref()))?;
    check_rules(&scanner, &args.rule)?;
    let options = ScanOptions {
        source: SourceSpec::Auto,
        paths: args.include.iter().map(|p| p.replace('\\', "/")).collect(),
        cache: args.no_cache.then_some(false),
        cache_dir: args.cache_dir.clone(),
        threads: args.threads.filter(|&n| n > 0),
        triage_file: args.triage.clone(),
        ..Default::default()
    };
    let result = scanner.scan(&options)?;
    let fix_opts = FixOptions {
        rules: args.rule.clone(),
        ..Default::default()
    };
    let mut paths: Vec<String> = result
        .findings
        .iter()
        .filter(|f| fix::is_selected(f, &fix_opts))
        .map(|f| f.path.clone())
        .collect();
    paths.dedup();
    if paths.is_empty() {
        if !quiet {
            let _ = writeln!(io.err, "Nothing to fix.");
        }
        return Ok(exit::OK);
    }
    let files = scanner.fix_files(&paths, &fix_opts, &options)?;
    let mut code = exit::OK;
    let changed: Vec<&FileFix> = files
        .iter()
        .filter(|f| f.changed() && f.problem.is_none())
        .collect();
    let mut written: Vec<&FileFix> = Vec::new();
    if args.write {
        for f in &changed {
            match write_back(&root, f) {
                Ok(()) => written.push(f),
                Err(e) => {
                    let _ = writeln!(io.err, "error: {}: {e}", f.path);
                    code = exit::INTERNAL;
                }
            }
        }
    } else {
        let color = use_color(args.color, io.out_is_tty);
        for f in &changed {
            let old = String::from_utf8_lossy(&f.original);
            let new = String::from_utf8_lossy(&f.fixed);
            let diff = mote_report::diff::unified(&f.path, &old, &new);
            let _ = io.out.write_all(paint_diff(&diff, color).as_bytes());
        }
        if !changed.is_empty() {
            code = exit::GATE_FAILED;
        }
    }
    for f in &files {
        if let Some(p) = &f.problem {
            if p.is_discard() || !quiet {
                let _ = writeln!(io.err, "warning: {}: {}", f.path, p.describe());
            }
            if matches!(p, fix::FixProblem::Unreadable(_)) {
                code = exit::INTERNAL;
            }
        }
    }
    if !quiet {
        let shown = if args.write { &written } else { &changed };
        let mut summary = Vec::new();
        write_summary(&mut summary, shown, &files, args.write);
        // The diff is the dry run's output: keep stdout a clean patch.
        let out: &mut dyn Write = if args.write { io.out } else { io.err };
        let _ = out.write_all(&summary);
    }
    Ok(code)
}

/// Every `--rule` must name a rule that offers fixes.
fn check_rules(scanner: &Scanner, rules: &[String]) -> Result<(), CliError> {
    let registry = scanner.registry();
    let fixable = || {
        let ids: Vec<&str> = registry
            .rule_ids()
            .into_iter()
            .filter(|id| registry.rule(id).is_some_and(|e| e.meta.fixable))
            .collect();
        ids.join(", ")
    };
    for id in rules {
        match registry.rule(id) {
            None => {
                return Err(CliError::usage(format!(
                    "unknown rule `{id}` (rules with fixes: {})",
                    fixable()
                )))
            }
            Some(e) if !e.meta.fixable => {
                return Err(CliError::usage(format!(
                    "`{id}` offers no fixes (rules with fixes: {})",
                    fixable()
                )))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Write the fixed bytes, unless the file changed since it was read.
fn write_back(root: &std::path::Path, f: &FileFix) -> Result<(), String> {
    let full = root.join(&f.path);
    let now = std::fs::read(&full).map_err(|e| format!("cannot read it again: {e}"))?;
    if now != f.original {
        return Err("changed on disk while it was being fixed; left alone".into());
    }
    std::fs::write(&full, &f.fixed).map_err(|e| format!("cannot write it: {e}"))
}

fn paint_diff(diff: &str, color: bool) -> String {
    if !color {
        return diff.to_string();
    }
    let mut out = String::with_capacity(diff.len() + 64);
    for line in diff.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let end = &line[body.len()..];
        let code = if line.starts_with("+++") || line.starts_with("---") {
            "1"
        } else if line.starts_with('+') {
            "32"
        } else if line.starts_with('-') {
            "31"
        } else if line.starts_with("@@") {
            "36"
        } else {
            ""
        };
        if code.is_empty() {
            out.push_str(line);
        } else {
            out.push_str(&format!("\x1b[{code}m{body}\x1b[0m{end}"));
        }
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Fixes per rule (and in how many files), then what was left.
fn write_summary(w: &mut dyn Write, shown: &[&FileFix], all: &[FileFix], wrote: bool) {
    let total: u32 = shown.iter().map(|f| f.applied_total()).sum();
    let mut by_rule: BTreeMap<&str, (u32, usize)> = BTreeMap::new();
    for f in shown {
        for (rule, n) in &f.applied {
            let e = by_rule.entry(rule.as_str()).or_default();
            e.0 += n;
            e.1 += 1;
        }
    }
    if total > 0 {
        let _ = writeln!(
            w,
            "{} {} in {}{}",
            if wrote { "Fixed" } else { "Would fix" },
            plural(total as usize, "finding", "findings"),
            plural(shown.len(), "file", "files"),
            if by_rule.is_empty() { "." } else { ":" }
        );
        let width = by_rule.keys().map(|r| r.len()).max().unwrap_or(0);
        for (rule, (n, files)) in &by_rule {
            let _ = writeln!(
                w,
                "  {rule:<width$}  {n:>6}  in {}",
                plural(*files, "file", "files")
            );
        }
    } else {
        let _ = writeln!(w, "Nothing to fix.");
    }
    let remaining: u32 = all
        .iter()
        .filter(|f| f.problem.is_none())
        .map(|f| f.remaining)
        .sum();
    if remaining > 0 {
        let _ = writeln!(
            w,
            "{} still offered after {} passes over a file (their edits overlap); run `mote fix` again",
            plural(remaining as usize, "fix is", "fixes are"),
            mote_core::fix::MAX_PASSES
        );
    }
    let discarded = all
        .iter()
        .filter(|f| f.problem.as_ref().is_some_and(|p| p.is_discard()))
        .count();
    if discarded > 0 {
        let _ = writeln!(
            w,
            "{} discarded: the fixes would have broken the code (please report this)",
            plural(discarded, "file's changes were", "files' changes were")
        );
    }
    if !wrote && total > 0 {
        let _ = writeln!(w, "Run `mote fix --write` to apply them.");
    }
}
