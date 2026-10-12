//! Cache correctness stress: cached results must always equal a fresh scan.
//!
//! On a temporary git repository (local only, no remote) holding a mix of Java and
//! JavaScript/TypeScript files, a seeded random sequence of steps edits, adds, deletes and renames
//! files, edits `.mote.yaml` (profiles, rules on/off, severities, parameters, `overrides`,
//! `test_rules`, `exclude`, `tests`, `suppressions.compat`, `skip_generated` /
//! `skip_generated_except`, `java.release`, duplication settings), a `pom.xml` Java release and a
//! `package.json` name, adds and removes library banners, generated-code markers and inline
//! suppressions, stages and commits. After every step the real binary scans with the cache and
//! without it (`--no-cache`), and the JSON results must be identical apart from the run
//! statistics. Also compared: a second configuration sharing the same cache directory, the default
//! cache location, `--base main` (new code), `--rev` and `--staged`.
//!
//! Ignored by default (minutes). Run in a release build:
//!
//! ```text
//! MOTE_STRESS_JAVA=/path/to/a/java/repo \
//! MOTE_STRESS_JS=/path/to/a/js/repo MOTE_STRESS_STEPS=300 \
//!   cargo test -p mote-cli --release --test cache_stress -- --ignored --nocapture
//! ```
//!
//! Environment (all optional): `MOTE_STRESS_JAVA` / `MOTE_STRESS_JS` — directories to
//! copy source files from (read only; default: the plugins' fixtures); `MOTE_STRESS_FILES` —
//! files per language (default 150); `MOTE_STRESS_SEED` (default 1);
//! `MOTE_STRESS_STEPS` (default 100); `MOTE_FUZZ_WORKDIR` — where the repository goes
//! (default: the system temp directory); `MOTE_STRESS_KEEP_GOING=1` — report every divergence
//! instead of stopping at the first (the repository is kept for inspection either way). `MOTE_STRESS_BIN` — the
//! `mote` binary to run (default: the one built with the test).

#![cfg(all(feature = "java", feature = "js"))]

mod common;

use common::{git, isolate, BIN};

/// The binary under test: `MOTE_STRESS_BIN` (e.g. a copy, so that rebuilding during a long run
/// does not replace it), else the one cargo built for this test.
fn bin() -> PathBuf {
    std::env::var_os("MOTE_STRESS_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BIN))
}
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut r = Rng(seed ^ 0x9E37_79B9_7F4A_7C15);
        r.next();
        r
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next() % den < num
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().replace('_', "").parse().ok())
        .unwrap_or(default)
}

const JS_EXTS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

/// Source files under `dir` (sorted, relative to `dir`), skipping `.git` and `node_modules`.
fn walk(base: &Path, dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            let name = e.file_name();
            if !matches!(name.to_string_lossy().as_ref(), ".git" | "node_modules") {
                walk(base, &p, exts, out);
            }
        } else if ft.is_file() {
            let ext = p
                .extension()
                .map(|x| x.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if exts.contains(&ext.as_str())
                && std::fs::metadata(&p).is_ok_and(|m| m.len() < 200_000)
            {
                out.push(p.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
}

/// A deterministic sample of `n` files under `dir`: `(relative path, content)`.
fn sample(dir: &Path, exts: &[&str], n: usize, rng: &mut Rng) -> Vec<(String, Vec<u8>)> {
    let mut all = Vec::new();
    walk(dir, dir, exts, &mut all);
    for i in (1..all.len()).rev() {
        let j = rng.below(i + 1);
        all.swap(i, j);
    }
    all.truncate(n);
    all.sort();
    all.into_iter()
        .filter_map(|p| {
            let bytes = std::fs::read(dir.join(&p)).ok()?;
            Some((p.to_string_lossy().replace('\\', "/"), bytes))
        })
        .collect()
}

/// A rule from `mote rules --format json --all`.
struct RuleInfo {
    id: String,
    /// `(name, default)` of each boolean or integer parameter.
    params: Vec<(String, serde_json::Value)>,
}

fn catalog() -> Vec<RuleInfo> {
    let out = Command::new(bin())
        .args(["rules", "--format", "json", "--all"])
        .output()
        .expect("mote rules");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("rules JSON");
    v.as_array()
        .unwrap()
        .iter()
        .filter(|r| r["status"] != "deprecated")
        .map(|r| RuleInfo {
            id: r["id"].as_str().unwrap().to_string(),
            params: r["params"]
                .as_array()
                .map(|ps| {
                    ps.iter()
                        .filter(|p| p["default"].is_boolean() || p["default"].is_u64())
                        .map(|p| {
                            (
                                p["name"].as_str().unwrap().to_string(),
                                p["default"].clone(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

/// The configuration under test, rendered to YAML.
#[derive(Clone, Default)]
struct Cfg {
    extends: &'static str,
    rules: BTreeMap<String, String>,
    test_rules: BTreeMap<String, String>,
    overrides: Vec<(String, BTreeMap<String, String>)>,
    exclude: Vec<String>,
    tests: Vec<String>,
    compat: String,
    require_reason: bool,
    skip_generated: bool,
    skip_generated_except: Vec<String>,
    java_release: Option<u32>,
    cpd_min_tokens: u32,
    cpd_ignore_literals: bool,
    cpd_include_tests: bool,
    cpd_exclude: Vec<String>,
    max_file_size: u64,
}

fn yaml_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format!("'{s}'")).collect();
    format!("[{}]", quoted.join(", "))
}

fn yaml_rules(rules: &BTreeMap<String, String>, indent: &str) -> String {
    if rules.is_empty() {
        return " {}\n".into();
    }
    let mut s = String::from("\n");
    for (k, v) in rules {
        s.push_str(&format!("{indent}'{k}': {v}\n"));
    }
    s
}

impl Cfg {
    fn initial() -> Cfg {
        Cfg {
            extends: "default",
            compat: "false".into(),
            require_reason: true,
            skip_generated: true,
            cpd_min_tokens: 60,
            cpd_ignore_literals: true,
            max_file_size: 1 << 20,
            ..Cfg::default()
        }
    }

    fn render(&self) -> String {
        let mut y = format!("extends: {}\n", self.extends);
        y.push_str(&format!("rules:{}", yaml_rules(&self.rules, "  ")));
        y.push_str(&format!(
            "test_rules:{}",
            yaml_rules(&self.test_rules, "  ")
        ));
        y.push_str("overrides:");
        if self.overrides.is_empty() {
            y.push_str(" []\n");
        } else {
            y.push('\n');
            for (glob, rules) in &self.overrides {
                y.push_str(&format!(
                    "  - paths: ['{glob}']\n    rules:{}",
                    yaml_rules(rules, "      ")
                ));
            }
        }
        y.push_str(&format!("exclude: {}\n", yaml_list(&self.exclude)));
        y.push_str(&format!("tests: {}\n", yaml_list(&self.tests)));
        y.push_str(&format!(
            "suppressions:\n  compat: {}\n  require_reason: {}\n",
            self.compat, self.require_reason
        ));
        y.push_str(&format!("skip_generated: {}\n", self.skip_generated));
        y.push_str(&format!(
            "skip_generated_except: {}\n",
            yaml_list(&self.skip_generated_except)
        ));
        if let Some(r) = self.java_release {
            y.push_str(&format!("java:\n  release: {r}\n"));
        }
        y.push_str(&format!(
            "cpd:\n  min_tokens: {}\n  min_lines: 3\n  ignore_literals: {}\n  include_tests: {}\n  exclude: {}\n",
            self.cpd_min_tokens,
            self.cpd_ignore_literals,
            self.cpd_include_tests,
            yaml_list(&self.cpd_exclude)
        ));
        y.push_str(&format!("max_file_size: {}\n", self.max_file_size));
        y
    }
}

/// Globs that match some of the repository's files.
const GLOBS: &[&str] = &[
    "java/**",
    "js/**",
    "**/test/**",
    "**/src/test/**",
    "**/*Test.java",
    "**/*.tsx",
    "**/*.ts",
    "**/*.js",
    "**/utils/**",
    "**/components/**",
    "**/compress/**",
    "**/archivers/**",
    "extra/**",
    "moved/**",
    "**/*.spec.*",
    "**/gen/**",
];

/// One random setting for `rule`.
fn rule_setting(rng: &mut Rng, rule: &RuleInfo) -> String {
    match rng.below(7) {
        0 => "off".into(),
        1 => "on".into(),
        2 => "LOW".into(),
        3 => "MEDIUM".into(),
        4 => "HIGH".into(),
        _ => {
            let mut parts = Vec::new();
            if rng.chance(1, 2) {
                parts.push(format!("enabled: {}", rng.chance(3, 4)));
            }
            if rng.chance(1, 2) {
                parts.push(format!(
                    "severity: {}",
                    rng.pick(&["LOW", "MEDIUM", "HIGH"])
                ));
            }
            if !rule.params.is_empty() {
                let (name, default) = &rule.params[rng.below(rule.params.len())];
                let value = if default.is_boolean() {
                    serde_json::Value::Bool(rng.chance(1, 2))
                } else {
                    let d = default.as_u64().unwrap_or(1);
                    serde_json::json!(1 + rng.below((d as usize * 2).clamp(2, 200)))
                };
                parts.push(format!("params: {{ {name}: {value} }}"));
            }
            format!("{{ {} }}", parts.join(", "))
        }
    }
}

fn rule_key(rng: &mut Rng, rules: &[RuleInfo]) -> (String, usize) {
    let i = rng.below(rules.len());
    if rng.chance(1, 8) {
        let lang = rules[i].id.split('/').next().unwrap();
        (format!("{lang}/*"), i)
    } else {
        (rules[i].id.clone(), i)
    }
}

fn mutate_config(cfg: &mut Cfg, rng: &mut Rng, rules: &[RuleInfo]) -> String {
    let glob = || -> String { String::new() };
    let _ = glob;
    match rng.below(16) {
        0 => {
            cfg.extends = rng.pick(&["default", "all", "conventions", "none"]);
            format!("extends {}", cfg.extends)
        }
        1..=3 => {
            let (key, i) = rule_key(rng, rules);
            if rng.chance(1, 4) {
                cfg.rules.remove(&key);
                format!("rules: drop {key}")
            } else {
                let s = rule_setting(rng, &rules[i]);
                cfg.rules.insert(key.clone(), s.clone());
                format!("rules: {key} = {s}")
            }
        }
        4 => {
            let (key, i) = rule_key(rng, rules);
            let s = rule_setting(rng, &rules[i]);
            cfg.test_rules.insert(key.clone(), s.clone());
            format!("test_rules: {key} = {s}")
        }
        5 => {
            if rng.chance(1, 3) && !cfg.overrides.is_empty() {
                cfg.overrides.remove(rng.below(cfg.overrides.len()));
                "overrides: drop one".into()
            } else {
                let glob = rng.pick(GLOBS).to_string();
                let mut m = BTreeMap::new();
                for _ in 0..1 + rng.below(3) {
                    let (key, i) = rule_key(rng, rules);
                    m.insert(key, rule_setting(rng, &rules[i]));
                }
                let d = format!("overrides: {glob} {m:?}");
                cfg.overrides.push((glob, m));
                d
            }
        }
        6 => toggle_glob(&mut cfg.exclude, rng, "exclude"),
        7 => toggle_glob(&mut cfg.tests, rng, "tests"),
        8 => {
            cfg.compat = match rng.below(4) {
                0 => "false".into(),
                1 => "true".into(),
                _ => {
                    let all = [
                        "checkstyle",
                        "eslint",
                        "intellij",
                        "nosonar",
                        "pmd",
                        "spotbugs",
                        "suppress-warnings-all",
                    ];
                    let chosen: Vec<String> = all
                        .iter()
                        .filter(|_| rng.chance(1, 2))
                        .map(|s| s.to_string())
                        .collect();
                    format!("[{}]", chosen.join(", "))
                }
            };
            format!("compat {}", cfg.compat)
        }
        9 => {
            cfg.require_reason = !cfg.require_reason;
            format!("require_reason {}", cfg.require_reason)
        }
        10 => {
            cfg.skip_generated = !cfg.skip_generated;
            format!("skip_generated {}", cfg.skip_generated)
        }
        11 => toggle_glob(&mut cfg.skip_generated_except, rng, "skip_generated_except"),
        12 => {
            cfg.java_release = *rng.pick(&[None, Some(8), Some(11), Some(17), Some(21)]);
            format!("java.release {:?}", cfg.java_release)
        }
        13 => {
            cfg.cpd_min_tokens = *rng.pick(&[20, 40, 60, 100]);
            cfg.cpd_ignore_literals = rng.chance(1, 2);
            cfg.cpd_include_tests = rng.chance(1, 2);
            format!(
                "cpd min_tokens {} ignore_literals {} include_tests {}",
                cfg.cpd_min_tokens, cfg.cpd_ignore_literals, cfg.cpd_include_tests
            )
        }
        14 => toggle_glob(&mut cfg.cpd_exclude, rng, "cpd.exclude"),
        _ => {
            cfg.max_file_size = *rng.pick(&[1 << 20, 20_000, 50_000]);
            format!("max_file_size {}", cfg.max_file_size)
        }
    }
}

fn toggle_glob(list: &mut Vec<String>, rng: &mut Rng, what: &str) -> String {
    if !list.is_empty() && rng.chance(1, 2) {
        let g = list.remove(rng.below(list.len()));
        format!("{what}: drop {g}")
    } else {
        let g = rng.pick(GLOBS).to_string();
        list.push(g.clone());
        format!("{what}: add {g}")
    }
}

struct Repo {
    root: PathBuf,
    files: Vec<String>,
    pool: Vec<(String, Vec<u8>)>,
    commits: u32,
}

impl Repo {
    fn abs(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    fn write(&self, rel: &str, bytes: &[u8]) {
        let p = self.abs(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.abs(rel)).unwrap_or_default()
    }
    fn source_files(&self, ext_filter: impl Fn(&str) -> bool) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| ext_filter(f.rsplit('.').next().unwrap_or("")))
            .cloned()
            .collect()
    }
}

fn is_js(ext: &str) -> bool {
    JS_EXTS.contains(&ext)
}

fn lines_of(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .split_inclusive('\n')
        .map(str::to_string)
        .collect()
}

const JAVA_SNIPPETS: &[&str] = &[
    "    void fuzzA() { try { run(); } catch (Exception e) { } }\n",
    "    boolean fuzzB(String a) { return a == \"x\"; }\n",
    "    void fuzzC() { System.out.println(\"debug\"); }\n",
    "    // TODO fuzz: remove this\n",
    "    private int unusedFuzzField;\n",
    "    void fuzzD() { int x = 1; x = x; }\n",
    "    @SuppressWarnings(\"all\")\n    void fuzzE() { try { run(); } catch (Exception e) { } }\n",
    "    // mote-ignore[java/empty-catch-block]: fuzz\n    void fuzzF() { try { run(); } catch (Exception e) { } }\n",
    "    // mote-ignore[java/empty-catch-block]\n    void fuzzG() { try { run(); } catch (Exception e) { } }\n",
    "    void fuzzH() { try { run(); } catch (Exception e) { } } // NOPMD\n",
    "    // CHECKSTYLE:OFF\n    void fuzzI() { try { run(); } catch (Exception e) { } }\n    // CHECKSTYLE:ON\n",
    "    //noinspection ALL\n    void fuzzJ() { try { run(); } catch (Exception e) { } }\n",
    "    void fuzzK() { try { run(); } catch (Exception e) { } } // NOSONAR\n",
];

const JS_SNIPPETS: &[&str] = &[
    "function fuzzA() { try { run(); } catch (e) { } }\n",
    "function fuzzB(a) { if (a == null) { debugger; } return a === a; }\n",
    "// eslint-disable-next-line no-empty\nfunction fuzzC() { try { run(); } catch (e) { } }\n",
    "/* eslint-disable */\n",
    "// mote-ignore[js/no-self-compare]: fuzz\nconst fuzzD = (x) => x === x;\n",
    "// mote-ignore-file[js/no-debugger]: fuzz\n",
    "var fuzzE = 1; var fuzzE = 2;\n",
    "// TODO fuzz\n",
    "console.log('fuzz');\n",
];

const GENERATED: &[&str] = &[
    "// Code generated by fuzz. DO NOT EDIT.\n",
    "/* @generated */\n",
    "// This file is auto-generated. Do not edit.\n",
];

/// Apply one random step; returns its description.
fn step(repo: &mut Repo, rng: &mut Rng, cfg: &mut Cfg, rules: &[RuleInfo]) -> String {
    let op = rng.below(100);
    let pick_file = |rng: &mut Rng, repo: &Repo, f: &dyn Fn(&str) -> bool| -> Option<String> {
        let files = repo.source_files(f);
        (!files.is_empty()).then(|| files[rng.below(files.len())].clone())
    };
    match op {
        // Edit: insert a snippet, delete lines, duplicate a block, or touch.
        0..=24 => {
            let Some(f) = pick_file(rng, repo, &|_| true) else {
                return "noop".into();
            };
            let mut lines = lines_of(&repo.read(&f));
            let java = f.ends_with(".java");
            let what = match rng.below(5) {
                0 | 1 => {
                    let snippets = if java { JAVA_SNIPPETS } else { JS_SNIPPETS };
                    let s = *rng.pick(snippets);
                    // Java: inside the first class body when there is one.
                    let at = if java {
                        lines
                            .iter()
                            .position(|l| l.contains(" class ") && l.trim_end().ends_with('{'))
                            .map_or(lines.len(), |i| i + 1)
                    } else {
                        rng.below(lines.len() + 1)
                    };
                    lines.insert(at.min(lines.len()), s.to_string());
                    format!("insert snippet at {at}")
                }
                2 => {
                    if lines.is_empty() {
                        return "noop".into();
                    }
                    let a = rng.below(lines.len());
                    let n = (1 + rng.below(5)).min(lines.len() - a);
                    lines.drain(a..a + n);
                    format!("delete {n} lines at {a}")
                }
                3 => {
                    if lines.len() < 2 {
                        return "noop".into();
                    }
                    let a = rng.below(lines.len() - 1);
                    let n = (5 + rng.below(40)).min(lines.len() - a);
                    let block: Vec<String> = lines[a..a + n].to_vec();
                    let at = a + n;
                    lines.splice(at..at, block);
                    format!("duplicate {n} lines at {a}")
                }
                _ => {
                    if rng.chance(1, 2) {
                        for l in &mut lines {
                            if l.ends_with('\n') && !l.ends_with("\r\n") {
                                l.pop();
                                l.push_str("\r\n");
                            }
                        }
                        "convert to CRLF".into()
                    } else {
                        "touch".into()
                    }
                }
            };
            repo.write(&f, lines.concat().as_bytes());
            format!("edit {f}: {what}")
        }
        // Add a file from the pool.
        25..=31 => {
            if repo.pool.is_empty() {
                return "noop".into();
            }
            let (src, bytes) = repo.pool.swap_remove(rng.below(repo.pool.len()));
            let name = src.rsplit('/').next().unwrap().to_string();
            let dir = rng.pick(&[
                "extra/src/main/java/p",
                "extra/src/test/java/p",
                "extra/gen",
                "moved/utils",
                "moved/__tests__",
            ]);
            let ext = name.rsplit('.').next().unwrap_or("");
            let base = if ext == "java" { "java" } else { "js" };
            let rel = if rng.chance(1, 2) {
                format!("{base}/{src}")
            } else {
                format!("{dir}/{name}")
            };
            if repo.files.contains(&rel) {
                return "noop".into();
            }
            repo.write(&rel, &bytes);
            repo.files.push(rel.clone());
            format!("add {rel}")
        }
        // Delete a file.
        32..=36 => {
            let Some(f) = pick_file(rng, repo, &|_| true) else {
                return "noop".into();
            };
            std::fs::remove_file(repo.abs(&f)).unwrap();
            repo.files.retain(|x| x != &f);
            format!("delete {f}")
        }
        // Rename: another directory, test status, extension.
        37..=43 => {
            let Some(f) = pick_file(rng, repo, &|_| true) else {
                return "noop".into();
            };
            let name = f.rsplit('/').next().unwrap();
            let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
            let new_ext = if is_js(ext) && rng.chance(1, 3) {
                rng.pick(&["js", "ts", "tsx", "jsx", "mjs"]).to_string()
            } else {
                ext.to_string()
            };
            let dir = rng.pick(&[
                "moved",
                "moved/test",
                "moved/src/test/java/q",
                "moved/src/main/java/q",
                "moved/components",
                "moved/gen",
            ]);
            let new_stem = if rng.chance(1, 4) {
                format!("{stem}Test")
            } else {
                stem.to_string()
            };
            let to = format!("{dir}/{new_stem}.{new_ext}");
            if repo.files.contains(&to) {
                return "noop".into();
            }
            let bytes = repo.read(&f);
            std::fs::remove_file(repo.abs(&f)).unwrap();
            repo.write(&to, &bytes);
            repo.files.retain(|x| x != &f);
            repo.files.push(to.clone());
            format!("rename {f} -> {to}")
        }
        // Configuration.
        44..=63 => {
            let d = mutate_config(cfg, rng, rules);
            repo.write(".mote.yaml", cfg.render().as_bytes());
            format!("config: {d}")
        }
        // package.json name.
        64..=67 => {
            let dir = rng.pick(&["js", "moved", "extra"]);
            let name = rng.pick(&["fuzz-app", "lodash", "jquery", "@scope/fuzz", "react"]);
            let rel = format!("{dir}/package.json");
            if rng.chance(1, 5) {
                let _ = std::fs::remove_file(repo.abs(&rel));
                return format!("remove {rel}");
            }
            repo.write(
                &rel,
                format!("{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\"\n}}\n").as_bytes(),
            );
            format!("package.json {rel} name {name}")
        }
        // Library banner on a JavaScript file.
        68..=73 => {
            let Some(f) = pick_file(rng, repo, &is_js) else {
                return "noop".into();
            };
            let text = String::from_utf8_lossy(&repo.read(&f)).into_owned();
            let new = if text.starts_with("/*!") {
                text.split_once('\n')
                    .map_or(String::new(), |x| x.1.to_string())
            } else {
                let lib = rng.pick(&["fuzz-app", "lodash", "jquery", "react", "@scope/fuzz"]);
                format!("/*! {lib} v1.2.3 | (c) Someone | MIT License */\n{text}")
            };
            repo.write(&f, new.as_bytes());
            format!("toggle banner {f}")
        }
        // Generated-code marker.
        74..=79 => {
            let Some(f) = pick_file(rng, repo, &|_| true) else {
                return "noop".into();
            };
            let text = String::from_utf8_lossy(&repo.read(&f)).into_owned();
            let new = if let Some(g) = GENERATED.iter().find(|g| text.starts_with(**g)) {
                text[g.len()..].to_string()
            } else {
                format!("{}{text}", rng.pick(GENERATED))
            };
            repo.write(&f, new.as_bytes());
            format!("toggle generated marker {f}")
        }
        // Inline suppression of a line (or removal of one).
        80..=85 => {
            let Some(f) = pick_file(rng, repo, &|_| true) else {
                return "noop".into();
            };
            let mut lines = lines_of(&repo.read(&f));
            if let Some(i) = lines.iter().position(|l| l.contains("mote-ignore")) {
                if rng.chance(1, 2) {
                    lines.remove(i);
                    repo.write(&f, lines.concat().as_bytes());
                    return format!("remove suppression {f}:{}", i + 1);
                }
            }
            let java = f.ends_with(".java");
            let rule = &rules[rng.below(rules.len())].id;
            let s = match rng.below(4) {
                0 => format!("// mote-ignore[{rule}]: fuzz\n"),
                1 => format!("// mote-ignore[{rule}]\n"),
                2 => "// mote-ignore: blanket\n".into(),
                _ if java => "// NOPMD\n".into(),
                _ => "// eslint-disable-next-line\n".into(),
            };
            let at = rng.below(lines.len() + 1);
            lines.insert(at, s.clone());
            repo.write(&f, lines.concat().as_bytes());
            format!("suppression {f}:{}: {}", at + 1, s.trim())
        }
        // Java release in a build file.
        86..=89 => {
            let rel = rng.pick(&["java/pom.xml", "moved/pom.xml", "extra/build.gradle"]);
            let release = rng.pick(&[8, 11, 17, 21]);
            if rng.chance(1, 4) {
                let _ = std::fs::remove_file(repo.abs(rel));
                return format!("remove {rel}");
            }
            let text = if rel.ends_with("pom.xml") {
                format!(
                    "<project>\n  <modelVersion>4.0.0</modelVersion>\n  <properties>\n    <maven.compiler.release>{release}</maven.compiler.release>\n  </properties>\n</project>\n"
                )
            } else {
                format!("java {{\n  toolchain {{ }}\n}}\ntasks.withType(JavaCompile) {{ options.release = {release} }}\n")
            };
            repo.write(rel, text.as_bytes());
            format!("{rel} release {release}")
        }
        // .gitignore.
        90..=91 => {
            let text = String::from_utf8_lossy(&repo.read(".gitignore")).into_owned();
            let new = if text.contains("moved/gen/") {
                text.replace("moved/gen/\n", "")
            } else {
                format!("{text}moved/gen/\n")
            };
            repo.write(".gitignore", new.as_bytes());
            "toggle .gitignore moved/gen/".into()
        }
        // Stage some changes.
        92..=95 => {
            git(&repo.root, &["add", "-A"]);
            "stage all".into()
        }
        // Commit.
        _ => {
            git(&repo.root, &["add", "-A"]);
            git(&repo.root, &["commit", "-q", "--allow-empty", "-m", "step"]);
            repo.commits += 1;
            "commit".into()
        }
    }
}

/// Run `mote scan --format json` with `extra` arguments; returns the results without the
/// run statistics.
fn scan(root: &Path, extra: &[&str]) -> Result<serde_json::Value, String> {
    let mut cmd = Command::new(bin());
    isolate(&mut cmd)
        .current_dir(root)
        .args(["scan", "--format", "json", "--no-gate"])
        .args(extra);
    let out = cmd.output().expect("run mote");
    if !out.status.success() {
        return Err(format!(
            "exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let mut v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("bad JSON: {e}: {}", String::from_utf8_lossy(&out.stderr)))?;
    if let Some(o) = v.as_object_mut() {
        o.remove("stats");
    }
    // Where a result came from is not part of it.
    if let Some(files) = v["files"].as_array_mut() {
        for f in files {
            if f["status"] == "cached" {
                f["status"] = "analyzed".into();
            }
        }
    }
    Ok(v)
}

/// Describe how two results differ (top-level keys, then the first differing finding or file).
fn describe_diff(a: &serde_json::Value, b: &serde_json::Value) -> String {
    let mut out = Vec::new();
    if let (Some(ao), Some(bo)) = (a.as_object(), b.as_object()) {
        for (k, av) in ao {
            if bo.get(k) != Some(av) {
                let mut d = format!("`{k}` differs");
                if let (Some(aa), Some(ba)) = (av.as_array(), bo.get(k).and_then(|x| x.as_array()))
                {
                    d.push_str(&format!(" ({} vs {} items)", aa.len(), ba.len()));
                    let only_a: Vec<&serde_json::Value> =
                        aa.iter().filter(|x| !ba.contains(x)).take(2).collect();
                    let only_b: Vec<&serde_json::Value> =
                        ba.iter().filter(|x| !aa.contains(x)).take(2).collect();
                    d.push_str(&format!(
                        "\n      cached only: {}\n      fresh only:  {}",
                        serde_json::to_string(&only_a).unwrap_or_default(),
                        serde_json::to_string(&only_b).unwrap_or_default()
                    ));
                } else {
                    let s = |v: Option<&serde_json::Value>| {
                        let t = v.map(|v| v.to_string()).unwrap_or_default();
                        t.chars().take(600).collect::<String>()
                    };
                    d.push_str(&format!(
                        "\n      cached: {}\n      fresh:  {}",
                        s(Some(av)),
                        s(bo.get(k))
                    ));
                }
                out.push(d);
            }
        }
    }
    out.join("\n    ")
}

#[test]
#[ignore = "long-running cache stress; see the module docs"]
fn cache_stress() {
    let seed = env_u64("MOTE_STRESS_SEED", 1);
    let steps = env_u64("MOTE_STRESS_STEPS", 100);
    let per_lang = env_u64("MOTE_STRESS_FILES", 150) as usize;
    let keep_going = std::env::var("MOTE_STRESS_KEEP_GOING").is_ok_and(|v| v == "1");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let java_dir = std::env::var_os("MOTE_STRESS_JAVA")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../mote-lang-java/tests/fixtures"));
    let js_dir = std::env::var_os("MOTE_STRESS_JS")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../mote-lang-js/tests/fixtures"));
    let work = std::env::var_os("MOTE_FUZZ_WORKDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mote-fuzz"))
        .join(format!("cache-stress-{seed}"));
    let _ = std::fs::remove_dir_all(&work);
    let root = work.join("repo");
    let cache = work.join("cache");
    let alt_config = work.join("alt.yaml");
    std::fs::create_dir_all(&root).unwrap();

    let mut rng = Rng::new(seed);
    let rules = catalog();
    let mut java = sample(&java_dir, &["java"], per_lang * 2, &mut rng);
    let mut js = sample(&js_dir, JS_EXTS, per_lang * 2, &mut rng);
    let java_pool = java.split_off(java.len().min(per_lang));
    let js_pool = js.split_off(js.len().min(per_lang));
    let mut repo = Repo {
        root: root.clone(),
        files: Vec::new(),
        pool: java_pool.into_iter().chain(js_pool).collect(),
        commits: 0,
    };
    common::init_repo(&root);
    for (rel, bytes) in &java {
        let r = format!("java/{rel}");
        repo.write(&r, bytes);
        repo.files.push(r);
    }
    for (rel, bytes) in &js {
        let r = format!("js/{rel}");
        repo.write(&r, bytes);
        repo.files.push(r);
    }
    repo.write(
        "js/package.json",
        b"{\n  \"name\": \"fuzz-app\",\n  \"version\": \"1.0.0\"\n}\n",
    );
    let mut cfg = Cfg::initial();
    repo.write(".mote.yaml", cfg.render().as_bytes());
    // A second configuration that shares the cache directory.
    let mut alt = Cfg::initial();
    alt.extends = "all";
    alt.compat = "true".into();
    alt.skip_generated = false;
    std::fs::write(&alt_config, alt.render()).unwrap();
    common::commit_all(&root, "base");
    git(&root, &["checkout", "-q", "-b", "feature"]);
    println!(
        "cache stress: seed {seed}, {} Java + {} JS/TS files, {steps} steps, repo {}",
        java.len(),
        js.len(),
        root.display()
    );

    let cache_s = cache.to_string_lossy().into_owned();
    let alt_s = alt_config.to_string_lossy().into_owned();
    let t0 = Instant::now();
    let mut log = Vec::new();
    let mut divergences = Vec::new();
    let mut comparisons = 0u64;
    let mut compare = |label: &str,
                       step_no: u64,
                       desc: &str,
                       cached: &[&str],
                       fresh: &[&str],
                       divergences: &mut Vec<String>| {
        comparisons += 1;
        let a = scan(&root, cached);
        let b = scan(&root, fresh);
        match (a, b) {
            (Ok(a), Ok(b)) if a == b => {}
            (Err(ea), Err(eb)) if ea == eb => {}
            (a, b) => {
                let detail = match (&a, &b) {
                    (Ok(a), Ok(b)) => describe_diff(a, b),
                    _ => format!("cached: {:?}\n    fresh: {:?}", a.err(), b.err()),
                };
                let msg = format!("step {step_no} ({desc}) [{label}]:\n    {detail}");
                println!("DIVERGENCE {msg}");
                divergences.push(msg);
            }
        }
    };
    // Warm the cache first.
    compare(
        "worktree",
        0,
        "initial",
        &["--cache-dir", &cache_s],
        &["--no-cache"],
        &mut divergences,
    );
    for n in 1..=steps {
        let desc = step(&mut repo, &mut rng, &mut cfg, &rules);
        log.push(format!("{n}: {desc}"));
        if n % 20 == 0 {
            println!("step {n}/{steps} ({:.0?}): {desc}", t0.elapsed());
        }
        let before = divergences.len();
        compare(
            "worktree",
            n,
            &desc,
            &["--cache-dir", &cache_s],
            &["--no-cache"],
            &mut divergences,
        );
        if n % 3 == 0 {
            // The other configuration, same cache directory.
            if rng.chance(1, 3) {
                let d = mutate_config(&mut alt, &mut rng, &rules);
                log.push(format!("{n}: alt config: {d}"));
                std::fs::write(&alt_config, alt.render()).unwrap();
            }
            compare(
                "alt-config",
                n,
                &desc,
                &["--config", &alt_s, "--cache-dir", &cache_s],
                &["--config", &alt_s, "--no-cache"],
                &mut divergences,
            );
        }
        if n % 4 == 0 {
            // The default cache location inside the repository (git-ignored).
            compare(
                "default-cache",
                n,
                &desc,
                &[],
                &["--no-cache"],
                &mut divergences,
            );
        }
        if n % 5 == 0 {
            compare(
                "base",
                n,
                &desc,
                &["--base", "main", "--cache-dir", &cache_s],
                &["--base", "main", "--no-cache"],
                &mut divergences,
            );
        }
        if desc == "commit" {
            compare(
                "rev",
                n,
                &desc,
                &["--rev", "HEAD~1", "--cache-dir", &cache_s],
                &["--rev", "HEAD~1", "--no-cache"],
                &mut divergences,
            );
            compare(
                "rev-base",
                n,
                &desc,
                &["--rev", "HEAD", "--base", "main", "--cache-dir", &cache_s],
                &["--rev", "HEAD", "--base", "main", "--no-cache"],
                &mut divergences,
            );
        }
        if desc == "stage all" {
            compare(
                "staged",
                n,
                &desc,
                &["--staged", "--cache-dir", &cache_s],
                &["--staged", "--no-cache"],
                &mut divergences,
            );
        }
        if divergences.len() > before && !keep_going {
            break;
        }
    }
    std::fs::write(work.join("steps.log"), log.join("\n")).unwrap();
    println!(
        "cache stress: {} steps, {comparisons} comparisons, {} commits, {:.1?}; log {}",
        log.len(),
        repo.commits,
        t0.elapsed(),
        work.join("steps.log").display()
    );
    assert!(
        divergences.is_empty(),
        "{} divergences:\n{}",
        divergences.len(),
        divergences.join("\n")
    );
}
