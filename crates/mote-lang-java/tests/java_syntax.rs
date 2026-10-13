//! Java language levels: every fixture, and the Java 22–25 samples in `tests/syntax/`, parse
//! without a syntax error, and every rule runs on the samples without panicking.

use mote_core::testing::ruleset_all;
use mote_core::{FileAnalysis, LanguagePlugin, RuleSet, SourceFile};
use mote_lang_java::JavaPlugin;
use std::path::{Path, PathBuf};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn java_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "java"))
        .collect();
    files.sort();
    files
}

fn analyze(plugin: &JavaPlugin, path: &Path, rules: &RuleSet) -> FileAnalysis {
    let text = std::fs::read_to_string(path).expect("read");
    plugin.analyze(
        &SourceFile {
            path: "src/main/java/Sample.java",
            text: &text,
            is_test: false,
        },
        rules,
    )
}

/// Fixtures that contain syntax errors on purpose: invisible characters and Unicode escapes
/// outside literals, which the grammar does not accept (the rule reports them all the same).
const BROKEN_ON_PURPOSE: &[&str] = &["invisible-character.java", "invisible-character.bom.java"];

#[test]
fn fixtures_parse_without_syntax_errors() {
    let plugin = JavaPlugin::new();
    let none = RuleSet::new(Default::default());
    let mut broken = Vec::new();
    for path in java_files(&dir("fixtures")) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let a = analyze(&plugin, &path, &none);
        if a.syntax_errors > 0 && !BROKEN_ON_PURPOSE.contains(&name.as_str()) {
            broken.push(format!("{name}: {} syntax errors", a.syntax_errors));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn java_22_to_25_samples_parse_and_every_rule_runs() {
    let plugin = JavaPlugin::new();
    let all = ruleset_all(&plugin);
    let samples = java_files(&dir("syntax"));
    assert!(samples.len() >= 5, "{samples:?}");
    for path in samples {
        let a = analyze(&plugin, &path, &all);
        assert_eq!(a.syntax_errors, 0, "{}", path.display());
    }
}
