//! MCP tools: scan, summary, work_units, findings, rule, verify, explain_config.
//!
//! The server keeps one [`Scanner`] per repository root (rebuilt when the config file changes)
//! and the results of the last [`MAX_SCANS`] scans, addressed by `scan_id`. The analysis cache
//! (a locked database file) is only open during a tool call: every scanner closes it when the call
//! ends, so an idle server never keeps CLI scans (or another server) from using the cache, and a
//! scanner rebuilt after a config edit can open it.

use super::{INTERNAL_ERROR, INVALID_PARAMS};
use mote_core::{Finding, FindingStatus, Severity};
use mote_engine::config::CONFIG_FILE_NAMES;
use mote_engine::{Config, Registry, ScanOptions, ScanResult, Scanner, SourceSpec};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Scans kept in memory (oldest evicted first).
pub const MAX_SCANS: usize = 8;
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;
const DEFAULT_UNIT_SIZE: usize = 25;

/// A group of files that must stay together in one work unit: (key, files, findings).
type Atom = (String, Vec<String>, usize);

/// A protocol-level failure: (JSON-RPC code, message).
type RpcError = (i64, String);

/// Outcome of a tool: structured content, or a tool error shown to the model.
enum ToolError {
    /// Arguments don't match the input schema → JSON-RPC -32602.
    InvalidParams(String),
    /// Well-formed call that failed → `isError: true`.
    Failed(String),
}

type ToolResult = Result<Value, ToolError>;

fn failed(msg: impl Into<String>) -> ToolError {
    ToolError::Failed(msg.into())
}

fn invalid(msg: impl Into<String>) -> ToolError {
    ToolError::InvalidParams(msg.into())
}

struct ScannerSlot {
    config_key: Option<String>,
    scanner: Arc<Scanner>,
}

/// Which findings a work unit (or a query) is about.
#[derive(Clone, Debug, Default)]
struct Selection {
    min_severity: Option<Severity>,
    rule: Option<String>,
    new_only: bool,
}

impl Selection {
    fn accepts(&self, f: &Finding, has_base: bool) -> bool {
        f.suppressed.is_none()
            && self.min_severity.is_none_or(|m| f.severity >= m)
            && self
                .rule
                .as_deref()
                .is_none_or(|r| rule_matches(r, &f.rule_id))
            && (!self.new_only
                || !has_base
                || matches!(f.status, FindingStatus::New | FindingStatus::Worsened))
    }
}

fn rule_matches(pattern: &str, id: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => id.starts_with(prefix),
        None => pattern == id,
    }
}

struct Unit {
    files: Vec<String>,
    selection: Selection,
}

struct ScanState {
    id: String,
    root: PathBuf,
    scanner: Arc<Scanner>,
    /// Options of the scan (cache settings are reused by `verify`).
    opts: ScanOptions,
    result: ScanResult,
    units: HashMap<String, Unit>,
    next_unit: u64,
}

impl ScanState {
    fn has_base(&self) -> bool {
        self.result.new_code.is_some()
    }
}

/// The MCP server state.
pub struct Server {
    registry: Arc<Registry>,
    default_root: PathBuf,
    scanners: HashMap<PathBuf, ScannerSlot>,
    scans: Vec<ScanState>,
    next_scan: u64,
}

// --- arguments ------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct ScanArgs {
    root: Option<String>,
    paths: Option<Vec<String>>,
    base: Option<String>,
    baseline: Option<String>,
    rev: Option<String>,
    new_only: Option<bool>,
    changed_only: Option<bool>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum GroupBy {
    Severity,
    Rule,
    File,
    Directory,
}

#[derive(Deserialize)]
struct SummaryArgs {
    scan_id: String,
    group_by: Option<GroupBy>,
    new_only: Option<bool>,
    limit: Option<usize>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Strategy {
    ByFile,
    #[serde(alias = "by_dir")]
    ByDirectory,
}

#[derive(Deserialize)]
struct WorkUnitsArgs {
    scan_id: String,
    severity: Option<String>,
    rule: Option<String>,
    new_only: Option<bool>,
    max_findings_per_unit: Option<usize>,
    strategy: Option<Strategy>,
}

#[derive(Deserialize)]
struct FindingsArgs {
    scan_id: String,
    unit_id: Option<String>,
    path: Option<String>,
    rule: Option<String>,
    severity: Option<String>,
    new_only: Option<bool>,
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct RuleArgs {
    id: String,
}

#[derive(Deserialize)]
struct VerifyArgs {
    scan_id: String,
    unit_id: Option<String>,
    paths: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ExplainArgs {
    path: String,
    root: Option<String>,
}

fn parse_args<T: for<'de> Deserialize<'de>>(args: &Value) -> Result<T, ToolError> {
    serde_json::from_value(args.clone()).map_err(|e| invalid(format!("invalid arguments: {e}")))
}

fn parse_sev(s: Option<&str>) -> Result<Option<Severity>, ToolError> {
    match s {
        None => Ok(None),
        Some(s) => mote_report::parse_severity(s)
            .map(Some)
            .ok_or_else(|| invalid(format!("invalid severity `{s}`: use HIGH, MEDIUM or LOW"))),
    }
}

// --- tool definitions -------------------------------------------------------------------------------

const SEVERITY_SCHEMA: &str = "Minimum severity: HIGH, MEDIUM or LOW";

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        },
        "annotations": {"readOnlyHint": true, "openWorldHint": false},
    })
}

/// `tools/list` entries.
pub fn definitions() -> Vec<Value> {
    let sev = json!({"type": "string", "enum": ["HIGH", "MEDIUM", "LOW"], "description": SEVERITY_SCHEMA});
    vec![
        tool(
            "scan",
            "Scan a repository (incremental: unchanged files come from the cache). Returns a scan_id and finding counts.",
            json!({
                "root": {"type": "string", "description": "Repository root (default: the server's root)"},
                "paths": {"type": "array", "items": {"type": "string"}, "description": "Only these path prefixes or globs"},
                "base": {"type": "string", "description": "Compare with merge-base(base, HEAD): findings become new/unchanged/worsened"},
                "baseline": {"type": "string", "description": "Compare with a saved results file (`mote scan --json`, path relative to root) instead of a git base: findings not in it are new"},
                "rev": {"type": "string", "description": "Scan this git revision instead of the working tree"},
                "changed_only": {"type": "boolean", "description": "Only files changed vs base"},
                "new_only": {"type": "boolean", "description": "Count only new findings in the returned summary"}
            }),
            &[],
        ),
        tool(
            "summary",
            "Finding counts of a scan grouped by severity, rule, file or directory.",
            json!({
                "scan_id": {"type": "string"},
                "group_by": {"type": "string", "enum": ["severity", "rule", "file", "directory"], "default": "severity"},
                "new_only": {"type": "boolean"},
                "limit": {"type": "integer", "minimum": 1, "description": "Max groups (default 50)"}
            }),
            &["scan_id"],
        ),
        tool(
            "work_units",
            "Partition the selected findings into disjoint file sets (no file is in two units), balanced by finding count, so parallel fixers never edit the same file.",
            json!({
                "scan_id": {"type": "string"},
                "severity": sev,
                "rule": {"type": "string", "description": "Rule id, or a prefix ending in * (e.g. java/*)"},
                "new_only": {"type": "boolean"},
                "max_findings_per_unit": {"type": "integer", "minimum": 1, "default": DEFAULT_UNIT_SIZE},
                "strategy": {"type": "string", "enum": ["by_file", "by_directory"], "default": "by_file"}
            }),
            &["scan_id"],
        ),
        tool(
            "findings",
            "Findings of a scan (or of one work unit), grouped by file, paginated with an opaque cursor.",
            json!({
                "scan_id": {"type": "string"},
                "unit_id": {"type": "string"},
                "path": {"type": "string", "description": "A file or directory (repo-relative)"},
                "rule": {"type": "string", "description": "Rule id, or a prefix ending in *"},
                "severity": sev,
                "new_only": {"type": "boolean"},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT},
                "cursor": {"type": "string", "description": "next_cursor of the previous page"}
            }),
            &["scan_id"],
        ),
        tool(
            "rule",
            "What a rule detects, why it matters, how to fix it (with examples), and its parameters.",
            json!({"id": {"type": "string", "description": "Rule id, e.g. java/empty-catch-block"}}),
            &["id"],
        ),
        tool(
            "verify",
            "Re-analyze the files of a work unit (or the given paths) after editing, against the rest of the scan (duplication is checked across the whole scan): reports each prior finding as fixed or still_present, and any introduced findings.",
            json!({
                "scan_id": {"type": "string"},
                "unit_id": {"type": "string"},
                "paths": {"type": "array", "items": {"type": "string"}}
            }),
            &["scan_id"],
        ),
        tool(
            "explain_config",
            "Effective language, test status, exclusion and rules (severity, parameters) for a file.",
            json!({
                "path": {"type": "string"},
                "root": {"type": "string", "description": "Repository root (default: the server's root)"}
            }),
            &["path"],
        ),
    ]
}

// --- helpers --------------------------------------------------------------------------------------------

fn lean(f: &Finding, with_path: bool) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(f.id));
    if with_path {
        m.insert("path".into(), json!(f.path));
    }
    m.insert("rule".into(), json!(f.rule_id));
    m.insert("sev".into(), json!(f.severity.as_str()));
    m.insert("line".into(), json!(f.range.start_line));
    m.insert("col".into(), json!(f.range.start_col));
    m.insert("end_line".into(), json!(f.range.end_line));
    m.insert("end_col".into(), json!(f.range.end_col));
    m.insert("msg".into(), json!(f.message));
    if let Some(s) = &f.symbol {
        m.insert("symbol".into(), json!(s));
    }
    if f.status != FindingStatus::Unknown {
        m.insert("status".into(), json!(mote_report::status_str(f.status)));
    }
    if let Some(v) = f.value {
        m.insert("value".into(), json!(v));
    }
    if !f.secondary.is_empty() {
        let rel: Vec<Value> = f
            .secondary
            .iter()
            .map(|l| json!({"path": l.path, "line": l.range.start_line, "end_line": l.range.end_line}))
            .collect();
        m.insert("related".into(), Value::Array(rel));
    }
    Value::Object(m)
}

fn sev_counts<'a>(it: impl Iterator<Item = &'a Finding>) -> (u64, u64, u64) {
    let (mut h, mut m, mut l) = (0, 0, 0);
    for f in it {
        match f.severity {
            Severity::High => h += 1,
            Severity::Medium => m += 1,
            Severity::Low => l += 1,
        }
    }
    (h, m, l)
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or(".")
}

fn query_hash(parts: &[&dyn std::fmt::Debug]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in parts {
        format!("{p:?}").hash(&mut h);
    }
    h.finish()
}

fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, ToolError> {
    catch_unwind(AssertUnwindSafe(f))
        .map_err(|_| failed("internal error (panic) while running the tool"))
}

/// Partition atoms `(key, files, count)` into bins of at most `cap` findings (an atom larger
/// than `cap` gets a bin of its own), balancing loads: most loaded atoms first, each into the
/// least loaded bin that still has room.
fn partition(mut atoms: Vec<Atom>, cap: usize) -> Vec<Vec<Atom>> {
    let total: usize = atoms.iter().map(|a| a.2).sum();
    atoms.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let k = total.div_ceil(cap).max(1);
    let mut bins: Vec<(usize, Vec<Atom>)> = (0..k).map(|_| (0, Vec::new())).collect();
    for atom in atoms {
        let (i, load) = bins
            .iter()
            .enumerate()
            .map(|(i, b)| (i, b.0))
            .min_by_key(|&(i, load)| (load, i))
            .unwrap_or((0, 0));
        if load > 0 && load + atom.2 > cap {
            bins.push((atom.2, vec![atom]));
        } else {
            bins[i].0 += atom.2;
            bins[i].1.push(atom);
        }
    }
    let mut out: Vec<Vec<Atom>> = bins
        .into_iter()
        .filter(|b| !b.1.is_empty())
        .map(|mut b| {
            b.1.sort_by(|x, y| x.0.cmp(&y.0));
            b.1
        })
        .collect();
    out.sort_by(|a, b| a[0].0.cmp(&b[0].0));
    out
}

// --- server ---------------------------------------------------------------------------------------------

impl Server {
    /// A server for `default_root` using the plugins of `registry`.
    pub fn new(registry: Arc<Registry>, default_root: PathBuf) -> Server {
        Server {
            registry,
            default_root,
            scanners: HashMap::new(),
            scans: Vec::new(),
            next_scan: 1,
        }
    }

    /// `tools/call`.
    pub(super) fn call_tool(&mut self, params: &Value) -> Result<Value, RpcError> {
        let Some(obj) = params.as_object() else {
            return Err((INVALID_PARAMS, "params must be an object".into()));
        };
        let Some(name) = obj.get("name").and_then(Value::as_str) else {
            return Err((INVALID_PARAMS, "missing tool `name`".into()));
        };
        let args = match obj.get("arguments") {
            None | Some(Value::Null) => Value::Object(Map::new()),
            Some(v @ Value::Object(_)) => v.clone(),
            Some(_) => return Err((INVALID_PARAMS, "`arguments` must be an object".into())),
        };
        let started = Instant::now();
        let outcome = match name {
            "scan" => self.tool_scan(&args),
            "summary" => self.tool_summary(&args),
            "work_units" => self.tool_work_units(&args),
            "findings" => self.tool_findings(&args),
            "rule" => self.tool_rule(&args),
            "verify" => self.tool_verify(&args),
            "explain_config" => self.tool_explain(&args),
            other => return Err((INVALID_PARAMS, format!("Unknown tool: {other}"))),
        };
        self.release_caches();
        tracing::debug!("tool {name} took {:?}", started.elapsed());
        match outcome {
            Ok(v) => {
                let text =
                    serde_json::to_string(&v).map_err(|e| (INTERNAL_ERROR, e.to_string()))?;
                Ok(json!({
                    "content": [{"type": "text", "text": text}],
                    "structuredContent": v,
                    "isError": false,
                }))
            }
            Err(ToolError::InvalidParams(m)) => Err((INVALID_PARAMS, m)),
            Err(ToolError::Failed(m)) => Ok(json!({
                "content": [{"type": "text", "text": m}],
                "isError": true,
            })),
        }
    }

    fn resolve_root(&self, root: Option<&str>) -> PathBuf {
        match root {
            None => self.default_root.clone(),
            Some(r) => {
                let p = PathBuf::from(r);
                if p.is_absolute() {
                    p
                } else {
                    self.default_root.join(p)
                }
            }
        }
    }

    /// Close the cache database of every scanner (current ones and those kept by stored scans
    /// for `verify`); the next scan reopens it.
    fn release_caches(&self) {
        for slot in self.scanners.values() {
            slot.scanner.close_cache();
        }
        for st in &self.scans {
            st.scanner.close_cache();
        }
    }

    /// The warm scanner for `root`, rebuilt when its config file changed.
    fn scanner_for(&mut self, root: &Path) -> Result<Arc<Scanner>, ToolError> {
        if !root.is_dir() {
            return Err(failed(format!("{}: not a directory", root.display())));
        }
        let mut key = None;
        let mut origin = None;
        for name in CONFIG_FILE_NAMES {
            let p = root.join(name);
            if p.is_file() {
                let text = std::fs::read_to_string(&p)
                    .map_err(|e| failed(format!("cannot read {}: {e}", p.display())))?;
                key = Some(text);
                origin = Some(p);
                break;
            }
        }
        let map_key = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        if let Some(slot) = self.scanners.get(&map_key) {
            if slot.config_key == key {
                return Ok(slot.scanner.clone());
            }
        }
        let config = match (&key, &origin) {
            (Some(text), Some(p)) => Config::from_yaml_named(text, &p.display().to_string())
                .map_err(|e| failed(crate::config_error(e, None).message))?,
            _ => Config::default(),
        };
        let origin_s = origin.map(|p| p.display().to_string());
        let scanner = Scanner::new(self.registry.clone(), config, root).map_err(|e| {
            failed(crate::commands::scan::scanner_error(e, origin_s.as_deref()).message)
        })?;
        let scanner = Arc::new(scanner);
        // The replaced scanner lives on in stored scans (for `verify`) but must not hold the
        // cache: the new one opens it.
        if let Some(old) = self.scanners.get(&map_key) {
            old.scanner.close_cache();
        }
        self.scanners.insert(
            map_key,
            ScannerSlot {
                config_key: key,
                scanner: scanner.clone(),
            },
        );
        Ok(scanner)
    }

    fn state(&self, scan_id: &str) -> Result<&ScanState, ToolError> {
        self.scans.iter().find(|s| s.id == scan_id).ok_or_else(|| {
            failed(format!(
                "unknown scan_id `{scan_id}` (the server keeps the last {MAX_SCANS} scans; run `scan` again)"
            ))
        })
    }

    fn state_mut(&mut self, scan_id: &str) -> Result<&mut ScanState, ToolError> {
        self.scans
            .iter_mut()
            .find(|s| s.id == scan_id)
            .ok_or_else(|| failed(format!("unknown scan_id `{scan_id}`; run `scan` again")))
    }

    fn tool_scan(&mut self, args: &Value) -> ToolResult {
        let a: ScanArgs = parse_args(args)?;
        let root = self.resolve_root(a.root.as_deref());
        if a.base.is_some() && a.baseline.is_some() {
            return Err(invalid("give `base` or `baseline`, not both"));
        }
        let baseline = match &a.baseline {
            Some(file) => {
                let path = root.join(file);
                let b = crate::commands::scan::load_baseline(&path, file)
                    .map_err(|e| failed(e.message))?;
                Some(Arc::new(b))
            }
            None => None,
        };
        let scanner = self.scanner_for(&root)?;
        let opts = ScanOptions {
            baseline,
            source: match &a.rev {
                Some(r) => SourceSpec::Rev(r.clone()),
                None => SourceSpec::Auto,
            },
            paths: a.paths.clone().unwrap_or_default(),
            base: a.base.clone(),
            changed_only: a.changed_only.unwrap_or(false) && a.base.is_some(),
            ..Default::default()
        };
        let started = Instant::now();
        let result = guarded(|| scanner.scan(&opts))?
            .map_err(|e| failed(format!("scan failed: {}", crate::CliError::from(e).message)))?;
        let ms = started.elapsed().as_millis() as u64;
        let id = format!("s{}", self.next_scan);
        self.next_scan += 1;
        let state = ScanState {
            id: id.clone(),
            root,
            scanner,
            opts,
            result,
            units: HashMap::new(),
            next_unit: 1,
        };
        let summary = scan_summary(&state, a.new_only.unwrap_or(false), ms);
        self.scans.push(state);
        if self.scans.len() > MAX_SCANS {
            self.scans.remove(0);
        }
        Ok(summary)
    }

    fn tool_summary(&mut self, args: &Value) -> ToolResult {
        let a: SummaryArgs = parse_args(args)?;
        let st = self.state(&a.scan_id)?;
        let group_by = a.group_by.unwrap_or(GroupBy::Severity);
        let sel = Selection {
            new_only: a.new_only.unwrap_or(false),
            ..Default::default()
        };
        let base = st.has_base();
        let mut groups: BTreeMap<String, [u64; 3]> = BTreeMap::new();
        let mut total = 0u64;
        for f in st.result.findings.iter().filter(|f| sel.accepts(f, base)) {
            total += 1;
            let key = match group_by {
                GroupBy::Severity => f.severity.as_str().to_string(),
                GroupBy::Rule => f.rule_id.clone(),
                GroupBy::File => f.path.clone(),
                GroupBy::Directory => dir_of(&f.path).to_string(),
            };
            let e = groups.entry(key).or_default();
            match f.severity {
                Severity::High => e[0] += 1,
                Severity::Medium => e[1] += 1,
                Severity::Low => e[2] += 1,
            }
        }
        let mut list: Vec<(String, [u64; 3])> = groups.into_iter().collect();
        if group_by == GroupBy::Severity {
            let rank = |k: &str| match k {
                "HIGH" => 0,
                "MEDIUM" => 1,
                _ => 2,
            };
            list.sort_by_key(|(k, _)| rank(k));
        } else {
            list.sort_by(|a, b| {
                let (ca, cb) = (a.1.iter().sum::<u64>(), b.1.iter().sum::<u64>());
                cb.cmp(&ca).then_with(|| a.0.cmp(&b.0))
            });
        }
        let limit = a.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, 1000);
        let more = list.len().saturating_sub(limit);
        let groups: Vec<Value> = list
            .into_iter()
            .take(limit)
            .map(|(k, c)| {
                if group_by == GroupBy::Severity {
                    json!({"key": k, "count": c.iter().sum::<u64>()})
                } else {
                    json!({"key": k, "count": c.iter().sum::<u64>(), "high": c[0], "medium": c[1], "low": c[2]})
                }
            })
            .collect();
        let mut out = json!({"scan_id": st.id, "group_by": group_by_str(group_by), "total": total, "groups": groups});
        if more > 0 {
            out["more_groups"] = json!(more);
        }
        Ok(out)
    }

    fn tool_work_units(&mut self, args: &Value) -> ToolResult {
        let a: WorkUnitsArgs = parse_args(args)?;
        let selection = Selection {
            min_severity: parse_sev(a.severity.as_deref())?,
            rule: a.rule.clone(),
            new_only: a.new_only.unwrap_or(false),
        };
        let cap = a.max_findings_per_unit.unwrap_or(DEFAULT_UNIT_SIZE).max(1);
        let strategy = a.strategy.unwrap_or(Strategy::ByFile);
        let st = self.state_mut(&a.scan_id)?;
        let base = st.has_base();
        // atom key → (files, count)
        let mut atoms: BTreeMap<String, (Vec<String>, usize)> = BTreeMap::new();
        let mut total = 0usize;
        for f in st
            .result
            .findings
            .iter()
            .filter(|f| selection.accepts(f, base))
        {
            total += 1;
            let key = match strategy {
                Strategy::ByFile => f.path.clone(),
                Strategy::ByDirectory => dir_of(&f.path).to_string(),
            };
            let e = atoms.entry(key).or_default();
            if e.0.last() != Some(&f.path) && !e.0.contains(&f.path) {
                e.0.push(f.path.clone());
            }
            e.1 += 1;
        }
        let bins = partition(
            atoms
                .into_iter()
                .map(|(k, (files, n))| (k, files, n))
                .collect(),
            cap,
        );
        let mut units = Vec::with_capacity(bins.len());
        for bin in bins {
            let mut files: Vec<String> = bin.into_iter().flat_map(|a| a.1).collect();
            files.sort();
            let fileset: HashSet<&str> = files.iter().map(String::as_str).collect();
            let sel: Vec<&Finding> = st
                .result
                .findings
                .iter()
                .filter(|f| fileset.contains(f.path.as_str()) && selection.accepts(f, base))
                .collect();
            let (h, m, l) = sev_counts(sel.iter().copied());
            let id = format!("u{}", st.next_unit);
            st.next_unit += 1;
            units.push(json!({"id": id, "files": files, "findings": sel.len(), "high": h, "medium": m, "low": l}));
            st.units.insert(
                id,
                Unit {
                    files,
                    selection: selection.clone(),
                },
            );
        }
        Ok(json!({
            "scan_id": st.id,
            "strategy": match strategy { Strategy::ByFile => "by_file", Strategy::ByDirectory => "by_directory" },
            "total_findings": total,
            "units": units,
        }))
    }

    fn tool_findings(&mut self, args: &Value) -> ToolResult {
        let a: FindingsArgs = parse_args(args)?;
        let min = parse_sev(a.severity.as_deref())?;
        let st = self.state(&a.scan_id)?;
        let base = st.has_base();
        let explicit = Selection {
            min_severity: min,
            rule: a.rule.clone(),
            new_only: a.new_only.unwrap_or(false),
        };
        let unit = match &a.unit_id {
            Some(u) => Some(st.units.get(u).ok_or_else(|| {
                failed(format!(
                    "unknown unit_id `{u}` for scan {} (call work_units first)",
                    st.id
                ))
            })?),
            None => None,
        };
        let unit_files: Option<HashSet<&str>> =
            unit.map(|u| u.files.iter().map(String::as_str).collect());
        let path = a
            .path
            .as_deref()
            .map(|p| p.replace('\\', "/").trim_end_matches('/').to_string());
        let list: Vec<&Finding> = st
            .result
            .findings
            .iter()
            .filter(|f| explicit.accepts(f, base))
            .filter(|f| unit.is_none_or(|u| u.selection.accepts(f, base)))
            .filter(|f| {
                unit_files
                    .as_ref()
                    .is_none_or(|s| s.contains(f.path.as_str()))
            })
            .filter(|f| {
                path.as_deref().is_none_or(|p| {
                    p.is_empty()
                        || f.path == p
                        || f.path.strip_prefix(p).is_some_and(|r| r.starts_with('/'))
                })
            })
            .collect();
        let limit = a.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let qh = query_hash(&[
            &a.scan_id,
            &a.unit_id,
            &path,
            &a.rule,
            &a.severity,
            &a.new_only,
        ]);
        let offset = match &a.cursor {
            None => 0,
            Some(c) => {
                let (o, h) = c
                    .split_once('.')
                    .and_then(|(o, h)| {
                        Some((
                            usize::from_str_radix(o, 16).ok()?,
                            u64::from_str_radix(h, 16).ok()?,
                        ))
                    })
                    .ok_or_else(|| invalid(format!("invalid cursor `{c}`")))?;
                if h != qh {
                    return Err(invalid(
                        "the cursor belongs to a different query; repeat the same arguments",
                    ));
                }
                o
            }
        };
        let page = list.iter().skip(offset).take(limit);
        let mut files: Vec<Value> = Vec::new();
        let mut cur_path: Option<&str> = None;
        let mut cur: Vec<Value> = Vec::new();
        let mut returned = 0usize;
        for f in page {
            if cur_path.is_some_and(|p| p != f.path) {
                files.push(json!({"path": cur_path, "findings": std::mem::take(&mut cur)}));
            }
            cur_path = Some(&f.path);
            cur.push(lean(f, false));
            returned += 1;
        }
        if let Some(p) = cur_path {
            files.push(json!({"path": p, "findings": cur}));
        }
        let mut out =
            json!({"scan_id": st.id, "total": list.len(), "returned": returned, "files": files});
        if offset + returned < list.len() {
            out["next_cursor"] = json!(format!("{:x}.{:x}", offset + returned, qh));
        }
        Ok(out)
    }

    fn tool_rule(&mut self, args: &Value) -> ToolResult {
        let a: RuleArgs = parse_args(args)?;
        let info = crate::commands::rules::find_rule(&self.registry, &a.id).map_err(failed)?;
        serde_json::to_value(info).map_err(|e| failed(e.to_string()))
    }

    fn tool_verify(&mut self, args: &Value) -> ToolResult {
        let a: VerifyArgs = parse_args(args)?;
        let st = self.state(&a.scan_id)?;
        let base = st.has_base();
        let (files, selection): (Vec<String>, Selection) = match (&a.unit_id, &a.paths) {
            (Some(u), _) => {
                let unit = st
                    .units
                    .get(u)
                    .ok_or_else(|| failed(format!("unknown unit_id `{u}` for scan {}", st.id)))?;
                (unit.files.clone(), unit.selection.clone())
            }
            (None, Some(paths)) if !paths.is_empty() => {
                let mut files = Vec::new();
                for p in paths {
                    let rel = mote_engine::source::relativize(&st.root, p)
                        .ok_or_else(|| failed(format!("{p} is outside the scanned root")))?;
                    files.push(rel);
                }
                files.sort();
                files.dedup();
                (files, Selection::default())
            }
            _ => return Err(invalid("give `unit_id` or a non-empty `paths`")),
        };
        let fileset: HashSet<&str> = files.iter().map(String::as_str).collect();
        let prior_all: HashSet<&str> = st
            .result
            .findings
            .iter()
            .filter(|f| f.suppressed.is_none() && fileset.contains(f.path.as_str()))
            .map(|f| f.id.as_str())
            .collect();
        let prior: Vec<&Finding> = st
            .result
            .findings
            .iter()
            .filter(|f| fileset.contains(f.path.as_str()) && selection.accepts(f, base))
            .collect();
        let scanner = st.scanner.clone();
        let started = Instant::now();
        // Only these files are read and analyzed again; every other file of the scan comes from
        // the cache, so duplication (and everything else cross-file) is checked against the
        // whole scan, not just this unit.
        let current = guarded(|| scanner.rescan(&st.result, &files, &st.opts))?.map_err(|e| {
            failed(format!(
                "analysis failed: {}",
                crate::CliError::from(e).message
            ))
        })?;
        let ms = started.elapsed().as_millis() as u64;
        let now_findings: Vec<&Finding> = current
            .findings
            .iter()
            .filter(|f| f.suppressed.is_none() && fileset.contains(f.path.as_str()))
            .collect();
        let now: HashSet<&str> = now_findings.iter().map(|f| f.id.as_str()).collect();
        let mut fixed = Vec::new();
        let mut still = Vec::new();
        for f in &prior {
            if now.contains(f.id.as_str()) {
                still.push(f.id.as_str());
            } else {
                fixed.push(f.id.as_str());
            }
        }
        let introduced: Vec<Value> = now_findings
            .iter()
            .filter(|f| !prior_all.contains(f.id.as_str()))
            .map(|f| lean(f, true))
            .collect();
        let mut out = json!({
            "scan_id": st.id,
            "files": files.len(),
            "counts": {"fixed": fixed.len(), "still_present": still.len(), "introduced": introduced.len()},
            "fixed": fixed,
            "still_present": still,
            "introduced": introduced,
            "ms": ms,
        });
        // Problems with these files, plus files the scan does not cover.
        let known: HashSet<&str> = current.files.iter().map(|f| f.path.as_str()).collect();
        let mut warnings: Vec<String> = current
            .diagnostics
            .iter()
            .filter(|d| d.path.as_deref().is_some_and(|p| fileset.contains(p)))
            .map(|d| format!("{}: {}", d.path.as_deref().unwrap_or(""), d.message))
            .collect();
        for f in &files {
            if !known.contains(f.as_str()) {
                warnings.push(format!(
                    "{f}: not part of this scan (deleted, outside its path filters, or no language plugin)"
                ));
            }
        }
        if !warnings.is_empty() {
            out["warnings"] = json!(warnings);
        }
        Ok(out)
    }

    fn tool_explain(&mut self, args: &Value) -> ToolResult {
        let a: ExplainArgs = parse_args(args)?;
        let root = self.resolve_root(a.root.as_deref());
        let scanner = self.scanner_for(&root)?;
        let rel = mote_engine::source::relativize(&root, &a.path)
            .ok_or_else(|| failed(format!("{} is outside {}", a.path, root.display())))?;
        let eff = scanner
            .effective_rules(&rel)
            .ok_or_else(|| failed(format!("no language plugin handles {rel}")))?;
        Ok(crate::commands::config::effective_json(&rel, &eff))
    }
}

fn group_by_str(g: GroupBy) -> &'static str {
    match g {
        GroupBy::Severity => "severity",
        GroupBy::Rule => "rule",
        GroupBy::File => "file",
        GroupBy::Directory => "directory",
    }
}

fn scan_summary(st: &ScanState, new_only: bool, ms: u64) -> Value {
    let r = &st.result;
    let base = st.has_base();
    let sel = Selection {
        new_only,
        ..Default::default()
    };
    let active: Vec<&Finding> = r.findings.iter().filter(|f| sel.accepts(f, base)).collect();
    let (h, m, l) = sev_counts(active.iter().copied());
    let (mut analyzed, mut cached, mut skipped) = (0u64, 0u64, 0u64);
    for f in &r.files {
        match f.status {
            mote_engine::result::FileStatus::Analyzed => analyzed += 1,
            mote_engine::result::FileStatus::Cached => cached += 1,
            mote_engine::result::FileStatus::Skipped => skipped += 1,
        }
    }
    let mut by_rule: BTreeMap<&str, u64> = BTreeMap::new();
    for f in &active {
        *by_rule.entry(f.rule_id.as_str()).or_default() += 1;
    }
    let mut top: Vec<(&str, u64)> = by_rule.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    top.truncate(5);
    let mut out = json!({
        "scan_id": st.id,
        "root": st.root.display().to_string().replace('\\', "/"),
        "source": r.source.kind,
        "files": {"analyzed": analyzed, "cached": cached, "skipped": skipped},
        "findings": {"total": active.len(), "high": h, "medium": m, "low": l},
        "suppressed": r.findings.iter().filter(|f| f.suppressed.is_some()).count(),
        "gate": {
            "passed": r.gate.passed,
            "failed": r.gate.conditions.iter().filter(|c| !c.passed).map(|c| c.name.as_str()).collect::<Vec<_>>(),
        },
        "top_rules": top.iter().map(|(r, n)| json!({"rule": r, "count": n})).collect::<Vec<_>>(),
        "ms": ms,
    });
    if let Some(c) = &r.source.commit {
        out["commit"] = json!(mote_report::short_commit(c));
    }
    if let Some(nc) = &r.new_code {
        let n = &nc.measures;
        out["new_code"] = json!({
            "base": nc.base,
            "changed_files": n.changed_files,
            "new_lines": n.new_lines,
            "new": {"high": n.new_findings.high, "medium": n.new_findings.medium, "low": n.new_findings.low},
            "worsened": n.worsened_findings.total(),
            "fixed": n.fixed_findings.total(),
        });
        if nc.baseline.is_some() {
            out["new_code"]["baseline"] = json!(true);
        }
    }
    if let Some(t) = &r.triage {
        out["triage"] = json!({
            "file": t.file,
            "entries": t.entries,
            "matched": t.matched,
            "stale": t.stale.len(),
        });
    }
    if new_only {
        out["new_only"] = json!(true);
    }
    if !r.diagnostics.is_empty() {
        out["warnings"] = json!(r.diagnostics.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_is_disjoint_and_balanced() {
        let atoms: Vec<Atom> = (0..40)
            .map(|i| {
                (
                    format!("f{i:02}"),
                    vec![format!("f{i:02}")],
                    1 + (i * 7) % 11,
                )
            })
            .collect();
        let total: usize = atoms.iter().map(|a| a.2).sum();
        let bins = partition(atoms, 20);
        let mut seen = HashSet::new();
        let mut sum = 0;
        for b in &bins {
            let load: usize = b.iter().map(|a| a.2).sum();
            assert!(load <= 20, "{load}");
            sum += load;
            for a in b {
                assert!(seen.insert(a.0.clone()), "{} twice", a.0);
            }
        }
        assert_eq!(sum, total);
        assert!(bins.len() <= total.div_ceil(20) + 1, "{} bins", bins.len());
        // an oversized atom gets its own unit
        let bins = partition(
            vec![
                ("big".into(), vec!["big".into()], 50),
                ("s".into(), vec!["s".into()], 1),
            ],
            10,
        );
        assert_eq!(bins.len(), 2);
    }

    #[test]
    fn rule_patterns() {
        assert!(rule_matches("java/*", "java/x"));
        assert!(!rule_matches("java/*", "js/x"));
        assert!(rule_matches("js/x", "js/x"));
    }
}
