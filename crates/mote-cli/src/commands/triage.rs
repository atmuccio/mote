//! `mote triage add|list|prune`: edit the triage file (see `mote_engine::triage`).
//!
//! The file is always rewritten whole, entries sorted, so the same decisions give the same bytes
//! (and small, reviewable diffs). Findings are looked up in a results file (`scan --json`); the
//! command never scans. Paths are relative to the current directory, which is the scan root.

use crate::cli::{TriageArgs, TriageCommand, TriageFileArgs, TriageFormat, TriageStatusArg};
use crate::commands::scan::read_results;
use crate::util::{self, write_file};
use crate::{exit, CliError, CliResult, Io};
use mote_core::Finding;
use mote_engine::triage::{self, TriageEntry, TriageFile, TriageStatus};
use mote_engine::ScanResult;
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The triage file: `--file`, else `triage.file` of the configuration, relative to `cwd`.
fn triage_path(args: &TriageFileArgs, cwd: &Path) -> Result<PathBuf, CliError> {
    if let Some(f) = &args.file {
        return Ok(f.clone());
    }
    let (config, _) = crate::commands::config::load(args.config.as_deref(), cwd)?;
    Ok(cwd.join(config.triage.file.trim()))
}

/// Read the triage file (a missing file is an empty one).
fn read_file(path: &Path) -> Result<TriageFile, CliError> {
    match std::fs::read_to_string(path) {
        Ok(text) => TriageFile::parse(&text)
            .map_err(|e| CliError::usage(format!("invalid triage file {}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(TriageFile::default()),
        Err(e) => Err(CliError::usage(format!(
            "cannot read triage file {}: {e}",
            path.display()
        ))),
    }
}

fn status(s: TriageStatusArg) -> TriageStatus {
    match s {
        TriageStatusArg::Accepted => TriageStatus::Accepted,
        TriageStatusArg::FalsePositive => TriageStatus::FalsePositive,
    }
}

/// The finding `key` (an id or a fingerprint) names in `r`.
fn lookup<'a>(r: &'a ScanResult, key: &str) -> Result<&'a Finding, CliError> {
    let key = key.trim().to_ascii_lowercase();
    let hits: Vec<&Finding> = match key.len() {
        12 => r.findings.iter().filter(|f| f.id == key).collect(),
        32 => r.findings.iter().filter(|f| f.fingerprint == key).collect(),
        _ => {
            return Err(CliError::usage(format!(
                "`{key}` is neither a finding id (12 hex digits) nor a fingerprint (32)"
            )))
        }
    };
    match hits.as_slice() {
        [] => Err(CliError::usage(format!(
            "no finding `{key}` in the results (scan again with --json; ids change when a file moves)"
        ))),
        [f] => Ok(f),
        many => Err(CliError::usage(format!(
            "fingerprint `{key}` is in {} files; name the finding by its id instead: {}",
            many.len(),
            many.iter()
                .map(|f| format!("{} ({})", f.id, f.path))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn today() -> String {
    util::civil_date(util::now_secs())
}

fn describe(e: &TriageEntry) -> String {
    let key = e
        .fingerprint
        .as_deref()
        .map(|f| format!("fingerprint {}", &f[..f.len().min(12)]))
        .or_else(|| e.id.as_deref().map(|i| format!("id {i}")))
        .unwrap_or_default();
    format!("{} {} in {} ({key})", e.status.as_str(), e.rule, e.path)
}

pub fn run(args: &TriageArgs, io: &mut Io<'_>) -> CliResult {
    let cwd = util::cwd()?;
    match &args.command {
        TriageCommand::Add {
            finding,
            status: st,
            reason,
            results,
            by,
            date,
            file,
        } => {
            let path = triage_path(file, &cwd)?;
            let reason = reason.trim();
            if reason.is_empty() {
                return Err(CliError::usage(
                    "--reason must say why the finding is accepted or a false positive",
                ));
            }
            let date = match date {
                Some(d) if !valid_date(d.trim()) => {
                    return Err(CliError::usage(format!(
                        "--date must be YYYY-MM-DD, got `{d}`"
                    )))
                }
                Some(d) => d.trim().to_string(),
                None => today(),
            };
            let r = read_results(results)?;
            let f = lookup(&r, finding)?;
            if f.rule_id.starts_with("mote/") {
                return Err(CliError::usage(format!(
                    "`{}` findings are about the analysis itself and cannot be triaged",
                    f.rule_id
                )));
            }
            let mut tf = read_file(&path)?;
            let entry = TriageEntry {
                fingerprint: Some(f.fingerprint.clone()),
                id: None,
                rule: f.rule_id.clone(),
                path: f.path.clone(),
                status: status(*st),
                reason: reason.to_string(),
                by: by
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                date: Some(date),
                line: 0,
            };
            let what = describe(&entry);
            let replaced = tf.upsert(entry);
            write_file(&path, tf.render().as_bytes())?;
            let _ = writeln!(
                io.out,
                "{} {what} at line {} in {}",
                if replaced { "Updated:" } else { "Added:" },
                f.range.start_line,
                path.display()
            );
            Ok(exit::OK)
        }
        TriageCommand::List {
            results,
            format,
            file,
        } => {
            let path = triage_path(file, &cwd)?;
            let tf = read_file(&path)?;
            let states: Option<Vec<&'static str>> = match results {
                Some(p) => Some(states(&tf, &read_results(p)?)),
                None => None,
            };
            let state = |i: usize| states.as_ref().map(|s| s[i]);
            match format {
                TriageFormat::Json => {
                    let entries: Vec<_> = tf
                        .entries
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            let mut v = json!({
                                "fingerprint": e.fingerprint,
                                "id": e.id,
                                "rule": e.rule,
                                "path": e.path,
                                "status": e.status.as_str(),
                                "reason": e.reason,
                                "by": e.by,
                                "date": e.date,
                            });
                            if let Some(s) = state(i) {
                                v["state"] = json!(s);
                            }
                            v
                        })
                        .collect();
                    let v = json!({
                        "file": path.to_string_lossy().replace('\\', "/"),
                        "entries": entries,
                    });
                    let _ = writeln!(
                        io.out,
                        "{}",
                        serde_json::to_string_pretty(&v).unwrap_or_default()
                    );
                }
                TriageFormat::Text => {
                    if tf.entries.is_empty() {
                        let _ = writeln!(io.out, "No triage entries in {}.", path.display());
                    }
                    for (i, e) in tf.entries.iter().enumerate() {
                        let mut line = describe(e);
                        if let Some(s) = state(i) {
                            line = format!("[{s}] {line}");
                        }
                        let who: Vec<&str> = [e.by.as_deref(), e.date.as_deref()]
                            .into_iter()
                            .flatten()
                            .collect();
                        let _ = writeln!(io.out, "{line}");
                        let _ = writeln!(
                            io.out,
                            "    {}{}",
                            e.reason,
                            if who.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", who.join(", "))
                            }
                        );
                    }
                }
            }
            Ok(exit::OK)
        }
        TriageCommand::Prune {
            results,
            dry_run,
            file,
        } => {
            let path = triage_path(file, &cwd)?;
            let mut tf = read_file(&path)?;
            let r = read_results(results)?;
            let Some(report) = &r.triage else {
                return Err(CliError::usage(format!(
                    "{} has no triage information: scan with the triage file in place (and --json) first",
                    results.display()
                )));
            };
            let before = tf.entries.len();
            let mut removed = Vec::new();
            tf.entries.retain(|e| {
                let stale = report.stale.iter().any(|k| k.is(e));
                if stale {
                    removed.push(describe(e));
                }
                !stale
            });
            for d in &removed {
                let _ = writeln!(
                    io.out,
                    "{} {d}",
                    if *dry_run {
                        "Would remove:"
                    } else {
                        "Removed:"
                    }
                );
            }
            if removed.is_empty() {
                let _ = writeln!(io.out, "No stale entries in {}.", path.display());
            } else if !dry_run {
                write_file(&path, tf.render().as_bytes())?;
                let _ = writeln!(
                    io.out,
                    "{} of {before} entries left in {}.",
                    tf.entries.len(),
                    path.display()
                );
            }
            Ok(exit::OK)
        }
    }
}

/// `matched`, `stale` or `not covered` for each entry against a results file: matching is
/// recomputed; staleness is what the scan reported.
fn states(tf: &TriageFile, r: &ScanResult) -> Vec<&'static str> {
    let files: HashSet<&str> = r.files.iter().map(|f| f.path.as_str()).collect();
    let view: Vec<&Finding> = r.findings.iter().collect();
    let m = triage::match_entries(&tf.entries, &view, &|p| files.contains(p));
    let mut used = vec![false; tf.entries.len()];
    for e in m.into_iter().flatten() {
        used[e] = true;
    }
    tf.entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            if used[i] {
                "matched"
            } else if r
                .triage
                .as_ref()
                .is_some_and(|t| t.stale.iter().any(|k| k.is(e)))
            {
                "stale"
            } else {
                "not covered"
            }
        })
        .collect()
}
