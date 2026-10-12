//! `mote gate --results FILE [--coverage GLOB...]`: re-evaluate the gate without analysis.
//!
//! Coverage and the gate are applied with the engine's own functions
//! ([`gate::apply_coverage`], [`gate::reevaluate_gate`]), so the verdict matches what `scan`
//! would have computed with the same reports.

use crate::cli::{GateArgs, GateCondition, GateFormat};
use crate::util::{self, render};
use crate::{exit, plugins, CliError, CliResult, Io};
use mote_engine::result::Diagnostic;
use mote_engine::{coverage, gate, ScanResult};
use mote_report::markdown::MarkdownOptions;
use mote_report::text::{write_gate, Painter};

/// `--only`: keep just the selected conditions and recompute the verdict (a skipped condition
/// still counts as passed). A selected condition the gate doesn't have (not configured) is noted
/// as a warning; with none left the gate passes.
fn restrict_gate(result: &mut ScanResult, only: &[GateCondition], diags: &mut Vec<Diagnostic>) {
    for c in only {
        if !result.gate.conditions.iter().any(|g| c.matches(&g.name)) {
            diags.push(Diagnostic::warning(
                None,
                format!(
                    "--only {}: the gate has no such condition (not set in the configuration)",
                    c.as_str()
                ),
            ));
        }
    }
    result
        .gate
        .conditions
        .retain(|g| only.iter().any(|c| c.matches(&g.name)));
    result.gate.passed = result.gate.conditions.iter().all(|g| g.passed);
}

pub fn run(args: &GateArgs, io: &mut Io<'_>) -> CliResult {
    let cwd = util::cwd()?;
    let text = std::fs::read_to_string(&args.results)
        .map_err(|e| CliError::usage(format!("cannot read {}: {e}", args.results.display())))?;
    let mut result: ScanResult = mote_report::json::read_json(&text).map_err(|e| {
        CliError::usage(format!(
            "{} is not a mote results file (from `mote scan --json`): {e}",
            args.results.display()
        ))
    })?;
    // The same validation as `scan`: a typo in the config is an error here too.
    let (config, origin) = crate::commands::config::load(args.config.as_deref(), &cwd)?;
    let registry = plugins::registry()?;
    config
        .validate(&registry)
        .map_err(|e| crate::config_error(e, origin.as_deref()))?;
    let mut diags = Vec::new();
    if !args.coverage.is_empty() || !config.coverage.reports.is_empty() {
        let (mut files, warnings) = util::expand_reports(&args.coverage, &cwd);
        for w in warnings {
            diags.push(Diagnostic::warning(None, w));
        }
        // Config globs that match nothing are normal in a gate job: no warning for those.
        files.extend(util::expand_reports(&config.coverage.reports, &cwd).0);
        files.sort();
        files.dedup();
        let (report, mut d) =
            coverage::load_reports(&files, |f| f.display().to_string().replace('\\', "/"));
        diags.append(&mut d);
        if let Some(report) = report {
            let repo_files: Vec<&str> = result.files.iter().map(|f| f.path.as_str()).collect();
            let (mapped, mut d) = coverage::map_reports(report, &repo_files);
            diags.append(&mut d);
            gate::apply_coverage(&mut result, &mapped);
        } else if !files.is_empty() || !args.coverage.is_empty() {
            diags.push(Diagnostic::warning(None, "no usable coverage report"));
        }
    }
    gate::reevaluate_gate(&mut result, &config);
    // What this command writes has every field of the current format (missing ones defaulted).
    result.format_version = mote_engine::result::RESULTS_FORMAT_VERSION;
    if !args.only.is_empty() {
        restrict_gate(&mut result, &args.only, &mut diags);
    }
    for d in &diags {
        let _ = match &d.path {
            Some(p) => writeln!(io.err, "warning: {p}: {}", d.message),
            None => writeln!(io.err, "warning: {}", d.message),
        };
    }
    result.diagnostics.extend(diags);

    let out = match args.format {
        GateFormat::Text => {
            let p = Painter {
                enabled: util::use_color(args.color, io.out_is_tty),
            };
            render(|w| {
                if let Some(c) = &result.measures.coverage {
                    writeln!(
                        w,
                        "Coverage: {:.1}% overall ({} of {} lines)",
                        c.coverage_pct, c.covered_lines, c.coverable_lines
                    )?;
                }
                if let Some(nc) = &result.new_code {
                    if let (Some(pct), Some(cov), Some(all)) = (
                        nc.measures.new_coverage_pct,
                        nc.measures.new_covered_lines,
                        nc.measures.new_coverable_lines,
                    ) {
                        writeln!(w, "Coverage on new code: {pct:.1}% ({cov} of {all} lines)")?;
                    }
                }
                write_gate(&result.gate, &p, w)
            })
        }
        GateFormat::Markdown => mote_report::markdown::render(
            &result,
            &MarkdownOptions {
                // Repository-relative locations: the scan recorded where its root sits.
                path_prefix: &result.source.prefix,
                ..Default::default()
            },
        )
        .into_bytes(),
        GateFormat::Json => render(|w| mote_report::json::write_json(&result, true, w)),
    };
    io.out
        .write_all(&out)
        .map_err(|e| CliError::internal(format!("cannot write to stdout: {e}")))?;
    Ok(if result.gate.passed {
        exit::OK
    } else {
        exit::GATE_FAILED
    })
}
