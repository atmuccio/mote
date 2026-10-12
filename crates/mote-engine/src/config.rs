//! `.mote.yaml`: schema, loading, validation, and per-file rule resolution.
//!
//! # Rule precedence
//!
//! The effective [`RuleSet`] of a file is built in layers:
//!
//! 1. the profile selected by `extends` (`default`, `conventions`, `all`, `none`);
//! 2. `rules`;
//! 3. `test_rules`, when the file is test code;
//! 4. every entry of `overrides` whose `paths` match the file, in order.
//!
//! Within one layer, keys are applied in an order that does not depend on how the YAML was
//! written: glob keys first (less specific before more specific, where specificity is the number
//! of non-wildcard characters), then exact rule ids. A string value (`on`, `LOW`, `MEDIUM`,
//! `HIGH`) enables the rule; `off` disables it; a map changes `severity` / `params` and, for an
//! exact id, also enables the rule unless it says `enabled: false`. Glob keys never enable
//! deprecated rules.

mod problems;

use crate::grades::Grade;
use crate::registry::{CatalogEntry, Registry};
use crate::suppress::{CompatSet, COMPAT_SOURCES};
use globset::{GlobBuilder, GlobMatcher, GlobSet, GlobSetBuilder};
use mote_core::{ActiveRule, ParamValue, RuleSet, RuleStatus, Severity};
use problems::{key_path, path_text, suggest, Pending, Seg};
use rustc_hash::{FxHashMap, FxHashSet};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use xxhash_rust::xxh3::Xxh3;

/// Engine version (the crate version), checked against the config's `mote` requirement.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Config file names looked up at the repository root, in order.
pub const CONFIG_FILE_NAMES: [&str; 2] = [".mote.yaml", ".mote.yml"];

/// The `.mote.yaml` configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Semver requirement on the engine version, e.g. `">=0.1"`. A mismatch is an error.
    pub mote: Option<String>,
    /// Base profile: `default` (rules enabled by default), `conventions` (`default` plus the
    /// style rules tagged `conventions`), `all` (every non-deprecated rule), or `none`.
    pub extends: Profile,
    /// Repo-relative globs of files to skip entirely.
    pub exclude: Vec<String>,
    /// Extra globs identifying test code (added to each language's defaults).
    pub tests: Vec<String>,
    /// Files larger than this many bytes are skipped.
    pub max_file_size: u64,
    /// Skip generated files (header markers, Yarn Plug'n'Play files), minified files, and vendored
    /// JavaScript (library banners, bundler output).
    pub skip_generated: bool,
    /// Repo-relative globs of files analyzed even when they look generated, minified or vendored
    /// (exceptions to `skip_generated`). `exclude` still wins, and files larger than
    /// `max_file_size` or binary files are still skipped.
    pub skip_generated_except: Vec<String>,
    /// Rule settings; keys are rule ids or globs such as `java/*`.
    pub rules: BTreeMap<String, RuleConfig>,
    /// Rule settings applied on top of `rules` for test files.
    pub test_rules: BTreeMap<String, RuleConfig>,
    /// Path-specific rule settings, applied in order.
    pub overrides: Vec<Override>,
    /// Quality gate.
    pub gate: GateConfig,
    /// Duplication detection.
    pub cpd: CpdConfig,
    /// Coverage report ingestion.
    pub coverage: CoverageConfig,
    /// Comment suppressions.
    pub suppressions: SuppressionConfig,
    /// Triage decisions kept in a file (accepted findings, false positives).
    pub triage: TriageConfig,
    /// Result cache.
    pub cache: CacheConfig,
    /// Java settings.
    pub java: JavaConfig,
    /// Measures reported with the results.
    pub metrics: MetricsConfig,
    /// Maintainability and reliability grade bands.
    pub grades: GradesConfig,
    /// Where the configuration was read from, to point problems at their line and column. Not a
    /// setting: ignored by `==`, never serialized.
    #[serde(skip)]
    #[schemars(skip)]
    pub source: ConfigSource,
}

/// The file and text a [`Config`] was read from (empty for a configuration built in code). Two
/// sources always compare equal: a configuration is its settings, wherever it came from.
#[derive(Clone, Default)]
pub struct ConfigSource(Option<Arc<SourceText>>);

struct SourceText {
    path: Option<PathBuf>,
    text: String,
}

impl ConfigSource {
    fn new(path: Option<&Path>, text: &str) -> ConfigSource {
        ConfigSource(Some(Arc::new(SourceText {
            path: path.map(Path::to_path_buf),
            text: text.to_string(),
        })))
    }

    /// The file (or other origin, such as `HEAD:.mote.yaml`), when known.
    pub fn path(&self) -> Option<&Path> {
        self.0.as_ref().and_then(|s| s.path.as_deref())
    }

    /// The text, when the configuration was parsed from one.
    pub fn text(&self) -> Option<&str> {
        self.0.as_ref().map(|s| s.text.as_str())
    }
}

impl PartialEq for ConfigSource {
    fn eq(&self, _: &ConfigSource) -> bool {
        true
    }
}

impl fmt::Debug for ConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.path() {
            Some(p) => write!(f, "ConfigSource({})", p.display()),
            None => f.write_str("ConfigSource"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mote: None,
            extends: Profile::Default,
            exclude: Vec::new(),
            tests: Vec::new(),
            max_file_size: 1024 * 1024,
            skip_generated: true,
            skip_generated_except: Vec::new(),
            rules: BTreeMap::new(),
            test_rules: BTreeMap::new(),
            overrides: Vec::new(),
            gate: GateConfig::default(),
            cpd: CpdConfig::default(),
            coverage: CoverageConfig::default(),
            suppressions: SuppressionConfig::default(),
            triage: TriageConfig::default(),
            cache: CacheConfig::default(),
            java: JavaConfig::default(),
            metrics: MetricsConfig::default(),
            grades: GradesConfig::default(),
            source: ConfigSource::default(),
        }
    }
}

/// The rule tag that adds an off-by-default rule to the [`Profile::Conventions`] profile.
pub const CONVENTIONS_TAG: &str = "conventions";

/// Base rule profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    /// Every rule marked `default_enabled` (except deprecated ones).
    #[default]
    Default,
    /// `default` plus every rule tagged `conventions`, an opt-in style pass (except deprecated
    /// ones).
    Conventions,
    /// Every rule except deprecated ones.
    All,
    /// No rules; enable them one by one.
    None,
}

/// Path-specific rule settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Repo-relative globs.
    pub paths: Vec<String>,
    /// Rule settings for matching files.
    #[serde(default)]
    pub rules: BTreeMap<String, RuleConfig>,
}

/// Setting for one rule (or a glob of rules).
#[derive(Clone, Debug, PartialEq)]
pub enum RuleConfig {
    /// `off`, `on`, `LOW`, `MEDIUM`, `HIGH` (or `true` / `false`).
    Level(RuleLevel),
    /// `{ enabled, severity, params }`.
    Options(RuleOptions),
}

/// Short string form of a rule setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleLevel {
    Off,
    On,
    Severity(Severity),
}

/// Map form of a rule setting.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleOptions {
    /// Enable or disable the rule. Omitted: exact ids are enabled, globs unchanged.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Severity override.
    #[serde(default, deserialize_with = "de_opt_severity")]
    #[schemars(schema_with = "opt_severity_schema")]
    pub severity: Option<Severity>,
    /// Parameter overrides (merged with earlier layers).
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
}

/// Case-insensitive match of the whole string against `words` in a JSON Schema pattern
/// (ECMA-262 has no inline case-insensitivity flag): `off` → `[Oo][Ff][Ff]`.
fn ci_pattern(words: &[&str]) -> String {
    let alts: Vec<String> = words
        .iter()
        .map(|w| {
            w.chars()
                .map(|c| {
                    if c.is_ascii_alphabetic() {
                        format!("[{}{}]", c.to_ascii_uppercase(), c.to_ascii_lowercase())
                    } else {
                        c.to_string()
                    }
                })
                .collect()
        })
        .collect();
    format!("^(?:{})$", alts.join("|"))
}

/// A severity as the parser accepts it: `LOW`, `MEDIUM`, `HIGH` in any case.
fn severity_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "string",
        "anyOf": [
            { "enum": ["LOW", "MEDIUM", "HIGH", "low", "medium", "high"] },
            { "pattern": ci_pattern(&["low", "medium", "high"]) }
        ]
    })
}

fn opt_severity_schema(_: &mut SchemaGenerator) -> Schema {
    let s = severity_schema();
    json_schema!({
        "description": "Severity override: `LOW`, `MEDIUM` or `HIGH` (any case).",
        "anyOf": [s, { "type": "null" }]
    })
}

fn parse_severity(s: &str) -> Option<Severity> {
    match s.to_ascii_uppercase().as_str() {
        "LOW" => Some(Severity::Low),
        "MEDIUM" => Some(Severity::Medium),
        "HIGH" => Some(Severity::High),
        _ => None,
    }
}

fn de_opt_severity<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Severity>, D::Error> {
    let s: Option<String> = Option::deserialize(d)?;
    match s {
        None => Ok(None),
        Some(s) => parse_severity(&s).map(Some).ok_or_else(|| {
            de::Error::custom(format!(
                "invalid severity `{s}`, expected LOW, MEDIUM or HIGH"
            ))
        }),
    }
}

impl<'de> Deserialize<'de> for RuleConfig {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = RuleConfig;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("`off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, `severity`, `params`")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<RuleConfig, E> {
                Ok(RuleConfig::Level(if v {
                    RuleLevel::On
                } else {
                    RuleLevel::Off
                }))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<RuleConfig, E> {
                if v.eq_ignore_ascii_case("off") {
                    Ok(RuleConfig::Level(RuleLevel::Off))
                } else if v.eq_ignore_ascii_case("on") {
                    Ok(RuleConfig::Level(RuleLevel::On))
                } else {
                    parse_severity(v)
                        .map(|s| RuleConfig::Level(RuleLevel::Severity(s)))
                        .ok_or_else(|| E::invalid_value(de::Unexpected::Str(v), &self))
                }
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<RuleConfig, A::Error> {
                RuleOptions::deserialize(de::value::MapAccessDeserializer::new(map))
                    .map(RuleConfig::Options)
            }
        }
        d.deserialize_any(V)
    }
}

impl Serialize for RuleConfig {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            RuleConfig::Level(RuleLevel::Off) => s.serialize_str("off"),
            RuleConfig::Level(RuleLevel::On) => s.serialize_str("on"),
            RuleConfig::Level(RuleLevel::Severity(sev)) => s.serialize_str(sev.as_str()),
            RuleConfig::Options(o) => o.serialize(s),
        }
    }
}

impl JsonSchema for RuleConfig {
    fn schema_name() -> Cow<'static, str> {
        "RuleConfig".into()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let options = generator.subschema_for::<RuleOptions>();
        json_schema!({
            "description": "`off`, `on`, a severity (any case), or a map with `enabled`, `severity` and `params`.",
            "anyOf": [
                { "type": "string", "enum": ["off", "on", "LOW", "MEDIUM", "HIGH", "low", "medium", "high"] },
                { "type": "string", "pattern": ci_pattern(&["off", "on", "low", "medium", "high"]) },
                { "type": "boolean" },
                options
            ]
        })
    }
}

/// Quality gate settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct GateConfig {
    /// New findings at or above this severity fail the gate (`none` disables the condition).
    pub fail_on: FailOn,
    /// Maximum duplication on new code, in percent (`null` disables).
    pub new_duplication_pct: Option<f64>,
    /// Minimum coverage on new code, in percent (`null` disables; needs coverage reports).
    pub new_coverage_pct: Option<f64>,
    /// The duplication and coverage conditions are skipped below this many new lines (the
    /// new-code grade conditions follow `grades.min_new_lines`).
    pub min_new_lines: u32,
    /// What to do with findings whose measured value got worse.
    pub worsened: WorsenedPolicy,
    /// Worst maintainability grade allowed overall (`null`: not a condition).
    pub maintainability_grade: Option<Grade>,
    /// Worst reliability grade allowed overall (`null`: not a condition).
    pub reliability_grade: Option<Grade>,
    /// Worst maintainability grade allowed on new code (needs a base; `null`: not a condition).
    pub new_maintainability_grade: Option<Grade>,
    /// Worst reliability grade allowed on new code (needs a base; `null`: not a condition).
    pub new_reliability_grade: Option<Grade>,
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig {
            fail_on: FailOn::High,
            new_duplication_pct: Some(3.0),
            new_coverage_pct: Some(80.0),
            min_new_lines: 20,
            worsened: WorsenedPolicy::Annotate,
            maintainability_grade: None,
            reliability_grade: None,
            new_maintainability_grade: None,
            new_reliability_grade: None,
        }
    }
}

/// Severity threshold for the gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailOn {
    #[serde(rename = "HIGH", alias = "high")]
    High,
    #[serde(rename = "MEDIUM", alias = "medium")]
    Medium,
    #[serde(rename = "LOW", alias = "low")]
    Low,
    #[serde(rename = "none", alias = "NONE")]
    None,
}

impl JsonSchema for FailOn {
    fn schema_name() -> Cow<'static, str> {
        "FailOn".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        // Every spelling the parser accepts (names and aliases).
        json_schema!({
            "description": "Severity threshold for the gate.",
            "type": "string",
            "enum": ["HIGH", "MEDIUM", "LOW", "none", "high", "medium", "low", "NONE"]
        })
    }
}

impl FailOn {
    /// The minimum failing severity, if any.
    pub fn severity(self) -> Option<Severity> {
        match self {
            FailOn::High => Some(Severity::High),
            FailOn::Medium => Some(Severity::Medium),
            FailOn::Low => Some(Severity::Low),
            FailOn::None => None,
        }
    }
}

/// Handling of worsened findings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorsenedPolicy {
    /// Report them, don't fail.
    Annotate,
    /// Fail the gate when any exist.
    Fail,
}

/// Duplication detection settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct CpdConfig {
    pub enabled: bool,
    /// Minimum duplicated tokens.
    pub min_tokens: u32,
    /// Minimum duplicated lines.
    pub min_lines: u32,
    /// Globs of files excluded from duplication detection.
    pub exclude: Vec<String>,
    /// Treat all literals of a kind as equal.
    pub ignore_literals: bool,
    /// Include test files.
    pub include_tests: bool,
}

impl Default for CpdConfig {
    fn default() -> Self {
        CpdConfig {
            enabled: true,
            min_tokens: 100,
            min_lines: 10,
            exclude: Vec::new(),
            ignore_literals: true,
            include_tests: false,
        }
    }
}

/// Coverage settings.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct CoverageConfig {
    /// Globs (repo-relative) of LCOV, JaCoCo XML or Cobertura XML reports.
    pub reports: Vec<String>,
}

/// Suppression settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct SuppressionConfig {
    /// Report `mote-ignore` comments that give no reason.
    pub require_reason: bool,
    /// Other tools' suppression comments and annotations to honor: `true` (eslint, nosonar, pmd,
    /// checkstyle, suppress-warnings-all), `false` (none), or a list of sources.
    pub compat: CompatSetting,
}

impl Default for SuppressionConfig {
    fn default() -> Self {
        SuppressionConfig {
            require_reason: true,
            compat: CompatSetting::Flag(false),
        }
    }
}

/// `suppressions.compat`: `true`, `false`, or a list of sources (see
/// [`COMPAT_SOURCES`](crate::suppress::COMPAT_SOURCES)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatSetting {
    /// `true`: the sources honored before they could be chosen
    /// ([`COMPAT_TRUE`](crate::suppress::COMPAT_TRUE)); `false`: none.
    Flag(bool),
    /// Exactly these sources (a single name is a one-item list).
    Sources(Vec<String>),
}

impl Default for CompatSetting {
    fn default() -> Self {
        CompatSetting::Flag(false)
    }
}

impl CompatSetting {
    /// The selected sources (unknown names, reported by [`Config::validate`], select nothing).
    pub fn sources(&self) -> CompatSet {
        match self {
            CompatSetting::Flag(false) => CompatSet::NONE,
            CompatSetting::Flag(true) => CompatSet::legacy(),
            CompatSetting::Sources(names) => CompatSet::of(names.iter().map(String::as_str)),
        }
    }
}

impl<'de> Deserialize<'de> for CompatSetting {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = CompatSetting;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    "`true`, `false`, or a list of sources ({})",
                    COMPAT_SOURCES.join(", ")
                )
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<CompatSetting, E> {
                Ok(CompatSetting::Flag(v))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<CompatSetting, E> {
                Ok(CompatSetting::Sources(vec![v.to_string()]))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<CompatSetting, A::Error> {
                let mut names = Vec::new();
                while let Some(name) = seq.next_element::<String>()? {
                    names.push(name);
                }
                Ok(CompatSetting::Sources(names))
            }
        }
        d.deserialize_any(V)
    }
}

impl Serialize for CompatSetting {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            CompatSetting::Flag(b) => s.serialize_bool(*b),
            CompatSetting::Sources(names) => names.serialize(s),
        }
    }
}

impl JsonSchema for CompatSetting {
    fn schema_name() -> Cow<'static, str> {
        "CompatSetting".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "`true` (eslint, nosonar, pmd, checkstyle, suppress-warnings-all), `false`, or a list of sources.",
            "anyOf": [
                { "type": "boolean" },
                { "type": "array", "items": { "type": "string", "enum": COMPAT_SOURCES } },
                { "type": "string", "enum": COMPAT_SOURCES }
            ]
        })
    }
}

/// Triage file settings (see [`triage`](crate::triage)).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct TriageConfig {
    /// Triage file, relative to the scan root (default `.mote-triage.yaml`). A missing file
    /// means no decisions.
    pub file: String,
}

impl Default for TriageConfig {
    fn default() -> Self {
        TriageConfig {
            file: crate::triage::DEFAULT_TRIAGE_FILE.to_string(),
        }
    }
}

/// Cache settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct CacheConfig {
    pub enabled: bool,
    /// Cache directory (default `<repo>/.mote/cache`).
    pub dir: Option<String>,
    /// Size above which entries unused by the current scan are pruned.
    pub max_size_mb: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig {
            enabled: true,
            dir: None,
            max_size_mb: 1024,
        }
    }
}

/// Java settings.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct JavaConfig {
    /// Java language release the code is compiled for (`8`, `11`, `17`, `21`, …). Rules about
    /// newer language features stay silent below the release that introduced them. Unset: read
    /// for each file from the nearest `pom.xml` / `build.gradle(.kts)` up the directory tree
    /// (as data, never run); unknown when no build file declares it.
    #[schemars(range(min = 1, max = 99))]
    pub release: Option<u32>,
}

/// Measures settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct MetricsConfig {
    /// Directory levels below the scan root that get rolled-up measures in the results
    /// (`measures.directories`); 0 turns the rollups off.
    #[schemars(range(min = 0, max = 10))]
    pub depth: u32,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig { depth: 2 }
    }
}

/// Grade bands: the upper limits of grades A, B, C and D (a value at or above the last is an E),
/// and the fewest new lines the new code is graded from. See [`grades`](crate::grades).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct GradesConfig {
    /// Maintainability: estimated hours to fix the maintainability findings per 1,000 lines of
    /// code.
    pub maintainability: [f64; 4],
    /// Reliability: weighted reliability findings (HIGH 1, MEDIUM 0.25, LOW 0) per 10,000 lines
    /// of code.
    pub reliability: [f64; 4],
    /// New code with fewer new lines than this is not rated (no new-code grades, and the gate's
    /// new-code grade conditions are skipped).
    pub min_new_lines: u32,
}

/// Default of `grades.min_new_lines`: below about a hundred new lines a single finding moves a
/// density by a whole grade or more.
pub const DEFAULT_GRADES_MIN_NEW_LINES: u32 = 100;

impl Default for GradesConfig {
    fn default() -> Self {
        GradesConfig {
            maintainability: [1.0, 2.0, 3.0, 4.0],
            reliability: [0.5, 1.0, 2.0, 4.0],
            min_new_lines: DEFAULT_GRADES_MIN_NEW_LINES,
        }
    }
}

/// One configuration problem, at its place in the file when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProblem {
    /// What is wrong, starting with the setting it is about (`gate.fail_on: …`).
    pub message: String,
    /// 1-based line of the key or value in error.
    pub line: Option<usize>,
    /// 1-based column of the key or value in error.
    pub column: Option<usize>,
}

impl ConfigProblem {
    fn new(message: impl Into<String>) -> ConfigProblem {
        ConfigProblem {
            message: message.into(),
            line: None,
            column: None,
        }
    }
}

/// A configuration that cannot be used: a file that cannot be read, is not YAML, has unknown
/// keys or invalid values, or fails validation. Every problem is listed, in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError {
    /// The config file, when known.
    pub path: Option<PathBuf>,
    /// The problems, sorted by line and column (those without a position last).
    pub problems: Vec<ConfigProblem>,
    /// A parse failure keeps the configuration without the entries in error, so that
    /// [`complete`](Self::complete) can validate the rest too.
    partial: Option<Box<Config>>,
}

impl ConfigError {
    fn new(path: Option<&Path>, problems: Vec<ConfigProblem>) -> Self {
        ConfigError {
            path: path.map(Path::to_path_buf),
            problems,
            partial: None,
        }
    }

    /// The error of a parse failure, with the validation problems of the rest of the
    /// configuration (unknown rule ids, parameters, globs, …) added: `config validate` and `scan`
    /// report everything wrong with a file in one run. Other errors are returned as they are.
    pub fn complete(mut self, registry: &Registry) -> ConfigError {
        if let Some(partial) = self.partial.take() {
            if let Err(more) = partial.validate(registry) {
                self.problems.extend(more.problems);
                problems::sort_problems(&mut self.problems);
            }
        }
        self
    }
}

impl fmt::Display for ConfigError {
    /// `invalid configuration in FILE:LINE:COLUMN: problem`, or with several problems a heading
    /// and one `FILE:LINE:COLUMN: problem` line each (the form editors and terminals link).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self.path.as_ref().map(|p| p.display().to_string());
        let at = |p: &ConfigProblem| match (&file, p.line, p.column) {
            (Some(file), Some(l), Some(c)) => format!("{file}:{l}:{c}"),
            (Some(file), _, _) => file.clone(),
            (None, Some(l), Some(c)) => format!("line {l}, column {c}"),
            (None, _, _) => String::new(),
        };
        match self.problems.as_slice() {
            [p] => {
                let at = at(p);
                if at.is_empty() {
                    write!(f, "invalid configuration: {}", p.message)
                } else if file.is_some() {
                    write!(f, "invalid configuration in {at}: {}", p.message)
                } else {
                    write!(f, "invalid configuration at {at}: {}", p.message)
                }
            }
            problems => {
                f.write_str("invalid configuration")?;
                if let Some(file) = &file {
                    write!(f, " in {file}")?;
                }
                write!(f, " ({} problems):", problems.len())?;
                for p in problems {
                    match at(p) {
                        at if at.is_empty() => write!(f, "\n  {}", p.message)?,
                        at => write!(f, "\n  {at}: {}", p.message)?,
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Rule names of other tools (ESLint, PMD), recognized in the rule maps of a configuration so
/// that validation names the Mote rule to write instead. The engine knows no other tool's
/// rules: the command-line tool registers the tables `mote migrate` uses
/// ([`Registry::with_foreign_rule_names`]).
pub trait ForeignRuleNames: Send + Sync {
    /// `key`, written as a rule id but not one, read as another tool's rule name.
    fn explain(&self, key: &str, registry: &Registry) -> Option<ForeignRule>;
}

/// What another tool's rule name means for Mote: the end of an `unknown rule` message,
/// such as ``that is an ESLint rule; the Mote rule is `js/no-unused-vars` (…)``.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForeignRule {
    /// The tool's rule has a Mote equivalent, which the text names.
    Equivalent(String),
    /// It has none; shown when no rule id is close enough to suggest instead.
    NoEquivalent(String),
}

impl Config {
    /// Parse YAML text (an empty document yields the defaults).
    pub fn from_yaml_str(text: &str) -> Result<Config, ConfigError> {
        Self::parse(text, None)
    }

    fn parse(text: &str, path: Option<&Path>) -> Result<Config, ConfigError> {
        let source = ConfigSource::new(path, text);
        if text.trim().is_empty() {
            return Ok(Config {
                source,
                ..Config::default()
            });
        }
        match serde_yaml_ng::from_str::<Config>(text) {
            Ok(config) => Ok(Config { source, ..config }),
            Err(e) => {
                // Find every problem, not only the first one the parser stops at.
                let (pending, partial) = problems::check_document(text, &e);
                Err(ConfigError {
                    partial: partial.map(|c| Box::new(Config { source, ..c })),
                    ..ConfigError::new(path, problems::resolve(Some(text), pending))
                })
            }
        }
    }

    /// Load a config file.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::new(
                Some(path),
                vec![ConfigProblem::new(format!("cannot read file: {e}"))],
            )
        })?;
        Self::parse(&text, Some(path))
    }

    /// Find `.mote.yaml` / `.mote.yml` at `root`. Returns the defaults when there is none.
    pub fn discover(root: &Path) -> Result<(Config, Option<PathBuf>), ConfigError> {
        for name in CONFIG_FILE_NAMES {
            let p = root.join(name);
            if p.is_file() {
                return Ok((Config::load(&p)?, Some(p)));
            }
        }
        Ok((Config::default(), None))
    }

    /// Parse config text read from somewhere other than the file system (e.g. a git revision);
    /// `origin` is used in error messages.
    pub fn from_yaml_named(text: &str, origin: &str) -> Result<Config, ConfigError> {
        Self::parse(text, Some(Path::new(origin)))
    }

    /// JSON Schema of the configuration file.
    pub fn json_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(Config);
        let mut v = serde_json::to_value(schema).unwrap_or(serde_json::Value::Null);
        widen_string_types(&mut v);
        v
    }

    /// Check the engine version requirement and every rule id, parameter and glob against
    /// `registry`. All problems are reported at once, at their line and column when the
    /// configuration was parsed from text, and named after the source file when there is one.
    /// Rule ids of other tools are explained with the registry's
    /// [`ForeignRuleNames`](Registry::foreign_rule_names).
    pub fn validate(&self, registry: &Registry) -> Result<(), ConfigError> {
        let pending = self.validation_problems(registry);
        if pending.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::new(
                self.source.path(),
                problems::resolve(self.source.text(), pending),
            ))
        }
    }

    fn validation_problems(&self, registry: &Registry) -> Vec<Pending> {
        let mut problems = Vec::new();
        if let Some(req) = &self.mote {
            match semver::VersionReq::parse(req) {
                Err(e) => problems.push(Pending::value(
                    key_path("mote"),
                    format!("`mote: {req}` is not a valid version requirement: {e}"),
                )),
                Ok(r) => {
                    if let Ok(v) = semver::Version::parse(ENGINE_VERSION) {
                        if !r.matches(&v) {
                            problems.push(Pending::value(
                                key_path("mote"),
                                format!(
                                    "this configuration requires mote {req}, but this is version {ENGINE_VERSION}"
                                ),
                            ));
                        }
                    }
                }
            }
        }
        let rules = RuleCheck::new(registry);
        rules.check(&key_path("rules"), &self.rules, &mut problems);
        rules.check(&key_path("test_rules"), &self.test_rules, &mut problems);
        for (i, o) in self.overrides.iter().enumerate() {
            let at = vec![Seg::Key("overrides".into()), Seg::Index(i)];
            let mut paths = at.clone();
            paths.push(Seg::Key("paths".into()));
            if o.paths.is_empty() {
                problems.push(Pending::value(
                    paths.clone(),
                    format!("{}: `paths` must not be empty", path_text(&at)),
                ));
            }
            check_path_globs(&paths, &o.paths, &mut problems);
            let mut rules_at = at;
            rules_at.push(Seg::Key("rules".into()));
            rules.check(&rules_at, &o.rules, &mut problems);
        }
        for (name, globs) in [
            ("exclude", &self.exclude),
            ("skip_generated_except", &self.skip_generated_except),
            ("tests", &self.tests),
            ("cpd.exclude", &self.cpd.exclude),
            ("coverage.reports", &self.coverage.reports),
        ] {
            check_path_globs(&key_path(name), globs, &mut problems);
        }
        if self.cpd.min_tokens == 0 {
            problems.push(Pending::value(
                key_path("cpd.min_tokens"),
                "cpd.min_tokens must be at least 1",
            ));
        }
        for (name, limits) in [
            ("grades.maintainability", &self.grades.maintainability),
            ("grades.reliability", &self.grades.reliability),
        ] {
            let ok = limits.iter().all(|l| l.is_finite() && *l > 0.0)
                && limits.windows(2).all(|w| w[0] < w[1]);
            if !ok {
                problems.push(Pending::value(
                    key_path(name),
                    format!(
                        "{name} must be four positive, increasing limits (A, B, C, D), got {limits:?}"
                    ),
                ));
            }
        }
        if self.triage.file.trim().is_empty() {
            problems.push(Pending::value(
                key_path("triage.file"),
                "triage.file must not be empty",
            ));
        }
        if let CompatSetting::Sources(names) = &self.suppressions.compat {
            for (i, name) in names.iter().enumerate() {
                if !COMPAT_SOURCES.contains(&name.trim()) {
                    let hint = match suggest(name.trim(), &COMPAT_SOURCES) {
                        Some(s) => format!("; did you mean `{s}`?"),
                        None => format!(" (sources: {})", COMPAT_SOURCES.join(", ")),
                    };
                    let mut at = key_path("suppressions.compat");
                    at.push(Seg::Index(i));
                    problems.push(Pending::value(
                        at,
                        format!("suppressions.compat: unknown source `{name}`{hint}"),
                    ));
                }
            }
        }
        if self.metrics.depth > 10 {
            problems.push(Pending::value(
                key_path("metrics.depth"),
                format!(
                    "metrics.depth must be between 0 and 10, got {}",
                    self.metrics.depth
                ),
            ));
        }
        if let Some(r) = self.java.release {
            if !(1..=99).contains(&r) {
                problems.push(Pending::value(
                    key_path("java.release"),
                    format!(
                        "java.release must be a Java release number such as 8, 11, 17 or 21, got {r}"
                    ),
                ));
            }
        }
        for (name, v) in [
            ("gate.new_duplication_pct", self.gate.new_duplication_pct),
            ("gate.new_coverage_pct", self.gate.new_coverage_pct),
        ] {
            if let Some(v) = v {
                if !(0.0..=100.0).contains(&v) {
                    problems.push(Pending::value(
                        key_path(name),
                        format!("{name} must be between 0 and 100, got {v}"),
                    ));
                }
            }
        }
        problems
    }

    /// Hash of the settings that change per-file analysis output other than the rule set
    /// (lexer, suppressions, skip rules, which files keep duplication tokens). Part of the cache key.
    pub fn analysis_settings_hash(&self) -> u64 {
        self.analysis_settings_hash_for(self.skip_generated)
    }

    /// [`analysis_settings_hash`](Self::analysis_settings_hash) of a file for which the
    /// generated / minified / vendored checks are `skip_generated` (off for the files
    /// `skip_generated_except` matches): the cache key of such a file depends on whether the
    /// checks apply to it, not on the list, so editing the list only re-analyzes the files whose
    /// status changes.
    pub fn analysis_settings_hash_for(&self, skip_generated: bool) -> u64 {
        let mut h = Xxh3::new();
        h.update(&[
            u8::from(self.cpd.ignore_literals),
            u8::from(self.suppressions.require_reason),
        ]);
        // The selected compatibility sources, not their spelling: `true` and the explicit list
        // of the same sources share cache entries.
        h.update(&self.suppressions.compat.sources().bits().to_le_bytes());
        h.update(&[u8::from(skip_generated)]);
        h.update(&self.max_file_size.to_le_bytes());
        // Which files keep duplication tokens.
        h.update(&[u8::from(self.cpd.enabled), u8::from(self.cpd.include_tests)]);
        h.update(&self.cpd.min_tokens.to_le_bytes());
        for g in &self.cpd.exclude {
            h.update(g.as_bytes());
            h.update(&[0]);
        }
        h.digest()
    }
}

/// The YAML parser reads any scalar where the config expects a string (`mote: 0.1` is
/// `"0.1"`, `exclude: [2024]` is `["2024"]`): let the schema accept numbers and booleans there
/// too. Enumerations, patterns and alternatives stay as they are.
fn widen_string_types(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Object(m) => {
            let plain = !["enum", "pattern", "anyOf", "oneOf", "const"]
                .iter()
                .any(|k| m.contains_key(*k));
            if plain {
                match m.get_mut("type") {
                    Some(t @ Value::String(_)) if t == "string" => {
                        *t = serde_json::json!(["string", "number", "boolean"]);
                    }
                    Some(Value::Array(a)) if a.iter().any(|x| x == "string") => {
                        for extra in ["number", "boolean"] {
                            if !a.iter().any(|x| x == extra) {
                                a.push(extra.into());
                            }
                        }
                    }
                    _ => {}
                }
            }
            for x in m.values_mut() {
                widen_string_types(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(widen_string_types),
        _ => {}
    }
}

fn is_glob(s: &str) -> bool {
    s.contains(['*', '?', '[', '{'])
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Suggest a known rule id for an unknown one.
pub fn did_you_mean(unknown: &str, ids: &[&str]) -> Option<String> {
    // A name in PMD's style: `EmptyCatchBlock` → `empty-catch-block`.
    if !unknown.contains('/') && unknown.starts_with(|c: char| c.is_ascii_uppercase()) {
        if let Some(s) = did_you_mean(&kebab_case(unknown), ids) {
            return Some(s);
        }
    }
    // Missing language prefix: `cognitive-complexity` → `java/cognitive-complexity`.
    if !unknown.contains('/') {
        let hits: Vec<&&str> = ids
            .iter()
            .filter(|id| id.rsplit('/').next() == Some(unknown))
            .collect();
        if !hits.is_empty() {
            return Some(
                hits.iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(" or "),
            );
        }
    }
    let best = ids
        .iter()
        .map(|id| (edit_distance(unknown, id), *id))
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))?;
    let limit = (unknown.chars().count() / 3).max(2);
    (best.0 <= limit).then(|| format!("`{}`", best.1))
}

/// `EmptyCatchBlock` → `empty-catch-block`.
fn kebab_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn rule_glob(key: &str) -> Result<GlobMatcher, String> {
    GlobBuilder::new(key)
        .literal_separator(false)
        .build()
        .map(|g| g.compile_matcher())
        .map_err(|e| e.to_string())
}

fn param_compatible(default: &ParamValue, given: &ParamValue) -> bool {
    matches!(
        (default, given),
        (ParamValue::Int(_), ParamValue::Int(_))
            | (
                ParamValue::Float(_),
                ParamValue::Float(_) | ParamValue::Int(_)
            )
            | (ParamValue::Bool(_), ParamValue::Bool(_))
            | (ParamValue::Str(_), ParamValue::Str(_))
    )
}

fn type_name(v: &ParamValue) -> &'static str {
    match v {
        ParamValue::Bool(_) => "a boolean",
        ParamValue::Int(_) => "an integer",
        ParamValue::Float(_) => "a number",
        ParamValue::Str(_) => "a string",
    }
}

/// `at` + `key`.
fn child(at: &[Seg], key: &str) -> Vec<Seg> {
    let mut path = at.to_vec();
    path.push(Seg::Key(key.to_string()));
    path
}

fn validate_params(
    ctx: &str,
    at: &[Seg],
    entries: &[&CatalogEntry],
    params: &BTreeMap<String, ParamValue>,
    problems: &mut Vec<Pending>,
) {
    let params_at = child(at, "params");
    for (name, value) in params {
        let specs: Vec<_> = entries
            .iter()
            .filter_map(|e| e.meta.params.iter().find(|p| p.name == name))
            .collect();
        if specs.is_empty() && name == crate::java_release::JAVA_RELEASE_PARAM {
            problems.push(Pending::key(
                child(&params_at, name),
                format!(
                    "{ctx}: `{name}` is set by the engine; configure `java: {{ release: N }}` instead"
                ),
            ));
            continue;
        }
        if specs.is_empty() {
            let known: FxHashSet<&str> = entries
                .iter()
                .flat_map(|e| e.meta.params.iter().map(|p| p.name))
                .collect();
            let mut known: Vec<&str> = known.into_iter().collect();
            known.sort_unstable();
            let hint = match suggest(name, &known) {
                Some(s) => format!("; did you mean `{s}`?"),
                None if known.is_empty() => " (the rule has no parameters)".to_string(),
                None => format!(" (known: {})", known.join(", ")),
            };
            problems.push(Pending::key(
                child(&params_at, name),
                format!("{ctx}: unknown parameter `{name}`{hint}"),
            ));
            continue;
        }
        for spec in specs {
            let default = spec.default.to_value();
            if !param_compatible(&default, value) {
                problems.push(Pending::value(
                    child(&params_at, name),
                    format!(
                        "{ctx}: parameter `{name}` must be {}, got {}",
                        type_name(&default),
                        type_name(value)
                    ),
                ));
                break;
            }
        }
    }
}

/// Validates rule maps (`rules`, `test_rules`, `overrides[i].rules`) against the registry.
struct RuleCheck<'a> {
    registry: &'a Registry,
    ids: Vec<&'static str>,
}

impl<'a> RuleCheck<'a> {
    fn new(registry: &'a Registry) -> RuleCheck<'a> {
        RuleCheck {
            registry,
            ids: registry.rule_ids(),
        }
    }

    fn check(
        &self,
        section: &[Seg],
        rules: &BTreeMap<String, RuleConfig>,
        problems: &mut Vec<Pending>,
    ) {
        let section_text = path_text(section);
        for (key, cfg) in rules {
            let at = child(section, key);
            let ctx = format!("{section_text}.{key}");
            let entries: Vec<&CatalogEntry> = if is_glob(key) {
                match rule_glob(key) {
                    Err(e) => {
                        problems.push(Pending::key(at, format!("{ctx}: invalid rule glob: {e}")));
                        continue;
                    }
                    Ok(m) => {
                        let hits: Vec<&CatalogEntry> = self
                            .registry
                            .catalog()
                            .iter()
                            .filter(|e| m.is_match(e.meta.id))
                            .collect();
                        if hits.is_empty() {
                            problems.push(Pending::key(
                                at,
                                format!("{ctx}: the glob matches no known rule"),
                            ));
                            continue;
                        }
                        hits
                    }
                }
            } else {
                match self.registry.rule(key) {
                    Some(e) => vec![e],
                    None => {
                        problems.push(Pending::key(
                            at,
                            format!(
                                "{section_text}: unknown rule `{key}`{}",
                                self.unknown_rule_hint(key)
                            ),
                        ));
                        continue;
                    }
                }
            };
            if let RuleConfig::Options(o) = cfg {
                validate_params(&ctx, &at, &entries, &o.params, problems);
            }
        }
    }

    /// Another tool's rule with an equivalent, else the closest rule id, else what the name
    /// looks like.
    fn unknown_rule_hint(&self, key: &str) -> String {
        let foreign = self
            .registry
            .foreign_rule_names()
            .and_then(|f| f.explain(key, self.registry));
        if let Some(ForeignRule::Equivalent(text)) = &foreign {
            return format!(": {text}");
        }
        if let Some(s) = did_you_mean(key, &self.ids) {
            return format!("; did you mean {s}?");
        }
        if let Some(ForeignRule::NoEquivalent(text)) = &foreign {
            return format!(": {text}");
        }
        let known_prefix = key.split_once('/').is_some_and(|(prefix, _)| {
            prefix == "mote" || self.registry.plugin_index(prefix).is_some()
        });
        if known_prefix {
            " (`mote rules --all` lists the rules)".to_string()
        } else {
            let languages: Vec<String> = self
                .registry
                .plugins()
                .iter()
                .map(|p| format!("`{}/`", p.id()))
                .collect();
            format!(
                " (rule ids start with their language: {}; `mote rules --all` lists them)",
                languages.join(", ")
            )
        }
    }
}

fn check_path_globs(at: &[Seg], globs: &[String], problems: &mut Vec<Pending>) {
    let ctx = path_text(at);
    for (i, g) in globs.iter().enumerate() {
        for pat in expand_path_glob(g) {
            if let Err(e) = GlobBuilder::new(&pat).literal_separator(true).build() {
                let mut path = at.to_vec();
                path.push(Seg::Index(i));
                problems.push(Pending::value(
                    path,
                    format!("{ctx}: invalid glob `{g}`: {e}"),
                ));
                break;
            }
        }
    }
}

/// Normalize a user path glob into the patterns actually matched against repo-relative paths:
/// `\` → `/`, a leading `./` or `/` anchors at the root, a pattern without `/` matches at any
/// depth (`*.min.js` → `**/*.min.js`), and every pattern also matches everything below a
/// matching directory (`legacy` → `legacy`, `legacy/**`).
pub fn expand_path_glob(glob: &str) -> Vec<String> {
    let mut g = glob.trim().replace('\\', "/");
    let anchored = g.starts_with('/') || g.starts_with("./");
    g = g
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    while g.ends_with('/') {
        g.pop();
    }
    if g.is_empty() {
        return vec!["**".to_string()];
    }
    if !anchored && !g.contains('/') && !g.starts_with("**") {
        g = format!("**/{g}");
    }
    if g.ends_with("/**") || g == "**" {
        vec![g]
    } else {
        vec![g.clone(), format!("{g}/**")]
    }
}

/// Compile path globs (see [`expand_path_glob`]) into one set. Invalid globs are skipped
/// (validation reports them).
pub fn compile_path_globs<S: AsRef<str>>(globs: &[S]) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for g in globs {
        for pat in expand_path_glob(g.as_ref()) {
            if let Ok(glob) = GlobBuilder::new(&pat).literal_separator(true).build() {
                b.add(glob);
            }
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Path filter from the command line: globs (containing `*?[{`) or path prefixes.
#[derive(Clone, Debug, Default)]
pub struct PathFilter {
    globs: Option<GlobSet>,
    prefixes: Vec<String>,
}

impl PathFilter {
    /// Build a filter; an empty list matches everything.
    pub fn new<S: AsRef<str>>(patterns: &[S]) -> Result<PathFilter, String> {
        let mut globs = GlobSetBuilder::new();
        let mut has_glob = false;
        let mut prefixes = Vec::new();
        for p in patterns {
            let p = p.as_ref().trim().replace('\\', "/");
            let p = p.trim_start_matches("./").trim_end_matches('/').to_string();
            if is_glob(&p) {
                // Same semantics as config globs (`*.java` matches at any depth).
                for pat in expand_path_glob(&p) {
                    let g = GlobBuilder::new(&pat)
                        .literal_separator(true)
                        .build()
                        .map_err(|e| format!("invalid path glob `{p}`: {e}"))?;
                    globs.add(g);
                }
                has_glob = true;
            } else if p.is_empty() || p == "." {
                prefixes.push(String::new());
            } else {
                // A leading `/` anchors at the root, which a prefix is anyway.
                prefixes.push(p.trim_start_matches('/').to_string());
            }
        }
        let globs = if has_glob {
            Some(globs.build().map_err(|e| e.to_string())?)
        } else {
            None
        };
        Ok(PathFilter { globs, prefixes })
    }

    /// Whether the filter is empty (matches everything).
    pub fn is_empty(&self) -> bool {
        self.globs.is_none() && self.prefixes.is_empty()
    }

    /// Whether `path` passes the filter.
    pub fn matches(&self, path: &str) -> bool {
        if self.is_empty() {
            return true;
        }
        if self.prefixes.iter().any(|p| {
            p.is_empty()
                || path == p
                || (path.starts_with(p.as_str()) && path.as_bytes().get(p.len()) == Some(&b'/'))
        }) {
            return true;
        }
        self.globs.as_ref().is_some_and(|g| g.is_match(path))
    }
}

// ---------------------------------------------------------------------------------------------
// Compiled configuration and rule resolution.

#[derive(Clone, Debug)]
struct RuleState {
    enabled: bool,
    severity: Severity,
    params: BTreeMap<String, ParamValue>,
}

enum KeyMatcher {
    Exact(String),
    Glob(GlobMatcher),
}

struct LayerEntry {
    matcher: KeyMatcher,
    config: RuleConfig,
}

/// A compiled `rules` map with deterministic application order.
struct Layer {
    entries: Vec<LayerEntry>,
}

impl Layer {
    fn compile(rules: &BTreeMap<String, RuleConfig>) -> Layer {
        let mut globs: Vec<(usize, &String, &RuleConfig)> = Vec::new();
        let mut exact: Vec<(&String, &RuleConfig)> = Vec::new();
        for (k, v) in rules {
            if is_glob(k) {
                let specificity = k
                    .chars()
                    .filter(|c| !matches!(c, '*' | '?' | '[' | ']' | '{' | '}'))
                    .count();
                globs.push((specificity, k, v));
            } else {
                exact.push((k, v));
            }
        }
        globs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
        let mut entries = Vec::new();
        for (_, k, v) in globs {
            if let Ok(m) = rule_glob(k) {
                entries.push(LayerEntry {
                    matcher: KeyMatcher::Glob(m),
                    config: v.clone(),
                });
            }
        }
        for (k, v) in exact {
            entries.push(LayerEntry {
                matcher: KeyMatcher::Exact(k.clone()),
                config: v.clone(),
            });
        }
        Layer { entries }
    }

    fn apply(&self, entry: &CatalogEntry, state: &mut RuleState) {
        let id = entry.meta.id;
        let deprecated = entry.meta.status == RuleStatus::Deprecated;
        for e in &self.entries {
            let explicit = match &e.matcher {
                KeyMatcher::Exact(k) => {
                    if k != id {
                        continue;
                    }
                    true
                }
                KeyMatcher::Glob(g) => {
                    if !g.is_match(id) {
                        continue;
                    }
                    false
                }
            };
            let may_enable = explicit || !deprecated;
            match &e.config {
                RuleConfig::Level(RuleLevel::Off) => state.enabled = false,
                RuleConfig::Level(RuleLevel::On) => {
                    if may_enable {
                        state.enabled = true;
                    }
                }
                RuleConfig::Level(RuleLevel::Severity(s)) => {
                    if may_enable {
                        state.enabled = true;
                        state.severity = *s;
                    }
                }
                RuleConfig::Options(o) => {
                    if let Some(s) = o.severity {
                        state.severity = s;
                    }
                    for (k, v) in &o.params {
                        if entry.meta.params.iter().any(|p| p.name == k) {
                            state.params.insert(k.clone(), v.clone());
                        }
                    }
                    match o.enabled {
                        Some(false) => state.enabled = false,
                        Some(true) => {
                            if may_enable {
                                state.enabled = true;
                            }
                        }
                        None => {
                            if explicit {
                                state.enabled = true;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The effective rules for a group of files.
#[derive(Debug)]
pub struct ResolvedRules {
    /// The rule set handed to the plugin and used by engine rules.
    pub rules: RuleSet,
    /// Deterministic hash of `rules` (ids, severities, parameters).
    pub hash: u64,
    /// Enabled rule ids, sorted.
    pub enabled: Vec<&'static str>,
}

/// Plugin, test flag, matching overrides, Java release.
type MemoKey = (usize, bool, Box<[u32]>, Option<u32>);

/// A [`Config`] compiled against a [`Registry`]: glob sets and memoized per-file rule sets.
pub struct CompiledConfig {
    config: Config,
    exclude: GlobSet,
    skip_generated_except: GlobSet,
    tests: GlobSet,
    plugin_tests: Vec<GlobSet>,
    cpd_exclude: GlobSet,
    base: Layer,
    test_layer: Layer,
    overrides: Vec<(GlobSet, Layer)>,
    memo: Mutex<FxHashMap<MemoKey, Arc<ResolvedRules>>>,
}

impl fmt::Debug for CompiledConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledConfig")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CompiledConfig {
    /// Validate `config` against `registry` and compile it.
    pub fn new(config: Config, registry: &Registry) -> Result<CompiledConfig, ConfigError> {
        config.validate(registry)?;
        let plugin_tests = registry
            .plugins()
            .iter()
            .map(|p| compile_path_globs(p.default_test_globs()))
            .collect();
        Ok(CompiledConfig {
            exclude: compile_path_globs(&config.exclude),
            skip_generated_except: compile_path_globs(&config.skip_generated_except),
            tests: compile_path_globs(&config.tests),
            plugin_tests,
            cpd_exclude: compile_path_globs(&config.cpd.exclude),
            base: Layer::compile(&config.rules),
            test_layer: Layer::compile(&config.test_rules),
            overrides: config
                .overrides
                .iter()
                .map(|o| (compile_path_globs(&o.paths), Layer::compile(&o.rules)))
                .collect(),
            memo: Mutex::new(FxHashMap::default()),
            config,
        })
    }

    /// The source configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Whether `path` is excluded by `exclude`.
    pub fn is_excluded(&self, path: &str) -> bool {
        self.exclude.is_match(path)
    }

    /// Whether `path` is skipped when it looks generated, minified or vendored: `skip_generated`,
    /// unless `skip_generated_except` matches the path.
    pub fn skips_generated(&self, path: &str) -> bool {
        self.config.skip_generated && !self.skip_generated_except.is_match(path)
    }

    /// Whether `path` (handled by plugin `plugin`) is test code.
    pub fn is_test(&self, plugin: usize, path: &str) -> bool {
        self.tests.is_match(path)
            || self
                .plugin_tests
                .get(plugin)
                .is_some_and(|g| g.is_match(path))
    }

    /// Whether `path` is excluded from duplication detection by `cpd.exclude`.
    pub fn is_cpd_excluded(&self, path: &str) -> bool {
        self.cpd_exclude.is_match(path)
    }

    /// Effective rules for a file (without a known Java release). Memoized: files with the same
    /// plugin, test status, set of matching overrides (and Java release, see
    /// [`resolve_with_release`](Self::resolve_with_release)) share one [`ResolvedRules`].
    pub fn resolve(
        &self,
        registry: &Registry,
        plugin: usize,
        is_test: bool,
        path: &str,
    ) -> Arc<ResolvedRules> {
        self.resolve_with_release(registry, plugin, is_test, path, None)
    }

    /// [`resolve`](Self::resolve) for a file whose Java release is known: `java_release` is
    /// added as the parameter [`JAVA_RELEASE_PARAM`] of every enabled rule of the `java`
    /// plugin (so it reaches the plugin and is part of [`ResolvedRules::hash`], the cache key).
    /// Ignored for other plugins.
    ///
    /// [`JAVA_RELEASE_PARAM`]: crate::java_release::JAVA_RELEASE_PARAM
    pub fn resolve_with_release(
        &self,
        registry: &Registry,
        plugin: usize,
        is_test: bool,
        path: &str,
        java_release: Option<u32>,
    ) -> Arc<ResolvedRules> {
        let java_release =
            java_release.filter(|_| registry.plugin(plugin).id() == crate::java_release::JAVA);
        let matched: Box<[u32]> = self
            .overrides
            .iter()
            .enumerate()
            .filter(|(_, (g, _))| g.is_match(path))
            .map(|(i, _)| i as u32)
            .collect();
        let key = (plugin, is_test, matched, java_release);
        {
            let memo = self.memo.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(r) = memo.get(&key) {
                return r.clone();
            }
        }
        let resolved = Arc::new(self.compute(registry, plugin, is_test, &key.2, java_release));
        let mut memo = self.memo.lock().unwrap_or_else(|p| p.into_inner());
        memo.entry(key).or_insert(resolved).clone()
    }

    /// All distinct rule sets resolved so far.
    pub fn resolved_sets(&self) -> Vec<Arc<ResolvedRules>> {
        let memo = self.memo.lock().unwrap_or_else(|p| p.into_inner());
        memo.values().cloned().collect()
    }

    fn compute(
        &self,
        registry: &Registry,
        plugin: usize,
        is_test: bool,
        overrides: &[u32],
        java_release: Option<u32>,
    ) -> ResolvedRules {
        let profile = self.config.extends;
        let mut map = FxHashMap::default();
        let mut enabled = Vec::new();
        let mut hasher = Xxh3::new();
        for entry in registry.rules_for_plugin(plugin) {
            let meta = entry.meta;
            let on_by_profile = meta.status != RuleStatus::Deprecated
                && match profile {
                    Profile::Default => meta.default_enabled,
                    Profile::Conventions => {
                        meta.default_enabled || meta.tags.contains(&CONVENTIONS_TAG)
                    }
                    Profile::All => true,
                    Profile::None => false,
                };
            let mut state = RuleState {
                enabled: on_by_profile,
                severity: meta.severity,
                params: meta
                    .params
                    .iter()
                    .map(|p| (p.name.to_string(), p.default.to_value()))
                    .collect(),
            };
            self.base.apply(entry, &mut state);
            if is_test {
                self.test_layer.apply(entry, &mut state);
            }
            for &i in overrides {
                self.overrides[i as usize].1.apply(entry, &mut state);
            }
            if let Some(r) = java_release {
                // Not a declared parameter (config validation rejects it): set by the engine
                // only, and hashed below like any parameter.
                state.params.insert(
                    crate::java_release::JAVA_RELEASE_PARAM.to_string(),
                    ParamValue::Int(i64::from(r)),
                );
            }
            if state.enabled {
                // rules_for_plugin is sorted by id, so the hash input order is deterministic.
                hasher.update(meta.id.as_bytes());
                hasher.update(&[0, state.severity as u8]);
                for (k, v) in &state.params {
                    hasher.update(k.as_bytes());
                    hash_param(&mut hasher, v);
                }
                hasher.update(&[0xFF]);
                enabled.push(meta.id);
                map.insert(
                    meta.id.to_string(),
                    ActiveRule {
                        severity: state.severity,
                        params: state.params,
                    },
                );
            }
        }
        ResolvedRules {
            rules: RuleSet::new(map),
            hash: hasher.digest(),
            enabled,
        }
    }
}

fn hash_param(h: &mut Xxh3, v: &ParamValue) {
    match v {
        ParamValue::Bool(b) => h.update(&[1, u8::from(*b)]),
        ParamValue::Int(i) => {
            h.update(&[2]);
            h.update(&i.to_le_bytes());
        }
        ParamValue::Float(f) => {
            h.update(&[3]);
            h.update(&f.to_bits().to_le_bytes());
        }
        ParamValue::Str(s) => {
            h.update(&[4]);
            h.update(s.as_bytes());
            h.update(&[0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{registry, FakePlugin};

    #[test]
    fn yaml_off_is_a_string() {
        let v: serde_yaml_ng::Value = serde_yaml_ng::from_str("x: off").unwrap();
        assert!(v["x"].is_string());
    }

    #[test]
    fn parses_full_example() {
        let text = r#"
mote: ">=0.1"
extends: default
exclude: ["**/generated/**"]
tests: ["**/it/**"]
max_file_size: 1048576
skip_generated: true
rules:
  fk/empty-block: off
  fk/loud: { severity: HIGH }
  fk/cognitive-complexity: { params: { threshold: 20 } }
  "fk/*": { enabled: false }
test_rules: { fk/magic: off }
overrides:
  - paths: ["legacy/**"]
    rules: { fk/cognitive-complexity: { params: { threshold: 30 } } }
gate:
  fail_on: HIGH
  new_duplication_pct: 3.0
  new_coverage_pct: null
  min_new_lines: 20
  worsened: annotate
cpd: { enabled: true, min_tokens: 100, min_lines: 10, exclude: [], ignore_literals: true, include_tests: false }
coverage: { reports: [] }
suppressions: { require_reason: true, compat: false }
cache: { enabled: true, dir: null, max_size_mb: 1024 }
"#;
        let c = Config::from_yaml_str(text).unwrap();
        assert_eq!(c.rules.len(), 4);
        assert_eq!(c.rules["fk/empty-block"], RuleConfig::Level(RuleLevel::Off));
        assert_eq!(c.gate.new_coverage_pct, None);
        assert_eq!(c.gate.new_duplication_pct, Some(3.0));
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        c.validate(&reg).unwrap();
        assert!(Config::from_yaml_str("").unwrap() == Config::default());
    }

    #[test]
    fn rejects_unknown_fields_and_bad_values() {
        let e = Config::from_yaml_str("exclud: []").unwrap_err();
        assert!(
            e.to_string()
                .contains("unknown key `exclud`; did you mean `exclude`?"),
            "{e}"
        );
        let e = Config::from_yaml_str("rules: { a/b: loud }").unwrap_err();
        assert!(e.to_string().contains("loud"), "{e}");
        let e = Config::from_yaml_str("rules: { a/b: { severity: CRITICAL } }").unwrap_err();
        assert!(e.to_string().contains("CRITICAL"), "{e}");
        let e = Config::from_yaml_str("rules: { a/b: { sevrity: LOW } }").unwrap_err();
        assert!(e.to_string().contains("sevrity"), "{e}");
        let e = Config::from_yaml_str("gate: { fail_on: CRITICAL }").unwrap_err();
        assert!(e.to_string().contains("CRITICAL"), "{e}");
        // booleans and lowercase severities are accepted
        let c = Config::from_yaml_str("rules: { a/b: false, a/c: true, a/d: high }").unwrap();
        assert_eq!(c.rules["a/b"], RuleConfig::Level(RuleLevel::Off));
        assert_eq!(
            c.rules["a/d"],
            RuleConfig::Level(RuleLevel::Severity(Severity::High))
        );
    }

    #[test]
    fn validation_reports_unknown_rules_with_suggestions() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let c = Config::from_yaml_str(
            "rules:\n  fk/cognitive-complexit: off\n  todo-comment: off\n  fk/loud: { params: { nope: 1 } }\n  fk/cognitive-complexity: { params: { threshold: \"x\" } }\n  \"zz/*\": off\nexclude: [\"a[\"]\nmote: \">=99\"\n",
        )
        .unwrap();
        let e = c.validate(&reg).unwrap_err();
        let msg = e.to_string();
        assert!(
            msg.contains(
                "unknown rule `fk/cognitive-complexit`; did you mean `fk/cognitive-complexity`?"
            ),
            "{msg}"
        );
        assert!(msg.contains("did you mean `fk/todo-comment`?"), "{msg}");
        assert!(msg.contains("unknown parameter `nope`"), "{msg}");
        assert!(
            msg.contains("parameter `threshold` must be an integer"),
            "{msg}"
        );
        assert!(msg.contains("matches no known rule"), "{msg}");
        assert!(msg.contains("invalid glob `a[`"), "{msg}");
        assert!(msg.contains("requires mote >=99"), "{msg}");
        assert_eq!(e.problems.len(), 7, "{msg}");
    }

    /// `(line, column, message)` of each problem.
    fn listed(e: &ConfigError) -> Vec<(usize, usize, String)> {
        e.problems
            .iter()
            .map(|p| {
                (
                    p.line.unwrap_or(0),
                    p.column.unwrap_or(0),
                    p.message.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn every_parse_problem_at_its_position_in_file_order() {
        let text = "\
exclud: [a]
gate:
  fail_on: hihg
  min_new_lnes: 5
  worsened: anotate
rules:
  fk/loud: { sevrity: LOW, params: { x: [1] } }
  fk/magic: error
  fk/todo-comment: [warn, {}]
  fk/style: hgih
  fk/nope: off
overrides:
  - path: [legacy]
    rules: { fk/loud: 2 }
extends: defualt
metrics: { depth: deep }
";
        let e = Config::from_yaml_named(text, ".mote.yaml").unwrap_err();
        let got = listed(&e);
        let want: Vec<(usize, usize, &str)> = vec![
            (1, 1, "unknown key `exclud`; did you mean `exclude`?"),
            (3, 12, "gate.fail_on: unknown variant `hihg`, expected one of `HIGH`, `high`, `MEDIUM`, `medium`, `LOW`, `low`, `NONE`, `none`; did you mean `HIGH`?"),
            (4, 3, "gate: unknown key `min_new_lnes`; did you mean `min_new_lines`?"),
            (5, 13, "gate.worsened: unknown variant `anotate`, expected `annotate` or `fail`; did you mean `annotate`?"),
            (7, 14, "rules.fk/loud: unknown key `sevrity`; did you mean `severity`?"),
            (7, 41, "rules.fk/loud.params.x: expected a boolean, a number or a string"),
            (8, 13, "rules.fk/magic: invalid value: string \"error\", expected `off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, `severity`, `params`; `error` is an ESLint level: write `on` (or a severity: LOW, MEDIUM, HIGH)"),
            (9, 20, "rules.fk/todo-comment: invalid type: list, expected `off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, `severity`, `params`; ESLint's `[level, options]` form is not read here: write `on`, `off` or a severity, and rule options as `{ params: { … } }`"),
            (10, 13, "rules.fk/style: invalid value: string \"hgih\", expected `off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, `severity`, `params`; did you mean `HIGH`?"),
            (13, 5, "overrides[0]: missing key `paths` (the globs of the files these rule settings apply to)"),
            (13, 5, "overrides[0]: unknown key `path`; did you mean `paths`?"),
            (14, 23, "overrides[0].rules.fk/loud: invalid type: integer `2`, expected `off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, `severity`, `params`; `2` is an ESLint level: write `on` (or a severity: LOW, MEDIUM, HIGH)"),
            (15, 10, "extends: unknown variant `defualt`, expected one of `default`, `conventions`, `all`, `none`; did you mean `default`?"),
            (16, 19, "metrics.depth: invalid type: string \"deep\", expected a whole number of 0 or more"),
        ];
        let want: Vec<(usize, usize, String)> = want
            .into_iter()
            .map(|(l, c, m)| (l, c, m.to_string()))
            .collect();
        assert_eq!(got, want);
        assert_eq!(e.path.as_deref(), Some(Path::new(".mote.yaml")));
        // The display lists them, one `file:line:column: problem` line each.
        let shown = e.to_string();
        assert!(
            shown.starts_with("invalid configuration in .mote.yaml (14 problems):\n  .mote.yaml:1:1: unknown key `exclud`"),
            "{shown}"
        );
        // Validation problems of the rest (the unknown rule) join them, in file order.
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let all = e.clone().complete(&reg);
        assert_eq!(all.problems.len(), 15, "{all}");
        assert_eq!(
            listed(&all)[9],
            (
                11,
                3,
                "rules: unknown rule `fk/nope` (`mote rules --all` lists the rules)".to_string()
            )
        );
        // Completing twice adds nothing.
        assert_eq!(all.clone().complete(&reg), all);
    }

    #[test]
    fn a_single_problem_stays_on_one_line() {
        let e = Config::from_yaml_named("gate:\n  fial_on: HIGH\n", "c.yaml").unwrap_err();
        assert_eq!(
            e.to_string(),
            "invalid configuration in c.yaml:2:3: gate: unknown key `fial_on`; did you mean `fail_on`?"
        );
        let e = Config::from_yaml_str("cpd: { min_tokens: x }").unwrap_err();
        assert_eq!(
            e.to_string(),
            "invalid configuration at line 1, column 20: cpd.min_tokens: invalid type: string \"x\", expected a whole number of 0 or more"
        );
        // Not YAML: the parser's message, at its position.
        let e = Config::from_yaml_named("rules: [a, b\n", "c.yaml").unwrap_err();
        assert_eq!(e.problems.len(), 1, "{e}");
        assert!(
            e.to_string()
                .starts_with("invalid configuration in c.yaml:2:1: not valid YAML: "),
            "{e}"
        );
        // Not a mapping.
        let e = Config::from_yaml_named("- a\n- b\n", "c.yaml").unwrap_err();
        assert!(e.to_string().contains("expected a map of settings"), "{e}");
        // Unknown keys without a close one list the keys of their level.
        let e = Config::from_yaml_str("cache: { size: 3 }").unwrap_err();
        assert!(
            e.to_string()
                .contains("cache: unknown key `size` (keys: dir, enabled, max_size_mb)"),
            "{e}"
        );
        // Scalars read as in a whole-file parse: a number where a string is expected is fine.
        let e = Config::from_yaml_str("mote: 0.1\nexclude: [2024]\nextends: nope\n").unwrap_err();
        assert_eq!(e.problems.len(), 1, "{e}");
    }

    #[test]
    fn validation_problems_have_positions_too() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let text = "rules:\n  fk/cognitive-complexit: off\n  fk/loud: { params: { threshhold: 3 } }\n  fk/cognitive-complexity: { params: { threshhold: 3 } }\nsuppressions:\n  compat: [pmd, spotbug]\nexclude:\n  - ok\n  - \"a[\"\n";
        let c = Config::from_yaml_named(text, "n.yaml").unwrap();
        let e = c.validate(&reg).unwrap_err();
        assert_eq!(e.path.as_deref(), Some(Path::new("n.yaml")));
        assert_eq!(
            listed(&e),
            vec![
                (2, 3, "rules: unknown rule `fk/cognitive-complexit`; did you mean `fk/cognitive-complexity`?".to_string()),
                (3, 24, "rules.fk/loud: unknown parameter `threshhold` (the rule has no parameters)".to_string()),
                (4, 40, "rules.fk/cognitive-complexity: unknown parameter `threshhold`; did you mean `threshold`?".to_string()),
                (6, 17, "suppressions.compat: unknown source `spotbug`; did you mean `spotbugs`?".to_string()),
                (9, 5, "exclude: invalid glob `a[`: error parsing glob '**/a[': unclosed character class; missing ']'".to_string()),
            ]
        );
        // A configuration built in code has no positions.
        let mut built = Config::default();
        built.cpd.min_tokens = 0;
        let e = built.validate(&reg).unwrap_err();
        assert_eq!(
            e.to_string(),
            "invalid configuration: cpd.min_tokens must be at least 1"
        );
    }

    struct Foreign;

    impl ForeignRuleNames for Foreign {
        fn explain(&self, key: &str, _: &Registry) -> Option<ForeignRule> {
            match key {
                "LoudRule" => Some(ForeignRule::Equivalent(
                    "that is a PMD rule; the Mote rule is `fk/loud`".into(),
                )),
                "fk/loudd" | "plugin/other" => Some(ForeignRule::NoEquivalent(
                    "that is an ESLint plugin rule, and Mote has no equivalent".into(),
                )),
                _ => None,
            }
        }
    }

    #[test]
    fn other_tools_rule_names_are_explained() {
        let reg =
            registry(vec![FakePlugin::boxed("fk")]).with_foreign_rule_names(Box::new(Foreign));
        let c = Config::from_yaml_str(
            "rules:\n  LoudRule: off\n  fk/loudd: off\n  plugin/other: off\n  TodoComment: off\n  zzz: off\n",
        )
        .unwrap();
        let msgs: Vec<String> = c
            .validate(&reg)
            .unwrap_err()
            .problems
            .into_iter()
            .map(|p| p.message)
            .collect();
        assert_eq!(
            msgs,
            vec![
                "rules: unknown rule `LoudRule`: that is a PMD rule; the Mote rule is `fk/loud`",
                // A close rule id wins over "no equivalent".
                "rules: unknown rule `fk/loudd`; did you mean `fk/loud`?",
                "rules: unknown rule `plugin/other`: that is an ESLint plugin rule, and Mote has no equivalent",
                // PMD-style names are tried in kebab case.
                "rules: unknown rule `TodoComment`; did you mean `fk/todo-comment`?",
                "rules: unknown rule `zzz` (rule ids start with their language: `fk/`; `mote rules --all` lists them)",
            ]
        );
    }

    #[test]
    fn skip_generated_except_is_per_path() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let compiled =
            |yaml: &str| CompiledConfig::new(Config::from_yaml_str(yaml).unwrap(), &reg).unwrap();
        let c = compiled("skip_generated_except: [\"web/js\", \"*.bundle.js\", \"./top.js\"]");
        assert!(c.skips_generated("src/a.js"));
        assert!(!c.skips_generated("web/js/app.js"));
        assert!(!c.skips_generated("dist/app.bundle.js"));
        assert!(!c.skips_generated("top.js"));
        assert!(
            c.skips_generated("sub/top.js"),
            "a leading ./ anchors at the root"
        );
        let off = compiled("skip_generated: false\nskip_generated_except: [\"web/js\"]");
        assert!(!off.skips_generated("src/a.js") && !off.skips_generated("web/js/app.js"));
        // The list is not part of the settings hash: a file's cache key depends on whether the
        // checks apply to that file (see `analysis_settings_hash_for`).
        let plain = Config::default();
        assert_eq!(
            c.config().analysis_settings_hash(),
            plain.analysis_settings_hash()
        );
        assert_eq!(
            off.config().analysis_settings_hash(),
            plain.analysis_settings_hash_for(false)
        );
        assert_ne!(
            plain.analysis_settings_hash_for(true),
            plain.analysis_settings_hash_for(false)
        );
        let e = Config::from_yaml_str("skip_generated_except: [\"ok/**\", \"a[\"]")
            .unwrap()
            .validate(&reg)
            .unwrap_err();
        assert!(
            e.to_string()
                .contains("skip_generated_except: invalid glob `a[`"),
            "{e}"
        );
    }

    #[test]
    fn compat_sources() {
        let compat = |yaml: &str| Config::from_yaml_str(yaml).unwrap().suppressions.compat;
        assert_eq!(compat(""), CompatSetting::Flag(false));
        assert_eq!(
            compat("suppressions: { compat: true }"),
            CompatSetting::Flag(true)
        );
        assert_eq!(
            compat("suppressions: { compat: [pmd, intellij] }"),
            CompatSetting::Sources(vec!["pmd".into(), "intellij".into()])
        );
        assert_eq!(
            compat("suppressions: { compat: spotbugs }"),
            CompatSetting::Sources(vec!["spotbugs".into()])
        );
        // `true` keeps meaning what it meant: no spotbugs, no intellij.
        let legacy = CompatSetting::Flag(true).sources();
        assert_eq!(legacy, CompatSet::legacy());
        assert!(legacy.contains("pmd") && legacy.contains("nosonar") && legacy.contains("eslint"));
        assert!(!legacy.contains("spotbugs") && !legacy.contains("intellij"));
        // Serialized as written.
        let c = Config::from_yaml_str("suppressions: { compat: [pmd] }").unwrap();
        let back = Config::from_yaml_str(&serde_yaml_ng::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        // The cache key depends on the selected sources, not on how they are written.
        let hash = |yaml: &str| {
            Config::from_yaml_str(yaml)
                .unwrap()
                .analysis_settings_hash()
        };
        assert_eq!(
            hash("suppressions: { compat: true }"),
            hash("suppressions: { compat: [suppress-warnings-all, pmd, nosonar, eslint, checkstyle] }")
        );
        assert_eq!(hash(""), hash("suppressions: { compat: [] }"));
        assert_ne!(
            hash("suppressions: { compat: true }"),
            hash("suppressions: { compat: [pmd] }")
        );
        // Unknown names are reported by `validate`, with a suggestion.
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let e = Config::from_yaml_str("suppressions: { compat: [pmd, spotbug, foo] }")
            .unwrap()
            .validate(&reg)
            .unwrap_err();
        let msg = e.to_string();
        assert!(
            msg.contains("suppressions.compat: unknown source `spotbug`; did you mean `spotbugs`?"),
            "{msg}"
        );
        assert!(
            msg.contains("unknown source `foo` (sources: checkstyle, eslint"),
            "{msg}"
        );
        assert_eq!(e.problems.len(), 2, "{msg}");
        let e = Config::from_yaml_str("suppressions: { compat: yes }").unwrap();
        assert!(e.validate(&reg).is_err());
    }

    #[test]
    fn metrics_grades_and_grade_gates() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let c = Config::from_yaml_str(
            "metrics: { depth: 3 }\ngrades: { maintainability: [1, 2, 4, 8] }\ngate: { new_maintainability_grade: b, reliability_grade: A }\n",
        )
        .unwrap();
        c.validate(&reg).unwrap();
        assert_eq!(c.metrics.depth, 3);
        assert_eq!(c.grades.maintainability, [1.0, 2.0, 4.0, 8.0]);
        assert_eq!(c.grades.reliability, GradesConfig::default().reliability);
        assert_eq!(c.gate.new_maintainability_grade, Some(Grade::B));
        assert_eq!(c.gate.reliability_grade, Some(Grade::A));
        assert_eq!(c.gate.maintainability_grade, None);
        let e = Config::from_yaml_str("gate: { maintainability_grade: F }").unwrap_err();
        assert!(e.to_string().contains('F'), "{e}");
        assert!(Config::from_yaml_str("grades: { reliability: [1, 2, 3] }").is_err());
        let c = Config::from_yaml_str(
            "metrics: { depth: 11 }\ngrades: { maintainability: [1, 1, 2, 3], reliability: [0, 1, 2, 3] }\n",
        )
        .unwrap();
        let msg = c.validate(&reg).unwrap_err().to_string();
        assert!(
            msg.contains("metrics.depth must be between 0 and 10"),
            "{msg}"
        );
        assert!(
            msg.contains("grades.maintainability must be four positive, increasing"),
            "{msg}"
        );
        assert!(
            msg.contains("grades.reliability must be four positive"),
            "{msg}"
        );
    }

    /// `grades.min_new_lines` (new code is not rated below it) is its own setting, apart from the
    /// gate's minimum for its duplication and coverage conditions.
    #[test]
    fn grades_min_new_lines() {
        assert_eq!(GradesConfig::default().min_new_lines, 100);
        assert_eq!(GateConfig::default().min_new_lines, 20);
        let c = Config::from_yaml_str("grades: { min_new_lines: 40 }\ngate: { min_new_lines: 5 }")
            .unwrap();
        assert_eq!(c.grades.min_new_lines, 40);
        assert_eq!(c.gate.min_new_lines, 5);
        assert_eq!(
            c.grades.maintainability,
            GradesConfig::default().maintainability
        );
        let e = Config::from_yaml_named("grades:\n  min_new_lnes: 40\n", ".mote.yaml").unwrap_err();
        assert_eq!(
            listed(&e),
            vec![(
                2,
                3,
                "grades: unknown key `min_new_lnes`; did you mean `min_new_lines`?".to_string()
            )]
        );
        let e = Config::from_yaml_str("grades: { min_new_lines: -1 }").unwrap_err();
        assert!(e.to_string().contains("grades.min_new_lines"), "{e}");
    }

    #[test]
    fn path_glob_expansion() {
        let g = compile_path_globs(&["*.min.js", "legacy", "/root-only.js", "src/gen/**"]);
        assert!(g.is_match("a/b/x.min.js"));
        assert!(g.is_match("x.min.js"));
        assert!(g.is_match("legacy/a/b.java"));
        assert!(g.is_match("x/legacy/a.java"));
        assert!(g.is_match("root-only.js"));
        assert!(!g.is_match("sub/root-only.js"));
        assert!(g.is_match("src/gen/a/b.java"));
        assert!(!g.is_match("src/general.java"));
        let g = compile_path_globs(&["src/*.java"]);
        assert!(g.is_match("src/A.java"));
        assert!(!g.is_match("src/x/A.java"));
    }

    #[test]
    fn path_filter_globs_and_prefixes() {
        let f = PathFilter::new(&["src/main", "**/*.ts", ".\\docs\\"]).unwrap();
        assert!(f.matches("src/main/A.java"));
        assert!(f.matches("src/main"));
        assert!(!f.matches("src/mainline/A.java"));
        assert!(f.matches("web/x.ts"));
        assert!(f.matches("docs/a.md"));
        assert!(!f.matches("lib/a.js"));
        assert!(PathFilter::new::<&str>(&[]).unwrap().matches("anything"));
        // same depth semantics as config globs
        let f = PathFilter::new(&["*.java"]).unwrap();
        assert!(f.matches("A.java"));
        assert!(f.matches("deep/pkg/A.java"));
        assert!(!f.matches("deep/pkg/A.js"));
    }

    #[test]
    fn rule_resolution_layers() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let c = Config::from_yaml_str(
            r#"
tests: ["**/it/**"]
rules:
  "fk/*": { severity: HIGH }
  fk/todo-comment: off
  fk/cyclomatic-complexity: { params: { threshold: 5 } }
  fk/cognitive-complexity: { params: { threshold: 20 } }
test_rules: { fk/loud: off }
overrides:
  - paths: ["legacy/**"]
    rules: { fk/cognitive-complexity: { params: { threshold: 30 } }, fk/todo-comment: LOW }
"#,
        )
        .unwrap();
        let cc = CompiledConfig::new(c, &reg).unwrap();
        let main = cc.resolve(&reg, 0, false, "src/A.fk");
        assert!(!main.rules.is_enabled("fk/todo-comment"));
        assert!(
            main.rules.is_enabled("fk/cyclomatic-complexity"),
            "exact id with params enables"
        );
        assert_eq!(
            main.rules
                .param_i64("fk/cyclomatic-complexity", "threshold", 0),
            5
        );
        assert_eq!(
            main.rules
                .param_i64("fk/cognitive-complexity", "threshold", 0),
            20
        );
        assert_eq!(main.rules.get("fk/loud").unwrap().severity, Severity::High);
        assert!(
            !main.rules.is_enabled("fk/deprecated"),
            "globs never enable deprecated rules"
        );
        assert!(
            !main.rules.is_enabled("fk/off-by-default"),
            "glob options do not enable"
        );
        assert!(main.rules.is_enabled("mote/unused-suppression"));

        assert!(cc.is_test(0, "a/it/B.fk"));
        assert!(cc.is_test(0, "src/test/B.fk"), "plugin default test glob");
        let test = cc.resolve(&reg, 0, true, "a/it/B.fk");
        assert!(!test.rules.is_enabled("fk/loud"));
        assert_ne!(test.hash, main.hash);

        let legacy = cc.resolve(&reg, 0, false, "legacy/C.fk");
        assert_eq!(
            legacy
                .rules
                .param_i64("fk/cognitive-complexity", "threshold", 0),
            30
        );
        assert_eq!(
            legacy.rules.get("fk/todo-comment").unwrap().severity,
            Severity::Low
        );

        // memoized and deterministic
        let again = cc.resolve(&reg, 0, false, "other/D.fk");
        assert!(Arc::ptr_eq(&main, &again));
        let cc2 = CompiledConfig::new(cc.config().clone(), &reg).unwrap();
        assert_eq!(cc2.resolve(&reg, 0, false, "src/A.fk").hash, main.hash);
    }

    #[test]
    fn java_release_is_a_rule_parameter_of_java_rules() {
        use crate::java_release::JAVA_RELEASE_PARAM;
        let reg = registry(vec![FakePlugin::boxed("java"), FakePlugin::boxed("fk")]);
        let cc = CompiledConfig::new(Config::default(), &reg).unwrap();
        let java = reg.plugin_index("java").unwrap();
        let fk = reg.plugin_index("fk").unwrap();
        let unknown = cc.resolve(&reg, java, false, "a/A.java");
        let j17 = cc.resolve_with_release(&reg, java, false, "a/A.java", Some(17));
        let j11 = cc.resolve_with_release(&reg, java, false, "b/B.java", Some(11));
        assert_eq!(
            j17.rules.param_i64("java/loud", JAVA_RELEASE_PARAM, 0),
            17,
            "every enabled rule gets the release"
        );
        assert_eq!(
            j17.rules
                .param_i64("java/cognitive-complexity", JAVA_RELEASE_PARAM, 0),
            17
        );
        assert!(unknown
            .rules
            .param("java/loud", JAVA_RELEASE_PARAM)
            .is_none());
        // Part of the cache key, and of the memo key.
        assert_ne!(unknown.hash, j17.hash);
        assert_ne!(j11.hash, j17.hash);
        let again = cc.resolve_with_release(&reg, java, false, "c/C.java", Some(17));
        assert!(Arc::ptr_eq(&j17, &again));
        // Other plugins never get it.
        let other = cc.resolve_with_release(&reg, fk, false, "a/A.fk", Some(17));
        assert!(other.rules.param("fk/loud", JAVA_RELEASE_PARAM).is_none());
        assert!(Arc::ptr_eq(&other, &cc.resolve(&reg, fk, false, "a/A.fk")));
    }

    #[test]
    fn java_release_config_is_validated() {
        let reg = registry(vec![FakePlugin::boxed("java")]);
        let ok = Config::from_yaml_str("java: { release: 21 }").unwrap();
        assert_eq!(ok.java.release, Some(21));
        assert!(ok.validate(&reg).is_ok());
        let zero = Config::from_yaml_str("java: { release: 0 }").unwrap();
        let e = zero.validate(&reg).unwrap_err().to_string();
        assert!(e.contains("java.release"), "{e}");
        let per_rule =
            Config::from_yaml_str("rules: { java/loud: { params: { java_release: 17 } } }")
                .unwrap();
        let e = per_rule.validate(&reg).unwrap_err().to_string();
        assert!(e.contains("configure `java: { release: N }`"), "{e}");
    }

    #[test]
    fn profiles() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let all =
            CompiledConfig::new(Config::from_yaml_str("extends: all").unwrap(), &reg).unwrap();
        let r = all.resolve(&reg, 0, false, "a.fk");
        assert!(r.rules.is_enabled("fk/off-by-default"));
        assert!(!r.rules.is_enabled("fk/deprecated"));
        let none = CompiledConfig::new(
            Config::from_yaml_str("extends: none\nrules: { fk/deprecated: on }").unwrap(),
            &reg,
        )
        .unwrap();
        let r = none.resolve(&reg, 0, false, "a.fk");
        assert_eq!(r.enabled, vec!["fk/deprecated"]);
    }

    #[test]
    fn conventions_profile_adds_tagged_rules_to_default() {
        let reg = registry(vec![FakePlugin::boxed("fk")]);
        let compile =
            |yaml: &str| CompiledConfig::new(Config::from_yaml_str(yaml).unwrap(), &reg).unwrap();
        let r = compile("extends: conventions").resolve(&reg, 0, false, "a.fk");
        let d = compile("extends: default").resolve(&reg, 0, false, "a.fk");
        // Off by default and tagged `conventions`: on in this profile only.
        assert!(r.rules.is_enabled("fk/style"));
        assert!(!d.rules.is_enabled("fk/style"));
        // Everything the default profile enables stays on; nothing else is added.
        for id in &d.enabled {
            assert!(r.rules.is_enabled(id), "{id}");
        }
        assert_eq!(r.enabled.len(), d.enabled.len() + 1);
        // Off by default without the tag, and deprecated even with the tag: off.
        assert!(!r.rules.is_enabled("fk/off-by-default"));
        assert!(!r.rules.is_enabled("fk/deprecated"));
        // `rules` still applies on top of the profile.
        let t = compile("extends: conventions\nrules: { fk/style: off, fk/off-by-default: on }")
            .resolve(&reg, 0, false, "a.fk");
        assert!(!t.rules.is_enabled("fk/style"));
        assert!(t.rules.is_enabled("fk/off-by-default"));
        // The JSON Schema lists the value.
        assert!(Config::json_schema()
            .to_string()
            .contains("\"conventions\""));
    }

    /// A small JSON Schema checker for the keywords the config schema uses.
    fn valid(root: &serde_json::Value, s: &serde_json::Value, v: &serde_json::Value) -> bool {
        use serde_json::Value;
        let Some(s) = s.as_object() else {
            return s.as_bool().unwrap_or(true);
        };
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            let name = r.trim_start_matches("#/$defs/");
            if !valid(root, &root["$defs"][name], v) {
                return false;
            }
        }
        if let Some(t) = s.get("type") {
            let ty = |t: &str| match t {
                "string" => v.is_string(),
                "number" => v.is_number(),
                "integer" => v.is_i64() || v.is_u64(),
                "boolean" => v.is_boolean(),
                "null" => v.is_null(),
                "array" => v.is_array(),
                "object" => v.is_object(),
                _ => true,
            };
            let ok = match t {
                Value::String(t) => ty(t),
                Value::Array(ts) => ts.iter().filter_map(Value::as_str).any(ty),
                _ => true,
            };
            if !ok {
                return false;
            }
        }
        if let Some(e) = s.get("enum").and_then(Value::as_array) {
            if !e.contains(v) {
                return false;
            }
        }
        if let (Some(p), Some(text)) = (s.get("pattern").and_then(Value::as_str), v.as_str()) {
            if !regex::Regex::new(p).unwrap().is_match(text) {
                return false;
            }
        }
        if s.get("const").is_some_and(|c| c != v) {
            return false;
        }
        if let Some(n) = v.as_f64() {
            let below = s
                .get("minimum")
                .and_then(Value::as_f64)
                .is_some_and(|m| n < m);
            let above = s
                .get("maximum")
                .and_then(Value::as_f64)
                .is_some_and(|m| n > m);
            if below || above {
                return false;
            }
        }
        if let Some(any) = s.get("anyOf").and_then(Value::as_array) {
            if !any.iter().any(|x| valid(root, x, v)) {
                return false;
            }
        }
        if let Some(one) = s.get("oneOf").and_then(Value::as_array) {
            if one.iter().filter(|x| valid(root, x, v)).count() != 1 {
                return false;
            }
        }
        if let Some(obj) = v.as_object() {
            let props = s.get("properties").and_then(Value::as_object);
            for (k, x) in obj {
                let ok = match props.and_then(|p| p.get(k)) {
                    Some(ps) => valid(root, ps, x),
                    None => match s.get("additionalProperties") {
                        Some(ap) => valid(root, ap, x),
                        None => true,
                    },
                };
                if !ok {
                    return false;
                }
            }
        }
        if let (Some(items), Some(arr)) = (s.get("items"), v.as_array()) {
            if !arr.iter().all(|x| valid(root, items, x)) {
                return false;
            }
        }
        true
    }

    #[test]
    fn json_schema_accepts_what_the_parser_accepts() {
        let schema = Config::json_schema();
        let check = |yaml: &str| {
            let v: serde_json::Value = serde_yaml_ng::from_str(yaml).unwrap();
            (
                Config::from_yaml_str(yaml).is_ok(),
                valid(&schema, &schema, &v),
            )
        };
        for yaml in [
            "mote: 0.1",
            "mote: \">=0.1\"",
            "rules: { a/b: high, a/c: High, a/d: LOW, a/e: OFF, a/f: On, a/g: false }",
            "rules: { a/b: { severity: medium }, a/c: { severity: Medium, enabled: true } }",
            "gate: { fail_on: high }",
            "gate: { fail_on: NONE, worsened: fail, new_coverage_pct: null, new_duplication_pct: 5 }",
            "exclude: [2024, legacy]\ncache: { dir: 5, max_size_mb: 10 }",
            "overrides: [ { paths: [\"a/**\"], rules: { a/b: low } } ]",
            "java: { release: 17 }",
            "java: { release: null }",
            "java: {}",
            "suppressions: { compat: true }",
            "suppressions: { compat: [pmd, spotbugs, intellij] }",
            "suppressions: { compat: [] }",
            "suppressions: { compat: nosonar }",
            "skip_generated_except: [\"web/js/app.bundle.js\", \"legacy/**\"]",
            "skip_generated_except: []",
        ] {
            assert_eq!(check(yaml), (true, true), "{yaml}");
        }
        // An unknown source parses (`config validate` names it) but the schema flags it.
        assert_eq!(check("suppressions: { compat: [pmdx] }"), (true, false));
        assert_eq!(check("suppressions: { compat: 1 }"), (false, false));
        for yaml in [
            "rules: { a/b: loud }",
            "rules: { a/b: { severity: CRITICAL } }",
            "gate: { fail_on: Critical }",
            "extends: everything",
            "exclud: []",
            "java: { release: \"17\" }",
            "java: { release: 1.8 }",
            "java: { level: 17 }",
            "skip_generated_except: legacy",
            "skip_generated_except: { paths: [a] }",
        ] {
            assert_eq!(check(yaml), (false, false), "{yaml}");
        }
    }

    #[test]
    fn json_schema_is_generated() {
        let s = Config::json_schema();
        let text = s.to_string();
        assert!(text.contains("max_file_size"));
        assert!(text.contains("RuleConfig"));
    }
}
