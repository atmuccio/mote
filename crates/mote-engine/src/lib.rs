//! Mote scan engine.
//!
//! Turns a set of [`LanguagePlugin`](mote_core::LanguagePlugin)s, a [`Config`] and a
//! repository into a [`ScanResult`]:
//!
//! ```text
//! sources ─► filter ─► cache lookup ─► MAP (parallel): plugin + lexer + generic rules
//!                                      + suppressions + fingerprints
//!         ─► REDUCE: duplication (CPD), measures
//!         ─► TRIAGE (optional): the triage file's decisions as an overlay (never cached)
//!         ─► NEW CODE (optional): diff vs merge-base, base-blob analysis, matching —
//!            or a baseline results file, matched without a diff
//!         ─► coverage ─► GATE ─► ScanResult
//! ```
//!
//! Entry points: [`Registry::new`], [`Config::discover`] / [`scan::discover_config`],
//! [`Scanner::new`], [`Scanner::scan`], [`Scanner::rescan`] (incremental update of a previous
//! result), [`Scanner::analyze_paths`], [`Scanner::fix_files`] (the findings' fixes applied in
//! memory, for `mote fix`), [`gate::apply_coverage`] / [`gate::reevaluate_gate`] (coverage and
//! gate on a saved result), and [`git::clone_remote`] for remote repositories.
//! See the crate README for a walkthrough.

pub mod analyze;
pub mod baseline;
pub mod cache;
pub mod config;
pub mod coverage;
pub mod cpd;
pub mod fingerprint;
pub mod fix;
pub mod gate;
pub mod generic;
pub mod git;
pub mod grades;
pub mod java_release;
pub mod lexer;
pub mod matching;
pub mod measures;
pub mod newcode;
pub mod packages;
pub mod registry;
pub mod result;
pub mod scan;
pub mod source;
pub mod suppress;
pub mod triage;
pub mod vendored;
pub mod worktree;

pub use config::{
    CompiledConfig, Config, ConfigError, ConfigProblem, ForeignRule, ForeignRuleNames, JavaConfig,
    ENGINE_VERSION,
};
pub use gate::{ConditionResult, GateResult};
pub use java_release::{JavaRelease, ReleaseOrigin, JAVA_RELEASE_PARAM};
pub use registry::{Registry, RegistryError, RuleInfo};
pub use result::ScanResult;
pub use scan::{EngineError, ScanOptions, Scanner, SourceSpec, ANALYSIS_STACK_BYTES};

#[cfg(test)]
#[path = "../tests/common/fake_plugin.rs"]
mod fake_plugin;
#[cfg(test)]
mod testutil;
