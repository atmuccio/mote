//! SARIF 2.1.0 (one run; `tool.driver.rules` carries the rule catalog of the scan).
//!
//! Artifact URIs are relative to `%SRCROOT%`, the repository top level: when the scan root is a
//! subdirectory, [`SarifOptions::path_prefix`] is prepended (GitHub code scanning resolves URIs
//! from the repository root). `run.originalUriBaseIds` gives `%SRCROOT%` as an absolute `file:`
//! URI when the caller knows it. At most [`SarifOptions::max_results`] results are written (the
//! most severe; GitHub rejects runs over 25,000), the rest noted in `run.properties`.
//! `invocations[0].executionSuccessful` is false when some file could not be analyzed.

use crate::{failed_files, repo_path, INFORMATION_URI, VERSION};
use mote_core::{Finding, FindingStatus, Location, Quality, Severity, TextRange};
use mote_engine::result::DiagnosticLevel;
use mote_engine::triage::TriageStatus;
use mote_engine::{RuleInfo, ScanResult};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};

/// GitHub code scanning's limit of results per run.
pub const MAX_RESULTS: usize = 25_000;

/// SARIF options.
#[derive(Clone, Copy, Debug)]
pub struct SarifOptions<'a> {
    /// Lowest severity written.
    pub min_severity: Severity,
    /// Repository-relative path of the scan root (`backend/`), prepended to every artifact URI;
    /// empty when the scan root is the repository top level.
    pub path_prefix: &'a str,
    /// `%SRCROOT%` as an absolute `file:` URI ending in `/` (see [`directory_uri`]); written to
    /// `run.originalUriBaseIds` when known.
    pub srcroot_uri: Option<&'a str>,
    /// Most results written; the most severe are kept.
    pub max_results: usize,
}

impl Default for SarifOptions<'_> {
    fn default() -> Self {
        SarifOptions {
            min_severity: Severity::Low,
            path_prefix: "",
            srcroot_uri: None,
            max_results: MAX_RESULTS,
        }
    }
}

pub const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

#[derive(Serialize)]
pub struct Log {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub version: &'static str,
    pub runs: Vec<Run>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub tool: Tool,
    pub automation_details: AutomationDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_uri_base_ids: Option<BTreeMap<&'static str, BaseUri>>,
    pub column_kind: &'static str,
    pub results: Vec<SarifResult>,
    pub invocations: Vec<Invocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<RunProperties>,
}

/// An `originalUriBaseIds` entry.
#[derive(Serialize)]
pub struct BaseUri {
    pub uri: String,
}

/// `run.properties`, written when results were left out.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProperties {
    /// Results matching the options.
    pub total_results: usize,
    /// Results left out (the least severe) to stay within the limit.
    pub omitted_results: usize,
    pub note: String,
}

#[derive(Serialize)]
pub struct Tool {
    pub driver: Driver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    pub name: &'static str,
    pub version: &'static str,
    pub semantic_version: &'static str,
    pub information_uri: &'static str,
    pub rules: Vec<Descriptor>,
}

#[derive(Serialize)]
pub struct AutomationDetails {
    pub id: &'static str,
}

#[derive(Serialize)]
pub struct Message {
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub id: String,
    pub name: String,
    pub short_description: Message,
    pub full_description: Message,
    pub help: Help,
    pub default_configuration: DefaultConfiguration,
    pub properties: RuleProperties,
}

#[derive(Serialize)]
pub struct Help {
    pub text: String,
    pub markdown: String,
}

#[derive(Serialize)]
pub struct DefaultConfiguration {
    pub level: &'static str,
}

#[derive(Serialize)]
pub struct RuleProperties {
    pub tags: Vec<String>,
    pub quality: &'static str,
    pub severity: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    pub rule_index: usize,
    pub level: &'static str,
    pub message: Message,
    pub locations: Vec<SarifLocation>,
    pub partial_fingerprints: PartialFingerprints,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_state: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressions: Vec<Suppression>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related_locations: Vec<SarifLocation>,
    /// The finding's fix (`mote fix`), when its rule offers one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<SarifFix>,
    pub properties: ResultProperties,
}

/// A proposed fix (SARIF `fix`): one artifact change, the finding's file.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifFix {
    pub description: Message,
    pub artifact_changes: Vec<ArtifactChange>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactChange {
    pub artifact_location: ArtifactLocation,
    pub replacements: Vec<Replacement>,
}

/// Replace `deleted_region` (empty for an insertion) with `inserted_content` (absent for a
/// deletion).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Replacement {
    pub deleted_region: EditRegion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inserted_content: Option<Message>,
}

/// A region with every bound explicit: an omitted end would stretch it to the end of the line.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditRegion {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    pub physical_location: PhysicalLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalLocation {
    pub artifact_location: ArtifactLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLocation {
    pub uri: String,
    pub uri_base_id: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialFingerprints {
    pub primary_location_line_hash: String,
}

#[derive(Serialize)]
pub struct Suppression {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

#[derive(Serialize)]
pub struct ResultProperties {
    pub id: String,
    pub quality: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    pub execution_successful: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_execution_notifications: Vec<Notification>,
}

#[derive(Serialize)]
pub struct Notification {
    pub level: &'static str,
    pub message: Message,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<SarifLocation>,
}

/// SARIF `level` for a severity.
pub fn level(s: Severity) -> &'static str {
    match s {
        Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low => "note",
    }
}

fn baseline_state(s: FindingStatus) -> Option<&'static str> {
    match s {
        FindingStatus::Unknown => None,
        FindingStatus::New => Some("new"),
        FindingStatus::Unchanged => Some("unchanged"),
        FindingStatus::Worsened => Some("updated"),
    }
}

/// An absolute directory as a `file:` URI with a trailing `/`, for `originalUriBaseIds`:
/// `F:\repo` → `file:///F:/repo/`, `/home/me/repo` → `file:///home/me/repo/`. Windows verbatim
/// prefixes (`\\?\`) are dropped; other characters are percent-encoded like [`encode_uri`].
pub fn directory_uri(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = if let Some(unc) = p.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else {
        p.strip_prefix("//?/").map(str::to_string).unwrap_or(p)
    };
    let mut out = String::from("file://");
    if let Some(unc) = p.strip_prefix("//") {
        // file://server/share/...
        out.push_str(&encode_uri(unc));
    } else {
        let bytes = p.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            out.push('/');
            out.push_str(&p[..2]);
            out.push_str(&encode_uri(&p[2..]));
        } else {
            if !p.starts_with('/') {
                out.push('/');
            }
            out.push_str(&encode_uri(&p));
        }
    }
    if !out.ends_with('/') {
        out.push('/');
    }
    out
}

/// Percent-encode a repo-relative path as a URI reference (keeps `/` and unreserved characters).
pub fn encode_uri(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `empty-catch-block` → `EmptyCatchBlock`.
fn pascal_name(id: &str) -> String {
    let name = id.rsplit('/').next().unwrap_or(id);
    name.split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

fn first_paragraph(md: &str) -> String {
    md.split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty())
        .unwrap_or("")
        .replace('\n', " ")
}

fn descriptor(r: &RuleInfo) -> Descriptor {
    let mut markdown = if r.description.trim().is_empty() {
        r.name.clone()
    } else {
        r.description.trim_end().to_string()
    };
    if !r.params.is_empty() {
        markdown.push_str("\n\n**Parameters**\n");
        for p in &r.params {
            markdown.push_str(&format!(
                "\n- `{}` (default `{}`): {}",
                p.name,
                crate::rules::param_value(&p.default),
                p.description
            ));
        }
    }
    let full = first_paragraph(&r.description);
    let mut tags = r.tags.clone();
    tags.push(r.quality.as_str().to_string());
    tags.sort();
    tags.dedup();
    Descriptor {
        id: r.id.clone(),
        name: pascal_name(&r.id),
        short_description: Message {
            text: r.name.clone(),
        },
        full_description: Message {
            text: if full.is_empty() {
                r.name.clone()
            } else {
                full
            },
        },
        help: Help {
            text: markdown.clone(),
            markdown,
        },
        default_configuration: DefaultConfiguration {
            level: level(r.severity),
        },
        properties: RuleProperties {
            tags,
            quality: r.quality.as_str(),
            severity: r.severity.as_str(),
        },
    }
}

fn region(r: &TextRange) -> Option<Region> {
    if r.start_line == 0 {
        return None;
    }
    let start_column = (r.start_col >= 1).then_some(r.start_col);
    let (end_line, end_column) = if r.end_line > r.start_line {
        (Some(r.end_line), (r.end_col >= 1).then_some(r.end_col))
    } else if r.end_line == r.start_line && r.end_col > r.start_col && start_column.is_some() {
        (Some(r.end_line), Some(r.end_col))
    } else {
        // Empty or inverted range: point at the start only.
        (None, None)
    };
    Some(Region {
        start_line: r.start_line,
        start_column,
        end_line,
        end_column,
    })
}

fn physical(prefix: &str, path: &str, range: Option<&TextRange>) -> PhysicalLocation {
    PhysicalLocation {
        artifact_location: ArtifactLocation {
            uri: encode_uri(&repo_path(prefix, path)),
            uri_base_id: "%SRCROOT%",
        },
        region: range.and_then(region),
    }
}

fn related(prefix: &str, i: usize, l: &Location) -> SarifLocation {
    SarifLocation {
        id: Some(i + 1),
        physical_location: physical(prefix, &l.path, Some(&l.range)),
        message: l.message.clone().map(|text| Message { text }),
    }
}

fn result(f: &Finding, rule_index: usize, prefix: &str) -> SarifResult {
    SarifResult {
        rule_id: f.rule_id.clone(),
        rule_index,
        level: level(f.severity),
        message: Message {
            text: f.message.clone(),
        },
        locations: vec![SarifLocation {
            id: None,
            physical_location: physical(prefix, &f.path, Some(&f.range)),
            message: None,
        }],
        partial_fingerprints: PartialFingerprints {
            primary_location_line_hash: f.fingerprint.clone(),
        },
        baseline_state: baseline_state(f.status),
        suppressions: f
            .suppressed
            .as_ref()
            .map(|s| {
                vec![Suppression {
                    // Triage decisions live in a file next to the code, not in it.
                    kind: if TriageStatus::is_triage_kind(&s.kind) {
                        "external"
                    } else {
                        "inSource"
                    },
                    justification: s.justification.clone(),
                }]
            })
            .unwrap_or_default(),
        related_locations: f
            .secondary
            .iter()
            .enumerate()
            .map(|(i, l)| related(prefix, i, l))
            .collect(),
        fixes: f
            .fix
            .iter()
            .filter(|_| f.suppressed.is_none())
            .map(|fix| SarifFix {
                description: Message {
                    text: fix.description.clone(),
                },
                artifact_changes: vec![ArtifactChange {
                    artifact_location: ArtifactLocation {
                        uri: encode_uri(&repo_path(prefix, &f.path)),
                        uri_base_id: "%SRCROOT%",
                    },
                    replacements: fix
                        .edits
                        .iter()
                        .map(|e| Replacement {
                            deleted_region: EditRegion {
                                start_line: e.range.start_line,
                                start_column: e.range.start_col,
                                end_line: e.range.end_line,
                                end_column: e.range.end_col,
                            },
                            inserted_content: (!e.replacement.is_empty()).then(|| Message {
                                text: e.replacement.clone(),
                            }),
                        })
                        .collect(),
                }],
            })
            .collect(),
        properties: ResultProperties {
            id: f.id.clone(),
            quality: f.quality.as_str(),
            symbol: f.symbol.clone(),
        },
    }
}

/// The findings written as results: at least `min_severity`, at most `max_results` of them (the
/// most severe, ties in result order), in result order. Also returns how many matched.
fn selected<'a>(scan: &'a ScanResult, opts: &SarifOptions) -> (Vec<&'a Finding>, usize) {
    let mut list: Vec<(usize, &Finding)> = scan
        .findings
        .iter()
        .filter(|f| f.severity >= opts.min_severity)
        .enumerate()
        .collect();
    let total = list.len();
    if total > opts.max_results {
        list.sort_by(|a, b| b.1.severity.cmp(&a.1.severity).then(a.0.cmp(&b.0)));
        list.truncate(opts.max_results);
        list.sort_by_key(|&(i, _)| i);
    }
    (list.into_iter().map(|(_, f)| f).collect(), total)
}

/// How many findings [`build`] leaves out to stay within `max_results`.
pub fn omitted_results(scan: &ScanResult, opts: &SarifOptions) -> usize {
    let total = scan
        .findings
        .iter()
        .filter(|f| f.severity >= opts.min_severity)
        .count();
    total.saturating_sub(opts.max_results)
}

/// Build the SARIF log.
pub fn build(scan: &ScanResult, opts: &SarifOptions) -> Log {
    let prefix = opts.path_prefix;
    let mut rules: Vec<RuleInfo> = scan.rules.clone();
    let mut index: HashMap<String, usize> = HashMap::with_capacity(rules.len());
    for (i, r) in rules.iter().enumerate() {
        index.entry(r.id.clone()).or_insert(i);
    }
    // Defensive: a finding whose rule is missing from the catalog still gets a descriptor.
    let mut missing: Vec<&Finding> = scan
        .findings
        .iter()
        .filter(|f| !index.contains_key(&f.rule_id))
        .collect();
    missing.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
    missing.dedup_by(|a, b| a.rule_id == b.rule_id);
    for f in missing {
        index.insert(f.rule_id.clone(), rules.len());
        rules.push(RuleInfo {
            id: f.rule_id.clone(),
            name: f.rule_id.clone(),
            language: f.rule_id.split('/').next().unwrap_or("").to_string(),
            kind: if f.quality == Quality::Reliability {
                mote_core::RuleKind::Bug
            } else {
                mote_core::RuleKind::CodeSmell
            },
            quality: f.quality,
            severity: f.severity,
            status: mote_core::RuleStatus::Stable,
            default_enabled: true,
            tags: vec![],
            params: vec![],
            description: String::new(),
            remediation_minutes: 0,
            origin: mote_engine::registry::RuleOrigin::Plugin,
            fixable: false,
        });
    }
    let (kept, total) = selected(scan, opts);
    let omitted = total - kept.len();
    let results = kept
        .into_iter()
        .map(|f| result(f, index[&f.rule_id], prefix))
        .collect();
    let notifications = scan
        .diagnostics
        .iter()
        .map(|d| Notification {
            level: match d.level {
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Error => "error",
            },
            message: Message {
                text: d.message.clone(),
            },
            locations: d
                .path
                .as_deref()
                .map(|p| {
                    vec![SarifLocation {
                        id: None,
                        physical_location: physical(prefix, p, None),
                        message: None,
                    }]
                })
                .unwrap_or_default(),
        })
        .collect();
    Log {
        schema: SCHEMA,
        version: "2.1.0",
        runs: vec![Run {
            tool: Tool {
                driver: Driver {
                    name: "Mote",
                    version: VERSION,
                    semantic_version: VERSION,
                    information_uri: INFORMATION_URI,
                    rules: rules.iter().map(descriptor).collect(),
                },
            },
            automation_details: AutomationDetails { id: "mote/" },
            original_uri_base_ids: opts.srcroot_uri.map(|uri| {
                BTreeMap::from([(
                    "%SRCROOT%",
                    BaseUri {
                        uri: uri.to_string(),
                    },
                )])
            }),
            column_kind: "unicodeCodePoints",
            results,
            invocations: vec![Invocation {
                execution_successful: failed_files(scan) == 0,
                tool_execution_notifications: notifications,
            }],
            properties: (omitted > 0).then(|| RunProperties {
                total_results: total,
                omitted_results: omitted,
                note: format!(
                    "{omitted} of {total} results left out (the least severe) to stay within {} results per run",
                    opts.max_results
                ),
            }),
        }],
    }
}

/// Write SARIF (pretty-printed JSON).
pub fn write(scan: &ScanResult, opts: &SarifOptions, w: &mut dyn Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, &build(scan, opts))?;
    writeln!(w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, with_base, without_base};
    use serde_json::Value;

    fn render(r: &ScanResult, opts: SarifOptions) -> String {
        let mut out = Vec::new();
        write(r, &opts, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn obj<'a>(v: &'a Value, key: &str) -> &'a Value {
        v.get(key)
            .unwrap_or_else(|| panic!("missing required `{key}` in {v}"))
    }

    fn check_message(m: &Value) {
        assert!(obj(m, "text").is_string(), "message.text must be a string");
    }

    fn check_physical(p: &Value) {
        let a = obj(p, "artifactLocation");
        let uri = obj(a, "uri").as_str().unwrap();
        assert!(!uri.contains(' ') && !uri.contains('\\') && !uri.starts_with('/'));
        assert_eq!(obj(a, "uriBaseId"), "%SRCROOT%");
        if let Some(r) = p.get("region") {
            let sl = obj(r, "startLine").as_u64().unwrap();
            assert!(sl >= 1);
            if let Some(el) = r.get("endLine") {
                assert!(el.as_u64().unwrap() >= sl);
            }
            if let Some(c) = r.get("startColumn") {
                assert!(c.as_u64().unwrap() >= 1);
            }
            if let (Some(sc), Some(ec)) = (r.get("startColumn"), r.get("endColumn")) {
                if r.get("endLine").and_then(Value::as_u64) == Some(sl) {
                    assert!(ec.as_u64() > sc.as_u64(), "empty region {r}");
                }
            }
        }
    }

    /// The essentials of the SARIF 2.1.0 schema that consumers (GitHub code scanning) enforce.
    fn validate(log: &Value) {
        assert_eq!(obj(log, "version"), "2.1.0");
        assert!(obj(log, "$schema")
            .as_str()
            .unwrap()
            .contains("sarif-2.1.0"));
        let runs = obj(log, "runs").as_array().unwrap();
        assert_eq!(runs.len(), 1, "exactly one run");
        let run = &runs[0];
        let driver = obj(obj(run, "tool"), "driver");
        assert_eq!(obj(driver, "name"), "Mote");
        assert!(obj(driver, "version").is_string());
        let rules = obj(driver, "rules").as_array().unwrap();
        let mut ids = std::collections::HashSet::new();
        for r in rules {
            let id = obj(r, "id").as_str().unwrap();
            assert!(ids.insert(id.to_string()), "duplicate rule id {id}");
            check_message(obj(r, "shortDescription"));
            check_message(obj(r, "fullDescription"));
            let help = obj(r, "help");
            assert!(obj(help, "text").is_string() && obj(help, "markdown").is_string());
            let lvl = obj(obj(r, "defaultConfiguration"), "level")
                .as_str()
                .unwrap();
            assert!(["none", "note", "warning", "error"].contains(&lvl));
            let props = obj(r, "properties");
            assert!(obj(props, "tags").is_array());
        }
        assert_eq!(obj(obj(run, "automationDetails"), "id"), "mote/");
        assert_eq!(obj(run, "columnKind"), "unicodeCodePoints");
        for res in obj(run, "results").as_array().unwrap() {
            let rule_id = obj(res, "ruleId").as_str().unwrap();
            let idx = obj(res, "ruleIndex").as_u64().unwrap() as usize;
            assert_eq!(
                obj(&rules[idx], "id"),
                rule_id,
                "ruleIndex points at ruleId"
            );
            let lvl = obj(res, "level").as_str().unwrap();
            assert!(["none", "note", "warning", "error"].contains(&lvl));
            check_message(obj(res, "message"));
            let locs = obj(res, "locations").as_array().unwrap();
            assert_eq!(locs.len(), 1);
            check_physical(obj(&locs[0], "physicalLocation"));
            assert!(obj(obj(res, "partialFingerprints"), "primaryLocationLineHash").is_string());
            if let Some(b) = res.get("baselineState") {
                assert!(["new", "unchanged", "updated", "absent"].contains(&b.as_str().unwrap()));
            }
            if let Some(s) = res.get("suppressions") {
                for s in s.as_array().unwrap() {
                    assert!(["inSource", "external"].contains(&obj(s, "kind").as_str().unwrap()));
                }
            }
            if let Some(rel) = res.get("relatedLocations") {
                for l in rel.as_array().unwrap() {
                    assert!(obj(l, "id").as_u64().is_some());
                    check_physical(obj(l, "physicalLocation"));
                }
            }
        }
        let inv = obj(run, "invocations").as_array().unwrap();
        assert!(obj(&inv[0], "executionSuccessful").is_boolean());
    }

    #[test]
    fn snapshot_and_schema() {
        let text = render(&with_base(), SarifOptions::default());
        assert_snapshot("results.sarif", &text);
        let v: Value = serde_json::from_str(&text).unwrap();
        validate(&v);
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 6, "suppressed findings are kept");
        let by_id = |id: &str| {
            results
                .iter()
                .find(|r| r["properties"]["id"] == id)
                .unwrap()
                .clone()
        };
        assert_eq!(by_id("new000000001")["level"], "error");
        assert_eq!(by_id("new000000001")["baselineState"], "new");
        assert_eq!(by_id("wrs000000001")["baselineState"], "updated");
        assert_eq!(by_id("unc000000001")["baselineState"], "unchanged");
        assert_eq!(by_id("new000000002")["level"], "note");
        let sup = by_id("sup000000001");
        assert_eq!(sup["suppressions"][0]["kind"], "inSource");
        assert_eq!(sup["suppressions"][0]["justification"], "tracked in JIRA-1");
        // A triage decision is an external suppression.
        let mut triaged = with_base();
        for f in &mut triaged.findings {
            if let Some(s) = f.suppressed.as_mut() {
                s.kind = "false-positive".into();
            }
        }
        let v: Value = serde_json::from_str(&render(&triaged, SarifOptions::default())).unwrap();
        validate(&v);
        let ext = v["runs"][0]["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["properties"]["id"] == "sup000000001")
            .unwrap()
            .clone();
        assert_eq!(ext["suppressions"][0]["kind"], "external");
        let hi = by_id("new000000001");
        assert_eq!(
            hi["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/a/Order%20Service.java"
        );
        assert_eq!(
            hi["partialFingerprints"]["primaryLocationLineHash"],
            "fp-new000000001"
        );
        // empty single-line range (1:1-1:1): no end
        let low = by_id("new000000002");
        let region = &low["locations"][0]["physicalLocation"]["region"];
        assert!(region.get("endColumn").is_none(), "{region}");
        assert_eq!(by_id("dup000000001")["relatedLocations"][0]["id"], 1);
    }

    #[test]
    fn min_severity_and_no_base() {
        let v: Value = serde_json::from_str(&render(
            &without_base(),
            SarifOptions {
                min_severity: Severity::Medium,
                ..Default::default()
            },
        ))
        .unwrap();
        validate(&v);
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|r| r.get("baselineState").is_none()));
        assert!(results.iter().all(|r| r["level"] != "note"));
        let v: Value = serde_json::from_str(&render(&empty(), SarifOptions::default())).unwrap();
        validate(&v);
    }

    #[test]
    fn unknown_rule_gets_descriptor() {
        let mut r = with_base();
        r.rules.clear();
        let v: Value = serde_json::from_str(&render(&r, SarifOptions::default())).unwrap();
        validate(&v);
    }

    #[test]
    fn helpers() {
        assert_eq!(pascal_name("java/empty-catch-block"), "EmptyCatchBlock");
        assert_eq!(encode_uri("a b/ü%.js"), "a%20b/%C3%BC%25.js");
        assert_eq!(
            directory_uri(r"D:\work\My Repo"),
            "file:///D:/work/My%20Repo/"
        );
        assert_eq!(directory_uri(r"\\?\F:\repo\"), "file:///F:/repo/");
        assert_eq!(
            directory_uri(r"\\?\UNC\server\share\r"),
            "file://server/share/r/"
        );
        assert_eq!(directory_uri("/home/me/repo"), "file:///home/me/repo/");
        assert_eq!(directory_uri("C:/x/"), "file:///C:/x/");
    }

    /// A scan of a subdirectory (`mote scan backend`): every URI is repository-relative and
    /// `%SRCROOT%` is the repository top level.
    #[test]
    fn subdirectory_scan_uses_repository_relative_uris() {
        let opts = SarifOptions {
            path_prefix: "backend/",
            srcroot_uri: Some("file:///F:/repo/"),
            ..Default::default()
        };
        let v: Value = serde_json::from_str(&render(&with_base(), opts)).unwrap();
        validate(&v);
        let run = &v["runs"][0];
        assert_eq!(
            run["originalUriBaseIds"]["%SRCROOT%"]["uri"],
            "file:///F:/repo/"
        );
        let mut uris = Vec::new();
        for r in run["results"].as_array().unwrap() {
            uris.push(r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"].clone());
            for l in r["relatedLocations"].as_array().into_iter().flatten() {
                uris.push(l["physicalLocation"]["artifactLocation"]["uri"].clone());
            }
        }
        for n in run["invocations"][0]["toolExecutionNotifications"]
            .as_array()
            .unwrap()
        {
            for l in n["locations"].as_array().into_iter().flatten() {
                uris.push(l["physicalLocation"]["artifactLocation"]["uri"].clone());
            }
        }
        assert!(uris.len() > 6, "{uris:?}");
        assert!(
            uris.iter()
                .all(|u| u.as_str().unwrap().starts_with("backend/src/")
                    || u.as_str().unwrap() == "backend/coverage/lcov.info"),
            "{uris:?}"
        );
        assert!(uris.contains(&Value::from("backend/src/a/Order%20Service.java")));
        // Unknown top level: no originalUriBaseIds at all (never an entry without a uri).
        let v: Value =
            serde_json::from_str(&render(&with_base(), SarifOptions::default())).unwrap();
        assert!(v["runs"][0].get("originalUriBaseIds").is_none());
    }

    #[test]
    fn results_are_capped_keeping_the_most_severe() {
        let opts = SarifOptions {
            max_results: 3,
            ..Default::default()
        };
        let r = with_base();
        assert_eq!(omitted_results(&r, &opts), 3);
        assert_eq!(omitted_results(&r, &SarifOptions::default()), 0);
        let v: Value = serde_json::from_str(&render(&r, opts)).unwrap();
        validate(&v);
        let run = &v["runs"][0];
        let ids: Vec<&str> = run["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["properties"]["id"].as_str().unwrap())
            .collect();
        // HIGH first, then the MEDIUM ones in result order; written in result order.
        assert_eq!(ids, ["new000000001", "wrs000000001", "dup000000001"]);
        assert_eq!(run["properties"]["totalResults"], 6);
        assert_eq!(run["properties"]["omittedResults"], 3);
        assert!(run["properties"]["note"]
            .as_str()
            .unwrap()
            .contains("3 of 6 results left out"));
        let v: Value = serde_json::from_str(&render(&r, SarifOptions::default())).unwrap();
        assert!(v["runs"][0].get("properties").is_none());
    }

    #[test]
    fn execution_fails_when_files_could_not_be_analyzed() {
        let v: Value =
            serde_json::from_str(&render(&with_base(), SarifOptions::default())).unwrap();
        assert_eq!(v["runs"][0]["invocations"][0]["executionSuccessful"], true);
        let mut r = with_base();
        let mut f = r.findings[0].clone();
        f.rule_id = crate::INTERNAL_ERROR_RULE.into();
        r.findings.push(f);
        let v: Value = serde_json::from_str(&render(&r, SarifOptions::default())).unwrap();
        validate(&v);
        assert_eq!(v["runs"][0]["invocations"][0]["executionSuccessful"], false);
    }
}
