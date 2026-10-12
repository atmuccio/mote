//! PMD rulesets (`ruleset.xml`) → a proposal.
//!
//! * `<rule ref="category/java/<category>.xml/<Name>"/>` (or the older `rulesets/java/<set>.xml/
//!   <Name>`) enables the mapped Mote rule; `<priority>` 1–2 → `HIGH`, 3 → `MEDIUM`, 4–5 →
//!   `LOW` (without a priority the rule keeps its own severity). Thresholds carry over as the
//!   rule's parameters ([`PROPERTIES`]); other rule properties are listed as not mapped.
//! * A reference to a whole category or ruleset cannot be expanded (which rules it holds is PMD's
//!   data, not the file's): it is listed as not mapped, and its `<exclude name="…"/>` entries turn
//!   the mapped rules off.
//! * `<exclude-pattern>` regular expressions become `exclude` globs where the translation is
//!   exact enough (`.*/generated/.*` → `**/generated/**`); others are listed.
//! * Custom rules (`<rule name="…" class="…">`), other languages and other files' rules are
//!   listed.

use super::{Level, Proposal};
use crate::CliError;
use mote_core::Severity;
use mote_engine::Registry;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use std::path::Path;

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.try_get_attribute(name).ok().flatten().and_then(|a| {
        a.normalized_value(XmlVersion::default())
            .ok()
            .map(|v| v.into_owned())
    })
}

/// A `<rule>` element as read.
#[derive(Default)]
struct RuleElem {
    reference: Option<String>,
    name: Option<String>,
    class: Option<String>,
    priority: Option<u32>,
    excludes: Vec<String>,
    /// `(name, value)`: the `value` attribute, or the text of a `<value>` child.
    properties: Vec<(String, Option<String>)>,
}

/// A PMD rule property carried over as a parameter of the Mote rule the PMD rule maps to.
pub struct Property {
    /// The PMD rule (current name).
    pub rule: &'static str,
    pub property: &'static str,
    /// The Mote rule's parameter.
    pub param: &'static str,
    /// Added to the property's value: PMD reports these metrics from its level on (`>=`) where
    /// Mote reports above its threshold (`>`), hence −1 (`methodReportLevel` 10 is
    /// `threshold` 9); the counts compare the same way in both tools (0).
    pub offset: i64,
    /// PMD's default for the property: a ruleset that enables the rule without setting it gets
    /// this (converted), not Mote's own default.
    pub pmd_default: i64,
}

/// The properties carried over, sorted by rule and property. Defaults and comparisons are those of
/// PMD 7.28.0 (the release the benchmark pins), read from its rule definitions:
/// `CyclomaticComplexity` (`methodReportLevel` 10), `CognitiveComplexity` (`reportLevel` 15) and
/// `ExcessiveParameterList` (`minimum` 10) report a value at or above the level;
/// `TooManyFields` (`maxfields` 15), `TooManyMethods` (`maxmethods` 10) and
/// `UnitTestContainsTooManyAsserts` (`maximumAsserts` 1) a count above it;
/// `AvoidDuplicateLiterals` a literal seen at least `maxDuplicateLiterals` (4) times whose content
/// (quotes excluded) is at least `minimumLength` (3) characters long.
pub const PROPERTIES: &[Property] = &[
    Property {
        rule: "AvoidDuplicateLiterals",
        property: "maxDuplicateLiterals",
        param: "threshold",
        offset: 0,
        pmd_default: 4,
    },
    Property {
        rule: "AvoidDuplicateLiterals",
        property: "minimumLength",
        param: "min_length",
        offset: 0,
        pmd_default: 3,
    },
    Property {
        rule: "CognitiveComplexity",
        property: "reportLevel",
        param: "threshold",
        offset: -1,
        pmd_default: 15,
    },
    Property {
        rule: "CyclomaticComplexity",
        property: "methodReportLevel",
        param: "threshold",
        offset: -1,
        pmd_default: 10,
    },
    Property {
        rule: "ExcessiveParameterList",
        property: "minimum",
        param: "max",
        offset: -1,
        pmd_default: 10,
    },
    Property {
        rule: "TooManyFields",
        property: "maxfields",
        param: "max",
        offset: 0,
        pmd_default: 15,
    },
    Property {
        rule: "TooManyMethods",
        property: "maxmethods",
        param: "max",
        offset: 0,
        pmd_default: 10,
    },
    Property {
        rule: "UnitTestContainsTooManyAsserts",
        property: "maximumAsserts",
        param: "max",
        offset: 0,
        pmd_default: 1,
    },
];

/// The carried-over property `property` of a (current) PMD rule name.
fn parameter(rule: &str, property: &str) -> Option<&'static Property> {
    PROPERTIES
        .iter()
        .find(|p| p.rule == rule && p.property == property)
}

/// The default of the integer parameter `param` of the Mote rule `id`.
fn mote_default(registry: &Registry, id: &str, param: &str) -> Option<i64> {
    let meta = &registry.rule(id)?.meta;
    match meta.params.iter().find(|p| p.name == param)?.default {
        mote_core::ParamDefault::Int(v) => Some(v),
        _ => None,
    }
}

/// The note on a converted level: `methodReportLevel=12 is threshold 11 (reported above it)`.
fn converted(prop: &Property, value: i64) -> Option<String> {
    (prop.offset != 0).then(|| {
        format!(
            "{}={value} is {} {} (reported above it)",
            prop.property,
            prop.param,
            value + prop.offset
        )
    })
}

/// The current name of a PMD rule (older names are renamed).
fn current_name(name: &str) -> &str {
    #[cfg(feature = "java")]
    if let Some((_, new)) = mote_lang_java::pmd::PMD_RENAMED
        .iter()
        .find(|(old, _)| *old == name)
    {
        return new;
    }
    name
}

/// What the file holds.
#[derive(Default)]
struct Ruleset {
    rules: Vec<RuleElem>,
    exclude_patterns: Vec<String>,
    include_patterns: Vec<String>,
}

fn parse(text: &str) -> Result<Ruleset, String> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut set = Ruleset::default();
    let mut stack: Vec<String> = Vec::new();
    let mut current: Option<RuleElem> = None;
    let mut saw_ruleset = false;
    loop {
        let ev = reader
            .read_event()
            .map_err(|e| format!("invalid XML at byte {}: {e}", reader.buffer_position()))?;
        let (e, empty) = match ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(_) => {
                if stack.pop().as_deref() == Some("rule") {
                    if let Some(r) = current.take() {
                        set.rules.push(r);
                    }
                }
                continue;
            }
            Event::Text(t) => {
                let text = t.xml10_content().trim().to_string();
                match stack.last().map(String::as_str) {
                    Some("exclude-pattern") if stack.len() == 2 => set.exclude_patterns.push(text),
                    Some("include-pattern") if stack.len() == 2 => set.include_patterns.push(text),
                    Some("priority") => {
                        if let Some(r) = current.as_mut() {
                            r.priority = text.parse().ok();
                        }
                    }
                    Some("value") => {
                        let last = current.as_mut().and_then(|r| r.properties.last_mut());
                        if let Some((_, v @ None)) = last {
                            *v = Some(text);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            Event::CData(t) => {
                if stack.last().map(String::as_str) == Some("exclude-pattern") && stack.len() == 2 {
                    set.exclude_patterns.push(t.into_inner().trim().to_string());
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = e.local_name().as_ref().to_string();
        match (name.as_str(), stack.len()) {
            ("ruleset", 0) => saw_ruleset = true,
            ("rule", 1) => {
                let r = RuleElem {
                    reference: attr(&e, "ref"),
                    name: attr(&e, "name"),
                    class: attr(&e, "class"),
                    ..Default::default()
                };
                if empty {
                    set.rules.push(r);
                } else {
                    current = Some(r);
                }
            }
            ("exclude", 2) => {
                if let (Some(r), Some(n)) = (current.as_mut(), attr(&e, "name")) {
                    r.excludes.push(n);
                }
            }
            ("property", _) if current.is_some() => {
                if let (Some(r), Some(n)) = (current.as_mut(), attr(&e, "name")) {
                    r.properties.push((n, attr(&e, "value")));
                }
            }
            _ => {}
        }
        if !empty {
            stack.push(name);
        }
    }
    if !saw_ruleset {
        return Err("not a PMD ruleset (no <ruleset> root element)".into());
    }
    Ok(set)
}

fn severity(priority: u32) -> Option<Severity> {
    match priority {
        1 | 2 => Some(Severity::High),
        3 => Some(Severity::Medium),
        4 | 5 => Some(Severity::Low),
        _ => None,
    }
}

/// `.*/generated/.*` → `**/generated/**`, when the regular expression only uses `.*`, `\.` and
/// literal path characters.
pub fn regex_to_glob(re: &str) -> Option<String> {
    let re = re.trim().trim_start_matches('^').trim_end_matches('$');
    let mut out = String::new();
    let mut chars = re.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '.' if chars.peek() == Some(&'*') => {
                chars.next();
                out.push('*');
            }
            '.' if chars.peek() == Some(&'+') => {
                chars.next();
                out.push('*');
            }
            '\\' => match chars.next() {
                Some(n @ ('.' | '/' | '-' | '_' | '$')) => out.push(n),
                _ => return None,
            },
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' => {
                return None
            }
            c => out.push(c),
        }
    }
    if out.is_empty() || out.chars().all(|c| c == '*') {
        return None;
    }
    let segs: Vec<String> = out
        .split('/')
        .map(|s| {
            if s.chars().all(|c| c == '*') {
                "**".to_string()
            } else {
                s.to_string()
            }
        })
        .collect();
    let mut glob = segs.join("/");
    // PMD matches the whole (absolute) path: a pattern without a directory part is a file name
    // anywhere (`.*Test\.java` → `**/*Test.java`).
    if !glob.contains('/') {
        glob = format!("**/{glob}");
    }
    Some(glob.trim_start_matches('/').to_string())
}

/// Read `path` and propose a configuration.
pub fn propose(path: &Path, name: &str, registry: &Registry) -> Result<Proposal, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("cannot read {}: {e}", path.display())))?;
    let set = parse(&text).map_err(|e| CliError::usage(format!("{}: {e}", path.display())))?;
    let mut p = Proposal::new(format!("pmd {name}"));
    p.compat = &["pmd"];
    p.compat_note =
        "PMD also honors `@SuppressWarnings(\"unused\")`: the intellij source covers it";
    p.unmapped_note = "the mapping tables cover PMD's quickstart ruleset and the other PMD rules \
         with a clear equivalent; for a rule they miss, `mote rules --all` may still list a \
         Mote rule for the same issue";
    #[cfg(feature = "java")]
    let map = |n: &str| mote_lang_java::pmd::mote_rule_for_pmd(n);
    #[cfg(not(feature = "java"))]
    let map = |_: &str| -> Option<&'static str> { None };
    let known = |id: &str| registry.rule(id).is_some();

    for r in &set.rules {
        let Some(reference) = r.reference.as_deref() else {
            p.seen += 1;
            p.unmapped(format!(
                "custom rule {} ({}): write it as a Mote rule or drop it",
                r.name.as_deref().unwrap_or("?"),
                r.class.as_deref().unwrap_or("no class")
            ));
            continue;
        };
        let java =
            reference.starts_with("category/java/") || reference.starts_with("rulesets/java/");
        let (file, rule) = match reference.rsplit_once(".xml/") {
            Some((f, n)) => (format!("{f}.xml"), Some(n)),
            None => (reference.to_string(), None),
        };
        match rule {
            Some(rule_name) => {
                p.seen += 1;
                let id = map(rule_name).filter(|id| java && known(id));
                match id {
                    Some(id) => {
                        p.mapped += 1;
                        let level = r
                            .priority
                            .and_then(severity)
                            .map(Level::Severity)
                            .unwrap_or(Level::On);
                        let from = match r.priority {
                            Some(pr) => format!("{rule_name}, priority {pr}"),
                            None => rule_name.to_string(),
                        };
                        p.rules.set(id, level, from);
                    }
                    None if !java => {
                        p.unmapped(format!("{reference}: not a Java rule"));
                    }
                    None => p.unmapped(format!(
                        "{rule_name} ({reference}): no equivalent in the mapping table"
                    )),
                }
                let mut left = Vec::new();
                let current = current_name(rule_name);
                for (name, value) in &r.properties {
                    let carried = id
                        .zip(parameter(current, name))
                        .zip(value.as_deref().and_then(|v| v.trim().parse::<i64>().ok()));
                    match carried {
                        Some(((id, prop), v)) if v + prop.offset >= 0 => {
                            p.rules.param(id, prop.param, v + prop.offset);
                            if let Some(note) = converted(prop, v) {
                                p.rules.note(id, note);
                            }
                        }
                        _ => left.push(match value {
                            Some(v) => format!("{name}={v}"),
                            None => name.clone(),
                        }),
                    }
                }
                // A threshold the ruleset leaves at PMD's default keeps PMD's behavior, not
                // Mote's own default.
                if let Some(id) = id {
                    for prop in PROPERTIES.iter().filter(|x| x.rule == current) {
                        let set = p
                            .rules
                            .0
                            .get(id)
                            .is_some_and(|s| s.params.contains_key(prop.param));
                        if set || r.properties.iter().any(|(n, _)| n == prop.property) {
                            continue;
                        }
                        let value = prop.pmd_default + prop.offset;
                        if mote_default(registry, id, prop.param) == Some(value) {
                            continue;
                        }
                        p.rules.param(id, prop.param, value);
                        p.rules.note(
                            id,
                            match converted(prop, prop.pmd_default) {
                                Some(c) => format!("PMD's default {c}"),
                                None => {
                                    format!("PMD's default {}={}", prop.property, prop.pmd_default)
                                }
                            },
                        );
                    }
                }
                if !left.is_empty() {
                    p.unmapped(format!(
                        "properties of {rule_name} not carried over: {}",
                        left.join(", ")
                    ));
                }
            }
            None => {
                let what = if file.ends_with(".xml")
                    && (file.starts_with("category/") || file.starts_with("rulesets/"))
                {
                    format!("{file}: a whole category or ruleset is not expanded; enable the Mote rules you want from `mote rules --all`")
                } else {
                    format!("{reference}: another ruleset file is not read; migrate it separately")
                };
                p.unmapped(what);
                for ex in &r.excludes {
                    p.seen += 1;
                    match map(ex).filter(|id| java && known(id)) {
                        Some(id) => {
                            p.mapped += 1;
                            p.rules
                                .set(id, Level::Off, format!("{ex} excluded from {file}"));
                        }
                        None => p.unmapped(format!(
                            "{ex} (excluded from {file}): no equivalent in the mapping table to turn off"
                        )),
                    }
                }
            }
        }
    }
    for re in &set.exclude_patterns {
        match regex_to_glob(re) {
            Some(g) => {
                if !p.exclude.iter().any(|(x, _)| x == &g) {
                    p.exclude.push((g, format!("<exclude-pattern>{re}</exclude-pattern>")));
                }
            }
            None => p.unmapped(format!(
                "<exclude-pattern>{re}</exclude-pattern>: not translatable to a glob; add an `exclude` glob by hand"
            )),
        }
    }
    for re in &set.include_patterns {
        p.unmapped(format!(
            "<include-pattern>{re}</include-pattern>: exceptions to excludes are not expressible; narrow the `exclude` globs instead"
        ));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regexes_to_globs() {
        let cases = [
            (".*/generated/.*", Some("**/generated/**")),
            (".*/src/test/.*", Some("**/src/test/**")),
            (".*Test\\.java", Some("**/*Test.java")),
            ("^.*/target/.*$", Some("**/target/**")),
            (".*/gen-src/.+", Some("**/gen-src/**")),
            (".*/(a|b)/.*", None),
            (".*[Tt]est.*", None),
            (".*", None),
        ];
        for (re, want) in cases {
            assert_eq!(regex_to_glob(re).as_deref(), want, "{re}");
        }
    }

    #[test]
    fn parses_rulesets() {
        let xml = r#"<?xml version="1.0"?>
<ruleset name="team" xmlns="http://pmd.sourceforge.net/ruleset/2.0.0">
  <description>Team rules</description>
  <exclude-pattern>.*/generated/.*</exclude-pattern>
  <include-pattern>.*/generated/keep/.*</include-pattern>
  <rule ref="category/java/errorprone.xml/EmptyCatchBlock"/>
  <rule ref="category/java/design.xml/CyclomaticComplexity">
    <priority>2</priority>
    <properties><property name="methodReportLevel" value="12"/></properties>
  </rule>
  <rule ref="category/java/bestpractices.xml">
    <exclude name="UnusedPrivateField"/>
    <exclude name="JUnitTestContainsTooManyAsserts"/>
  </rule>
  <rule name="NoFoo" class="net.sourceforge.pmd.lang.rule.xpath.XPathRule" message="x"/>
</ruleset>"#;
        let s = parse(xml).unwrap();
        assert_eq!(s.rules.len(), 4);
        assert_eq!(s.exclude_patterns, vec![".*/generated/.*".to_string()]);
        assert_eq!(s.rules[1].priority, Some(2));
        assert_eq!(
            s.rules[1].properties,
            vec![("methodReportLevel".to_string(), Some("12".to_string()))]
        );
        assert_eq!(s.rules[2].excludes.len(), 2);
        assert!(s.rules[3].reference.is_none());
        assert!(parse("<project/>").is_err());
        assert!(parse("<ruleset><rule").is_err());
    }

    /// PMD's defaults fill in thresholds a ruleset leaves unset, never one it sets.
    #[cfg(feature = "java")]
    #[test]
    fn unset_thresholds_get_pmds_defaults() {
        let registry = crate::plugins::registry().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ruleset.xml");
        let xml = r#"<ruleset name="t">
  <rule ref="category/java/design.xml/CyclomaticComplexity">
    <properties><property name="methodReportLevel" value="15"/></properties>
  </rule>
  <rule ref="category/java/design.xml/CyclomaticComplexity"/>
  <rule ref="category/java/design.xml/TooManyFields"/>
  <rule ref="category/java/errorprone.xml/AvoidDuplicateLiterals">
    <properties><property name="minimumLength" value="5"/></properties>
  </rule>
</ruleset>"#;
        std::fs::write(&path, xml).unwrap();
        let p = propose(&path, "ruleset.xml", &registry).unwrap();
        let params = |id: &str| p.rules.0[id].params.clone();
        assert_eq!(params("java/cyclomatic-complexity")["threshold"], 14);
        assert_eq!(params("java/too-many-fields")["max"], 15);
        let dup = params("java/duplicate-string-literal");
        // The `minimumLength` set carries over; `maxDuplicateLiterals`, unset, is PMD's 4.
        assert_eq!(dup.get("min_length"), Some(&5));
        assert_eq!(dup.get("threshold"), Some(&4));
        assert!(p.rules.0["java/too-many-fields"]
            .from
            .contains(&"PMD's default maxfields=15".to_string()));
    }
}
