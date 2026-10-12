//! `mote scan`.

use crate::cli::{Format, ScanArgs};
use crate::util::{self, render, write_file};
use crate::{exit, plugins, CliError, CliResult, Io};
use mote_core::Severity;
use mote_engine::baseline::Baseline;
use mote_engine::result::DiagnosticLevel;
use mote_engine::scan::discover_config;
use mote_engine::{Config, EngineError, ScanOptions, ScanResult, Scanner, SourceSpec};
use mote_report::checkstyle::CheckstyleOptions;
use mote_report::github::GithubOptions;
use mote_report::gitlab::GitlabOptions;
use mote_report::html::HtmlOptions;
use mote_report::junit::JunitOptions;
use mote_report::markdown::MarkdownOptions;
use mote_report::sarif::SarifOptions;
use mote_report::text::TextOptions;
use mote_report::{fmt_ms, thousands, Filter};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use mote_report::failed_files;

/// Where the scan root sits in its git repository.
///
/// Path policy: text, JSON and JSONL output keep paths relative to the scan root (what a local
/// user or agent works with, and what `--include` and the MCP tools take). The formats CI
/// services consume — Checks annotations (`--github-checks`, `--format github`), SARIF, the
/// Markdown summary, GitLab Code Quality, Checkstyle and JUnit — are resolved from the
/// repository root, so they get [`Self::prefix`] in front.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoLocation {
    /// Repository-relative path of the scan root, `/`-separated with a trailing `/` (`backend/`);
    /// empty at the top level or outside a repository.
    pub prefix: String,
    /// What SARIF URIs are relative to (the repository top level, or the scanned directory), as
    /// a `file:` URI.
    pub srcroot_uri: Option<String>,
}

/// Locate `root` in its repository. `source_kind` is the scan's `source.kind`: only git sources
/// get a prefix (a `directory` scan did not treat the root as part of a repository).
pub fn repo_location(root: &Path, source_kind: &str) -> RepoLocation {
    use mote_engine::worktree::{self, Located};
    let uri = |p: &Path| mote_report::sarif::directory_uri(&p.to_string_lossy());
    match source_kind {
        "worktree" | "rev" | "staged" => {
            let found = match worktree::locate(root) {
                Located::Repo(layout) => Some((layout.toplevel, layout.prefix)),
                // Unusual layouts: ask git (`rev-parse --show-toplevel --show-prefix`).
                _ => mote_engine::git::Git::open(root)
                    .ok()
                    .map(|g| (g.toplevel().to_path_buf(), g.prefix().to_string())),
            };
            match found {
                Some((toplevel, prefix)) => {
                    let mut prefix = prefix.replace('\\', "/");
                    if !prefix.is_empty() && !prefix.ends_with('/') {
                        prefix.push('/');
                    }
                    RepoLocation {
                        prefix,
                        srcroot_uri: Some(uri(&toplevel)),
                    }
                }
                None => RepoLocation::default(),
            }
        }
        "directory" => RepoLocation {
            prefix: String::new(),
            srcroot_uri: std::fs::canonicalize(root).ok().map(|p| uri(&p)),
        },
        _ => RepoLocation::default(),
    }
}

/// Load `--config FILE` or discover the config for `root` (at `rev` when scanning a revision).
pub fn load_config(
    explicit: Option<&Path>,
    root: &Path,
    rev: Option<&str>,
) -> Result<(Config, Option<String>), CliError> {
    match explicit {
        Some(p) => Ok((Config::load(p)?, Some(p.display().to_string()))),
        None => discover_config(root, rev).map_err(CliError::from),
    }
}

/// Map a scanner-construction error, naming the config file for validation problems.
pub fn scanner_error(e: EngineError, origin: Option<&str>) -> CliError {
    match e {
        EngineError::Config(c) => crate::config_error(c, origin.map(Path::new)),
        other => CliError::from(other),
    }
}

/// Map a scan error; an unknown revision gets a hint. `--base origin/main` is what the docs and
/// CI recipes show, and it fails in a clone without that remote branch: a local clone whose
/// branch is `main`, or a shallow / single-branch CI checkout that did not fetch the base.
fn scan_error(e: EngineError, root: &Path) -> CliError {
    use mote_engine::git::GitError;
    let EngineError::Git(GitError::UnknownRev(rev)) = &e else {
        return CliError::from(e);
    };
    let mut message = e.to_string();
    if let Some((_, branch)) = rev
        .split_once('/')
        .filter(|(r, b)| !r.is_empty() && !b.is_empty() && !rev.contains(['~', '^', ':', '@']))
    {
        let local = mote_engine::git::Git::open(root)
            .ok()
            .is_some_and(|g| g.resolve_commit(branch).is_ok());
        if local {
            message.push_str(&format!("; did you mean the local branch `{branch}`?"));
        } else {
            message.push('.');
        }
        message.push_str(&format!(
            " A remote branch must be fetched first: CI checkouts are often shallow or \
             single-branch (`git fetch origin {branch}`, or `fetch-depth: 0` with \
             actions/checkout; see docs/ci.md)"
        ));
    }
    CliError::usage(message)
}

/// Scanning a directory below the repository top level reads only that directory's
/// configuration: say so when it has none and the top level has one, which would otherwise be
/// ignored without a word (`mote scan lib/core` in a monorepo).
fn config_not_found_note(root: &Path) -> Option<String> {
    use mote_engine::worktree::{self, Located};
    let Located::Repo(layout) = worktree::locate(root) else {
        return None;
    };
    if layout.prefix.is_empty() {
        return None;
    }
    let name = mote_engine::config::CONFIG_FILE_NAMES
        .iter()
        .find(|n| layout.toplevel.join(n).is_file())?;
    let sub = layout.prefix.trim_end_matches('/');
    // The top-level file as the user would type it from the current directory.
    let config = std::env::current_dir()
        .ok()
        .and_then(|cwd| std::fs::canonicalize(cwd).ok())
        .and_then(|cwd| {
            let below = cwd.strip_prefix(&layout.toplevel).ok()?;
            let mut up: Vec<&str> = below.components().map(|_| "..").collect();
            up.push(name);
            Some(up.join("/"))
        })
        .unwrap_or_else(|| format!("<repository top level>/{name}"));
    Some(format!(
        "note: {sub} has no {name}, so this scan uses the default settings; the repository's \
         {name} is read only when the scan starts at the top level. Scan from there with \
         `--include '{sub}/**'` to apply it (or pass `--config {config}`: its globs are then \
         relative to {sub})"
    ))
}

/// Read a results file (`scan --json`) as a baseline; `label` names it in reports.
pub fn load_baseline(path: &Path, label: &str) -> Result<Baseline, CliError> {
    let result = read_results(path)?;
    Ok(Baseline::from_result(label.replace('\\', "/"), result))
}

/// Read a results file written by `mote scan --json` (usage error when it is not one).
pub fn read_results(path: &Path) -> Result<ScanResult, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("cannot read {}: {e}", path.display())))?;
    mote_report::json::read_json(&text).map_err(|e| {
        CliError::usage(format!(
            "{} is not a mote results file (from `mote scan --json`): {e}",
            path.display()
        ))
    })
}

struct Prepared {
    scanner: Scanner,
    options: ScanOptions,
    root: PathBuf,
}

fn prepare(args: &ScanArgs, io: &mut Io<'_>, quiet: bool) -> Result<Prepared, CliError> {
    let registry = plugins::registry()?;
    let cwd = util::cwd()?;
    let (root, source, config, origin) = if let Some(url) = &args.remote {
        let cache_dir = args.cache_dir.clone().unwrap_or_else(util::user_cache_dir);
        tracing::info!("cloning {url} into {}", cache_dir.join("repos").display());
        let co = mote_engine::git::clone_remote(url, args.rev.as_deref(), &cache_dir)
            .map_err(|e| CliError::internal(format!("cannot clone {url}: {e}")))?;
        let (config, origin) = load_config(args.config.as_deref(), &co.dir, Some(&co.commit))?;
        (co.dir, SourceSpec::Rev(co.commit), config, origin)
    } else {
        let root = args.path.clone().unwrap_or_else(|| PathBuf::from("."));
        if !root.is_dir() {
            return Err(CliError::usage(format!(
                "{}: not a directory",
                root.display()
            )));
        }
        let source = match (&args.rev, args.staged) {
            (Some(rev), _) => SourceSpec::Rev(rev.clone()),
            (None, true) => SourceSpec::Staged,
            (None, false) => SourceSpec::Auto,
        };
        let (config, origin) = load_config(args.config.as_deref(), &root, args.rev.as_deref())?;
        if origin.is_none() && args.rev.is_none() && !quiet {
            if let Some(note) = config_not_found_note(&root) {
                let _ = writeln!(io.err, "{note}");
            }
        }
        (root, source, config, origin)
    };
    if let Some(o) = &origin {
        tracing::info!("configuration: {o}");
    }
    let scanner =
        Scanner::new(registry, config, &root).map_err(|e| scanner_error(e, origin.as_deref()))?;
    let (coverage_reports, warnings) = util::expand_reports(&args.coverage, &cwd);
    if !quiet {
        for w in warnings {
            let _ = writeln!(io.err, "warning: {w}");
        }
    }
    let baseline = match &args.baseline {
        Some(p) => Some(Arc::new(load_baseline(p, &p.to_string_lossy())?)),
        None => None,
    };
    let options = ScanOptions {
        source,
        paths: args.include.iter().map(|p| p.replace('\\', "/")).collect(),
        base: args.base.clone(),
        cache: args.no_cache.then_some(false),
        cache_dir: args.cache_dir.clone(),
        threads: args.threads.filter(|&n| n > 0),
        coverage_reports,
        changed_only: args.changed_only,
        baseline,
        triage_file: args.triage.clone(),
    };
    Ok(Prepared {
        scanner,
        options,
        root,
    })
}

fn sarif_options<'a>(args: &ScanArgs, loc: &'a RepoLocation) -> SarifOptions<'a> {
    SarifOptions {
        min_severity: args.sarif_min_severity,
        path_prefix: &loc.prefix,
        srcroot_uri: loc.srcroot_uri.as_deref(),
        ..Default::default()
    }
}

/// Render `result` in `format`. `loc` places the scan root in its repository (for the formats
/// GitHub consumes; see [`RepoLocation`]).
pub fn render_format(
    format: Format,
    result: &ScanResult,
    args: &ScanArgs,
    filter: Filter,
    color: bool,
    loc: &RepoLocation,
) -> Vec<u8> {
    // `--show-suppressed` is for the human-readable text output only.
    let listing = Filter {
        show_suppressed: false,
        ..filter
    };
    match format {
        Format::Text => {
            render(|w| mote_report::text::write(result, &TextOptions { filter, color }, w))
        }
        Format::Json => render(|w| mote_report::json::write_json(result, true, w)),
        Format::Jsonl => render(|w| mote_report::json::write_jsonl(result, &listing, w)),
        Format::Sarif => {
            render(|w| mote_report::sarif::write(result, &sarif_options(args, loc), w))
        }
        Format::Markdown => mote_report::markdown::render(
            result,
            &MarkdownOptions {
                filter: listing,
                path_prefix: &loc.prefix,
                ..Default::default()
            },
        )
        .into_bytes(),
        Format::Github => render(|w| {
            mote_report::github::write(
                result,
                &GithubOptions {
                    markdown: MarkdownOptions {
                        filter: listing,
                        ..Default::default()
                    },
                    path_prefix: &loc.prefix,
                    ..Default::default()
                },
                w,
            )
        }),
        Format::Gitlab => render(|w| {
            mote_report::gitlab::write(
                result,
                &GitlabOptions {
                    path_prefix: &loc.prefix,
                },
                w,
            )
        }),
        Format::Checkstyle => mote_report::checkstyle::render(
            result,
            &CheckstyleOptions {
                path_prefix: &loc.prefix,
            },
        )
        .into_bytes(),
        Format::Junit => mote_report::junit::render(
            result,
            &JunitOptions {
                filter: listing,
                path_prefix: &loc.prefix,
                ..Default::default()
            },
        )
        .into_bytes(),
        Format::Html => mote_report::html::render(result, &HtmlOptions::default()).into_bytes(),
    }
}

/// Formats read by tools that resolve paths from the repository root (CI services), which get
/// the scan root's repository prefix (see [`RepoLocation`]).
fn repository_relative(format: Format) -> bool {
    matches!(
        format,
        Format::Sarif
            | Format::Markdown
            | Format::Github
            | Format::Gitlab
            | Format::Checkstyle
            | Format::Junit
    )
}

pub fn run(args: &ScanArgs, quiet: bool, io: &mut Io<'_>) -> CliResult {
    let t_start = Instant::now();
    let startup_ms = crate::START
        .get()
        .map(|s| t_start.duration_since(*s).as_secs_f64() * 1e3);
    let prepared = prepare(args, io, quiet)?;
    let t_setup = t_start.elapsed();
    let result = prepared
        .scanner
        .scan(&prepared.options)
        .map_err(|e| scan_error(e, &prepared.root))?;
    let t_scanned = Instant::now();

    let filter = Filter {
        new_only: args.new_only,
        min_severity: args.min_severity.unwrap_or(Severity::Low),
        show_suppressed: args.show_suppressed,
    };
    let color = args.format == Format::Text
        && args.output.is_none()
        && util::use_color(args.color, io.out_is_tty);
    let side: [(Option<&PathBuf>, Format); 9] = [
        (args.json.as_ref(), Format::Json),
        (args.jsonl.as_ref(), Format::Jsonl),
        (args.sarif.as_ref(), Format::Sarif),
        (args.markdown.as_ref(), Format::Markdown),
        (args.github_checks.as_ref(), Format::Github),
        (args.gitlab_codequality.as_ref(), Format::Gitlab),
        (args.checkstyle.as_ref(), Format::Checkstyle),
        (args.junit.as_ref(), Format::Junit),
        (args.html.as_ref(), Format::Html),
    ];
    let wanted = |f: Format| args.format == f || side.iter().any(|(p, g)| p.is_some() && *g == f);
    // Only the formats CI services consume need to know where the root sits in the repository.
    // A remote clone is scanned from its top level, and its cache location means nothing to them.
    let github_formats = repository_relative(args.format)
        || args.bitbucket_insights.is_some()
        || side
            .iter()
            .any(|(p, g)| p.is_some() && repository_relative(*g));
    let mut loc = if github_formats && args.remote.is_none() {
        repo_location(&prepared.root, &result.source.kind)
    } else {
        RepoLocation::default()
    };
    // The scan records where its root sits in the repository; prefer that.
    if github_formats && !result.source.prefix.is_empty() {
        loc.prefix.clone_from(&result.source.prefix);
    }
    let primary = render_format(args.format, &result, args, filter, color, &loc);
    match &args.output {
        Some(path) => write_file(path, &primary)?,
        None => io
            .out
            .write_all(&primary)
            .map_err(|e| CliError::internal(format!("cannot write to stdout: {e}")))?,
    }
    for (path, format) in side {
        if let Some(path) = path {
            write_file(
                path,
                &render_format(format, &result, args, filter, false, &loc),
            )?;
        }
    }
    if let Some(path) = &args.summary_json {
        let date = util::utc_timestamp(util::now_secs());
        let record = mote_report::summary::render(
            &result,
            &mote_report::summary::SummaryOptions { date: Some(&date) },
        );
        write_file(path, record.as_bytes())?;
    }
    if let Some(dir) = &args.bitbucket_insights {
        let opts = mote_report::bitbucket::BitbucketOptions {
            path_prefix: &loc.prefix,
            ..Default::default()
        };
        let (report, annotations) = mote_report::bitbucket::render(&result, &opts);
        write_file(&dir.join("report.json"), report.as_bytes())?;
        write_file(&dir.join("annotations.json"), annotations.as_bytes())?;
    }
    let _ = io.out.flush();
    let t_output = t_scanned.elapsed();

    if wanted(Format::Sarif) && !quiet {
        let omitted = mote_report::sarif::omitted_results(&result, &sarif_options(args, &loc));
        if omitted > 0 {
            let _ = writeln!(
                io.err,
                "warning: SARIF: {} of the least severe results left out to stay within {} results per run (see run.properties)",
                thousands(omitted as u64),
                thousands(mote_report::sarif::MAX_RESULTS as u64)
            );
        }
    }
    if wanted(Format::Html) && !quiet {
        let omitted = mote_report::html::omitted_findings(&result, &HtmlOptions::default());
        if omitted > 0 {
            let _ = writeln!(
                io.err,
                "warning: HTML: {} of the least severe findings left out to stay within {} findings (the page says so)",
                thousands(omitted as u64),
                thousands(mote_report::html::MAX_FINDINGS as u64)
            );
        }
    }
    if !quiet {
        const MAX_DIAGNOSTICS: usize = 20;
        for d in result.diagnostics.iter().take(MAX_DIAGNOSTICS) {
            let level = match d.level {
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Error => "error",
            };
            let _ = match &d.path {
                Some(p) => writeln!(io.err, "{level}: {p}: {}", d.message),
                None => writeln!(io.err, "{level}: {}", d.message),
            };
        }
        if result.diagnostics.len() > MAX_DIAGNOSTICS {
            let _ = writeln!(
                io.err,
                "… and {} more diagnostics (see the JSON results)",
                result.diagnostics.len() - MAX_DIAGNOSTICS
            );
        }
    }
    if args.timings {
        write_timings(
            &result,
            startup_ms,
            t_setup.as_secs_f64() * 1e3,
            t_output.as_secs_f64() * 1e3,
            t_start.elapsed().as_secs_f64() * 1e3,
            io.err,
        );
    }

    let failed = failed_files(&result);
    if args.strict && failed > 0 {
        let _ = writeln!(
            io.err,
            "error: {failed} file(s) could not be analyzed (--strict)"
        );
        return Ok(exit::INTERNAL);
    }
    if !result.gate.passed && !args.no_gate {
        return Ok(exit::GATE_FAILED);
    }
    Ok(exit::OK)
}

fn write_timings(
    r: &ScanResult,
    startup_ms: Option<f64>,
    setup_ms: f64,
    output_ms: f64,
    total_ms: f64,
    w: &mut dyn Write,
) {
    let t = &r.stats.timings;
    let s = &r.stats;
    let mut out = String::new();
    out.push_str("Timings (wall clock)\n");
    if let Some(ms) = startup_ms {
        out.push_str(&format!(
            "  {:<26}{:>12}\n",
            "process start to command",
            fmt_ms(ms)
        ));
    }
    for (name, ms) in [
        ("setup (registry, config)", setup_ms),
        ("discovery", t.discovery_ms),
        ("diff vs base", t.diff_ms),
        ("analysis (map)", t.map_ms),
        ("duplication (cpd)", t.cpd_ms),
        ("new code", t.new_code_ms),
        ("coverage", t.coverage_ms),
        ("cache write", t.cache_write_ms),
        ("engine total", t.total_ms),
        ("output", output_ms),
        ("command total", total_ms),
    ] {
        out.push_str(&format!("  {name:<26}{:>12}\n", fmt_ms(ms)));
    }
    if !s.languages.is_empty() {
        out.push_str("Languages (fresh analysis; time summed over threads)\n");
        for (lang, l) in &s.languages {
            let mb = l.bytes as f64 / 1_048_576.0;
            let rate = if l.analyze_ms > 0.0 {
                format!("{:.1} MB/s per thread", mb / (l.analyze_ms / 1e3))
            } else {
                "-".into()
            };
            out.push_str(&format!(
                "  {lang:<10}{:>9} files {:>9.1} MB {:>12}   {rate}\n",
                thousands(l.files),
                mb,
                fmt_ms(l.analyze_ms)
            ));
        }
    }
    out.push_str(&format!(
        "Files: {} seen, {} matched, {} analyzed, {} from cache, {} skipped; threads: {}\n",
        thousands(s.files_seen),
        thousands(s.files_matched),
        thousands(s.files_analyzed),
        thousands(s.cache_hits),
        thousands(s.files_skipped),
        s.threads
    ));
    if r.new_code.is_some() {
        out.push_str(&format!(
            "Base: {} files analyzed, {} from cache\n",
            thousands(s.base_files_analyzed),
            thousands(s.base_cache_hits)
        ));
    }
    out.push_str(&match &s.cache_path {
        Some(p) => format!("Cache: {p}\n"),
        None => "Cache: disabled\n".to_string(),
    });
    let _ = w.write_all(out.as_bytes());
}
