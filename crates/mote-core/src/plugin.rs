//! The language-plugin contract.
//!
//! A plugin turns one source file into a [`FileAnalysis`]: its own rule findings plus the raw
//! material (functions, comments, suppressions) that the engine's generic rules, metrics,
//! duplication detection, and suppression handling consume. Plugins never see other files,
//! configuration files, git, or the network — that is the engine's job.

use crate::model::{ParamValue, RawFinding, RuleMeta, Severity, TextRange};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

/// A file handed to a plugin.
pub struct SourceFile<'a> {
    /// Repo-relative path with forward slashes.
    pub path: &'a str,
    pub text: &'a str,
    /// Matched a test-code glob; plugins may relax rules for tests.
    pub is_test: bool,
}

/// Effective settings of one enabled rule for one file.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveRule {
    pub severity: Severity,
    pub params: BTreeMap<String, ParamValue>,
}

/// The enabled rules (with their parameters) for one file.
///
/// The engine builds one `RuleSet` per distinct effective configuration (plugin, test status,
/// set of matching `overrides`, Java release) and hands that same instance to every file that
/// shares it. A set never changes after [`RuleSet::new`], so whatever a plugin derives from it —
/// which of its rules are enabled, their parameters, dispatch tables filtered to the enabled
/// rules — can be computed **once per configuration** with [`RuleSet::memo`] (usually from
/// [`RuleSet::index`], the set in the plugin's own rule order) instead of once per file. The
/// string-keyed lookups below remain for occasional use.
#[derive(Clone, Default)]
pub struct RuleSet {
    rules: FxHashMap<String, ActiveRule>,
    /// The value of the first [`RuleSet::memo`] call, with the key it was built for.
    memo: OnceLock<(MemoKey, Arc<Memo>)>,
}

/// A memoized value. Unwind-safe like everything else in a `RuleSet`, so the set stays
/// `UnwindSafe` / `RefUnwindSafe`.
type Memo = dyn Any + Send + Sync + UnwindSafe + RefUnwindSafe;

impl fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleSet")
            .field("rules", &self.rules)
            .finish()
    }
}

/// Identifies one user of [`RuleSet::memo`] — in practice one plugin instance (its rule list) —
/// so that values derived from a rule set by different users never mix. Create one per user with
/// [`MemoKey::unique`] and keep it for the user's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoKey(u64);

impl MemoKey {
    /// A key no other call (in this process) returns.
    pub fn unique() -> MemoKey {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        MemoKey(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// A [`RuleSet`] resolved against a fixed list of rule ids — usually a plugin's own rules in its
/// own order (see [`RuleSet::index`]): position `i` holds the settings of the `i`-th id, or
/// nothing when that rule is disabled. Lookups are by position, without hashing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleIndex {
    rules: Vec<Option<ActiveRule>>,
    enabled: usize,
}

impl RuleIndex {
    /// Number of ids indexed (enabled or not).
    pub fn len(&self) -> usize {
        self.rules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
    /// Number of enabled rules among the ids.
    pub fn enabled_count(&self) -> usize {
        self.enabled
    }
    /// Whether the `i`-th id is enabled (`false` past the end).
    #[inline]
    pub fn is_enabled(&self, i: usize) -> bool {
        matches!(self.rules.get(i), Some(Some(_)))
    }
    /// Settings of the `i`-th id, when enabled.
    #[inline]
    pub fn get(&self, i: usize) -> Option<&ActiveRule> {
        self.rules.get(i).and_then(Option::as_ref)
    }
    /// A configured parameter of the `i`-th id (as [`RuleSet::param`] for its rule id).
    #[inline]
    pub fn param(&self, i: usize, name: &str) -> Option<&ParamValue> {
        self.get(i).and_then(|r| r.params.get(name))
    }
    /// Positions of the enabled ids, ascending.
    pub fn enabled(&self) -> impl Iterator<Item = usize> + '_ {
        self.rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_some())
            .map(|(i, _)| i)
    }
}

impl RuleSet {
    pub fn new(rules: FxHashMap<String, ActiveRule>) -> Self {
        RuleSet {
            rules,
            memo: OnceLock::new(),
        }
    }

    /// This set resolved against `ids` (a plugin's rule ids, in the plugin's order): position `i`
    /// of the result is the [`ActiveRule`] of the `i`-th id, or `None` when it is disabled. One
    /// hash lookup per id; build it once per set inside [`RuleSet::memo`], then look rules up by
    /// position.
    pub fn index<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> RuleIndex {
        let rules: Vec<Option<ActiveRule>> = ids
            .into_iter()
            .map(|id| self.rules.get(id).cloned())
            .collect();
        let enabled = rules.iter().filter(|r| r.is_some()).count();
        RuleIndex { rules, enabled }
    }

    /// A value derived from this set by `build`, computed **once per set** for `key` and shared
    /// by every later call with the same `key` and `T`, from any thread.
    ///
    /// For per-configuration work a plugin would otherwise repeat for every file: resolving the
    /// set into its own rule order ([`RuleSet::index`]), filtering its dispatch tables to the
    /// enabled rules, reading parameters. `build` must depend only on the set and on data of the
    /// `key`'s owner that never changes (its rule list) — never on a file.
    ///
    /// A set keeps one value: the first `key` (and `T`) to ask owns the slot. Any other caller
    /// still gets a correct value, built afresh on each call — fine for tests that share a set
    /// between plugin instances; the engine gives every plugin its own sets. Two threads asking
    /// first at the same time may both build; one result is kept.
    pub fn memo<T: Any + Send + Sync + UnwindSafe + RefUnwindSafe>(
        &self,
        key: MemoKey,
        build: impl FnOnce(&RuleSet) -> T,
    ) -> Arc<T> {
        if let Some((owner, value)) = self.memo.get() {
            if *owner == key {
                let value: Arc<Memo> = Arc::clone(value);
                let value: Arc<dyn Any + Send + Sync> = value;
                if let Ok(v) = value.downcast::<T>() {
                    return v;
                }
            }
            return Arc::new(build(self));
        }
        let value = Arc::new(build(self));
        let shared: Arc<Memo> = value.clone();
        // Lost a race to another first caller: `value` is still a correct result.
        let _ = self.memo.set((key, shared));
        value
    }
    pub fn is_enabled(&self, rule_id: &str) -> bool {
        self.rules.contains_key(rule_id)
    }
    pub fn get(&self, rule_id: &str) -> Option<&ActiveRule> {
        self.rules.get(rule_id)
    }
    pub fn param(&self, rule_id: &str, name: &str) -> Option<&ParamValue> {
        self.rules.get(rule_id).and_then(|r| r.params.get(name))
    }
    pub fn param_i64(&self, rule_id: &str, name: &str, fallback: i64) -> i64 {
        self.param(rule_id, name)
            .and_then(ParamValue::as_i64)
            .unwrap_or(fallback)
    }
    pub fn param_f64(&self, rule_id: &str, name: &str, fallback: f64) -> f64 {
        self.param(rule_id, name)
            .and_then(ParamValue::as_f64)
            .unwrap_or(fallback)
    }
    pub fn param_bool(&self, rule_id: &str, name: &str, fallback: bool) -> bool {
        self.param(rule_id, name)
            .and_then(ParamValue::as_bool)
            .unwrap_or(fallback)
    }
    pub fn param_str<'a>(&'a self, rule_id: &str, name: &str, fallback: &'a str) -> &'a str {
        self.param(rule_id, name)
            .and_then(ParamValue::as_str)
            .unwrap_or(fallback)
    }
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.rules.keys().map(String::as_str)
    }
    pub fn len(&self) -> usize {
        self.rules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Per-function measurements; generic rules (complexity, length, parameters) run on these.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FunctionMetric {
    /// Display name, e.g. `OrderService#total` or `<anonymous>`.
    pub name: String,
    /// Where to report (usually the function name / signature).
    pub range: TextRange,
    pub start_line: u32,
    pub end_line: u32,
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub params: u32,
    pub max_nesting: u32,
    pub statements: u32,
}

/// A source comment.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub start_line: u32,
    pub end_line: u32,
    pub start_col: u32,
    /// Raw text including delimiters (`//`, `/* */`).
    pub text: String,
    /// Whether code precedes the comment on its first line (a trailing comment).
    pub trailing: bool,
}

/// A suppression recognised by a plugin from language syntax (e.g. Java `@SuppressWarnings`).
/// Comment-based suppressions are handled by the engine from [`FileAnalysis::comments`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScopedSuppression {
    pub start_line: u32,
    pub end_line: u32,
    /// `None` = all rules.
    pub rules: Option<Vec<String>>,
    pub kind: String,
    pub justification: Option<String>,
}

/// How to tokenize the language for duplication detection and line counting.
#[derive(Clone, Debug)]
pub struct LexerSpec {
    pub line_comment: &'static str,
    pub block_comment: (&'static str, &'static str),
    /// Java `"""` text blocks.
    pub text_blocks: bool,
    /// JS/TS template literals.
    pub template_literals: bool,
    /// Lines starting with these (after whitespace) are excluded from duplication detection.
    pub cpd_skip_line_prefixes: &'static [&'static str],
}

/// Everything a plugin produces for one file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub findings: Vec<RawFinding>,
    pub functions: Vec<FunctionMetric>,
    pub comments: Vec<Comment>,
    pub suppressions: Vec<ScopedSuppression>,
    pub classes: u32,
    pub statements: u32,
    /// Syntax errors the parser recovered from.
    pub syntax_errors: u32,
    /// Opaque, plugin-defined facts for future cross-file (reduce-phase) analysis.
    #[serde(default)]
    pub facts: Option<Vec<u8>>,
}

/// A language plugin. Implementations must be deterministic and must not panic on any input
/// (the engine isolates panics anyway, but they cost the file its findings).
pub trait LanguagePlugin: Send + Sync {
    /// Short identifier used as the rule-id prefix, e.g. `java`.
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// File extensions handled, without dots.
    fn extensions(&self) -> &'static [&'static str];
    /// Bump when analysis output can change for the same input (invalidates caches).
    fn version(&self) -> u32;
    /// Metadata for the plugin's own rules (generic rules are added by the engine).
    fn rules(&self) -> Vec<&'static RuleMeta>;
    fn lexer(&self) -> LexerSpec;
    /// Globs identifying test code when the configuration doesn't say otherwise.
    fn default_test_globs(&self) -> &'static [&'static str];
    /// Analyze one file with the enabled `rules`. Files with the same effective configuration
    /// get the same `RuleSet` instance: derive per-configuration data once with
    /// [`RuleSet::memo`] rather than per file.
    fn analyze(&self, file: &SourceFile<'_>, rules: &RuleSet) -> FileAnalysis;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[(&str, i64)]) -> RuleSet {
        RuleSet::new(
            ids.iter()
                .map(|(id, max)| {
                    (
                        id.to_string(),
                        ActiveRule {
                            severity: Severity::Medium,
                            params: BTreeMap::from([("max".to_string(), ParamValue::Int(*max))]),
                        },
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn index_follows_the_given_order() {
        let rules = set(&[("x/b", 2), ("x/d", 4)]);
        let index = rules.index(["x/a", "x/b", "x/c", "x/d"]);
        assert_eq!(index.len(), 4);
        assert_eq!(index.enabled_count(), 2);
        assert_eq!(index.enabled().collect::<Vec<_>>(), vec![1, 3]);
        assert!(!index.is_enabled(0) && index.is_enabled(1) && !index.is_enabled(9));
        assert_eq!(index.param(3, "max"), Some(&ParamValue::Int(4)));
        assert_eq!(index.param(3, "min"), None);
        assert_eq!(index.param(0, "max"), None);
        assert_eq!(index.get(1), rules.get("x/b"));
    }

    #[test]
    fn memo_builds_once_per_set_and_key() {
        let key = MemoKey::unique();
        let builds = std::sync::atomic::AtomicUsize::new(0);
        let a = set(&[("x/a", 1)]);
        let b = set(&[("x/a", 5)]);
        let max = |rs: &RuleSet| {
            rs.memo(key, |rs| {
                builds.fetch_add(1, Ordering::Relaxed);
                rs.index(["x/a"]).param(0, "max").cloned()
            })
        };
        for _ in 0..3 {
            assert_eq!(*max(&a), Some(ParamValue::Int(1)));
            assert_eq!(*max(&b), Some(ParamValue::Int(5)));
        }
        assert_eq!(builds.load(Ordering::Relaxed), 2);
        // A clone carries the value along (same rules, same result).
        assert_eq!(*max(&a.clone()), Some(ParamValue::Int(1)));
        assert_eq!(builds.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn rule_sets_stay_thread_and_unwind_safe() {
        fn check<T: Send + Sync + UnwindSafe + RefUnwindSafe + Clone + Default>() {}
        check::<RuleSet>();
    }

    #[test]
    fn memo_of_another_key_or_type_is_built_afresh() {
        let (k1, k2) = (MemoKey::unique(), MemoKey::unique());
        assert_ne!(k1, k2);
        let rules = set(&[("x/a", 1)]);
        assert_eq!(*rules.memo(k1, |_| 1u32), 1);
        assert_eq!(*rules.memo(k2, |_| 2u32), 2);
        assert_eq!(*rules.memo(k1, |_| "other type"), "other type");
        assert_eq!(*rules.memo(k1, |_| 3u32), 1, "k1's value is kept");
        assert_eq!(*rules.memo(k2, |_| 4u32), 4, "k2 never owns the slot");
    }
}
