//! The serializable result of a scan.
//!
//! Everything in [`ScanResult`] except [`ScanStats`] is deterministic: the same input produces the
//! same value regardless of thread count (findings are sorted by path, line, column, rule,
//! message, fingerprint; files by path). `stats` holds timings and cache counters; reporters that
//! need byte-identical output should leave it out.

use crate::analyze::SkipReason;
use crate::gate::GateResult;
use crate::measures::{DirectoryMeasures, EffortMinutes, FileMeasures, Measures};
use crate::newcode::ChangeKind;
use crate::registry::RuleInfo;
use crate::source::SourceInfo;
use mote_core::{Finding, Severity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version of the results format written by this engine (see [`ScanResult::format_version`]).
pub const RESULTS_FORMAT_VERSION: u32 = 1;

/// The full result of a scan.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScanResult {
    /// Version of this results format ([`RESULTS_FORMAT_VERSION`]). Within one version, fields
    /// are only ever added; removing, renaming or retyping a field bumps it. Results written
    /// before the format was versioned read back as 0.
    #[serde(default)]
    pub format_version: u32,
    /// Engine version that produced the result.
    pub engine_version: String,
    /// What was scanned.
    pub source: SourceInfo,
    /// All findings (including suppressed ones, which carry `suppressed`), sorted.
    pub findings: Vec<Finding>,
    /// Every file with a language plugin, sorted by path.
    pub files: Vec<FileSummary>,
    /// Duplicated blocks with all their occurrences.
    pub duplications: Vec<DuplicationBlock>,
    /// Project and per-language measures.
    pub measures: ProjectMeasures,
    /// Maintainability and reliability grades (`None` only in results written before grades
    /// existed; `mote gate` computes them again).
    #[serde(default)]
    pub grades: Option<crate::grades::Grades>,
    /// New-code analysis, when a base was given.
    pub new_code: Option<NewCodeResult>,
    /// Quality gate.
    pub gate: GateResult,
    /// Metadata of every rule enabled for at least one scanned file, sorted by id (for SARIF).
    pub rules: Vec<RuleInfo>,
    /// Warnings and non-fatal errors.
    pub diagnostics: Vec<Diagnostic>,
    /// How the triage file was applied (`None` when there is none).
    #[serde(default)]
    pub triage: Option<crate::triage::TriageReport>,
    /// Counters and timings (not deterministic).
    pub stats: ScanStats,
    /// State for [`Scanner::rescan`](crate::Scanner::rescan) (not serialized; `None` when read
    /// back from JSON).
    #[serde(skip)]
    pub retained: Option<crate::scan::Retained>,
}

impl ScanResult {
    /// Findings that are not suppressed.
    pub fn active_findings(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.suppressed.is_none())
    }

    /// JSON Schema (draft 2020-12) of the results JSON (`mote scan --json`), for
    /// `mote config schema --results`.
    pub fn json_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(ScanResult);
        let mut v = serde_json::to_value(schema).unwrap_or(serde_json::Value::Null);
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "description".into(),
                format!(
                    "Mote scan results, format version {RESULTS_FORMAT_VERSION}. Fields are only added within a format version."
                )
                .into(),
            );
        }
        v
    }
}

/// How a file was handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// Analyzed in this scan.
    Analyzed,
    /// Result taken from the cache.
    Cached,
    /// Not analyzed; see `skip_reason`.
    Skipped,
}

/// One file of the scan.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileSummary {
    pub path: String,
    /// Plugin id.
    pub language: String,
    pub status: FileStatus,
    pub skip_reason: Option<SkipReason>,
    pub is_test: bool,
    /// Measures (absent for skipped files).
    pub measures: Option<FileMeasures>,
    /// Non-suppressed findings in the file.
    pub findings: u32,
    /// Estimated remediation effort of the file's findings, in minutes (see
    /// [`Measures::effort_minutes`]).
    #[serde(default)]
    pub effort_minutes: u64,
    /// How the file changed relative to the base (when a base was given and it changed).
    pub change: Option<ChangeKind>,
}

/// A duplicated block and its occurrences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DuplicationBlock {
    pub language: String,
    /// Duplicated tokens.
    pub tokens: u32,
    /// Lines of the longest occurrence.
    pub lines: u32,
    /// Sorted by path and line.
    pub occurrences: Vec<DuplicationOccurrence>,
}

/// One copy of a duplicated block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DuplicationOccurrence {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// Measures for the whole project and per language.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectMeasures {
    pub total: Measures,
    /// Keyed by plugin id.
    pub languages: BTreeMap<String, Measures>,
    /// Overall line coverage, when coverage reports were supplied.
    pub coverage: Option<CoverageSummary>,
    /// Main (non-test) code only.
    #[serde(default)]
    pub main: Measures,
    /// Test code only (`total` = `main` + `test`).
    #[serde(default)]
    pub test: Measures,
    /// Per-directory rollups down to `metrics.depth` levels, sorted by path (at most
    /// [`MAX_DIRECTORIES`](crate::measures::MAX_DIRECTORIES); the smallest are left out).
    #[serde(default)]
    pub directories: Vec<DirectoryMeasures>,
    /// Directories left out of `directories` to keep the result small.
    #[serde(default)]
    pub directories_omitted: u64,
}

/// Line coverage numbers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CoverageSummary {
    pub coverable_lines: u64,
    pub covered_lines: u64,
    pub coverage_pct: f64,
}

/// Counts by severity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SeverityCounts {
    pub high: u32,
    pub medium: u32,
    pub low: u32,
}

impl SeverityCounts {
    /// Increment the counter for `s`.
    pub fn add(&mut self, s: Severity, n: u32) {
        match s {
            Severity::High => self.high += n,
            Severity::Medium => self.medium += n,
            Severity::Low => self.low += n,
        }
    }

    /// Sum of all counters.
    pub fn total(&self) -> u32 {
        self.high + self.medium + self.low
    }
}

/// New-code analysis.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NewCodeResult {
    /// The base as requested.
    pub base: String,
    /// `merge-base(base, head)`, the commit actually compared with.
    pub merge_base: String,
    /// New or changed line ranges (inclusive) per analyzed head file.
    pub new_lines: BTreeMap<String, Vec<(u32, u32)>>,
    /// Renamed or copied files: head path → base path.
    pub renamed: BTreeMap<String, String>,
    pub measures: NewCodeMeasures,
    /// Set when the comparison was with a baseline results file (`scan --baseline`) instead of
    /// a git base: `base` is then the file, `merge_base` the commit the baseline recorded (or
    /// empty), and there is no line diff (`new_lines` is empty, line-based measures are 0).
    #[serde(default)]
    pub baseline: Option<BaselineInfo>,
}

/// The baseline results file a scan was compared with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BaselineInfo {
    /// The file as given.
    pub file: String,
    /// Commit the baseline scan recorded, when known.
    pub commit: Option<String>,
    /// Engine version that wrote the baseline.
    pub engine_version: String,
    /// Findings in the baseline (suppressed ones included).
    pub findings: u64,
    /// Baseline findings that matched a finding of this scan.
    pub matched: u64,
}

/// Measures on new code.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NewCodeMeasures {
    /// Analyzed files that changed (including new ones).
    pub changed_files: u64,
    pub new_lines: u64,
    pub new_duplicated_lines: u64,
    pub new_duplication_pct: f64,
    /// New lines that coverage reports know about (`None` without reports).
    pub new_coverable_lines: Option<u64>,
    pub new_covered_lines: Option<u64>,
    pub new_coverage_pct: Option<f64>,
    /// New, non-suppressed findings.
    pub new_findings: SeverityCounts,
    /// Worsened, non-suppressed findings.
    pub worsened_findings: SeverityCounts,
    /// Base findings that disappeared in changed files.
    pub fixed_findings: SeverityCounts,
    /// Estimated remediation effort of the new findings of stable rules (see
    /// [`Measures::effort_minutes`]).
    #[serde(default)]
    pub new_effort_minutes: EffortMinutes,
    /// New, unsuppressed reliability findings of stable rules, by severity.
    #[serde(default)]
    pub new_reliability_findings: SeverityCounts,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

/// A warning or non-fatal error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub path: Option<String>,
    pub message: String,
}

impl Diagnostic {
    /// A warning.
    pub fn warning(path: Option<&str>, message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Warning,
            path: path.map(str::to_string),
            message: message.into(),
        }
    }
}

/// Counters and timings.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScanStats {
    /// Files found by the source (after path filters).
    pub files_seen: u64,
    /// Files handled by a language plugin.
    pub files_matched: u64,
    /// Files analyzed in this scan.
    pub files_analyzed: u64,
    /// Files whose result came from the cache.
    pub cache_hits: u64,
    pub files_skipped: u64,
    /// Skipped files by reason (`excluded`, `generated`, `minified`, `vendored`, `too-large`,
    /// `binary`).
    #[serde(default)]
    pub skipped_by_reason: BTreeMap<String, u64>,
    /// Base-revision files analyzed for new-code comparison (0 with a baseline file).
    pub base_files_analyzed: u64,
    pub base_cache_hits: u64,
    pub threads: usize,
    pub cache_enabled: bool,
    pub cache_path: Option<String>,
    pub timings: Timings,
    pub languages: BTreeMap<String, LanguageStats>,
}

/// Wall-clock phase durations in milliseconds.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Timings {
    pub discovery_ms: f64,
    pub diff_ms: f64,
    pub map_ms: f64,
    pub cpd_ms: f64,
    pub new_code_ms: f64,
    pub coverage_ms: f64,
    pub cache_write_ms: f64,
    pub total_ms: f64,
}

/// Per-language work done in this scan.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LanguageStats {
    /// Files freshly analyzed.
    pub files: u64,
    pub bytes: u64,
    /// Summed per-file analysis time across threads.
    pub analyze_ms: f64,
}
