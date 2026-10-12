//! `mote init --from pmd|eslint [CONFIG]`: one starter configuration holding both init's own
//! proposals (excludes for copied third-party code, the compat sources counted in the code, every
//! setting commented) and the migration of the other tool's configuration (rules, severities,
//! thresholds, excludes, path-specific settings), instead of two files to merge by hand.
//!
//! Without CONFIG, the file is looked for in DIR: ESLint's configuration files in the order
//! ESLint itself looks for them, and for PMD the rulesets a `pom.xml` (maven-pmd-plugin
//! `<ruleset>`) or a Gradle build (`ruleSetFiles`) names, then a few conventional names. All of
//! them are read as data; nothing is run.

use crate::cli::MigrateFrom;
use crate::commands::migrate::{self, Proposal};
use crate::CliError;
use mote_engine::suppress::COMPAT_SOURCES;
use std::path::{Path, PathBuf};

/// ESLint's configuration files, in the order ESLint looks for them.
pub const ESLINT_FILES: &[&str] = &[
    "eslint.config.js",
    "eslint.config.mjs",
    "eslint.config.cjs",
    "eslint.config.ts",
    "eslint.config.mts",
    "eslint.config.cts",
    ".eslintrc.js",
    ".eslintrc.cjs",
    ".eslintrc.yaml",
    ".eslintrc.yml",
    ".eslintrc.json",
    ".eslintrc",
    "package.json",
];

/// Conventional names of a PMD ruleset, looked at after the ones a build file names.
pub const PMD_FILES: &[&str] = &[
    "pmd-ruleset.xml",
    "pmd.xml",
    "ruleset.xml",
    "config/pmd/ruleset.xml",
    "config/pmd/pmd.xml",
    "config/pmd/pmd-ruleset.xml",
    "config/pmd.xml",
    "config/pmd-ruleset.xml",
];

/// `--from TOOL [CONFIG]`: the tool and its configuration file (found in `dir` when not given).
pub fn resolve(values: &[String], dir: &Path) -> Result<(MigrateFrom, PathBuf), CliError> {
    let tool = match values.first().map(|t| t.to_ascii_lowercase()).as_deref() {
        Some("pmd") => MigrateFrom::Pmd,
        Some("eslint") => MigrateFrom::Eslint,
        other => {
            return Err(CliError::usage(format!(
                "--from {}: the tool is `pmd` or `eslint`",
                other.unwrap_or("")
            )))
        }
    };
    let name = values[0].to_ascii_lowercase();
    if let Some(file) = values.get(1) {
        let path = PathBuf::from(file);
        if path.is_dir() {
            return Err(CliError::usage(format!(
                "{file} is a directory: --from takes the tool and, optionally, its configuration \
                 file; the directory to write .mote.yaml into comes first: `mote init \
                 {file} --from {name}`"
            )));
        }
        if !path.is_file() {
            return Err(CliError::usage(format!("{file}: no such file")));
        }
        return Ok((tool, path));
    }
    let found = match tool {
        MigrateFrom::Eslint => eslint_config_in(dir).into_iter().collect::<Vec<_>>(),
        MigrateFrom::Pmd => pmd_rulesets_in(dir),
    };
    match found.as_slice() {
        // `./eslint.config.js` is `eslint.config.js` in messages and comments.
        [one] => Ok((
            tool,
            one.strip_prefix(".")
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| one.clone()),
        )),
        [] => Err(CliError::usage(match tool {
            MigrateFrom::Eslint => format!(
                "no ESLint configuration found in {} (looked for {}, and `eslintConfig` in \
                 package.json): name it, `mote init --from eslint <file>`",
                dir.display(),
                ESLINT_FILES[..ESLINT_FILES.len() - 1].join(", ")
            ),
            MigrateFrom::Pmd => format!(
                "no PMD ruleset found in {} (looked at the <ruleset> entries of pom.xml, \
                 `ruleSetFiles` in build.gradle(.kts), and {}): name it, `mote init --from \
                 pmd <ruleset.xml>`",
                dir.display(),
                PMD_FILES.join(", ")
            ),
        })),
        several => Err(CliError::usage(format!(
            "several PMD rulesets found in {}: {}; name the one to migrate, `mote init \
             --from pmd <ruleset.xml>`",
            dir.display(),
            several
                .iter()
                .map(|p| shown(p, dir))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// `path` as written in messages: relative to `dir` when below it, with `/`.
pub fn shown(path: &Path, dir: &Path) -> String {
    path.strip_prefix(dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The ESLint configuration ESLint would use in `dir`.
fn eslint_config_in(dir: &Path) -> Option<PathBuf> {
    ESLINT_FILES.iter().map(|n| dir.join(n)).find(|p| {
        if !p.is_file() {
            return false;
        }
        if p.file_name().is_some_and(|n| n == "package.json") {
            return std::fs::read_to_string(p)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .is_some_and(|v| v.get("eslintConfig").is_some());
        }
        true
    })
}

/// Whether `path` is a PMD ruleset file.
fn is_ruleset(path: &Path) -> bool {
    path.is_file() && std::fs::read_to_string(path).is_ok_and(|t| t.contains("<ruleset"))
}

/// The text of each `<tag>…</tag>` element in `text`.
fn elements<'t>(text: &'t str, tag: &str) -> Vec<&'t str> {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(&close) else {
            break;
        };
        out.push(after[..end].trim());
        rest = &after[end + close.len()..];
    }
    out
}

/// The quoted strings ending in `.xml` on the lines of a Gradle build that mention
/// `ruleSetFiles`.
fn gradle_rulesets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| l.contains("ruleSetFiles")) {
        for (i, part) in line.split(['"', '\'']).enumerate() {
            if i % 2 == 1 && part.ends_with(".xml") {
                out.push(part.to_string());
            }
        }
    }
    out
}

/// The PMD rulesets of the project in `dir`: those its build names, else conventional names.
fn pmd_rulesets_in(dir: &Path) -> Vec<PathBuf> {
    let mut named: Vec<String> = Vec::new();
    if let Ok(pom) = std::fs::read_to_string(dir.join("pom.xml")) {
        for r in elements(&pom, "ruleset") {
            let r = r
                .trim_start_matches("${basedir}/")
                .trim_start_matches("${project.basedir}/");
            named.push(r.to_string());
        }
    }
    for build in ["build.gradle", "build.gradle.kts"] {
        if let Ok(text) = std::fs::read_to_string(dir.join(build)) {
            named.extend(gradle_rulesets(&text));
        }
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for n in named {
        // PMD's own rulesets (`/category/java/bestpractices.xml`) are not files of the project.
        let p = dir.join(n.trim_start_matches('/'));
        if !n.contains("${") && is_ruleset(&p) && !found.contains(&p) {
            found.push(p);
        }
    }
    if found.is_empty() {
        found = PMD_FILES
            .iter()
            .map(|n| dir.join(n))
            .filter(|p| is_ruleset(p))
            .collect();
    }
    found
}

/// The rule examples of the starter, replaced by the migrated rules.
pub const RULES_EXAMPLE: &str = "# rules:
#   java/empty-catch-block: off
#   js/no-console: { severity: HIGH }
#   java/cognitive-complexity: { params: { threshold: 20 } }
#   \"js/*\": { enabled: false }
";

/// The override example of the starter, replaced by the migrated overrides.
pub const OVERRIDES_EXAMPLE: &str = "# overrides:
#   - paths: [\"legacy/**\"]
#     rules: { java/cognitive-complexity: { params: { threshold: 30 } } }
";

/// The `compat` source of the migrated tool, when the code's markers did not already propose it.
pub fn compat_source(p: &Proposal, proposed: &[&str]) -> Option<&'static str> {
    let tool = migrate::tool(p);
    COMPAT_SOURCES
        .iter()
        .copied()
        .find(|s| *s == tool)
        .filter(|s| !proposed.contains(s))
}

/// The starter `starter` (from [`super::starter_for`]) with the migration `p` merged in: its
/// excludes after the starter's own, its rules and overrides in place of the commented examples,
/// and what it could not carry over listed at the end.
pub fn merge(starter: &str, p: &Proposal) -> String {
    let mut text = super::comment_lines(
        &format!(
            "Starter written by `mote init --from {}`: besides init's own proposals, it holds \
             the rules, severities, thresholds and excludes migrated from that file. They are \
             mapped by intent, not identical behavior: review them (docs/migration.md), and the \
             list of what was not carried over at the end.",
            p.source.replace(['\n', '\r'], " ")
        ),
        "# ",
        "# ",
        100,
    );
    text.push_str(starter);
    let own: Vec<&str> = super::STARTER_EXCLUDES.to_vec();
    let lines = migrate::exclude_lines(p, &own);
    if !lines.is_empty() {
        let block = format!("{}{lines}", migrate::exclude_warning(p, "  "));
        text = text.replacen(
            super::EXCLUDE_LAST,
            &format!("{}{block}", super::EXCLUDE_LAST),
            1,
        );
    }
    let rules = migrate::rules_block(p);
    if !rules.is_empty() {
        text = text.replacen(RULES_EXAMPLE, &rules, 1);
    }
    let overrides = migrate::overrides_block(p);
    if !overrides.is_empty() {
        text = text.replacen(OVERRIDES_EXAMPLE, &overrides, 1);
    }
    text.push('\n');
    text.push_str(&format!(
        "# Migrated from {}:\n",
        p.source.replace(['\n', '\r'], " ")
    ));
    text.push_str(&migrate::unmapped_comments(p));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_files_name_rulesets() {
        let pom = "<plugin><configuration><rulesets>\n  <ruleset>${basedir}/src/conf/pmd.xml</ruleset>\n  <ruleset>/category/java/bestpractices.xml</ruleset>\n</rulesets></configuration></plugin>";
        assert_eq!(
            elements(pom, "ruleset"),
            vec![
                "${basedir}/src/conf/pmd.xml",
                "/category/java/bestpractices.xml"
            ]
        );
        let gradle =
            "pmd {\n    ruleSetFiles = files(\"config/pmd/team.xml\")\n    ruleSets = []\n}\n";
        assert_eq!(gradle_rulesets(gradle), vec!["config/pmd/team.xml"]);
        assert_eq!(
            gradle_rulesets("ruleSetFiles = files('a.xml', 'b.xml')"),
            vec!["a.xml", "b.xml"]
        );
    }

    #[test]
    fn examples_are_in_the_starter() {
        assert!(super::super::STARTER.contains(RULES_EXAMPLE));
        assert!(super::super::STARTER.contains(OVERRIDES_EXAMPLE));
    }
}
