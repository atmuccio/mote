//! Configuration problems: every problem of a file in one pass, each at its line and column.
//!
//! * **Parsing.** A file that deserializes is taken as it is. Otherwise [`check_document`] walks
//!   it entry by entry — each entry deserialized on its own, through YAML text, so that scalars
//!   are read exactly as in a whole-file parse (a plain `0.1` is a string where a string is
//!   expected) — and collects every unknown key, with the closest known one, and every invalid
//!   value. What remains is the *partial* configuration, which validation then checks too (see
//!   [`ConfigError::complete`](super::ConfigError::complete)), so one run reports both kinds.
//! * **Positions.** A problem names its place as a key path (`gate.fail_on`,
//!   `overrides[0].rules.<id>`). [`locate`] reads the text again with a deserializer that fails
//!   on purpose at that path: the YAML parser attaches the position of the failing key or value
//!   to the error. Configuration files are small and problems few, so a pass per problem costs
//!   nothing, and positions are exact in block and flow style alike.

use super::{
    edit_distance, CacheConfig, Config, ConfigProblem, CoverageConfig, CpdConfig, GateConfig,
    GradesConfig, JavaConfig, MetricsConfig, RuleConfig, RuleOptions, SuppressionConfig,
    TriageConfig,
};
use mote_core::ParamValue;
use schemars::JsonSchema;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor,
};
use serde_yaml_ng::{Mapping, Value};
use std::fmt::{self, Write as _};

/// One step of a key path into the YAML document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Seg {
    Key(String),
    Index(usize),
}

/// A key path from `.`-separated keys: `key_path("gate.fail_on")`.
pub(crate) fn key_path(dotted: &str) -> Vec<Seg> {
    dotted.split('.').map(|k| Seg::Key(k.to_string())).collect()
}

/// `gate.fail_on`, `overrides[0].rules`.
pub(crate) fn path_text(path: &[Seg]) -> String {
    let mut s = String::new();
    for seg in path {
        match seg {
            Seg::Key(k) => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(k);
            }
            Seg::Index(i) => {
                let _ = write!(s, "[{i}]");
            }
        }
    }
    s
}

/// A problem whose position is still to be found.
#[derive(Clone, Debug)]
pub(crate) struct Pending {
    pub path: Vec<Seg>,
    /// Point at the key (an unknown key, an unknown rule id) rather than at its value.
    pub at_key: bool,
    pub message: String,
    /// Known already (a YAML syntax error).
    pub location: Option<(usize, usize)>,
}

impl Pending {
    /// A problem with the key at `path`.
    pub fn key(path: Vec<Seg>, message: impl Into<String>) -> Pending {
        Pending {
            path,
            at_key: true,
            message: message.into(),
            location: None,
        }
    }

    /// A problem with the value at `path`.
    pub fn value(path: Vec<Seg>, message: impl Into<String>) -> Pending {
        Pending {
            path,
            at_key: false,
            message: message.into(),
            location: None,
        }
    }

    /// A problem without a place in the file (a file that cannot be read).
    pub fn general(message: impl Into<String>) -> Pending {
        Pending::value(Vec::new(), message)
    }
}

/// Find the positions of `pending` in `text` (when there is a text) and sort the problems.
pub(crate) fn resolve(text: Option<&str>, pending: Vec<Pending>) -> Vec<ConfigProblem> {
    let mut out: Vec<ConfigProblem> = pending
        .into_iter()
        .map(|p| {
            let at = p.location.or_else(|| {
                text.filter(|_| !p.path.is_empty())
                    .and_then(|t| locate(t, &p.path, p.at_key))
            });
            ConfigProblem {
                message: p.message,
                line: at.map(|a| a.0),
                column: at.map(|a| a.1),
            }
        })
        .collect();
    sort_problems(&mut out);
    out
}

/// File order: by line and column (problems without a position last), then by message; exact
/// duplicates once.
pub(crate) fn sort_problems(problems: &mut Vec<ConfigProblem>) {
    problems.sort_by(|a, b| {
        (a.line.is_none(), a.line, a.column, &a.message).cmp(&(
            b.line.is_none(),
            b.line,
            b.column,
            &b.message,
        ))
    });
    problems.dedup();
}

// ---------------------------------------------------------------------------------------------
// Positions.

/// The message of the probe's deliberate error (never a message the parser writes).
const PROBE: &str = "\u{0}mote-config-probe";

/// 1-based line and column of the key (`at_key`) or value at `path` in `text`. A path that is
/// not in the text (a single source written where a list of them is allowed, say) falls back to
/// the nearest enclosing key.
pub(crate) fn locate(text: &str, path: &[Seg], at_key: bool) -> Option<(usize, usize)> {
    let mut path = path;
    let mut at_key = at_key;
    loop {
        if let Some(at) = probe(text, path, at_key) {
            return Some(at);
        }
        let (_, parent) = path.split_last()?;
        if parent.is_empty() {
            return None;
        }
        path = parent;
        at_key = true;
    }
}

fn probe(text: &str, path: &[Seg], at_key: bool) -> Option<(usize, usize)> {
    let de = serde_yaml_ng::Deserializer::from_str(text);
    match (Probe { path, at_key }).deserialize(de) {
        Err(e) if e.to_string().contains(PROBE) => e.location().map(|l| (l.line(), l.column())),
        _ => None,
    }
}

/// Walks to `path`, skipping everything else, and fails there.
struct Probe<'a> {
    path: &'a [Seg],
    at_key: bool,
}

impl Probe<'_> {
    fn here<E: de::Error>(&self) -> Result<(), E> {
        if self.path.is_empty() {
            Err(E::custom(PROBE))
        } else {
            Ok(())
        }
    }
}

impl<'de> DeserializeSeed<'de> for Probe<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Probe<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any YAML value")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        self.here()
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        self.here()
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        self.here()
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        self.here()
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        self.here()
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        self.here()
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        self.here()
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let Some((Seg::Key(want), rest)) = self.path.split_first() else {
            return self.here();
        };
        let fail_on_key = rest.is_empty() && self.at_key;
        while let Some(hit) = map.next_key_seed(KeyProbe {
            want,
            fail: fail_on_key,
        })? {
            if hit {
                return map.next_value_seed(Probe {
                    path: rest,
                    at_key: self.at_key,
                });
            }
            map.next_value::<IgnoredAny>()?;
        }
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let Some((Seg::Index(want), rest)) = self.path.split_first() else {
            return self.here();
        };
        for _ in 0..*want {
            if seq.next_element::<IgnoredAny>()?.is_none() {
                return Ok(());
            }
        }
        seq.next_element_seed(Probe {
            path: rest,
            at_key: self.at_key,
        })
        .map(|_| ())
    }
}

/// Reads a mapping key: `true` when it is the wanted one (or fails there, for `fail`).
struct KeyProbe<'a> {
    want: &'a str,
    fail: bool,
}

impl KeyProbe<'_> {
    fn check<E: de::Error>(&self, key: &str) -> Result<bool, E> {
        if key != self.want {
            Ok(false)
        } else if self.fail {
            Err(E::custom(PROBE))
        } else {
            Ok(true)
        }
    }
}

impl<'de> DeserializeSeed<'de> for KeyProbe<'_> {
    type Value = bool;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<bool, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for KeyProbe<'_> {
    type Value = bool;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a mapping key")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<bool, E> {
        self.check(v)
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<bool, E> {
        self.check(&v.to_string())
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<bool, E> {
        self.check(&v.to_string())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<bool, E> {
        self.check(&v.to_string())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<bool, E> {
        self.check(&v.to_string())
    }
    fn visit_unit<E: de::Error>(self) -> Result<bool, E> {
        self.check("null")
    }
}

// ---------------------------------------------------------------------------------------------
// Suggestions and messages.

/// The candidate closest to `unknown` (ignoring case) when it is close enough to be a typo:
/// at most a third of its characters (at least two) differ, and not all of them.
pub(crate) fn suggest<'a, S: AsRef<str>>(unknown: &str, candidates: &'a [S]) -> Option<&'a str> {
    let u = unknown.to_ascii_lowercase();
    let len = u.chars().count();
    candidates
        .iter()
        .map(AsRef::as_ref)
        .map(|c| (edit_distance(&u, &c.to_ascii_lowercase()), c))
        .filter(|&(d, _)| d <= (len / 3).max(2) && d < len)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, c)| c)
}

/// Strip the parser's ` at line L column C` / ` at position N` from a message.
pub(crate) fn strip_positions(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(i) = rest.find(" at ") {
        let tail = &rest[i + 4..];
        let skip = position_len(tail);
        out.push_str(&rest[..i]);
        if skip == 0 {
            out.push_str(" at ");
            rest = tail;
        } else {
            rest = &tail[skip..];
        }
    }
    out.push_str(rest);
    out
}

/// Length of `line L column C` or `position N` at the start of `s` (0 when neither).
fn position_len(s: &str) -> usize {
    fn number(s: &str) -> usize {
        s.bytes().take_while(u8::is_ascii_digit).count()
    }
    if let Some(r) = s.strip_prefix("line ") {
        let l = number(r);
        if l > 0 {
            if let Some(c) = r[l..].strip_prefix(" column ") {
                let n = number(c);
                if n > 0 {
                    return "line ".len() + l + " column ".len() + n;
                }
            }
        }
    } else if let Some(r) = s.strip_prefix("position ") {
        let n = number(r);
        if n > 0 {
            return "position ".len() + n;
        }
    }
    0
}

/// The parser's wording in the configuration's terms (`u32` → a whole number, …), plus the
/// closest value for an unknown enumeration value.
fn polish(message: &str) -> String {
    let mut s = message.to_string();
    for (from, to) in [
        ("expected u8", "expected a whole number of 0 or more"),
        ("expected u16", "expected a whole number of 0 or more"),
        ("expected u32", "expected a whole number of 0 or more"),
        ("expected u64", "expected a whole number of 0 or more"),
        ("expected usize", "expected a whole number of 0 or more"),
        ("expected i32", "expected a whole number"),
        ("expected i64", "expected a whole number"),
        ("expected f64", "expected a number"),
        ("expected a sequence", "expected a list"),
        ("invalid type: sequence", "invalid type: list"),
        ("unit value", "empty value"),
        (
            "data did not match any variant of untagged enum ParamValue",
            "expected a boolean, a number or a string",
        ),
    ] {
        s = s.replace(from, to);
    }
    // `expected struct GateConfig` → `expected a map of settings`
    if let Some(i) = s.find("expected struct ") {
        let rest = &s[i + "expected struct ".len()..];
        let name = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        s = format!("{}expected a map of settings{}", &s[..i], &rest[name..]);
    }
    if let Some(hint) = variant_hint(message) {
        s.push_str(&hint);
    }
    s
}

/// `; did you mean `HIGH`?` for `unknown variant `hihg`, expected one of `HIGH`, …`.
fn variant_hint(message: &str) -> Option<String> {
    let rest = message.strip_prefix("unknown variant `")?;
    let (variant, rest) = rest.split_once('`')?;
    let expected = &rest[rest.find("expected")?..];
    let names: Vec<&str> = expected.split('`').skip(1).step_by(2).collect();
    suggest(variant, &names).map(|s| format!("; did you mean `{s}`?"))
}

// ---------------------------------------------------------------------------------------------
// The lenient pass.

/// Every problem of `text`, which does not deserialize as a [`Config`] (`typed` is that error),
/// and the configuration without the entries in error (`None` when the text is not YAML).
pub(crate) fn check_document(
    text: &str,
    typed: &serde_yaml_ng::Error,
) -> (Vec<Pending>, Option<Config>) {
    let value: Value = match serde_yaml_ng::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return (
                vec![Pending {
                    location: located(&e),
                    ..Pending::general(format!(
                        "not valid YAML: {}",
                        strip_positions(&e.to_string())
                    ))
                }],
                None,
            )
        }
    };
    let mut ck = Checker::default();
    let cleaned = match &value {
        Value::Mapping(m) => Some(check_struct::<Config>(
            m,
            &mut Vec::new(),
            &mut ck,
            &config_section,
        )),
        _ => None,
    };
    if ck.found.is_empty() {
        // Nothing the walk explains (a document that is not a mapping, …): the parser's own
        // message, at its position.
        ck.found.push(Pending {
            location: located(typed),
            ..Pending::general(polish(&strip_positions(&typed.to_string())))
        });
    }
    let partial = cleaned.and_then(|m| reparse::<Config>(&Value::Mapping(m)).ok());
    (ck.found, partial)
}

fn located(e: &serde_yaml_ng::Error) -> Option<(usize, usize)> {
    e.location().map(|l| (l.line(), l.column()))
}

/// Deserialize `value` as `T` through YAML text (see the module documentation); the error
/// message has no position.
fn reparse<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    let text = serde_yaml_ng::to_string(value).map_err(|e| e.to_string())?;
    serde_yaml_ng::from_str::<T>(&text).map_err(|e| strip_positions(&e.to_string()))
}

/// A mapping key as written (`1` for a number key).
fn key_string(k: &Value) -> String {
    match k {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        other => serde_yaml_ng::to_string(other)
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
    }
}

/// The keys a settings struct accepts (from its JSON Schema, which the configuration's schema
/// tests keep in line with the parser), sorted.
fn field_names<T: JsonSchema>() -> Vec<String> {
    let schema = schemars::SchemaGenerator::default().into_root_schema_for::<T>();
    let mut names: Vec<String> = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

#[derive(Default)]
struct Checker {
    found: Vec<Pending>,
}

impl Checker {
    fn unknown_key(&mut self, path: &[Seg], name: &str, known: &[String]) {
        let parent = path_text(&path[..path.len().saturating_sub(1)]);
        let at = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}: ")
        };
        let hint = match suggest(name, known) {
            Some(s) => format!("; did you mean `{s}`?"),
            None => format!(" (keys: {})", known.join(", ")),
        };
        self.found.push(Pending::key(
            path.to_vec(),
            format!("{at}unknown key `{name}`{hint}"),
        ));
    }

    fn value(&mut self, path: &[Seg], message: String) {
        self.found.push(Pending::value(path.to_vec(), message));
    }
}

/// Checks a section's value that did not deserialize; returns what to keep of it.
type Nested = dyn Fn(&str, &Value, &mut Vec<Seg>, &mut Checker) -> Option<Value>;

fn no_sections(_: &str, _: &Value, _: &mut Vec<Seg>, _: &mut Checker) -> Option<Value> {
    None
}

/// Check each entry of a settings struct `T` (whose other fields have defaults) on its own.
fn check_struct<T: DeserializeOwned + JsonSchema>(
    map: &Mapping,
    path: &mut Vec<Seg>,
    ck: &mut Checker,
    nested: &Nested,
) -> Mapping {
    let fields = field_names::<T>();
    let mut out = Mapping::new();
    for (k, v) in map {
        let name = key_string(k);
        path.push(Seg::Key(name.clone()));
        if !fields.contains(&name) {
            ck.unknown_key(path, &name, &fields);
        } else {
            let mut entry = Mapping::new();
            entry.insert(k.clone(), v.clone());
            match reparse::<T>(&Value::Mapping(entry)) {
                Ok(_) => {
                    out.insert(k.clone(), v.clone());
                }
                Err(msg) => {
                    let before = ck.found.len();
                    let kept = nested(&name, v, path, ck);
                    if ck.found.len() == before {
                        ck.value(path, entry_message(path, &name, &msg));
                    } else if let Some(kept) = kept {
                        out.insert(k.clone(), kept);
                    }
                }
            }
        }
        path.pop();
    }
    out
}

/// `gate.fail_on: unknown variant …` from the parser's `fail_on: unknown variant …` (its path
/// starts at the entry's key; anything below it is kept: `exclude[1]: …`).
fn entry_message(path: &[Seg], name: &str, msg: &str) -> String {
    let (sub, body) = match msg.strip_prefix(name) {
        Some(rest) if rest.starts_with(['.', '[', ':']) => match rest.find(": ") {
            Some(i) => (&rest[..i], &rest[i + 2..]),
            None => ("", msg),
        },
        _ => ("", msg),
    };
    format!("{}{sub}: {}", path_text(path), polish(body))
}

fn config_section(name: &str, v: &Value, path: &mut Vec<Seg>, ck: &mut Checker) -> Option<Value> {
    let m = match (name, v) {
        ("rules" | "test_rules", Value::Mapping(m)) => {
            return Some(Value::Mapping(check_rules(m, path, ck)))
        }
        ("overrides", Value::Sequence(items)) => {
            return Some(Value::Sequence(check_overrides(items, path, ck)))
        }
        (_, Value::Mapping(m)) => m,
        _ => return None,
    };
    let out = match name {
        "gate" => check_struct::<GateConfig>(m, path, ck, &no_sections),
        "cpd" => check_struct::<CpdConfig>(m, path, ck, &no_sections),
        "coverage" => check_struct::<CoverageConfig>(m, path, ck, &no_sections),
        "suppressions" => check_struct::<SuppressionConfig>(m, path, ck, &no_sections),
        "triage" => check_struct::<TriageConfig>(m, path, ck, &no_sections),
        "cache" => check_struct::<CacheConfig>(m, path, ck, &no_sections),
        "java" => check_struct::<JavaConfig>(m, path, ck, &no_sections),
        "metrics" => check_struct::<MetricsConfig>(m, path, ck, &no_sections),
        "grades" => check_struct::<GradesConfig>(m, path, ck, &no_sections),
        _ => return None,
    };
    Some(Value::Mapping(out))
}

/// The settings of a rule map (`rules`, `test_rules`, `overrides[i].rules`). Rule ids are
/// validated later, against the registry.
fn check_rules(map: &Mapping, path: &mut Vec<Seg>, ck: &mut Checker) -> Mapping {
    let mut out = Mapping::new();
    for (k, v) in map {
        path.push(Seg::Key(key_string(k)));
        match reparse::<RuleConfig>(v) {
            Ok(_) => {
                out.insert(k.clone(), v.clone());
            }
            Err(msg) => match v {
                Value::Mapping(options) => {
                    let before = ck.found.len();
                    let kept = check_struct::<RuleOptions>(options, path, ck, &rule_options);
                    if ck.found.len() == before {
                        ck.value(path, format!("{}: {}", path_text(path), polish(&msg)));
                    } else {
                        out.insert(k.clone(), Value::Mapping(kept));
                    }
                }
                _ => {
                    ck.value(
                        path,
                        format!("{}: {}{}", path_text(path), polish(&msg), level_hint(v)),
                    );
                    // A stand-in setting, so that the rule id is still validated.
                    out.insert(k.clone(), Value::String("on".into()));
                }
            },
        }
        path.pop();
    }
    out
}

fn rule_options(name: &str, v: &Value, path: &mut Vec<Seg>, ck: &mut Checker) -> Option<Value> {
    let ("params", Value::Mapping(params)) = (name, v) else {
        return None;
    };
    let mut out = Mapping::new();
    for (k, v) in params {
        path.push(Seg::Key(key_string(k)));
        match reparse::<ParamValue>(v) {
            Ok(_) => {
                out.insert(k.clone(), v.clone());
            }
            Err(msg) => ck.value(path, format!("{}: {}", path_text(path), polish(&msg))),
        }
        path.pop();
    }
    Some(Value::Mapping(out))
}

/// The levels a rule setting takes, for suggestions.
const LEVELS: [&str; 5] = ["off", "on", "LOW", "MEDIUM", "HIGH"];

/// What to write instead of an invalid rule setting: ESLint's levels (`error`, `warn`, `0`–`2`,
/// `[level, options]`) are common in configurations carried over by hand.
fn level_hint(v: &Value) -> String {
    let eslint_level = |v: &Value| -> Option<bool> {
        match v {
            Value::Number(n) => match n.as_u64() {
                Some(0) => Some(false),
                Some(1 | 2) => Some(true),
                _ => None,
            },
            Value::String(s) => matches!(
                s.to_ascii_lowercase().as_str(),
                "error" | "warn" | "warning"
            )
            .then_some(true),
            _ => None,
        }
    };
    match v {
        Value::Sequence(items) if items.first().and_then(eslint_level).is_some() => {
            "; ESLint's `[level, options]` form is not read here: write `on`, `off` or a \
             severity, and rule options as `{ params: { … } }`"
                .to_string()
        }
        _ => match eslint_level(v) {
            Some(false) => format!("; `{}` is an ESLint level: write `off`", key_string(v)),
            Some(true) => format!(
                "; `{}` is an ESLint level: write `on` (or a severity: LOW, MEDIUM, HIGH)",
                key_string(v)
            ),
            None => match v {
                Value::String(s) => suggest(s, &LEVELS)
                    .map(|l| format!("; did you mean `{l}`?"))
                    .unwrap_or_default(),
                _ => String::new(),
            },
        },
    }
}

/// `overrides`: each entry needs `paths`; its `rules` are checked like `rules`.
fn check_overrides(items: &[Value], path: &mut Vec<Seg>, ck: &mut Checker) -> Vec<Value> {
    const FIELDS: [&str; 2] = ["paths", "rules"];
    let fields: Vec<String> = FIELDS.iter().map(|s| s.to_string()).collect();
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        path.push(Seg::Index(i));
        match item {
            Value::Mapping(m) => {
                let mut kept = Mapping::new();
                let mut paths_ok = false;
                for (k, v) in m {
                    let name = key_string(k);
                    path.push(Seg::Key(name.clone()));
                    match (name.as_str(), v) {
                        ("paths", _) => match reparse::<Vec<String>>(v) {
                            Ok(_) => {
                                paths_ok = true;
                                kept.insert(k.clone(), v.clone());
                            }
                            Err(msg) => {
                                ck.value(path, format!("{}: {}", path_text(path), polish(&msg)))
                            }
                        },
                        ("rules", Value::Mapping(r)) => {
                            kept.insert(k.clone(), Value::Mapping(check_rules(r, path, ck)));
                        }
                        ("rules", _) => {
                            let msg = reparse::<RuleMap>(v).err().unwrap_or_default();
                            ck.value(path, format!("{}: {}", path_text(path), polish(&msg)));
                        }
                        _ => ck.unknown_key(path, &name, &fields),
                    }
                    path.pop();
                }
                if !m.keys().any(|k| key_string(k) == "paths") {
                    ck.value(
                        path,
                        format!(
                            "{}: missing key `paths` (the globs of the files these rule settings \
                             apply to)",
                            path_text(path)
                        ),
                    );
                }
                if !paths_ok {
                    // A stand-in, so that the entry's rules are still validated.
                    kept.insert(
                        Value::String("paths".into()),
                        Value::Sequence(vec![Value::String("**".into())]),
                    );
                }
                out.push(Value::Mapping(kept));
            }
            _ => {
                let msg = reparse::<super::Override>(item).err().unwrap_or_default();
                ck.value(path, format!("{}: {}", path_text(path), polish(&msg)));
            }
        }
        path.pop();
    }
    out
}

type RuleMap = std::collections::BTreeMap<String, RuleConfig>;

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, path: &str, key: bool) -> Option<(usize, usize)> {
        let path: Vec<Seg> = path
            .split('.')
            .map(|s| match s.parse::<usize>() {
                Ok(i) => Seg::Index(i),
                Err(_) => Seg::Key(s.to_string()),
            })
            .collect();
        locate(text, &path, key)
    }

    #[test]
    fn locates_keys_and_values_in_block_and_flow_style() {
        let doc = "# comment\nexclude: [a]\ngate:\n  fail_on: CRITICAL\n  min_new_lines: { x: 1 }\nrules: { \"a/b\": off, 'c/d': { severity: x } }\noverrides:\n  - paths: [x]\n    rules:\n      no-unused-vars: off\n";
        assert_eq!(at(doc, "exclude", true), Some((2, 1)));
        assert_eq!(at(doc, "exclude.0", false), Some((2, 11)));
        assert_eq!(at(doc, "gate.fail_on", true), Some((4, 3)));
        assert_eq!(at(doc, "gate.fail_on", false), Some((4, 12)));
        assert_eq!(at(doc, "gate.min_new_lines", false), Some((5, 18)));
        assert_eq!(at(doc, "rules.a/b", true), Some((6, 10)));
        assert_eq!(at(doc, "rules.c/d.severity", false), Some((6, 41)));
        assert_eq!(
            at(doc, "overrides.0.rules.no-unused-vars", true),
            Some((10, 7))
        );
        assert_eq!(at(doc, "overrides.0", false), Some((8, 5)));
        // Not in the text: the nearest enclosing key.
        assert_eq!(at(doc, "gate.fail_on.0", false), Some((4, 3)));
        assert_eq!(at(doc, "nope", true), None);
        assert_eq!(at("1: x\ntrue: y\n", "true", true), Some((2, 1)));
        // Text that stops being YAML further on still locates what comes before.
        assert_eq!(at("a: 1\nb: [1, 2\n", "a", true), Some((1, 1)));
        assert_eq!(at("a: 1\nb: [1, 2\nc: 3\n", "c", true), None);
    }

    #[test]
    fn suggestions_need_some_likeness() {
        let keys = ["exclude", "extends", "gate", "tests"];
        assert_eq!(suggest("exclud", &keys), Some("exclude"));
        assert_eq!(suggest("EXCLUDE", &keys), Some("exclude"));
        assert_eq!(suggest("gat", &keys), Some("gate"));
        assert_eq!(suggest("zzzzzz", &keys), None);
        assert_eq!(suggest("F", &["A", "B", "C", "D", "E"]), None);
        assert_eq!(suggest("of", &LEVELS), Some("off"));
        assert_eq!(suggest("hgih", &LEVELS), Some("HIGH"));
    }

    #[test]
    fn positions_are_stripped_from_messages() {
        assert_eq!(
            strip_positions("did not find expected key at line 3 column 1, while parsing a block mapping at line 1 column 1"),
            "did not find expected key, while parsing a block mapping"
        );
        assert_eq!(strip_positions("bad at position 12"), "bad");
        assert_eq!(strip_positions("look at this"), "look at this");
        assert_eq!(strip_positions("at line 3 column"), "at line 3 column");
    }

    #[test]
    fn messages_speak_configuration() {
        assert_eq!(
            polish("invalid type: string \"x\", expected u32"),
            "invalid type: string \"x\", expected a whole number of 0 or more"
        );
        assert_eq!(
            polish("invalid type: unit value, expected struct GateConfig"),
            "invalid type: empty value, expected a map of settings"
        );
        assert_eq!(
            polish("unknown variant `defualt`, expected one of `default`, `conventions`, `all`, `none`"),
            "unknown variant `defualt`, expected one of `default`, `conventions`, `all`, `none`; did you mean `default`?"
        );
        assert_eq!(
            polish("unknown variant `anotate`, expected `annotate` or `fail`"),
            "unknown variant `anotate`, expected `annotate` or `fail`; did you mean `annotate`?"
        );
        assert_eq!(
            entry_message(&key_path("gate.fail_on"), "fail_on", "fail_on: bad"),
            "gate.fail_on: bad"
        );
        assert_eq!(
            entry_message(&key_path("exclude"), "exclude", "exclude[1]: bad"),
            "exclude[1]: bad"
        );
        assert_eq!(
            entry_message(&key_path("gate"), "gate", "missing field `x`"),
            "gate: missing field `x`"
        );
    }
}
