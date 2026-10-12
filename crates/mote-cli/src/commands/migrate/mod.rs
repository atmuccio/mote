//! `mote migrate --from pmd|eslint FILE`: propose a `.mote.yaml` from another tool's
//! configuration.
//!
//! The configuration is read as data (XML, JSON, YAML); JavaScript configuration files are never
//! run, so `eslint.config.js` and friends are refused with a hint. Rules are mapped by intent
//! through explicit tables (PMD: [`mote_lang_java::pmd`]; ESLint: same name, or [`eslint`]'s
//! exceptions) and every rule that could not be mapped is listed. The proposal enables the mapped
//! rules on top of Mote's default profile, turns off the ones the source disables, carries over
//! file excludes it can translate, and selects the tool's `suppressions.compat` source (`pmd`,
//! `eslint`) so the suppression comments and annotations already in the code keep working; the
//! other sources are listed in a comment. It is printed; `-o FILE` writes it (refusing to
//! overwrite without `--force`). The proposal is validated like any configuration before it is
//! shown.

pub(crate) mod eslint;
mod js_config;
mod pmd;

use crate::cli::{MigrateArgs, MigrateFrom};
use crate::util::write_file;
use crate::{exit, plugins, CliError, CliResult, Io};
use mote_core::Severity;
use mote_engine::{Config, Registry};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// How a rule is set in the proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Off,
    On,
    Severity(Severity),
}

impl Level {
    /// Combine two settings of one Mote rule (several source rules can map to it): enabling
    /// wins over disabling, and the higher severity wins.
    fn merge(self, other: Level) -> Level {
        match (self, other) {
            (Level::Off, x) | (x, Level::Off) => x,
            (Level::Severity(a), Level::Severity(b)) => Level::Severity(a.max(b)),
            (Level::Severity(s), Level::On) | (Level::On, Level::Severity(s)) => Level::Severity(s),
            (Level::On, Level::On) => Level::On,
        }
    }

    fn yaml(self) -> &'static str {
        match self {
            Level::Off => "off",
            Level::On => "on",
            Level::Severity(s) => s.as_str(),
        }
    }
}

/// One rule of the proposal and where it came from.
#[derive(Clone, Debug)]
pub struct Setting {
    pub level: Level,
    /// Parameters carried over (thresholds).
    pub params: BTreeMap<&'static str, i64>,
    /// Source rule names (and notes) behind it.
    pub from: Vec<String>,
}

/// A layer of rule settings.
#[derive(Clone, Debug, Default)]
pub struct Rules(pub BTreeMap<String, Setting>);

impl Rules {
    /// Set `id` to `level` because of `from`; merged with an earlier setting of the same id.
    pub fn set(&mut self, id: &str, level: Level, from: String) {
        match self.0.get_mut(id) {
            Some(s) => {
                s.level = s.level.merge(level);
                if !s.from.contains(&from) {
                    s.from.push(from);
                }
            }
            None => {
                self.0.insert(
                    id.to_string(),
                    Setting {
                        level,
                        params: BTreeMap::new(),
                        from: vec![from],
                    },
                );
            }
        }
    }

    /// Set a parameter of `id` (which must have been [`set`](Self::set)).
    pub fn param(&mut self, id: &str, name: &'static str, value: i64) {
        if let Some(s) = self.0.get_mut(id) {
            s.params.insert(name, value);
        }
    }

    /// Add a note to the sources of `id` (which must have been [`set`](Self::set)).
    pub fn note(&mut self, id: &str, note: String) {
        if let Some(s) = self.0.get_mut(id) {
            if !s.from.contains(&note) {
                s.from.push(note);
            }
        }
    }
}

/// A path-specific layer (`overrides`).
#[derive(Clone, Debug)]
pub struct OverrideLayer {
    pub paths: Vec<String>,
    pub rules: Rules,
    pub from: String,
}

/// The proposed configuration and what could not be carried over.
#[derive(Clone, Debug, Default)]
pub struct Proposal {
    /// `pmd ruleset.xml`, `eslint .eslintrc.json`.
    pub source: String,
    pub rules: Rules,
    pub overrides: Vec<OverrideLayer>,
    /// Globs with the source pattern they come from.
    pub exclude: Vec<(String, String)>,
    /// Everything that was not carried over, one line each.
    pub unmapped: Vec<String>,
    /// Source rules looked at, and how many of them were mapped.
    pub seen: usize,
    pub mapped: usize,
    /// The `suppressions.compat` sources of the tool (empty: `true`).
    pub compat: &'static [&'static str],
    /// A note about the tool's other suppressions, for the `compat` comment.
    pub compat_note: &'static str,
    /// A note heading the list of what was not mapped (what the mapping covers).
    pub unmapped_note: &'static str,
    /// Layout rules turned on in the source, listed together as not mapped (ESLint).
    pub formatting: std::collections::BTreeSet<String>,
}

impl Proposal {
    fn new(source: String) -> Proposal {
        Proposal {
            source,
            ..Default::default()
        }
    }

    fn unmapped(&mut self, what: String) {
        if !self.unmapped.contains(&what) {
            self.unmapped.push(what);
        }
    }
}

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn comment(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

fn write_rules(s: &mut String, rules: &Rules, indent: &str) {
    for (id, r) in &rules.0 {
        let value = if r.params.is_empty() || r.level == Level::Off {
            r.level.yaml().to_string()
        } else {
            let params: Vec<String> = r.params.iter().map(|(k, v)| format!("{k}: {v}")).collect();
            let severity = match r.level {
                Level::Severity(sev) => format!("severity: {}, ", sev.as_str()),
                _ => String::new(),
            };
            format!("{{ {severity}params: {{ {} }} }}", params.join(", "))
        };
        let _ = writeln!(
            s,
            "{indent}{id}: {value:<7} # {}",
            comment(&r.from.join(", "))
        );
    }
}

/// The tool of a proposal: `pmd`, `eslint`.
pub fn tool(p: &Proposal) -> &str {
    p.source.split(' ').next().unwrap_or("")
}

/// The comment above excludes from ESLint's ignores (empty for other tools): ESLint only ever
/// looked at JavaScript and TypeScript files.
pub fn exclude_warning(p: &Proposal, indent: &str) -> String {
    if tool(p) != "eslint" {
        return String::new();
    }
    format!(
        "{indent}# From ESLint's ignores, which only concerned JavaScript and TypeScript files:\n\
         {indent}# `exclude` applies to every language, so check that none of these hides other code.\n"
    )
}

/// The proposal's `exclude` entries, one line each (`  - "glob"  # source`), but those in `skip`.
pub fn exclude_lines(p: &Proposal, skip: &[&str]) -> String {
    let mut s = String::new();
    for (glob, from) in &p.exclude {
        if !skip.contains(&glob.as_str()) {
            let _ = writeln!(s, "  - {}  # {}", q(glob), comment(from));
        }
    }
    s
}

/// The proposal's `rules:` block (empty without rules).
pub fn rules_block(p: &Proposal) -> String {
    let mut s = String::new();
    if !p.rules.0.is_empty() {
        s.push_str("rules:\n");
        write_rules(&mut s, &p.rules, "  ");
    }
    s
}

/// The proposal's `overrides:` block (empty without overrides).
pub fn overrides_block(p: &Proposal) -> String {
    let mut s = String::new();
    if !p.overrides.is_empty() {
        s.push_str("overrides:\n");
        for o in &p.overrides {
            let paths: Vec<String> = o.paths.iter().map(|x| q(x)).collect();
            let _ = writeln!(
                s,
                "  - paths: [{}]  # {}",
                paths.join(", "),
                comment(&o.from)
            );
            if o.rules.0.is_empty() {
                s.push_str("    rules: {}\n");
            } else {
                s.push_str("    rules:\n");
                write_rules(&mut s, &o.rules, "      ");
            }
        }
    }
    s
}

/// What was not carried over, as comment lines (or that everything was).
pub fn unmapped_comments(p: &Proposal) -> String {
    let mut s = String::new();
    if p.unmapped.is_empty() {
        s.push_str("# Everything was mapped.\n");
    } else {
        let _ = writeln!(s, "# Not mapped ({}):", p.unmapped.len());
        if !p.unmapped_note.is_empty() {
            let _ = writeln!(s, "#   ({})", comment(p.unmapped_note));
        }
        for u in &p.unmapped {
            let _ = writeln!(s, "#   - {}", comment(u));
        }
    }
    s
}

/// The proposal as `.mote.yaml` text, with comments saying where each setting comes from
/// and listing what was not mapped.
pub fn render(p: &Proposal) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# Proposed by `mote migrate --from {}`.\n\
         # Review before use: rules are mapped by intent, not identical behavior (docs/migration.md).",
        comment(&p.source)
    );
    s.push_str("mote: \">=0.1\"\nextends: default\n");
    if !p.exclude.is_empty() {
        s.push_str(&exclude_warning(p, ""));
        s.push_str("exclude:\n");
        s.push_str(&exclude_lines(p, &[]));
    }
    s.push_str(&rules_block(p));
    s.push_str(&overrides_block(p));
    s.push_str("suppressions:\n");
    if p.compat.is_empty() {
        s.push_str("  compat: true  # honor the suppression comments and annotations of the tools you used\n");
    } else {
        let others: Vec<&str> = mote_engine::suppress::COMPAT_SOURCES
            .iter()
            .filter(|src| !p.compat.contains(src))
            .copied()
            .collect();
        let tool = p.source.split(' ').next().unwrap_or("");
        let _ = writeln!(
            s,
            "  compat: [{}]  # honor the suppression comments and annotations {tool} reads",
            p.compat.join(", ")
        );
        let note = if p.compat_note.is_empty() {
            String::new()
        } else {
            format!("; {}", p.compat_note)
        };
        let _ = writeln!(
            s,
            "  # other tools' markers in your code? add their sources: {}{note} (docs/migration.md)",
            others.join(", ")
        );
    }
    s.push_str(&unmapped_comments(p));
    s
}

/// Check the proposal against the rule registry (a failure is a bug in the mapping).
pub fn validate(text: &str, registry: &Registry) -> Result<(), CliError> {
    let config = Config::from_yaml_str(text).map_err(|e| {
        CliError::internal(format!("the proposed configuration does not parse: {e}"))
    })?;
    config
        .validate(registry)
        .map_err(|e| CliError::internal(format!("the proposed configuration is invalid: {e}")))
}

/// Read `file`, another tool's configuration, and propose Mote settings for it, with globs
/// relative to `root`, the directory of the `.mote.yaml` the proposal is for.
pub fn propose(
    from: MigrateFrom,
    file: &std::path::Path,
    root: &std::path::Path,
    registry: &Registry,
) -> Result<Proposal, CliError> {
    let name = file.to_string_lossy().replace('\\', "/");
    match from {
        MigrateFrom::Pmd => pmd::propose(file, &name, registry),
        MigrateFrom::Eslint => eslint::propose(file, &name, root, registry),
    }
}

/// `mapped 3 of 5 rules; 2 items not mapped`.
pub fn summary(p: &Proposal) -> String {
    format!(
        "mapped {} of {} rules; {} item{} not mapped",
        p.mapped,
        p.seen,
        p.unmapped.len(),
        if p.unmapped.len() == 1 { "" } else { "s" }
    )
}

pub fn run(args: &MigrateArgs, quiet: bool, io: &mut Io<'_>) -> CliResult {
    let registry = plugins::registry()?;
    // The proposal is for the directory it is written to (the current one when printed).
    let root = match args.output.as_deref().and_then(std::path::Path::parent) {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let proposal = propose(args.from, &args.file, &root, &registry)?;
    let text = render(&proposal);
    validate(&text, &registry)?;
    let summary = summary(&proposal);
    let tool = tool(&proposal);
    match &args.output {
        Some(out) => {
            if out.exists() && !args.force {
                return Err(CliError::usage(format!(
                    "{} exists; use --force to overwrite it, or write the proposal to another \
                     file and copy its `rules`, `exclude` and `overrides` into yours (a starter \
                     from `mote init` holds proposals of its own: third-party excludes and \
                     compat sources). `mote init --force --from {tool} {}` writes one \
                     starter with both",
                    out.display(),
                    args.file.display()
                )));
            }
            write_file(out, text.as_bytes())?;
            let _ = writeln!(io.out, "Wrote {}: {summary}.", out.display());
            if !proposal.unmapped.is_empty() {
                let _ = writeln!(io.out, "Not mapped:");
                if !proposal.unmapped_note.is_empty() {
                    let _ = writeln!(io.out, "  ({})", proposal.unmapped_note);
                }
                for u in &proposal.unmapped {
                    let _ = writeln!(io.out, "  - {u}");
                }
            }
        }
        None => {
            io.out
                .write_all(text.as_bytes())
                .map_err(|e| CliError::internal(format!("cannot write to stdout: {e}")))?;
            if !quiet {
                let _ = writeln!(io.err, "{summary} (listed at the end)");
                if let Some(existing) = mote_engine::config::CONFIG_FILE_NAMES
                    .iter()
                    .find(|n| std::path::Path::new(n).is_file())
                {
                    let _ = writeln!(
                        io.err,
                        "note: {existing} exists here: copy the proposal's `rules`, `exclude` \
                         and `overrides` into it (and add its `compat` source if missing), or \
                         write the proposal to a file with -o; `mote init --force --from \
                         {tool} {}` replaces it with a starter holding both",
                        args.file.display()
                    );
                }
            }
        }
    }
    Ok(exit::OK)
}

/// Translate a gitignore-style or glob pattern (ESLint ignores) into a Mote exclude glob.
/// `None` for negations, which an exclude list cannot express.
pub fn ignore_to_glob(pattern: &str) -> Option<String> {
    let p = pattern.trim();
    if p.is_empty() || p.starts_with('#') || p.starts_with('!') {
        return None;
    }
    let anchored = p.starts_with('/');
    let mut body = p.trim_start_matches('/').to_string();
    let dir = body.ends_with('/');
    if dir {
        body = body.trim_end_matches('/').to_string();
    }
    if body.is_empty() {
        return None;
    }
    let mut glob = if anchored || body.contains('/') || body.starts_with("**") {
        body
    } else {
        format!("**/{body}")
    };
    // A directory (or a bare name, which may be one) covers everything below it.
    if dir
        || !glob
            .rsplit('/')
            .next()
            .unwrap_or("")
            .contains(['*', '.', '?', '['])
    {
        glob.push_str("/**");
    }
    Some(glob)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_merge_towards_enabled_and_severe() {
        use Level::*;
        assert_eq!(Off.merge(On), On);
        assert_eq!(On.merge(Off), On);
        assert_eq!(Off.merge(Off), Off);
        assert_eq!(
            Severity(mote_core::Severity::Low).merge(Severity(mote_core::Severity::High)),
            Severity(mote_core::Severity::High)
        );
        assert_eq!(
            On.merge(Severity(mote_core::Severity::Low)),
            Severity(mote_core::Severity::Low)
        );
    }

    #[test]
    fn ignore_patterns_to_globs() {
        let cases = [
            ("dist/", Some("**/dist/**")),
            ("/build", Some("build/**")),
            ("node_modules", Some("**/node_modules/**")),
            ("*.min.js", Some("**/*.min.js")),
            ("src/generated/**", Some("src/generated/**")),
            ("**/vendor/*.js", Some("**/vendor/*.js")),
            ("coverage/", Some("**/coverage/**")),
            ("!keep.js", None),
            ("", None),
        ];
        for (p, want) in cases {
            assert_eq!(ignore_to_glob(p).as_deref(), want, "{p}");
        }
    }

    /// `(name, equiv)` of the entries of a top-level section of the category map.
    fn equivs(yaml: &str, section: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut inside = false;
        for line in yaml.lines() {
            if !line.starts_with(' ') && !line.starts_with('#') && !line.is_empty() {
                inside = line.trim_end() == format!("{section}:");
                continue;
            }
            if !inside {
                continue;
            }
            let t = line.trim();
            let (Some((name, rest)), Some(eq)) = (t.split_once(": {"), t.find("equiv: ")) else {
                continue;
            };
            let _ = rest;
            let equiv: String = t[eq + "equiv: ".len()..]
                .chars()
                .take_while(|c| !matches!(c, ',' | ' ' | '}'))
                .collect();
            out.push((name.trim_matches('"').to_string(), equiv));
        }
        out
    }

    /// The mapping tables follow the `equiv` links of the maintainers' category map (part of the
    /// coverage benchmark, which is not in every checkout: without it this test only reports that
    /// it was skipped; `tables_name_existing_rules` still runs). When this fails after a rule was
    /// added or its category entry changed, update `crates/mote-lang-java/src/pmd.rs`
    /// (`PMD_EQUIVALENTS`) or, for an ESLint rule whose equivalent has another name,
    /// `eslint::EXCEPTIONS`.
    #[test]
    fn tables_follow_the_category_map() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/coverage/categories.yaml");
        let Ok(yaml) = std::fs::read_to_string(&path) else {
            eprintln!("tables_follow_the_category_map: skipped: category map not present");
            return;
        };
        let registry = crate::plugins::registry().unwrap();
        let eslint_map = equivs(&yaml, "eslint");
        assert!(eslint_map.len() > 20, "parsed the eslint section");
        for (name, equiv) in &eslint_map {
            assert_eq!(
                eslint::mote_rule(name, &registry).as_deref(),
                Some(equiv.as_str()),
                "ESLint `{name}` should map to `{equiv}` (category map)"
            );
        }
        for (name, id) in eslint::EXCEPTIONS {
            assert!(
                eslint_map.iter().any(|(n, e)| n == name && e == id),
                "eslint::EXCEPTIONS `{name}` → `{id}` has no matching `equiv` in the category map"
            );
        }
        #[cfg(feature = "java")]
        {
            use mote_lang_java::pmd::{PMD_EQUIVALENTS, PMD_MIGRATION_EQUIVALENTS};
            for (section, table, table_name) in [
                ("pmd", PMD_EQUIVALENTS, "PMD_EQUIVALENTS"),
                (
                    "pmd-migration",
                    PMD_MIGRATION_EQUIVALENTS,
                    "PMD_MIGRATION_EQUIVALENTS",
                ),
            ] {
                let pmd_map = equivs(&yaml, section);
                assert!(pmd_map.len() > 20, "parsed the {section} section");
                for (name, equiv) in &pmd_map {
                    assert!(
                        table.iter().any(|(n, e)| n == name && e == equiv),
                        "PMD `{name}` → `{equiv}` (category map, {section}) is missing from {table_name}"
                    );
                }
                for (name, id) in table {
                    assert!(
                        pmd_map.iter().any(|(n, e)| n == name && e == id),
                        "{table_name} `{name}` → `{id}` has no matching `equiv` in the {section} section of the category map"
                    );
                }
            }
        }
    }

    /// The parts of the mapping tables that need no category map: every target is a rule, and
    /// each carried-over PMD property names a parameter of that rule.
    #[test]
    fn tables_name_existing_rules() {
        let registry = crate::plugins::registry().unwrap();
        #[cfg(feature = "js")]
        for (name, id) in eslint::EXCEPTIONS {
            assert!(registry.rule(id).is_some(), "{name} → {id}: no such rule");
        }
        #[cfg(feature = "java")]
        {
            use mote_lang_java::pmd::{PMD_EQUIVALENTS, PMD_MIGRATION_EQUIVALENTS};
            for (name, id) in PMD_EQUIVALENTS.iter().chain(PMD_MIGRATION_EQUIVALENTS) {
                assert!(registry.rule(id).is_some(), "{name} → {id}: no such rule");
            }
            // Each carried-over property names a parameter of the rule its PMD rule maps to.
            for prop in pmd::PROPERTIES {
                let id = mote_lang_java::pmd::mote_rule_for_pmd(prop.rule)
                    .unwrap_or_else(|| panic!("{}: not mapped", prop.rule));
                let meta = &registry.rule(id).expect("a rule").meta;
                assert!(
                    meta.params.iter().any(|p| p.name == prop.param),
                    "{}.{} → {id} has no parameter `{}`",
                    prop.rule,
                    prop.property,
                    prop.param
                );
                assert!(prop.pmd_default + prop.offset >= 0);
            }
            assert!(pmd::PROPERTIES
                .windows(2)
                .all(|w| (w[0].rule, w[0].property) < (w[1].rule, w[1].property)));
        }
    }

    #[test]
    fn rendering_is_valid_yaml_with_sources() {
        let mut p = Proposal::new("pmd ruleset.xml".into());
        p.rules.set(
            "java/empty-catch-block",
            Level::On,
            "EmptyCatchBlock".into(),
        );
        p.rules.set(
            "java/local-variable-naming",
            Level::Off,
            "excluded: LocalVariableNamingConventions".into(),
        );
        p.rules.set(
            "java/local-variable-naming",
            Level::On,
            "FormalParameterNamingConventions".into(),
        );
        p.exclude
            .push(("**/generated/**".into(), ".*/generated/.*".into()));
        p.unmapped.push("AvoidDuplicateLiterals".into());
        let text = render(&p);
        assert!(text.contains("  java/local-variable-naming: on      # excluded: LocalVariableNamingConventions, FormalParameterNamingConventions"), "{text}");
        assert!(text.contains("#   - AvoidDuplicateLiterals"), "{text}");
        let c = Config::from_yaml_str(&text).unwrap();
        assert_eq!(c.exclude, vec!["**/generated/**".to_string()]);
        assert_eq!(
            c.suppressions.compat,
            mote_engine::config::CompatSetting::Flag(true),
            "a proposal without a tool keeps every source of `true`"
        );
        assert_eq!(c.rules.len(), 2);
    }
}
