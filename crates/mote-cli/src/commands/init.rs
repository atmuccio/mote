//! `mote init [DIR] [--force] [--suggest-excludes] [--suggest-compat]`: write a commented
//! starter `.mote.yaml` for the repository in DIR:
//!
//! * excludes proposed for third-party code copied into the tree ([`third_party`]), listed
//!   commented out;
//! * `suppressions.compat` set to the sources of the other tools' suppressions the repository
//!   already uses ([`compat`]), with their counts in a comment (`compat: false` when there are
//!   none).
//!
//! `--suggest-excludes` and `--suggest-compat` only print those proposals, next to an existing
//! configuration too (whose `exclude` and skip settings then apply).
//!
//! `--from pmd|eslint [CONFIG]` also migrates another tool's configuration into the starter
//! ([`from`]): its rules, severities, thresholds, excludes and path-specific settings, with the
//! tool's `compat` source.

mod compat;
mod from;
mod third_party;

use crate::cli::InitArgs;
use crate::util::write_file;
use crate::{exit, CliError, CliResult, Io};
use compat::Usage as CompatUsage;
use mote_engine::suppress::COMPAT_SOURCES;
use mote_engine::{Config, Registry};
use std::path::Path;
use third_party::{discovered_paths, suggest_in, Suggestion};

/// The last line of the starter's `exclude` list; suggestions are inserted after it.
const EXCLUDE_LAST: &str = "  - \"**/.yarn/**\"\n";

/// The starter's own `exclude` globs.
const STARTER_EXCLUDES: [&str; 4] = [
    "**/node_modules/**",
    "**/vendor/**",
    "**/generated/**",
    "**/.yarn/**",
];

/// The starter's `compat` setting, replaced by the proposal when other tools' suppressions are
/// found.
const COMPAT_DEFAULT: &str = "  compat: false              # other tools' markers to honor: true, or a list of sources such as
                             # [pmd, eslint, nosonar, checkstyle, spotbugs, intellij, suppress-warnings-all]
";

/// The starter configuration for a repository: `suggestions` listed, commented out, under
/// `exclude`, and `suppressions.compat` proposed from `usage` (unchanged when nothing was found),
/// plus `extra`, the source of a tool `init --from` migrates.
pub fn starter_for(
    suggestions: &[Suggestion],
    usage: &CompatUsage,
    extra: Option<&'static str>,
) -> String {
    let mut text = STARTER.to_string();
    if !suggestions.is_empty() {
        let mut block = String::from(
            "  # Possibly third-party code copied into the repository: still analyzed; uncomment a\n  \
             # line to skip it (`mote init --suggest-excludes` lists these again).\n",
        );
        for s in suggestions {
            let glob = serde_json::to_string(&s.glob).unwrap_or_default();
            let reason = s.reason.replace(['\n', '\r'], " ");
            block.push_str(&format!("  # - {glob}  # {reason}\n"));
        }
        text = text.replacen(EXCLUDE_LAST, &format!("{EXCLUDE_LAST}{block}"), 1);
    }
    if !usage.is_empty() || extra.is_some() {
        text = text.replacen(COMPAT_DEFAULT, &compat_block(usage, extra), 1);
    }
    text
}

/// The `compat` setting proposed from `usage` (and `extra`), under a comment with the counts.
fn compat_block(usage: &CompatUsage, extra: Option<&'static str>) -> String {
    let mut proposed = usage.proposed();
    let mut notes = Vec::new();
    if !usage.sources.is_empty() {
        let found: Vec<String> = usage
            .sources
            .iter()
            .map(|(s, t)| format!("{s} {}", t.occurrences))
            .collect();
        notes.push(format!(
            "Other tools' suppressions found in this repository (occurrences): {}.",
            found.join(", ")
        ));
    }
    for n in usage.notes() {
        notes.push(format!("{n}."));
    }
    if let Some(x) = extra.filter(|x| !proposed.contains(x)) {
        proposed.push(x);
        proposed.sort_by_key(|s| COMPAT_SOURCES.iter().position(|c| c == s));
        notes.push(format!(
            "`{x}` is selected for the migrated configuration (`mote init --from`): the \
             suppressions written for it keep working."
        ));
    }
    let others: Vec<&str> = COMPAT_SOURCES
        .iter()
        .copied()
        .filter(|s| !proposed.contains(s))
        .collect();
    if !others.is_empty() {
        notes.push(format!(
            "Other sources: {} (`mote init --suggest-compat` counts them again).",
            others.join(", ")
        ));
    }
    let mut out = String::new();
    for n in notes {
        out.push_str(&comment_lines(&n, "  # ", "  #   ", 100));
    }
    if proposed.is_empty() {
        out.push_str(COMPAT_DEFAULT);
    } else {
        out.push_str(&format!("  compat: [{}]\n", proposed.join(", ")));
    }
    out
}

/// `text` as comment lines of at most `width` characters (longer words stay whole).
fn comment_lines(text: &str, first: &str, rest: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line = first.to_string();
    let mut empty = true;
    for w in text.split_whitespace() {
        if !empty && line.chars().count() + 1 + w.chars().count() > width {
            out.push_str(&line);
            out.push('\n');
            line = rest.to_string();
            empty = true;
        }
        if !empty {
            line.push(' ');
        }
        line.push_str(w);
        empty = false;
    }
    out.push_str(&line);
    out.push('\n');
    out
}

/// Count the other tools' suppressions in the files of `root` a scan with `config` would analyze.
fn compat_usage(
    root: &Path,
    paths: &[String],
    registry: &Registry,
    config: &Config,
) -> CompatUsage {
    compat::count(root, paths, registry, &compat::Filter::of(config))
}

/// Print the `--suggest-excludes` listing.
fn print_excludes(io: &mut Io<'_>, suggestions: &[Suggestion]) {
    if suggestions.is_empty() {
        let _ = writeln!(io.out, "No third-party code found to exclude.");
    } else {
        let _ = writeln!(
            io.out,
            "Possibly third-party code (add the globs you agree with to `exclude` in .mote.yaml):"
        );
        for s in suggestions {
            let _ = writeln!(io.out, "  {}  # {}", s.glob, s.reason);
        }
    }
}

/// Print the `--suggest-compat` listing.
fn print_compat(io: &mut Io<'_>, usage: &CompatUsage) {
    if usage.is_empty() {
        let _ = writeln!(
            io.out,
            "No suppressions of other tools found: keep `suppressions: {{ compat: false }}`."
        );
        return;
    }
    if usage.sources.is_empty() {
        let _ = writeln!(io.out, "No suppressions of other tools found.");
    } else {
        let _ = writeln!(io.out, "Suppressions of other tools found:");
    }
    for (s, t) in &usage.sources {
        let _ = writeln!(
            io.out,
            "  {s:<22} {:>6} in {} file{}",
            t.occurrences,
            t.files,
            if t.files == 1 { "" } else { "s" }
        );
    }
    for n in usage.notes() {
        let _ = writeln!(io.out, "  note: {n}");
    }
    let proposed = usage.proposed();
    if proposed.is_empty() {
        let _ = writeln!(
            io.out,
            "Proposed: nothing to honor; keep `suppressions: {{ compat: false }}`."
        );
    } else {
        let _ = writeln!(
            io.out,
            "Proposed (in .mote.yaml): suppressions: {{ compat: [{}] }}",
            proposed.join(", ")
        );
    }
}

/// The starter configuration. Every rule example is commented out, so it validates against any
/// set of language plugins.
pub const STARTER: &str = r#"# Mote configuration. Reference: docs/cli.md and `mote config schema`.
# Check it with `mote config validate`; see the settings for one file with
# `mote config explain <file>`; list rules with `mote rules --all`.

# Minimum mote version this file needs (a semver requirement).
mote: ">=0.1"

# Rule profile: default (recommended rules), conventions (default plus the opt-in style rules),
# all (every non-deprecated rule) or none.
extends: default

# Files to skip entirely: repo-relative globs; a pattern without "/" matches at any depth.
exclude:
  - "**/node_modules/**"
  - "**/vendor/**"
  - "**/generated/**"
  - "**/.yarn/**"

# Extra globs for test code (added to each language's defaults).
# tests: ["**/it/**"]

# Skip files with generated-code markers (@generated, DO NOT EDIT, ...), Yarn Plug'n'Play files
# (.yarn/**, .pnp.cjs, .pnp.loader.mjs), minified files and vendored JavaScript (library banners
# such as /*! lib v1.2.3 | (c) ... */ that do not name this project's package, bundler output).
skip_generated: true
# Files analyzed even when they look generated, minified or vendored (`exclude` still wins).
# skip_generated_except: ["web/js/app.bundle.js"]
max_file_size: 1048576

# Rule settings: off | on | LOW | MEDIUM | HIGH, or { enabled, severity, params }.
# Keys are rule ids or globs such as "js/*".
# rules:
#   java/empty-catch-block: off
#   js/no-console: { severity: HIGH }
#   java/cognitive-complexity: { params: { threshold: 20 } }
#   "js/*": { enabled: false }

# Settings for test files only (applied after `rules`).
# test_rules:
#   java/magic-number: off

# Path-specific settings, applied in order.
# overrides:
#   - paths: ["legacy/**"]
#     rules: { java/cognitive-complexity: { params: { threshold: 30 } } }

# Quality gate. With `mote scan --base <rev>` it applies to new code only.
gate:
  fail_on: HIGH              # new findings at or above this severity fail: HIGH | MEDIUM | LOW | none
  new_duplication_pct: 3.0   # max duplication on new lines, in percent (null disables)
  new_coverage_pct: 80.0     # min coverage on new lines (needs coverage reports; null disables)
  min_new_lines: 20          # duplication and coverage conditions are skipped below this many new lines
  worsened: annotate         # findings whose value got worse: annotate | fail
  # new_maintainability_grade: B  # opt-in: worst grade allowed on new code (A best .. E); also
  #                               # new_reliability_grade, maintainability_grade, reliability_grade

# Duplication detection.
cpd:
  enabled: true
  min_tokens: 100
  min_lines: 10
  include_tests: false

# Coverage reports (LCOV, JaCoCo XML, Cobertura XML): repo-relative paths or globs.
# coverage:
#   reports: ["**/jacoco.xml", "coverage/lcov.info"]

# Comment suppressions: // mote-ignore[rule-id]: reason
suppressions:
  require_reason: true
  compat: false              # other tools' markers to honor: true, or a list of sources such as
                             # [pmd, eslint, nosonar, checkstyle, spotbugs, intellij, suppress-warnings-all]

# Decisions about single findings (accepted, false positive): `mote triage add`.
# triage:
#   file: .mote-triage.yaml

# Analysis cache (default location: <root>/.mote/cache; add .mote/ to .gitignore).
cache:
  enabled: true
  max_size_mb: 1024

# Grade bands: upper limits of grades A, B, C and D (see docs/cli.md, "Grades").
# grades:
#   maintainability: [1, 2, 3, 4]   # estimated hours of fixes per 1,000 lines of code
#   reliability: [0.5, 1, 2, 4]     # weighted reliability findings per 10,000 lines of code
#   min_new_lines: 100              # new code with fewer new lines is not rated

# Directory levels with rolled-up measures in the results JSON (0 = none).
# metrics:
#   depth: 2

# Java language release of the code. Unset: read for each file from the nearest pom.xml or
# build.gradle(.kts); rules about newer language features stay silent when it is unknown.
# java:
#   release: 17
"#;

pub fn run(args: &InitArgs, io: &mut Io<'_>) -> CliResult {
    if !args.dir.is_dir() {
        return Err(CliError::usage(format!(
            "{}: not a directory",
            args.dir.display()
        )));
    }
    if args.suggest_excludes || args.suggest_compat {
        // What an existing configuration already excludes is not looked at (nor proposed
        // again), and its skip settings decide which files are counted.
        let (config, _) = Config::discover(&args.dir)?;
        let paths = discovered_paths(&args.dir, &config.exclude);
        if args.suggest_excludes {
            print_excludes(io, &suggest_in(&args.dir, &paths));
        }
        if args.suggest_compat {
            if args.suggest_excludes {
                let _ = writeln!(io.out);
            }
            let registry = crate::plugins::registry()?;
            print_compat(io, &compat_usage(&args.dir, &paths, &registry, &config));
        }
        return Ok(exit::OK);
    }
    // `--from TOOL [CONFIG]`: the other tool's configuration, read before anything is written.
    let source = match &args.from {
        Some(values) => Some(from::resolve(values, &args.dir)?),
        None => None,
    };
    for name in mote_engine::config::CONFIG_FILE_NAMES {
        let existing = args.dir.join(name);
        if existing.exists() && !args.force {
            return Err(CliError::usage(format!(
                "{} already exists (use --force to overwrite)",
                existing.display()
            )));
        }
    }
    let registry = crate::plugins::registry()?;
    let migration = match source {
        Some((tool, file)) => Some(crate::commands::migrate::propose(
            tool, &file, &args.dir, &registry,
        )?),
        None => None,
    };
    let path = args.dir.join(".mote.yaml");
    // The starter replaces any existing configuration: its own excludes are the directories
    // `discovered_paths` never looks at, and its other settings are the defaults.
    let paths = discovered_paths(&args.dir, &[] as &[&str]);
    let suggestions = suggest_in(&args.dir, &paths);
    let usage = compat_usage(&args.dir, &paths, &registry, &Config::default());
    let counted = usage.proposed();
    let extra = migration
        .as_ref()
        .and_then(|p| from::compat_source(p, &counted));
    let mut text = starter_for(&suggestions, &usage, extra);
    if let Some(p) = &migration {
        text = from::merge(&text, p);
        crate::commands::migrate::validate(&text, &registry)?;
    }
    write_file(&path, text.as_bytes())?;
    let _ = writeln!(
        io.out,
        "wrote {}; check it with `mote config validate`",
        path.display().to_string().replace('\\', "/")
    );
    if let Some(p) = &migration {
        let _ = writeln!(
            io.out,
            "migrated from {}: {} (rules, excludes and overrides in the file; what was not \
             carried over is listed at its end): review them",
            p.source,
            crate::commands::migrate::summary(p)
        );
        if !p.unmapped.is_empty() {
            let _ = writeln!(io.out, "Not mapped:");
            if !p.unmapped_note.is_empty() {
                let _ = writeln!(io.out, "  ({})", p.unmapped_note);
            }
            for u in &p.unmapped {
                let _ = writeln!(io.out, "  - {u}");
            }
        }
    }
    if !suggestions.is_empty() {
        let n = suggestions.len();
        let _ = writeln!(
            io.out,
            "{n} possible third-party location{} listed under `exclude`, commented out: review them",
            if n == 1 { "" } else { "s" }
        );
    }
    if !counted.is_empty() {
        let mut proposed = counted.clone();
        proposed.extend(extra);
        proposed.sort_by_key(|s| COMPAT_SOURCES.iter().position(|c| c == s));
        let _ = writeln!(
            io.out,
            "suppressions.compat: [{}], from the suppressions of other tools found in the \
             repository (counts in the comment above it){}: review it",
            proposed.join(", "),
            match extra {
                Some(x) => format!(" and `{x}` for the migrated configuration"),
                None => String::new(),
            }
        );
    }
    let migrated = migration.as_ref().map(crate::commands::migrate::tool);
    for line in next_steps(&args.dir, &counted, migrated) {
        let _ = writeln!(io.out, "{line}");
    }
    Ok(exit::OK)
}

/// What to do after `init`: keep the cache out of git, take a first look, and carry over the
/// rule choices of the linters whose suppressions were found (but the one `--from` migrated).
fn next_steps(dir: &std::path::Path, proposed: &[&str], migrated: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let ignored = std::fs::read_to_string(dir.join(".gitignore")).is_ok_and(|t| {
        t.lines()
            .any(|l| l.trim().trim_matches('/').trim_start_matches("**/") == ".mote")
    });
    if !ignored {
        out.push("add `.mote/` to .gitignore: scans keep their analysis cache there".to_string());
    }
    out.push(
        "next: `mote scan --no-gate` for a first look; to gate only new problems, save a \
         baseline (`mote scan --no-gate --json mote-baseline.json`) or scan with `--base \
         <branch>`"
            .to_string(),
    );
    for (source, what) in [("pmd", "PMD ruleset"), ("eslint", "ESLint configuration")] {
        if proposed.contains(&source) && migrated != Some(source) {
            out.push(format!(
                "{source} markers found: `mote init --force --from {source}` writes this \
                 starter again with the rules of your {what} (found in the directory, or name \
                 it: `--from {source} <file>`); `mote migrate --from {source} <file>` only \
                 prints them"
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_is_valid() {
        let cfg = mote_engine::Config::from_yaml_str(STARTER).unwrap();
        let registry = crate::plugins::registry().unwrap();
        cfg.validate(&registry).unwrap();
        assert_eq!(cfg, {
            let mut d = mote_engine::Config {
                mote: Some(">=0.1".into()),
                exclude: vec![
                    "**/node_modules/**".into(),
                    "**/vendor/**".into(),
                    "**/generated/**".into(),
                    "**/.yarn/**".into(),
                ],
                ..Default::default()
            };
            d.gate = mote_engine::config::GateConfig::default();
            d
        });
    }

    #[test]
    fn compat_is_proposed_from_the_suppressions_found() {
        assert!(STARTER.contains(COMPAT_DEFAULT));
        let parse = |text: &str| Config::from_yaml_str(text).unwrap();
        let tally = |n: usize| compat::Tally {
            occurrences: n,
            files: 1,
        };
        // Nothing found: the default, unchanged.
        assert_eq!(starter_for(&[], &CompatUsage::default(), None), STARTER);
        let usage = CompatUsage {
            sources: [
                ("eslint", tally(412)),
                ("nosonar", tally(3)),
                ("intellij", tally(2)),
            ]
            .into_iter()
            .collect(),
            intellij_annotations: tally(2),
            shared: tally(57),
            shared_keys: [("rawtypes", 7), ("unused", 50)].into_iter().collect(),
            ..Default::default()
        };
        let text = starter_for(&[], &usage, None);
        let block = "  # Other tools' suppressions found in this repository (occurrences): eslint 412, intellij 2,\n  #   nosonar 3.\n";
        assert!(text.contains(block), "{text}");
        assert!(
            text.contains("  # Not proposed: intellij, with 0 //noinspection comments and 2 @SuppressWarnings"),
            "{text}"
        );
        assert!(
            text.contains(
                "  # 57 @SuppressWarnings with javac/Eclipse keys (rawtypes 7, unused 50)"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "  # Other sources: checkstyle, intellij, pmd, spotbugs, suppress-warnings-all"
            ),
            "{text}"
        );
        assert!(text.contains("\n  compat: [eslint, nosonar]\n"), "{text}");
        assert!(!text.contains("compat: false"), "{text}");
        for line in compat_block(&usage, None).lines() {
            assert!(line.chars().count() <= 100, "{line}");
        }
        let c = parse(&text);
        assert_eq!(
            c.suppressions.compat,
            mote_engine::config::CompatSetting::Sources(vec!["eslint".into(), "nosonar".into()])
        );
        let registry = crate::plugins::registry().unwrap();
        c.validate(&registry).unwrap();
        // Found, but nothing to propose: the default with the counts above it.
        let only_shared = CompatUsage {
            shared: tally(4),
            shared_keys: [("unused", 4)].into_iter().collect(),
            ..Default::default()
        };
        let text = starter_for(&[], &only_shared, None);
        assert!(
            text.contains("  # 4 @SuppressWarnings with javac/Eclipse keys (unused 4)"),
            "{text}"
        );
        assert!(text.contains(COMPAT_DEFAULT), "{text}");
        assert_eq!(parse(&text), parse(STARTER));
    }

    #[test]
    fn suggestions_are_listed_commented_out() {
        assert!(STARTER.contains(EXCLUDE_LAST));
        let starter_with = |s: &[Suggestion]| starter_for(s, &CompatUsage::default(), None);
        assert_eq!(starter_with(&[]), STARTER);
        let text = starter_with(&[Suggestion {
            glob: "lib/org/apache/wink/**".into(),
            reason: "1 Java file in org.apache.wink.*".into(),
        }]);
        assert!(
            text.contains(
                "  - \"**/.yarn/**\"\n  # Possibly third-party code copied into the repository"
            ),
            "{text}"
        );
        assert!(
            text.contains("  # - \"lib/org/apache/wink/**\"  # 1 Java file in org.apache.wink.*\n")
        );
        let with = mote_engine::Config::from_yaml_str(&text).unwrap();
        let without = mote_engine::Config::from_yaml_str(STARTER).unwrap();
        assert_eq!(
            with, without,
            "suggestions never exclude anything by themselves"
        );
    }
}
