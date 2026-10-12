//! Domain model shared by every crate: rules, findings, locations, severities.

use serde::{Deserialize, Serialize};
use std::fmt;

/// How bad a finding is. Ordered: `Low < Medium < High`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Low => "LOW",
            Severity::Medium => "MEDIUM",
            Severity::High => "HIGH",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The software quality a rule protects.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// Likely to misbehave at runtime.
    Reliability,
    /// Harder to read, change, or test.
    Maintainability,
}

impl Quality {
    pub fn as_str(self) -> &'static str {
        match self {
            Quality::Reliability => "reliability",
            Quality::Maintainability => "maintainability",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RuleKind {
    Bug,
    CodeSmell,
}

/// Lifecycle state. Incubating rules report but never fail a gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleStatus {
    Stable,
    Incubating,
    Deprecated,
}

/// A typed rule parameter value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl ParamValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ParamValue::Int(v) => Some(*v),
            ParamValue::Float(v) => Some(*v as i64),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ParamValue::Int(v) => Some(*v as f64),
            ParamValue::Float(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Bool(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ParamValue::Str(v) => Some(v),
            _ => None,
        }
    }
}

/// Static default for a parameter, declared by the rule.
#[derive(Clone, Copy, Debug)]
pub enum ParamDefault {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

impl ParamDefault {
    pub fn to_value(self) -> ParamValue {
        match self {
            ParamDefault::Bool(v) => ParamValue::Bool(v),
            ParamDefault::Int(v) => ParamValue::Int(v),
            ParamDefault::Float(v) => ParamValue::Float(v),
            ParamDefault::Str(v) => ParamValue::Str(v.to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub name: &'static str,
    pub default: ParamDefault,
    pub description: &'static str,
}

/// Everything known about a rule except its implementation.
#[derive(Clone, Debug)]
pub struct RuleMeta {
    /// Stable identifier, `<language>/<kebab-name>`, e.g. `java/empty-catch-block`.
    pub id: &'static str,
    /// Short human title.
    pub name: &'static str,
    pub language: &'static str,
    pub kind: RuleKind,
    pub quality: Quality,
    pub severity: Severity,
    pub status: RuleStatus,
    /// Active in the built-in `mote:default` profile.
    pub default_enabled: bool,
    pub tags: &'static [&'static str],
    pub params: &'static [ParamSpec],
    /// Markdown: what it detects, why it matters, how to fix.
    pub description: &'static str,
    /// Rough effort to fix one occurrence.
    pub remediation_minutes: u32,
}

impl RuleMeta {
    /// Convenience constructor with sensible defaults; override fields with struct update syntax.
    pub const fn new(id: &'static str, language: &'static str, name: &'static str) -> Self {
        RuleMeta {
            id,
            name,
            language,
            kind: RuleKind::CodeSmell,
            quality: Quality::Maintainability,
            severity: Severity::Medium,
            status: RuleStatus::Stable,
            default_enabled: true,
            tags: &[],
            params: &[],
            description: "",
            remediation_minutes: 5,
        }
    }
}

/// 1-based line and column (columns count Unicode scalar values).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Default,
    schemars::JsonSchema,
)]
pub struct TextRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl TextRange {
    pub fn line(line: u32) -> Self {
        TextRange {
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 1,
        }
    }
    pub fn lines(start: u32, end: u32) -> Self {
        TextRange {
            start_line: start,
            start_col: 1,
            end_line: end,
            end_col: 1,
        }
    }
}

/// What a plugin reports; the engine turns it into a [`Finding`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawFinding {
    pub rule_id: String,
    pub message: String,
    pub range: TextRange,
    /// Enclosing declaration path, e.g. `OrderService#total` — makes fingerprints survive line shifts.
    pub symbol: Option<String>,
    /// Measured value for threshold rules (e.g. a complexity score).
    pub value: Option<f64>,
    #[serde(default)]
    pub secondary: Vec<Location>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Location {
    pub path: String,
    pub range: TextRange,
    pub message: Option<String>,
}

/// Relationship of a finding to the base being compared against.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum FindingStatus {
    /// No base comparison was requested.
    #[default]
    Unknown,
    New,
    Unchanged,
    /// Matched a base finding but its measured value increased.
    Worsened,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SuppressionInfo {
    pub kind: String,
    pub justification: Option<String>,
}

/// A fully resolved finding as emitted by the engine.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Finding {
    /// Short, stable identifier (derived from the fingerprint).
    pub id: String,
    pub rule_id: String,
    pub message: String,
    pub path: String,
    pub range: TextRange,
    pub severity: Severity,
    pub quality: Quality,
    pub symbol: Option<String>,
    pub value: Option<f64>,
    pub fingerprint: String,
    #[serde(default)]
    pub status: FindingStatus,
    /// Incubating rules never gate.
    #[serde(default)]
    pub incubating: bool,
    pub suppressed: Option<SuppressionInfo>,
    #[serde(default)]
    pub secondary: Vec<Location>,
}

impl Finding {
    pub fn sort_key(&self) -> (&str, u32, u32, &str, &str) {
        (
            &self.path,
            self.range.start_line,
            self.range.start_col,
            &self.rule_id,
            &self.message,
        )
    }
}
