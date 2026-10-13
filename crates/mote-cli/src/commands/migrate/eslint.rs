//! ESLint configurations → a proposal.
//!
//! Read as data: `.eslintrc.json` / `.eslintrc` (JSON with comments, or YAML), `.eslintrc.yaml`
//! / `.yml`, `package.json` (`eslintConfig`), flat configurations written as JSON (an array of
//! configuration objects, e.g. `eslint.config.json` or the output of `eslint --print-config`),
//! and JavaScript or TypeScript files (`eslint.config.js`, `.mjs`, `.cjs`, `.ts`, …,
//! `.eslintrc.js` / `.cjs`), which are parsed and read statically, never run
//! ([`super::js_config`]; with the `js` feature). What a JavaScript file only computes when it
//! runs is listed as "not statically readable", and shared configurations it references by
//! name (`js.configs.recommended`) are listed as presets.
//!
//! * Rule names map to `js/<name>` for core rules and for `@typescript-eslint/`, `react/` and
//!   `react-hooks/` rules whose name Mote uses too (only when that rule exists), plus the
//!   explicit [`EXCEPTIONS`] (from the benchmark's category map). Threshold rules (`complexity`,
//!   `max-depth`, `max-params`, `max-lines`, `max-lines-per-function`) map to the generic rules
//!   with their limit ([`THRESHOLDS`]).
//! * `"off"` / `0` turns the rule off; `"error"` / `2` turns it on; `"warn"` / `1` turns it on
//!   and, for a rule Mote rates HIGH, sets it to MEDIUM so it does not fail the default gate
//!   (a warning did not fail the lint either). Other rule options are listed as not carried
//!   over.
//! * `ignorePatterns` and flat-config global `ignores` become `exclude` globs; eslintrc
//!   `overrides` and flat-config objects with `files` become `overrides`. Patterns are read as
//!   ESLint reads them, from the configuration file's directory ([`Patterns`]): a configuration
//!   below the proposal's root gets its directory in front of them, and its settings for every
//!   file become an override for that directory.
//! * `extends` presets and plugin rules without an equivalent are listed.

use super::js_config::{contains_marker, is_reserved, keys, show, spreads, Marker};
use super::{ignore_to_glob, Level, OverrideLayer, Proposal, Rules};
use crate::CliError;
use mote_core::Severity;
use mote_engine::Registry;
use serde_json::{Map, Value};
use std::path::Path;

/// ESLint rules whose Mote equivalent has another name (from the `equiv` links of the
/// maintainers' category map), sorted.
pub const EXCEPTIONS: &[(&str, &str)] = &[
    (
        "@typescript-eslint/no-empty-object-type",
        "js/no-empty-interface",
    ),
    (
        "@typescript-eslint/no-non-null-asserted-optional-chain",
        "js/no-extra-non-null-assertion",
    ),
    ("no-empty-static-block", "js/no-empty"),
    ("no-regex-spaces", "js/redundant-regex-syntax"),
];

/// ESLint limit rules → the Mote generic rule and its limit parameter, sorted.
pub const THRESHOLDS: &[(&str, &str, &str)] = &[
    ("complexity", "js/cyclomatic-complexity", "threshold"),
    ("max-depth", "js/deep-nesting", "max"),
    ("max-lines", "js/file-too-long", "max_lines"),
    (
        "max-lines-per-function",
        "js/function-too-long",
        "max_lines",
    ),
    ("max-params", "js/too-many-parameters", "max"),
];

/// Rules that Mote leaves off by default and that well-known shared configurations turn on:
/// `(preset, ESLint rule)`, sorted, the preset named as the migration names it (eslintrc
/// `extends` strings; flat configurations' `tseslint.configs.recommended` is
/// `plugin:@typescript-eslint/recommended`). A configuration that uses such a preset without
/// turning the rule off gets the Mote rule turned on; other presets are not expanded
/// (`extends: default` stands for them).
///
/// typescript-eslint's `recommended`, `strict` and `all` families include
/// `@typescript-eslint/no-explicit-any`; its `*-type-checked-only`, `stylistic*`, `base`,
/// `eslint-recommended` and `disable-type-checked` configurations do not, and neither does
/// `recommended-requiring-type-checking` (only type-aware rules up to version 5, an alias of
/// `recommended-type-checked` from version 6 on: which one is meant cannot be told, so it is left
/// out).
pub const PRESET_RULES: &[(&str, &str)] = &[
    (
        "plugin:@typescript-eslint/all",
        "@typescript-eslint/no-explicit-any",
    ),
    (
        "plugin:@typescript-eslint/recommended",
        "@typescript-eslint/no-explicit-any",
    ),
    (
        "plugin:@typescript-eslint/recommended-type-checked",
        "@typescript-eslint/no-explicit-any",
    ),
    (
        "plugin:@typescript-eslint/strict",
        "@typescript-eslint/no-explicit-any",
    ),
    (
        "plugin:@typescript-eslint/strict-type-checked",
        "@typescript-eslint/no-explicit-any",
    ),
];

/// The rules `preset` turns on, of those in [`PRESET_RULES`].
fn rules_of_preset(preset: &str) -> impl Iterator<Item = &'static str> + '_ {
    PRESET_RULES
        .iter()
        .filter(move |(p, _)| *p == preset)
        .map(|(_, rule)| *rule)
}

/// Whether `preset` turns `rule` on ([`PRESET_RULES`]).
fn preset_sets(preset: Option<&str>, rule: &str) -> bool {
    preset.is_some_and(|p| rules_of_preset(p).any(|r| r == rule))
}

/// The value standing for a rule a preset turns on, in a rules map before the rules written
/// after the preset (which replace it): read as on, noted as coming from the preset.
const FROM_PRESET: &str = "\u{0}preset ";

fn from_preset(preset: &str) -> Value {
    Value::String(format!("{FROM_PRESET}{preset}"))
}

/// The preset a rule setting stands for ([`from_preset`]).
fn preset_of(v: &Value) -> Option<&str> {
    v.as_str()?.strip_prefix(FROM_PRESET)
}

/// What a note about a preset adds when the migration carries some of its rules over.
fn carried_from(preset: &str) -> String {
    let rules: Vec<String> = rules_of_preset(preset).map(|r| format!("`{r}`")).collect();
    if rules.is_empty() {
        String::new()
    } else {
        format!(
            " beyond {}, which Mote leaves off by default",
            rules.join(", ")
        )
    }
}

/// The rules the presets of an `extends` value turn on ([`PRESET_RULES`]), as settings in the
/// order ESLint applies them: shared configurations by name (eslintrc), and recognized ones in a
/// flat configuration's `extends`.
fn extended_rules(v: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let mut stack: Vec<&Value> = v.into_iter().collect();
    let mut items = Vec::new();
    while let Some(x) = stack.pop() {
        match x {
            Value::Array(a) => stack.extend(a.iter().rev()),
            x => items.push(x),
        }
    }
    for e in items {
        let preset = match e {
            Value::String(s) => Some(s.clone()),
            e => Marker::of(e)
                .filter(|m| !m.rules_only)
                .and_then(|m| m.preset),
        };
        if let Some(preset) = preset {
            for rule in rules_of_preset(&preset) {
                out.insert(rule.to_string(), from_preset(&preset));
            }
        }
    }
    out
}

/// Plugins whose rule names Mote reuses.
const SAME_NAME_PLUGINS: &[&str] = &["@typescript-eslint/", "react/", "react-hooks/"];

/// ESLint's layout rules (the core rules ESLint deprecated in favor of a formatter or of
/// `@stylistic`), sorted. Mote checks no layout; a configuration lists dozens of them, so
/// they are reported on one line instead of one each.
const FORMATTING_RULES: &[&str] = &[
    "array-bracket-newline",
    "array-bracket-spacing",
    "array-element-newline",
    "arrow-parens",
    "arrow-spacing",
    "block-spacing",
    "brace-style",
    "comma-dangle",
    "comma-spacing",
    "comma-style",
    "computed-property-spacing",
    "dot-location",
    "eol-last",
    "func-call-spacing",
    "function-call-argument-newline",
    "function-paren-newline",
    "generator-star-spacing",
    "implicit-arrow-linebreak",
    "indent",
    "indent-legacy",
    "jsx-quotes",
    "key-spacing",
    "keyword-spacing",
    "linebreak-style",
    "lines-around-comment",
    "lines-between-class-members",
    "max-len",
    "max-statements-per-line",
    "multiline-ternary",
    "new-parens",
    "newline-per-chained-call",
    "no-extra-semi",
    "no-floating-decimal",
    "no-mixed-spaces-and-tabs",
    "no-multi-spaces",
    "no-multiple-empty-lines",
    "no-tabs",
    "no-trailing-spaces",
    "no-whitespace-before-property",
    "nonblock-statement-body-position",
    "object-curly-newline",
    "object-curly-spacing",
    "object-property-newline",
    "one-var-declaration-per-line",
    "operator-linebreak",
    "padded-blocks",
    "padding-line-between-statements",
    "quote-props",
    "quotes",
    "rest-spread-spacing",
    "semi",
    "semi-spacing",
    "semi-style",
    "space-before-blocks",
    "space-before-function-paren",
    "space-in-parens",
    "space-infix-ops",
    "space-unary-ops",
    "spaced-comment",
    "switch-colon-spacing",
    "template-curly-spacing",
    "template-tag-spacing",
    "wrap-iife",
    "wrap-regex",
    "yield-star-spacing",
];

/// Whether `name` is a layout rule: a core one, its `@typescript-eslint/` twin, or one of the
/// formatting plugins (`@stylistic/…`, `prettier/prettier`).
fn is_formatting(name: &str) -> bool {
    let base = name.strip_prefix("@typescript-eslint/").unwrap_or(name);
    FORMATTING_RULES.binary_search(&base).is_ok()
        || name.starts_with("@stylistic/")
        || name == "prettier/prettier"
}

/// The Mote rule for an ESLint rule name, when there is one in `registry`.
pub fn mote_rule(name: &str, registry: &Registry) -> Option<String> {
    let known = |id: &str| registry.rule(id).is_some();
    if let Ok(i) = EXCEPTIONS.binary_search_by(|(k, _)| k.cmp(&name)) {
        return Some(EXCEPTIONS[i].1.to_string()).filter(|id| known(id));
    }
    if let Some((_, id, _)) = THRESHOLDS.iter().find(|(n, _, _)| *n == name) {
        return Some(id.to_string()).filter(|id| known(id));
    }
    let base = match SAME_NAME_PLUGINS.iter().find_map(|p| name.strip_prefix(p)) {
        Some(rest) => rest,
        None if name.contains('/') => return None,
        None => name,
    };
    if base.is_empty() || !base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    Some(format!("js/{base}")).filter(|id| known(id))
}

/// Remove `//` and `/* */` comments outside strings (ESLint accepts them in JSON configs).
fn strip_json_comments(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            i += 1;
        } else if c == b'"' {
            in_str = true;
            out.push(c);
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            out.push(b' ');
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_json(text: &str) -> Result<Value, String> {
    serde_json::from_str(&strip_json_comments(text)).map_err(|e| e.to_string())
}

fn parse_yaml(text: &str) -> Result<Value, String> {
    mote_engine::yaml::from_str::<Value>(text).map_err(|e| e.to_string())
}

/// A configuration read as data.
pub(super) struct Loaded {
    pub value: Value,
    /// The line of each element of a flat configuration read from a JavaScript file.
    pub lines: Vec<u32>,
}

/// The extensions of JavaScript and TypeScript configuration files.
const JS_EXTENSIONS: [&str; 6] = [".js", ".cjs", ".mjs", ".ts", ".cts", ".mts"];

/// Read a JavaScript or TypeScript configuration statically (never run).
#[cfg(feature = "js")]
fn load_js(path: &Path, lower: &str) -> Result<Loaded, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("cannot read {}: {e}", path.display())))?;
    let flat = if lower.starts_with("eslint.config.") {
        Some(true)
    } else if lower.starts_with(".eslintrc") {
        Some(false)
    } else {
        None
    };
    let read = super::js_config::read(path, &text, flat)
        .map_err(|e| CliError::usage(format!("{}: {e}", path.display())))?;
    Ok(Loaded {
        value: read.value,
        lines: read.lines,
    })
}

#[cfg(not(feature = "js"))]
fn load_js(path: &Path, lower: &str) -> Result<Loaded, CliError> {
    let hint = if lower.starts_with(".eslintrc") {
        "When it only exports an object (`module.exports = { … }`), save that object as \
         .eslintrc.json and migrate it; otherwise"
    } else {
        "Either rewrite its configuration objects as a JSON array (flat-config.json) and \
         migrate that, keeping its `ignores` and `files` blocks, or"
    };
    Err(CliError::usage(format!(
        "{}: this build of mote reads no JavaScript or TypeScript (it lacks the `js` \
         feature). {hint} save the resolved configuration of one source file as JSON and \
         migrate that: `npx eslint --print-config <a source file> > eslint-resolved.json` (it \
         lists every rule its shared configs and plugins turn on, but not the ignores and \
         per-file blocks)",
        path.display()
    )))
}

/// Read the configuration as a JSON value.
pub(super) fn load(path: &Path) -> Result<Loaded, CliError> {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let lower = file.to_ascii_lowercase();
    if JS_EXTENSIONS.iter().any(|e| lower.ends_with(e)) {
        return load_js(path, &lower);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("cannot read {}: {e}", path.display())))?;
    let bad = |e: String| CliError::usage(format!("{}: {e}", path.display()));
    let value = if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        parse_yaml(&text).map_err(bad)?
    } else if lower.ends_with(".json") {
        parse_json(&text).map_err(bad)?
    } else {
        // `.eslintrc` without an extension: JSON or YAML.
        parse_json(&text)
            .or_else(|_| parse_yaml(&text))
            .map_err(bad)?
    };
    let value = if lower == "package.json" {
        value
            .get("eslintConfig")
            .cloned()
            .ok_or_else(|| CliError::usage(format!("{}: no `eslintConfig` key", path.display())))?
    } else {
        value
    };
    if !(value.is_object() || value.is_array()) {
        return Err(bad(
            "not an ESLint configuration (expected an object or an array of objects)".into(),
        ));
    }
    Ok(Loaded {
        value,
        lines: Vec::new(),
    })
}

/// `(0 off | 1 warn | 2 error, options present)` of a rule value.
fn level_of(v: &Value) -> Option<(u8, bool)> {
    let one = |v: &Value| match v {
        Value::Number(n) => n.as_u64().filter(|n| *n <= 2).map(|n| n as u8),
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "off" => Some(0),
            "warn" => Some(1),
            "error" => Some(2),
            _ => None,
        },
        _ => None,
    };
    match v {
        Value::Array(a) => Some((one(a.first()?)?, a.len() > 1)),
        v => Some((one(v)?, false)),
    }
}

/// The limit of a threshold rule's options: `["error", 10]` or `["error", {"max": 10}]`.
fn limit_of(v: &Value) -> Option<i64> {
    let opt = v.as_array()?.get(1)?;
    opt.as_i64().or_else(|| {
        opt.get("max")
            .or_else(|| opt.get("maximum"))
            .and_then(Value::as_i64)
    })
}

/// The `rules` object of a configuration object; what was not read in it (the whole object, a
/// spread of a preset's or another value's rules) is listed. `at` names it in notes.
fn rules_object(v: Option<&Value>, at: &str, p: &mut Proposal) -> Option<Map<String, Value>> {
    let v = v?;
    if let Some(m) = Marker::of(v) {
        note_marker(&m, at, "the rules it sets are unknown", p);
        // `rules: plugin.configs.recommended.rules`: what the preset is known to turn on.
        let preset = m.preset.as_deref()?;
        let known: Map<String, Value> = rules_of_preset(preset)
            .map(|rule| (rule.to_string(), from_preset(preset)))
            .collect();
        return Some(known).filter(|k| !k.is_empty());
    }
    let obj = v.as_object()?;
    let mut certain = obj.clone();
    let list = spreads(obj);
    for m in &list {
        note_marker(m, at, "the rules it sets are unknown", p);
        // A rule written before the spread may be set again by it: its setting is unknown
        // (unless the spread is a preset known to turn it on, below).
        for name in &m.before {
            if certain.remove(name).is_some() && !preset_sets(m.preset.as_deref(), name) {
                p.seen += 1;
                p.unmapped(format!(
                    "{name} ({at}): not carried over: `{}` (line {}), written after it, may \
                     change its setting",
                    m.source, m.line
                ));
            }
        }
    }
    // The rules a spread preset turns on ([`PRESET_RULES`]), unless written after it, or unless
    // a later spread with unknown keys may change them.
    for (i, m) in list.iter().enumerate() {
        let Some(preset) = m.preset.as_deref() else {
            continue;
        };
        for rule in rules_of_preset(preset) {
            let written_after = keys(obj).any(|k| k == rule) && !m.before.iter().any(|b| b == rule);
            if written_after {
                continue;
            }
            match list[i + 1..]
                .iter()
                .find(|later| !preset_sets(later.preset.as_deref(), rule))
            {
                Some(later) => p.unmapped(format!(
                    "{rule} ({at}): not carried over: preset `{preset}` turns it on, but `{}` \
                     (line {}), written after it, may change its setting",
                    later.source, later.line
                )),
                None => {
                    certain.insert(rule.to_string(), from_preset(preset));
                }
            }
        }
    }
    Some(certain)
}

/// Map a `rules` object into `into`; `where_` names the layer in notes.
fn map_rules(
    obj: &Map<String, Value>,
    where_: &str,
    into: &mut Rules,
    p: &mut Proposal,
    registry: &Registry,
) {
    let mut names: Vec<&String> = keys(obj).collect();
    names.sort();
    for name in names {
        let v = &obj[name];
        // Turned on by a preset (not a rule the configuration lists itself).
        if let Some(preset) = preset_of(v) {
            if let Some(id) = mote_rule(name, registry) {
                into.set(&id, Level::On, format!("{name}: on in {preset}"));
            }
            continue;
        }
        p.seen += 1;
        if let Some(m) = Marker::of(v) {
            p.unmapped(format!(
                "{name}{where_}: not statically readable: `{}` (line {}: {}); its setting is unknown",
                m.source, m.line, m.reason
            ));
            continue;
        }
        let Some((lvl, options)) = level_of(v) else {
            if contains_marker(v) {
                p.unmapped(format!(
                    "{name}{where_}: not statically readable: `{}`; its setting is unknown",
                    show(v)
                ));
            } else {
                p.unmapped(format!("{name}{where_}: unreadable setting `{v}`"));
            }
            continue;
        };
        let Some(id) = mote_rule(name, registry) else {
            if lvl > 0 && is_formatting(name) {
                p.formatting.insert(name.clone());
            } else if lvl > 0 {
                p.unmapped(format!("{name}{where_}: no Mote equivalent"));
            } else {
                p.mapped += 1; // off, and there is nothing to turn off
            }
            continue;
        };
        p.mapped += 1;
        let level = match lvl {
            0 => Level::Off,
            1 if registry
                .rule(&id)
                .is_some_and(|e| e.meta.severity == Severity::High) =>
            {
                Level::Severity(Severity::Medium)
            }
            _ => Level::On,
        };
        let from = match lvl {
            1 => format!("{name}: warn"),
            _ => name.clone(),
        };
        into.set(&id, level, from);
        let threshold = THRESHOLDS.iter().find(|(n, _, _)| n == name);
        match (threshold, limit_of(v)) {
            (Some((_, _, param)), Some(limit)) if lvl > 0 => into.param(&id, param, limit),
            _ if options && lvl > 0 => p.unmapped(format!(
                "options of {name}{where_} not carried over: {}",
                show(v)
            )),
            _ => {}
        }
    }
}

/// The patterns of an ignore list; what was not read in it is listed (`at` names it).
fn ignore_list(v: Option<&Value>, at: &str, p: &mut Proposal) -> Vec<String> {
    let items: Vec<&Value> = match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(v) => vec![v],
        None => Vec::new(),
    };
    let mut out = Vec::new();
    for x in items {
        match x {
            Value::String(s) => out.push(s.clone()),
            x => {
                if let Some(m) = Marker::of(x) {
                    note_marker(&m, at, "the files it ignores are unknown", p);
                }
            }
        }
    }
    out
}

/// The patterns of a `files` list, or why the list cannot be carried over: an override for part
/// of its files would apply settings elsewhere than ESLint did.
fn files_of(v: Option<&Value>, pats: &Patterns) -> Result<Vec<String>, String> {
    let items: Vec<&Value> = match v {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(a)) => a.iter().collect(),
        Some(v) => vec![v],
    };
    let mut out = Vec::new();
    for x in items {
        match x {
            Value::String(s) => out.push(pats.files(s)?),
            Value::Array(_) => {
                return Err(
                    "a nested pattern list (files matching all of its patterns) \
                            cannot be expressed"
                        .into(),
                )
            }
            x => {
                return Err(match Marker::of(x) {
                    Some(m) => format!(
                        "not statically readable: `{}` (line {}: {})",
                        m.source, m.line, m.reason
                    ),
                    None => format!("`{}` is not a pattern", show(x)),
                })
            }
        }
    }
    Ok(out)
}

/// How ESLint reads the `files` and ignore patterns of one configuration, and from where. ESLint
/// takes them relative to the configuration file's directory; a proposal's globs are relative to
/// its own root (the directory of the `.mote.yaml`), so they get that directory in front.
#[derive(Clone, Debug, Default)]
pub(super) struct Patterns {
    /// A flat configuration: every pattern is anchored to the configuration's directory (`dist`
    /// is `<dir>/dist` only; `**/dist` is any `dist` below it). An eslintrc configuration:
    /// `ignorePatterns` follow `.gitignore` (a name without `/` at any depth below the
    /// directory) and a `files` pattern without `/` matches the file name at any depth below it.
    pub flat: bool,
    /// The configuration's directory relative to the proposal's root, with `/` (`lib/core`;
    /// empty at the root).
    pub dir: String,
}

/// `rel` below `dir`, `.` and `..` resolved; `None` when it leaves the proposal's root.
fn below(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// A glob the root anchors: Mote reads a glob without `/` at any depth (`*.min.js` is
/// `**/*.min.js`), `./` keeps it at the root.
fn anchored(g: String) -> String {
    if g.contains('/') || g.starts_with("**") {
        g
    } else {
        format!("./{g}")
    }
}

impl Patterns {
    /// An ignore pattern as an `exclude` glob: `Ok(None)` for a blank line or a negation (which
    /// an exclude cannot express), `Err` for a pattern outside the proposal's root.
    fn ignore(&self, pattern: &str) -> Result<Option<String>, String> {
        let p = pattern.trim();
        if p.is_empty() || p.starts_with('!') || (!self.flat && p.starts_with('#')) {
            return Ok(None);
        }
        let outside = || {
            Err(format!(
                "`{p}` points outside the directory of the proposal"
            ))
        };
        if !self.flat {
            let Some(g) = ignore_to_glob(p) else {
                return Ok(None);
            };
            return match below(&self.dir, &g) {
                Some(g) if !g.is_empty() => Ok(Some(g)),
                _ => outside(),
            };
        }
        let body = p.trim_start_matches("./").trim_start_matches('/');
        let dir_only = body.ends_with('/');
        let Some(mut g) = below(&self.dir, body.trim_end_matches('/')) else {
            return outside();
        };
        if g.is_empty() {
            return outside();
        }
        // A directory (or a bare name, which may be one) covers everything below it.
        let last = g.rsplit('/').next().unwrap_or("");
        if (dir_only || !last.contains(['*', '.', '?', '['])) && !g.ends_with("**") {
            g.push_str("/**");
        }
        Ok(Some(anchored(g)))
    }

    /// A `files` pattern as an `overrides` path glob.
    fn files(&self, pattern: &str) -> Result<String, String> {
        let p = pattern.trim();
        if p.starts_with('!') {
            return Err(format!("the negated pattern `{p}` cannot be expressed"));
        }
        let body = p.trim_start_matches("./");
        let rooted = body.starts_with('/');
        let body = body.trim_start_matches('/');
        let body = if self.flat || rooted || body.contains('/') || body.starts_with("**") {
            body.to_string()
        } else {
            format!("**/{body}")
        };
        match below(&self.dir, &body) {
            Some(g) if !g.is_empty() => Ok(anchored(g)),
            _ => Err(format!(
                "`{p}` points outside the directory of the proposal"
            )),
        }
    }

    /// Paths a negated ignore pattern (`!core/**`, `!keep.min.js`) re-includes, with its
    /// wildcards filled in: enough to tell whether an earlier pattern ignores some of the same
    /// files.
    fn negation_samples(&self, negation: &str) -> Vec<String> {
        let positive = negation.trim().trim_start_matches('!');
        let Ok(Some(glob)) = self.ignore(positive) else {
            return Vec::new();
        };
        let glob = glob.trim_start_matches("./");
        // `**/` matches no directory or some: a sample of each.
        let mut out = Vec::new();
        for body in [glob.replace("**/", ""), glob.replace("**/", "x/")] {
            let sample = fill_wildcards(&body);
            if sample.is_empty() || out.contains(&sample) {
                continue;
            }
            if out.is_empty() {
                out.push(sample.clone());
                out.push(format!("{sample}/x"));
            } else {
                out.push(sample);
            }
        }
        out
    }
}

/// A path matching `glob` (no `**/`): each wildcard or character class is an `x`.
fn fill_wildcards(glob: &str) -> String {
    let mut sample = String::new();
    let mut in_class = false;
    for c in glob.chars() {
        match c {
            '[' if !in_class => in_class = true,
            ']' if in_class => {
                in_class = false;
                if !sample.ends_with('x') {
                    sample.push('x');
                }
            }
            _ if in_class => {}
            '*' | '?' => {
                if !sample.ends_with('x') {
                    sample.push('x');
                }
            }
            c => sample.push(c),
        }
    }
    sample.trim_end_matches('/').to_string()
}

/// Translate an ignore list into `exclude` globs. ESLint reads the list in order, and a negation
/// (`!core/**`) re-includes files an earlier pattern ignored; an exclude list cannot re-include.
/// So a pattern that ignores files a later negation brings back is not carried over: Jenkins'
/// `*/**` followed by `!core/**`, `!src/**`, … would otherwise exclude every directory, Java
/// sources included. It is listed instead, like the negations.
fn add_excludes(patterns: &[String], from: &str, pats: &Patterns, p: &mut Proposal) {
    let is_negation = |pat: &str| pat.trim().starts_with('!');
    let negations: Vec<(usize, &str)> = patterns
        .iter()
        .enumerate()
        .filter(|(_, x)| is_negation(x))
        .map(|(i, x)| (i, x.trim()))
        .collect();
    if !negations.is_empty() {
        p.unmapped(format!(
            "{from}: negations re-include files, which an exclude cannot express: {}",
            negations
                .iter()
                .map(|(_, n)| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for (i, pat) in patterns.iter().enumerate() {
        if pat.trim().is_empty() || is_negation(pat) {
            continue;
        }
        let g = match pats.ignore(pat) {
            Ok(Some(g)) => g,
            Ok(None) => continue,
            Err(why) => {
                p.unmapped(format!("{from}: {why}; it is not carried over"));
                continue;
            }
        };
        let set = mote_engine::config::compile_path_globs(&[g.as_str()]);
        let reincluded: Vec<&str> = negations
            .iter()
            .filter(|(j, n)| *j > i && pats.negation_samples(n).iter().any(|s| set.is_match(s)))
            .map(|(_, n)| *n)
            .collect();
        if let Some(first) = reincluded.first() {
            p.unmapped(format!(
                "{from}: `{pat}` not carried over: a later negation (`{first}`{}) re-includes part \
                 of what it ignores; add excludes for what should stay out by hand",
                if reincluded.len() > 1 { ", …" } else { "" }
            ));
            continue;
        }
        if !p.exclude.iter().any(|(x, _)| *x == g) {
            p.exclude.push((g, format!("{from}: {pat}")));
        }
    }
}

/// Where a layer of the configuration is, for notes: `config[3]` (a flat configuration's
/// element, with its line when read from JavaScript), `overrides[1]`, or the top level (`None`).
struct Place {
    name: Option<String>,
    line: Option<u32>,
}

impl Place {
    fn top() -> Place {
        Place {
            name: None,
            line: None,
        }
    }

    /// After a rule name: ` (config[3])`, ` (config[3], line 12)`, or nothing at the top level.
    fn suffix(&self) -> String {
        match (&self.name, self.line) {
            (Some(n), Some(l)) => format!(" ({n}, line {l})"),
            (Some(n), None) => format!(" ({n})"),
            (None, _) => String::new(),
        }
    }

    /// A key of the layer: `config[3].files`, `config[3].files (line 12)`, `ignorePatterns`.
    fn key(&self, key: &str) -> String {
        match (&self.name, self.line) {
            (Some(n), Some(l)) => format!("{n}.{key} (line {l})"),
            (Some(n), None) => format!("{n}.{key}"),
            (None, _) => key.to_string(),
        }
    }

    /// The layer itself: `config[3]`, or `configuration` at the top level.
    fn itself(&self) -> String {
        match (&self.name, self.line) {
            (Some(n), Some(l)) => format!("{n} (line {l})"),
            (Some(n), None) => n.clone(),
            (None, _) => "configuration".to_string(),
        }
    }
}

/// List a value that was not read (`at` names where it is): a shared configuration by name, or
/// anything else as not statically readable, saying what is lost (`consequence`).
fn note_marker(m: &Marker, at: &str, consequence: &str, p: &mut Proposal) {
    let what = match &m.preset {
        Some(name) if m.rules_only => format!(
            "{at}: the rules of preset `{name}` (`{}`, line {}) are not expanded{}; \
             `extends: default` covers Mote's own selection",
            m.source,
            m.line,
            carried_from(name)
        ),
        Some(name) => format!(
            "{at}: preset `{name}` (`{}`, line {}) is not expanded{}; `extends: default` covers \
             Mote's own selection",
            m.source,
            m.line,
            carried_from(name)
        ),
        None => format!(
            "{at}: not statically readable: `{}` (line {}: {}); {consequence}",
            m.source, m.line, m.reason
        ),
    };
    p.unmapped(what);
}

/// The keys of a configuration object that matter to the migration.
const CONFIG_KEYS: &[&str] = &[
    "excludedFiles",
    "extends",
    "files",
    "ignorePatterns",
    "ignores",
    "overrides",
    "rules",
];

/// List the spreads of a configuration object whose keys are unknown (`{ files, ...preset }`).
fn note_spreads(obj: &Map<String, Value>, place: &Place, p: &mut Proposal) {
    for m in spreads(obj) {
        let replaced: Vec<&str> = m
            .before
            .iter()
            .map(String::as_str)
            .filter(|k| CONFIG_KEYS.contains(k))
            .collect();
        let may_replace = if replaced.is_empty() {
            String::new()
        } else {
            format!(
                " (and it may replace the `{}` written before it)",
                replaced.join("`, `")
            )
        };
        match &m.preset {
            Some(name) => p.unmapped(format!(
                "{}: preset `{name}` (`{}`, line {}) is not expanded{may_replace}; \
                 `extends: default` covers Mote's own selection",
                place.itself(),
                m.source,
                m.line
            )),
            None => note_marker(
                &m,
                &place.itself(),
                &format!("what it sets is unknown{may_replace}"),
                p,
            ),
        }
    }
}

/// The spread of `obj` whose keys are unknown that may set `key`: one written after `key`, or
/// any when `key` is not written at all. A setting it may replace is not carried over.
fn unsure(obj: &Map<String, Value>, key: &str) -> Option<Marker> {
    let written = keys(obj).any(|k| k == key);
    spreads(obj)
        .into_iter()
        .find(|m| !written || m.before.iter().any(|k| k == key))
}

/// The note for settings whose files are unknown: a spread may set `files`.
fn scope_unknown(place: &Place, m: &Marker) -> String {
    format!(
        "{}: not carried over: `{}` (line {}) may set `files`, so the files they apply to are unknown",
        place.key("rules"),
        m.source,
        m.line
    )
}

/// Whether `obj` has rule settings to carry over.
fn has_rules(obj: &Map<String, Value>) -> bool {
    let enabling = |m: &Marker| {
        m.preset
            .as_deref()
            .is_some_and(|p| rules_of_preset(p).next().is_some())
    };
    match obj.get("rules") {
        Some(Value::Object(r)) => keys(r).next().is_some() || spreads(r).iter().any(enabling),
        Some(v) => match Marker::of(v) {
            Some(m) => enabling(&m),
            None => !v.is_null(),
        },
        None => false,
    }
}

/// The `rules` of `obj` unless a spread written after them may replace them (then listed).
fn certain_rules(
    obj: &Map<String, Value>,
    place: &Place,
    p: &mut Proposal,
) -> Option<Map<String, Value>> {
    if has_rules(obj) && obj.contains_key("rules") {
        if let Some(m) = unsure(obj, "rules") {
            p.unmapped(format!(
                "{}: not carried over: `{}` (line {}), written after them, may replace them",
                place.key("rules"),
                m.source,
                m.line
            ));
            return None;
        }
    }
    let at = place.key("rules");
    rules_object(obj.get("rules"), &at, p)
}

/// Whether `obj` sets rules: its own, or through presets it extends that turn rules on.
fn sets_rules(obj: &Map<String, Value>) -> bool {
    has_rules(obj) || !extended_rules(obj.get("extends")).is_empty()
}

/// The rules of configuration object `obj` as ESLint applies them: those the presets it extends
/// turn on ([`PRESET_RULES`]) first, then its own `rules` ([`certain_rules`]), which win. A rule
/// a preset turns on is not carried over (and is listed) when something that was not read may
/// change it: a spread of `obj` that may replace its `extends` or `rules`, rules that are not
/// statically readable, or a spread with unknown keys in them.
fn rules_with_extends(
    obj: &Map<String, Value>,
    place: &Place,
    p: &mut Proposal,
) -> Option<Map<String, Value>> {
    let rules = certain_rules(obj, place, p);
    let implied = extended_rules(obj.get("extends"));
    if implied.is_empty() {
        return rules;
    }
    let shown = |m: &Marker| format!("`{}` (line {})", m.source, m.line);
    let mut out = Map::new();
    for (name, v) in implied {
        if rules.as_ref().is_some_and(|r| r.contains_key(&name)) {
            continue; // set by the configuration itself
        }
        let unknown = unsure(obj, "extends")
            .or_else(|| unsure(obj, "rules"))
            .map(|m| shown(&m))
            .or_else(|| {
                let r = obj.get("rules")?;
                match Marker::of(r) {
                    Some(m) => (!preset_sets(m.preset.as_deref(), &name)).then(|| shown(&m)),
                    None => spreads(r.as_object()?)
                        .iter()
                        .find(|m| !preset_sets(m.preset.as_deref(), &name))
                        .map(shown),
                }
            });
        match unknown {
            Some(what) => p.unmapped(format!(
                "{name}{}: not carried over: preset `{}` turns it on, but {what} may change its \
                 setting",
                place.suffix(),
                preset_of(&v).unwrap_or_default()
            )),
            None => {
                out.insert(name, v);
            }
        }
    }
    out.extend(rules.unwrap_or_default());
    Some(out)
}

fn note_extends(v: Option<&Value>, place: &Place, p: &mut Proposal) {
    let items: Vec<&Value> = match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(v) => vec![v],
        None => Vec::new(),
    };
    for e in items {
        match e {
            Value::String(e) => p.unmapped(format!(
                "extends `{e}`{}: presets are not expanded{}; `extends: default` covers Mote's own selection",
                place.suffix(),
                carried_from(e)
            )),
            Value::Array(_) => note_extends(Some(e), place, p),
            e => match Marker::of(e) {
                Some(m) => note_marker(
                    &m,
                    &place.key("extends"),
                    "the configuration it extends is unknown",
                    p,
                ),
                None => p.unmapped(format!(
                    "{}: a configuration object written in `extends` is not carried over; move \
                     its settings into the configuration object itself",
                    place.key("extends")
                )),
            },
        }
    }
}

/// A flat configuration: an array of configuration objects. Objects without `files` apply to
/// every file, later ones winning per rule; objects with `files` become `overrides`.
fn propose_flat(
    items: &[Value],
    lines: &[u32],
    pats: &Patterns,
    p: &mut Proposal,
    registry: &Registry,
) {
    let mut base = Map::new();
    for (i, item) in items.iter().enumerate() {
        let place = Place {
            name: Some(format!("config[{i}]")),
            line: lines.get(i).copied(),
        };
        if let Some(m) = Marker::of(item) {
            note_marker(
                &m,
                &format!("config[{i}]"),
                "what it configures is unknown",
                p,
            );
            // A preset for every file (`...tseslint.configs.recommended`): the rules it is known
            // to turn on, here in the order of the configuration.
            base.extend(extended_rules(Some(item)));
            continue;
        }
        let Some(obj) = item.as_object() else {
            p.unmapped(format!(
                "config[{i}]: `{}` is not a configuration object (shared configs are not expanded)",
                show(item)
            ));
            continue;
        };
        note_spreads(obj, &place, p);
        note_extends(obj.get("extends"), &place, p);
        let ignores = ignore_list(obj.get("ignores"), &place.key("ignores"), p);
        let only_ignores =
            keys(obj).all(|k| k == "ignores" || k == "name") && spreads(obj).is_empty();
        if only_ignores {
            add_excludes(&ignores, &place.key("ignores"), pats, p);
            continue;
        }
        let files = match files_of(obj.get("files"), pats) {
            Ok(files) => files,
            Err(why) => {
                p.unmapped(format!(
                    "{}: {why}; the settings of config[{i}] are not carried over",
                    place.key("files")
                ));
                continue;
            }
        };
        if sets_rules(obj) {
            if let Some(m) = unsure(obj, "files") {
                p.unmapped(scope_unknown(&place, &m));
                continue;
            }
        }
        let rules = rules_with_extends(obj, &place, p);
        if files.is_empty() {
            for (k, v) in rules.into_iter().flatten() {
                if !is_reserved(&k) {
                    base.insert(k, v);
                }
            }
            if !ignores.is_empty() {
                p.unmapped(format!(
                    "{}: exceptions to a configuration object are not expressible",
                    place.key("ignores")
                ));
            }
        } else {
            let mut layer = Rules::default();
            if let Some(r) = rules {
                map_rules(&r, &place.suffix(), &mut layer, p, registry);
            }
            if !ignores.is_empty() {
                p.unmapped(format!(
                    "{}: exceptions to `files` are not expressible; the override applies to all of `files`",
                    place.key("ignores")
                ));
            }
            if !layer.0.is_empty() {
                p.overrides.push(OverrideLayer {
                    paths: files,
                    rules: layer,
                    from: place.key("files"),
                });
            }
        }
    }
    let mut rules = Rules::default();
    map_rules(&base, "", &mut rules, p, registry);
    set_base_rules(rules, pats, p);
}

/// The rules of a configuration for every file it applies to: the proposal's `rules`, or, for a
/// configuration in a sub-directory (which ESLint applies to the files below it only), an
/// override of that directory ahead of the others.
fn set_base_rules(rules: Rules, pats: &Patterns, p: &mut Proposal) {
    if pats.dir.is_empty() {
        p.rules = rules;
    } else if !rules.0.is_empty() {
        p.overrides.insert(
            0,
            OverrideLayer {
                paths: vec![format!("{}/**", pats.dir)],
                rules,
                from: format!(
                    "the configuration's own directory: ESLint applies it below {}/",
                    pats.dir
                ),
            },
        );
    }
}

/// An eslintrc configuration: one object with `rules`, `ignorePatterns` and `overrides`.
fn propose_eslintrc(
    obj: &Map<String, Value>,
    pats: &Patterns,
    p: &mut Proposal,
    registry: &Registry,
) {
    let top = Place::top();
    note_spreads(obj, &top, p);
    note_extends(obj.get("extends"), &top, p);
    let mut rules = Rules::default();
    if let Some(r) = rules_with_extends(obj, &top, p) {
        map_rules(&r, "", &mut rules, p, registry);
    }
    let ignores = ignore_list(obj.get("ignorePatterns"), "ignorePatterns", p);
    add_excludes(&ignores, "ignorePatterns", pats, p);
    let mut base = std::mem::take(&mut p.overrides);
    set_base_rules(rules, pats, p);
    p.overrides.append(&mut base);
    let list = match obj.get("overrides") {
        Some(Value::Array(list)) => list.as_slice(),
        Some(v) => {
            if let Some(m) = Marker::of(v) {
                note_marker(&m, "overrides", "the overrides it holds are unknown", p);
            }
            &[]
        }
        None => &[],
    };
    for (i, o) in list.iter().enumerate() {
        let place = Place {
            name: Some(format!("overrides[{i}]")),
            line: None,
        };
        if let Some(m) = Marker::of(o) {
            note_marker(
                &m,
                &format!("overrides[{i}]"),
                "what it configures is unknown",
                p,
            );
            continue;
        }
        let Some(o) = o.as_object() else {
            continue;
        };
        note_spreads(o, &place, p);
        note_extends(o.get("extends"), &place, p);
        let files = match files_of(o.get("files"), pats) {
            Ok(files) => files,
            Err(why) => {
                p.unmapped(format!(
                    "{}: {why}; the settings of overrides[{i}] are not carried over",
                    place.key("files")
                ));
                continue;
            }
        };
        if sets_rules(o) {
            if let Some(m) = unsure(o, "files") {
                p.unmapped(scope_unknown(&place, &m));
                continue;
            }
        }
        let mut rules = Rules::default();
        if let Some(r) = rules_with_extends(o, &place, p) {
            map_rules(&r, &place.suffix(), &mut rules, p, registry);
        }
        if o.get("excludedFiles").is_some() {
            p.unmapped(format!(
                "overrides[{i}].excludedFiles: exceptions to `files` are not expressible"
            ));
        }
        if !rules.0.is_empty() && !files.is_empty() {
            p.overrides.push(OverrideLayer {
                paths: files,
                rules,
                from: place.key("files"),
            });
        }
    }
}

/// The directory of `file` relative to `root`, with `/` (empty when it is `root`); `None` when
/// it is not below `root`.
pub(super) fn dir_below(file: &Path, root: &Path) -> Option<String> {
    let normalized = |p: &Path| -> Option<std::path::PathBuf> {
        let mut out = std::path::PathBuf::new();
        for c in std::path::absolute(p).ok()?.components() {
            match c {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    out.pop();
                }
                c => out.push(c.as_os_str()),
            }
        }
        Some(out)
    };
    let dir = normalized(file)?.parent()?.to_path_buf();
    let rel = dir.strip_prefix(normalized(root)?).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// Whether ESLint loads a file of this name by itself (and so reads its patterns from its
/// directory): `eslint.config.*`, `.eslintrc*`, `package.json`.
fn loaded_by_eslint(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    name == "package.json"
        || name == ".eslintrc"
        || [".eslintrc.", "eslint.config."]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Read `path` and propose a configuration whose globs are relative to `root` (the directory of
/// the `.mote.yaml` it is for).
pub fn propose(
    path: &Path,
    name: &str,
    root: &Path,
    registry: &Registry,
) -> Result<Proposal, CliError> {
    let loaded = load(path)?;
    let mut p = Proposal::new(format!("eslint {name}"));
    p.compat = &["eslint"];
    // Only a file ESLint loads itself has a directory its patterns are read from; a copy saved
    // under another name (a flat configuration as JSON, `eslint --print-config` output) is read
    // from the proposal's root.
    let below = dir_below(path, root);
    let dir = match below {
        Some(dir) if loaded_by_eslint(path) => dir,
        Some(dir) if dir.is_empty() => dir,
        Some(dir) => {
            p.unmapped(format!(
                "{name} is not a file ESLint loads by itself: its patterns are read from {}, \
                 where the proposal applies, not from {dir}/",
                root.display()
            ));
            String::new()
        }
        None => {
            p.unmapped(format!(
                "{name} is not below {}, where the proposal applies: its patterns are read as if \
                 it were there; check them",
                root.display()
            ));
            String::new()
        }
    };
    let pats = Patterns {
        flat: loaded.value.is_array(),
        dir,
    };
    match &loaded.value {
        // `export default tseslint.configs.recommended`: a flat configuration of one preset.
        v if Marker::of(v).is_some_and(|m| m.preset.is_some()) => propose_flat(
            std::slice::from_ref(v),
            &loaded.lines,
            &pats,
            &mut p,
            registry,
        ),
        v if Marker::of(v).is_some() => {
            if let Some(m) = Marker::of(v) {
                note_marker(
                    &m,
                    "the exported configuration",
                    "nothing in it could be read",
                    &mut p,
                );
            }
        }
        Value::Array(items) => propose_flat(items, &loaded.lines, &pats, &mut p, registry),
        Value::Object(obj) => propose_eslintrc(obj, &pats, &mut p, registry),
        _ => {}
    }
    if !p.formatting.is_empty() {
        let names: Vec<String> = std::mem::take(&mut p.formatting).into_iter().collect();
        p.unmapped(format!(
            "{} formatting rule{}, a formatter's job (Mote checks no layout): {}",
            names.len(),
            if names.len() == 1 { "" } else { "s" },
            names.join(", ")
        ));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_are_stripped_outside_strings() {
        let t = "{\n  // a comment\n  \"a\": \"http://x/*y*/\", /* block */ \"b\": 1\n}";
        let v = parse_json(t).unwrap();
        assert_eq!(v["a"], "http://x/*y*/");
        assert_eq!(v["b"], 1);
    }

    #[test]
    fn levels_and_limits() {
        use serde_json::json;
        assert_eq!(level_of(&json!("off")), Some((0, false)));
        assert_eq!(level_of(&json!(1)), Some((1, false)));
        assert_eq!(level_of(&json!(["error", "always"])), Some((2, true)));
        assert_eq!(level_of(&json!(["Warn"])), Some((1, false)));
        assert_eq!(level_of(&json!("sometimes")), None);
        assert_eq!(level_of(&json!(7)), None);
        assert_eq!(limit_of(&json!(["error", 12])), Some(12));
        assert_eq!(limit_of(&json!(["error", {"max": 5}])), Some(5));
        assert_eq!(limit_of(&json!("error")), None);
    }

    fn semantics(flat: bool, dir: &str) -> Patterns {
        Patterns {
            flat,
            dir: dir.to_string(),
        }
    }

    /// ESLint reads patterns relative to the configuration's directory: a flat configuration
    /// anchors every pattern there, eslintrc matches names without `/` at any depth below it.
    #[test]
    fn patterns_are_read_from_the_configuration_directory() {
        let ignore = |p: &Patterns, x: &str| p.ignore(x).unwrap().unwrap_or_default();
        let files = |p: &Patterns, x: &str| p.files(x).unwrap();
        for (dir, flat, cases) in [
            (
                "",
                true,
                &[
                    ("dist/", "dist/**"),
                    ("dist", "dist/**"),
                    ("/scripts", "scripts/**"),
                    ("*.min.js", "./*.min.js"),
                    (".pnp.cjs", "./.pnp.cjs"),
                    ("**/node_modules/", "**/node_modules/**"),
                    ("**/*.snap", "**/*.snap"),
                    ("src/gen/*.js", "src/gen/*.js"),
                ][..],
            ),
            (
                "lib/core",
                true,
                &[
                    ("dist/", "lib/core/dist/**"),
                    ("*.min.js", "lib/core/*.min.js"),
                    ("./coverage", "lib/core/coverage/**"),
                    ("**/node_modules/", "lib/core/**/node_modules/**"),
                    ("../shared/out/", "lib/shared/out/**"),
                ][..],
            ),
            (
                "",
                false,
                &[
                    ("dist/", "**/dist/**"),
                    ("/build", "build/**"),
                    ("*.min.js", "**/*.min.js"),
                    ("src/generated/**", "src/generated/**"),
                ][..],
            ),
            (
                "web",
                false,
                &[
                    ("dist/", "web/**/dist/**"),
                    ("/build", "web/build/**"),
                    ("*.min.js", "web/**/*.min.js"),
                    ("src/generated/**", "web/src/generated/**"),
                ][..],
            ),
        ] {
            let p = semantics(flat, dir);
            for (pattern, want) in cases {
                assert_eq!(ignore(&p, pattern), *want, "{dir} {flat} {pattern}");
            }
        }
        let p = semantics(true, "lib/core");
        assert!(p.ignore("../../../elsewhere/").is_err());
        assert_eq!(p.ignore("!keep.js").unwrap(), None);
        assert_eq!(files(&p, "**/*.ts"), "lib/core/**/*.ts");
        assert_eq!(files(&p, "*.mjs"), "lib/core/*.mjs");
        assert_eq!(files(&semantics(true, ""), "*.mjs"), "./*.mjs");
        assert_eq!(
            files(&semantics(true, ""), "./src/**/*.tsx"),
            "src/**/*.tsx"
        );
        assert!(p.files("!**/*.spec.ts").is_err());
        let p = semantics(false, "web");
        assert_eq!(files(&p, "*.test.ts"), "web/**/*.test.ts");
        assert_eq!(files(&p, "test/**/*.ts"), "web/test/**/*.ts");
        assert_eq!(files(&semantics(false, ""), "*.test.ts"), "**/*.test.ts");
        // Negations re-include files below the configuration's directory too.
        assert_eq!(
            semantics(true, "lib/core").negation_samples("!keep.min.js"),
            vec!["lib/core/keep.min.js", "lib/core/keep.min.js/x"]
        );
        assert_eq!(
            semantics(false, "web").negation_samples("!keep.min.js"),
            vec!["web/keep.min.js", "web/keep.min.js/x", "web/x/keep.min.js"]
        );
    }

    #[test]
    fn the_directory_of_the_configuration_below_the_root() {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            dir_below(
                &d.join("tests/fixtures/migrate/flat-js/eslint.config.mjs"),
                d
            )
            .as_deref(),
            Some("tests/fixtures/migrate/flat-js")
        );
        assert_eq!(
            dir_below(&d.join("src/../eslint.config.js"), d).as_deref(),
            Some("")
        );
        assert_eq!(dir_below(&d.join("../x/eslint.config.js"), d), None);
    }

    #[test]
    fn negations_keep_the_patterns_they_reopen_out_of_the_excludes() {
        let (flat, eslintrc) = (semantics(true, ""), semantics(false, ""));
        assert_eq!(
            eslintrc.negation_samples("!core/**"),
            vec!["core/x".to_string(), "core/x/x".to_string()]
        );
        assert_eq!(
            eslintrc.negation_samples("!keep.min.js"),
            vec!["keep.min.js", "keep.min.js/x", "x/keep.min.js"]
        );
        assert_eq!(
            flat.negation_samples("!keep.min.js"),
            vec!["keep.min.js", "keep.min.js/x"]
        );
        assert_eq!(
            eslintrc.negation_samples("!/src/[ab]*.js"),
            vec!["src/x.js", "src/x.js/x"]
        );
        let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Jenkins: ignore every directory, then re-include the source ones.
        let mut p = Proposal::default();
        let ignores = strings(&[
            "*/**",
            "!core/**",
            "!src/**",
            "**/target/",
            "dist/",
            "!dist/keep.js",
            "**/node/",
        ]);
        add_excludes(&ignores, "config[0].ignores", &flat, &mut p);
        let globs: Vec<&str> = p.exclude.iter().map(|(g, _)| g.as_str()).collect();
        assert_eq!(globs, ["**/target/**", "**/node/**"], "{p:?}");
        assert!(p.unmapped.iter().any(|u| u.starts_with(
            "config[0].ignores: `*/**` not carried over: a later negation (`!core/**`, …)"
        )));
        assert!(p
            .unmapped
            .iter()
            .any(|u| u.starts_with("config[0].ignores: `dist/` not carried over")));
        assert!(p.unmapped.iter().any(|u| u
            == "config[0].ignores: negations re-include files, which an exclude cannot express: `!core/**`, `!src/**`, `!dist/keep.js`"));
        // A pattern the negation does not touch is kept.
        let mut p = Proposal::default();
        add_excludes(
            &strings(&["dist/", "*.min.js", "!keep.min.js"]),
            "ignorePatterns",
            &eslintrc,
            &mut p,
        );
        let globs: Vec<&str> = p.exclude.iter().map(|(g, _)| g.as_str()).collect();
        assert_eq!(globs, ["**/dist/**"], "{p:?}");
    }

    /// typescript-eslint's `recommended`, `strict` and `all` families turn
    /// `@typescript-eslint/no-explicit-any` on; its other configurations do not.
    #[test]
    fn presets_that_turn_rules_on() {
        use serde_json::json;
        const ANY: &str = "@typescript-eslint/no-explicit-any";
        for preset in [
            "plugin:@typescript-eslint/recommended",
            "plugin:@typescript-eslint/recommended-type-checked",
            "plugin:@typescript-eslint/strict",
            "plugin:@typescript-eslint/strict-type-checked",
            "plugin:@typescript-eslint/all",
        ] {
            assert!(preset_sets(Some(preset), ANY), "{preset}");
        }
        for preset in [
            "plugin:@typescript-eslint/recommended-type-checked-only",
            "plugin:@typescript-eslint/strict-type-checked-only",
            "plugin:@typescript-eslint/recommended-requiring-type-checking",
            "plugin:@typescript-eslint/stylistic",
            "plugin:@typescript-eslint/eslint-recommended",
            "plugin:@typescript-eslint/base",
            "plugin:@typescript-eslint/disable-type-checked",
            "eslint:recommended",
            "prettier",
        ] {
            assert!(!preset_sets(Some(preset), ANY), "{preset}");
        }
        // `extends`, strings and nested lists in order; the later preset names the setting.
        let m = extended_rules(Some(&json!([
            "eslint:recommended",
            [
                "plugin:@typescript-eslint/recommended",
                "plugin:@typescript-eslint/strict"
            ],
            "prettier"
        ])));
        assert_eq!(m.len(), 1);
        assert_eq!(preset_of(&m[ANY]), Some("plugin:@typescript-eslint/strict"));
        assert!(extended_rules(Some(&json!("plugin:@typescript-eslint/stylistic"))).is_empty());
        assert_eq!(level_of(&from_preset("x")), None, "never read as a level");
    }

    /// `rules: { ...plugin.configs.recommended.rules, … }`: what the preset is known to turn on,
    /// unless written after it, or unless a spread with unknown keys follows it.
    #[test]
    fn rules_of_a_spread_preset() {
        use super::super::js_config::{MARKER, SPREADS};
        use serde_json::json;
        const ANY: &str = "@typescript-eslint/no-explicit-any";
        let spread = |preset: Option<&str>, line: u32, before: &[&str]| {
            let mut m =
                json!({"source": format!("...spread{line}"), "line": line, "before": before});
            if let Some(preset) = preset {
                m["preset"] = json!(preset);
                m["rules_only"] = json!(true);
            }
            json!({ MARKER: m })
        };
        let rec = Some("plugin:@typescript-eslint/recommended");
        let read = |rules: Value| {
            let mut p = Proposal::default();
            let r = rules_object(Some(&rules), "rules", &mut p).unwrap();
            (r, p.unmapped)
        };
        // Written before the preset: the preset's setting wins, without a note about it.
        let (r, notes) = read(
            json!({ ANY: "off", "no-var": "error", SPREADS: [spread(rec, 3, &[ANY, "no-var"])] }),
        );
        assert_eq!(preset_of(&r[ANY]), rec);
        assert!(!r.contains_key("no-var"));
        assert!(!notes.iter().any(|n| n.starts_with(ANY)), "{notes:?}");
        // Written after it: the configuration's own setting.
        let (r, _) = read(json!({ ANY: "off", SPREADS: [spread(rec, 3, &[])] }));
        assert_eq!(r[ANY], "off");
        // Not written: on.
        let (r, _) = read(json!({ SPREADS: [spread(rec, 3, &[])] }));
        assert_eq!(preset_of(&r[ANY]), rec);
        // A spread with unknown keys after it may turn it off again.
        let (r, notes) = read(json!({ SPREADS: [spread(rec, 3, &[]), spread(None, 4, &[])] }));
        assert!(!r.contains_key(ANY));
        assert!(
            notes.contains(&format!("{ANY} (rules): not carried over: preset `plugin:@typescript-eslint/recommended` turns it on, but `...spread4` (line 4), written after it, may change its setting")),
            "{notes:?}"
        );
        // The rules of a preset as a whole value.
        let mut p = Proposal::default();
        let r = rules_object(Some(&spread(rec, 2, &[])), "rules", &mut p).unwrap();
        assert_eq!(preset_of(&r[ANY]), rec);
    }

    #[test]
    fn tables_are_sorted() {
        assert!(PRESET_RULES.windows(2).all(|w| w[0] < w[1]));
        assert!(EXCEPTIONS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(THRESHOLDS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(FORMATTING_RULES.windows(2).all(|w| w[0] < w[1]));
        assert!(is_formatting("@typescript-eslint/indent") && is_formatting("@stylistic/semi"));
        assert!(!is_formatting("no-var") && !is_formatting("eqeqeq"));
    }
}
