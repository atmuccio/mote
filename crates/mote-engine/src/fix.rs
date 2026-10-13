//! Applying fixes (`mote fix`, the MCP `fixes` tool): the findings' [`Fix`]es of a file applied
//! in memory, the file analyzed again, repeated until no fix is left.
//!
//! [`Scanner::fix_files`] works on the working tree only. Each file is read once, and the fixes
//! of its findings — unsuppressed (in the source or by a triage decision), of a rule that is
//! enabled for the file and selected — are applied in passes: every fix that does not conflict
//! with another (see [`mote_core::fix::apply`]), then the text is analyzed again with the same
//! rules and settings a scan uses, and the next pass applies the fixes found then. Nothing is
//! written: the caller gets the original and the fixed bytes.
//!
//! Safety net: when the fixed text has more syntax errors than the original after any pass, all
//! of the file's changes are discarded and the file is reported. Only UTF-8 files (with or
//! without a byte-order mark) are fixed; the fixed bytes keep the mark, and line breaks the
//! fixes insert follow the file's own (`\r\n` stays `\r\n`).

use crate::analyze::{self, FileJob, FileOutcome};
use crate::java_release;
use crate::scan::{Lookups, ScanOptions, Scanner};
use crate::source::SourceInfo;
use crate::triage::{self, TriageEntry};
use crate::EngineError;
use mote_core::fix::{self as core_fix, line_break_at};
use mote_core::{Finding, Fix};
use rayon::prelude::*;
use std::collections::BTreeMap;

/// What to fix.
#[derive(Clone, Debug, Default)]
pub struct FixOptions {
    /// Only fixes of these rules (empty: every rule enabled for the file).
    pub rules: Vec<String>,
    /// Only the fixes of these findings (ids), in one pass (empty: every finding).
    pub findings: Vec<String>,
    /// Passes per file at most (0: [`core_fix::MAX_PASSES`]).
    pub max_passes: usize,
}

/// Why a file's fixes were not applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixProblem {
    /// The fixed text had more syntax errors than the original: every change was discarded.
    SyntaxErrors { before: u32, after: u32 },
    /// The fixed text could not be analyzed (the analyzer failed, or it would be skipped as
    /// generated or minified): every change was discarded.
    Unanalyzable(String),
    /// Not UTF-8 text: left alone.
    Encoding,
    /// The file could not be read.
    Unreadable(String),
}

impl FixProblem {
    /// One-line explanation.
    pub fn describe(&self) -> String {
        match self {
            FixProblem::SyntaxErrors { before, after } => format!(
                "changes discarded: the fixed file would have {after} syntax error{} instead of {before}",
                if *after == 1 { "" } else { "s" }
            ),
            FixProblem::Unanalyzable(why) => format!("changes discarded: {why}"),
            FixProblem::Encoding => "not UTF-8 text: left unchanged".into(),
            FixProblem::Unreadable(e) => format!("cannot read the file: {e}"),
        }
    }

    /// Whether fixes were computed and then thrown away (a fix that breaks code is a bug).
    pub fn is_discard(&self) -> bool {
        matches!(
            self,
            FixProblem::SyntaxErrors { .. } | FixProblem::Unanalyzable(_)
        )
    }
}

/// The outcome for one file.
#[derive(Clone, Debug)]
pub struct FileFix {
    /// Repo-relative path, as in scan results.
    pub path: String,
    /// The bytes read.
    pub original: Vec<u8>,
    /// The fixed bytes (equal to `original` when nothing was fixed or the changes were
    /// discarded).
    pub fixed: Vec<u8>,
    /// Fixes applied, by rule id (empty when the changes were discarded).
    pub applied: BTreeMap<String, u32>,
    /// Passes that applied a fix.
    pub passes: u32,
    /// Fixes still offered after the last pass (their edits kept conflicting, or the passes ran
    /// out).
    pub remaining: u32,
    pub problem: Option<FixProblem>,
}

impl FileFix {
    /// Whether the fixed bytes differ from the original.
    pub fn changed(&self) -> bool {
        self.fixed != self.original
    }

    /// Fixes applied, all rules.
    pub fn applied_total(&self) -> u32 {
        self.applied.values().sum()
    }
}

/// A finding whose fix may be applied under `opts`: unsuppressed, with a fix, of a selected rule
/// (and finding), and not matched by a triage decision.
pub fn is_selected(f: &Finding, opts: &FixOptions) -> bool {
    f.fix.is_some()
        && f.suppressed.is_none()
        && (opts.rules.is_empty() || opts.rules.contains(&f.rule_id))
        && (opts.findings.is_empty() || opts.findings.contains(&f.id))
}

/// Per-call state of [`Scanner::fix_files`].
struct Fixer<'s> {
    scanner: &'s Scanner,
    lookups: Lookups,
    /// Triage entries, for the files fixed.
    triage: Vec<TriageEntry>,
    opts: &'s FixOptions,
}

impl Scanner {
    /// Apply the fixes of `paths` (repo-relative, `/`-separated, as in scan results; usually the
    /// files of a scan with fixable findings) as described in the [module docs](crate::fix),
    /// without writing anything. `scan_opts` gives the triage file and the threads.
    pub fn fix_files(
        &self,
        paths: &[String],
        opts: &FixOptions,
        scan_opts: &ScanOptions,
    ) -> Result<Vec<FileFix>, EngineError> {
        let info = SourceInfo {
            kind: "worktree".into(),
            rev: None,
            commit: None,
            prefix: String::new(),
        };
        let triage = self
            .load_triage(scan_opts, &info)?
            .map(|t| t.file.entries)
            .unwrap_or_default();
        let fixer = Fixer {
            scanner: self,
            lookups: self.lookups(None),
            triage,
            opts,
        };
        Ok(self.with_pool(scan_opts, || {
            paths
                .par_iter()
                .with_max_len(1)
                .map(|p| fixer.fix_file(p))
                .collect()
        }))
    }
}

/// The analysis of one text, as a scan of the working tree would make it.
enum Analysis {
    Done {
        findings: Vec<Finding>,
        syntax_errors: u32,
    },
    Failed(String),
}

impl Fixer<'_> {
    fn analyze(&self, path: &str, bytes: &[u8]) -> Analysis {
        let s = self.scanner;
        let Some(plugin) = s.registry.plugin_for_path(path) else {
            return Analysis::Failed("no analyzer for this file".into());
        };
        if s.compiled.is_excluded(path) {
            return Analysis::Failed("excluded by the configuration".into());
        }
        let is_test = s.compiled.is_test(plugin, path);
        let (skip_generated, packages) = s.skip_checks(path, &self.lookups);
        let release = (s.registry.plugin(plugin).id() == java_release::JAVA)
            .then(|| self.lookups.releases.release_for(path))
            .flatten()
            .map(|r| r.release);
        let rules = s
            .compiled
            .resolve_with_release(&s.registry, plugin, is_test, path, release);
        let job = FileJob {
            registry: &s.registry,
            plugin,
            path,
            is_test,
            rules: &rules,
            settings: &s.settings,
            skip_generated,
            packages: packages.as_deref().unwrap_or(&[]),
            keep_tokens: false,
        };
        match analyze::analyze_bytes(&job, bytes) {
            FileOutcome::Skipped(reason) => {
                Analysis::Failed(format!("it would be skipped ({})", reason.as_str()))
            }
            FileOutcome::Analyzed(r) if r.internal_error => {
                Analysis::Failed("the analyzer failed on it".into())
            }
            FileOutcome::Analyzed(r) => Analysis::Done {
                syntax_errors: r.measures.syntax_errors,
                findings: r.findings,
            },
        }
    }

    /// The fixes to apply among `findings`: selected, not triaged, line breaks as in the file.
    fn fixes(&self, path: &str, text: &str, findings: &[Finding]) -> Vec<(String, Fix)> {
        let mut candidates: Vec<&Finding> = findings
            .iter()
            .filter(|f| is_selected(f, self.opts))
            .collect();
        if !self.triage.is_empty() && !candidates.is_empty() {
            let entries: Vec<TriageEntry> = self
                .triage
                .iter()
                .filter(|e| e.path == path)
                .cloned()
                .collect();
            if !entries.is_empty() {
                let matched = triage::match_entries(&entries, &candidates, &|_| true);
                let mut keep = matched.iter().map(Option::is_none);
                candidates.retain(|_| keep.next().unwrap_or(true));
            }
        }
        candidates
            .into_iter()
            .filter_map(|f| {
                let mut fix = f.fix.clone()?;
                for e in &mut fix.edits {
                    if e.replacement.contains('\n')
                        && line_break_at(text, e.start as usize) == "\r\n"
                    {
                        e.replacement = crlf(&e.replacement);
                    }
                }
                Some((f.rule_id.clone(), fix))
            })
            .collect()
    }

    fn fix_file(&self, path: &str) -> FileFix {
        let full = self.scanner.root.join(path);
        let mut out = FileFix {
            path: path.to_string(),
            original: Vec::new(),
            fixed: Vec::new(),
            applied: BTreeMap::new(),
            passes: 0,
            remaining: 0,
            problem: None,
        };
        let original = match std::fs::read(&full) {
            Ok(b) => b,
            Err(e) => {
                out.problem = Some(FixProblem::Unreadable(e.to_string()));
                return out;
            }
        };
        out.original = original.clone();
        out.fixed = original;
        const BOM: &[u8] = b"\xEF\xBB\xBF";
        let bom = out.original.starts_with(BOM);
        let body = if bom {
            &out.original[BOM.len()..]
        } else {
            &out.original[..]
        };
        let Ok(text) = std::str::from_utf8(body) else {
            out.problem = Some(FixProblem::Encoding);
            return out;
        };
        let encode = |t: &str| -> Vec<u8> {
            let mut b = Vec::with_capacity(t.len() + 3);
            if bom {
                b.extend_from_slice(BOM);
            }
            b.extend_from_slice(t.as_bytes());
            b
        };
        let (mut findings, before) = match self.analyze(path, &out.original) {
            Analysis::Done {
                findings,
                syntax_errors,
            } => (findings, syntax_errors),
            Analysis::Failed(_) => return out,
        };
        let max_passes = match (self.opts.max_passes, self.opts.findings.is_empty()) {
            (_, false) => 1,
            (0, true) => core_fix::MAX_PASSES,
            (n, true) => n,
        };
        let mut text = text.to_string();
        let mut applied: BTreeMap<String, u32> = BTreeMap::new();
        for _ in 0..max_passes {
            let fixes = self.fixes(path, &text, &findings);
            if fixes.is_empty() {
                break;
            }
            let refs: Vec<&Fix> = fixes.iter().map(|(_, f)| f).collect();
            let result = core_fix::apply(&text, &refs);
            if result.applied.is_empty() {
                break;
            }
            for &i in &result.applied {
                *applied.entry(fixes[i].0.clone()).or_default() += 1;
            }
            out.passes += 1;
            text = result.text;
            match self.analyze(path, &encode(&text)) {
                Analysis::Done {
                    findings: next,
                    syntax_errors,
                } => {
                    if syntax_errors > before {
                        out.problem = Some(FixProblem::SyntaxErrors {
                            before,
                            after: syntax_errors,
                        });
                        out.passes = 0;
                        return out;
                    }
                    findings = next;
                }
                Analysis::Failed(why) => {
                    out.problem = Some(FixProblem::Unanalyzable(why));
                    out.passes = 0;
                    return out;
                }
            }
            if !self.opts.findings.is_empty() {
                break;
            }
        }
        out.remaining = self.fixes(path, &text, &findings).len() as u32;
        if !self.opts.findings.is_empty() {
            // One pass over the named findings: what is left of them (their ids may have
            // changed with the code) is not counted.
            out.remaining = 0;
        }
        out.applied = applied;
        out.fixed = encode(&text);
        out
    }
}

/// `text` with every `\n` that is not part of `\r\n` turned into `\r\n`.
fn crlf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    let mut prev = '\0';
    for c in text.chars() {
        if c == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(c);
        prev = c;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::crlf;

    #[test]
    fn crlf_converts_bare_line_feeds_only() {
        assert_eq!(crlf("a\nb\r\nc\n"), "a\r\nb\r\nc\r\n");
        assert_eq!(crlf("x"), "x");
    }
}
