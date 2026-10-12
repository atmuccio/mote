//! Other tools' suppressions already in a repository, for `mote init`: counted per
//! `suppressions.compat` source, and the sources worth honoring proposed.
//!
//! Files are the ones a scan would analyze: [`discovered_paths`](super::third_party::discovered_paths)
//! handled by a language plugin, no larger than `max_file_size`, and — with `skip_generated` —
//! not generated, minified or vendored (unless `skip_generated_except` matches). What is counted,
//! the way the plugins recognize it:
//!
//! * `eslint` — comments starting with `eslint-disable`, `eslint-disable-line` or
//!   `eslint-disable-next-line` (JavaScript, TypeScript);
//! * `nosonar` — comments containing `NOSONAR` (every language);
//! * `pmd` — comments with the word `NOPMD` on their first line, `@SuppressWarnings` naming `PMD`
//!   or `PMD.<rule>` (Java);
//! * `checkstyle` — comments containing `CHECKSTYLE:OFF` (Java);
//! * `suppress-warnings-all` — `@SuppressWarnings("all")` (Java);
//! * `spotbugs` — `@SuppressFBWarnings` (Java);
//! * `intellij` — `//noinspection` comments, and `@SuppressWarnings` naming an IntelliJ inspection
//!   of the table the plugin maps (`ALL` included) (Java).
//!
//! Each comment or annotation counts once per source.
//!
//! **Which sources are proposed.** A source is proposed as soon as the repository holds one of its
//! markers: a `NOPMD`, an `eslint-disable`, a `@SuppressFBWarnings` exists only because someone
//! used that tool, and honoring it silences only what that marker was written to silence.
//! `intellij` is the exception: it takes at least [`MIN_INTELLIJ_MARKERS`] IntelliJ-only markers
//! (`//noinspection` comments and `@SuppressWarnings` naming IntelliJ inspections, together).
//! Honoring it also honors the keys javac and the Eclipse compiler define and IntelliJ accepts
//! (`unused`, `rawtypes`, `fallthrough`, `finally`, `resource`), which most Java code bases
//! carry by the dozen whatever the IDE — so one stray comment must not decide for all of them,
//! and a lone capitalized `@SuppressWarnings` name is weak evidence anyway (other checkers'
//! names look the same). Those keys are counted apart and never count towards `intellij`. On the
//! 39 benchmark repositories, the threshold proposes `intellij` for the seven that use IntelliJ's
//! suppressions (3 to 129 markers, six of them 7 or more) and not for the six with one or two
//! strays (a test's `//noinspection`, a `@SuppressWarnings("SimplifiableIfStatement")` in copied
//! code) next to up to 213 javac/Eclipse keys.

use mote_engine::analyze::{self, SkipReason};
use mote_engine::lexer::{self, LexOptions};
use mote_engine::packages::PackageNames;
use mote_engine::suppress::COMPAT_SOURCES;
use mote_engine::Registry;
use std::collections::BTreeMap;
use std::path::Path;

/// IntelliJ-only markers (`//noinspection` comments, `@SuppressWarnings` naming IntelliJ
/// inspections) needed to propose `intellij`.
pub const MIN_INTELLIJ_MARKERS: usize = 3;

/// `@SuppressWarnings` keys of javac and the Eclipse compiler that IntelliJ also accepts (and
/// the `intellij` source therefore honors): no sign of IntelliJ by themselves.
pub const SHARED_KEYS: [&str; 5] = ["fallthrough", "finally", "rawtypes", "resource", "unused"];

/// Names of SpotBugs' suppression annotation (as the Java plugin recognizes them).
const SPOTBUGS_ANNOTATIONS: &[&str] = &[
    "SuppressFBWarnings",
    "edu.umd.cs.findbugs.annotations.SuppressFBWarnings",
    "com.github.spotbugs.annotations.SuppressFBWarnings",
    "edu.umd.cs.findbugs.annotations.SuppressWarnings",
];

/// Occurrences of one kind of marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub occurrences: usize,
    pub files: usize,
}

impl Tally {
    fn add(&mut self, n: usize) {
        if n > 0 {
            self.occurrences += n;
            self.files += 1;
        }
    }

    fn merge(&mut self, o: Tally) {
        self.occurrences += o.occurrences;
        self.files += o.files;
    }
}

/// What a repository uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Per source of [`COMPAT_SOURCES`] (sources without markers are left out).
    pub sources: BTreeMap<&'static str, Tally>,
    /// `intellij`: `//noinspection` comments …
    pub intellij_comments: Tally,
    /// … and `@SuppressWarnings` naming IntelliJ inspections (other than [`SHARED_KEYS`]).
    pub intellij_annotations: Tally,
    /// `@SuppressWarnings` with one of [`SHARED_KEYS`] (and no IntelliJ-only name).
    pub shared: Tally,
    /// Occurrences of each of [`SHARED_KEYS`].
    pub shared_keys: BTreeMap<&'static str, usize>,
}

impl Usage {
    /// Whether no marker was found at all.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty() && self.shared.occurrences == 0
    }

    /// The sources to honor, in [`COMPAT_SOURCES`] order.
    pub fn proposed(&self) -> Vec<&'static str> {
        COMPAT_SOURCES
            .iter()
            .copied()
            .filter(|s| match *s {
                "intellij" => {
                    self.intellij_comments.occurrences + self.intellij_annotations.occurrences
                        >= MIN_INTELLIJ_MARKERS
                }
                _ => self.sources.contains_key(s),
            })
            .collect()
    }

    fn merge(&mut self, o: Usage) {
        for (s, t) in o.sources {
            self.sources.entry(s).or_default().merge(t);
        }
        self.intellij_comments.merge(o.intellij_comments);
        self.intellij_annotations.merge(o.intellij_annotations);
        self.shared.merge(o.shared);
        for (k, n) in o.shared_keys {
            *self.shared_keys.entry(k).or_default() += n;
        }
    }

    /// Lines describing what was found and what is not proposed (for the starter's comment and
    /// the `--suggest-compat` listing).
    pub fn notes(&self) -> Vec<String> {
        let mut out = Vec::new();
        let intellij = self.sources.get("intellij").copied().unwrap_or_default();
        if intellij.occurrences > 0 && !self.proposed().contains(&"intellij") {
            out.push(format!(
                "Not proposed: intellij, with {} //noinspection comment{} and {} @SuppressWarnings \
                 naming IntelliJ inspections (at least {MIN_INTELLIJ_MARKERS} markers are needed: \
                 intellij also honors the javac/Eclipse keys)",
                self.intellij_comments.occurrences,
                if self.intellij_comments.occurrences == 1 { "" } else { "s" },
                self.intellij_annotations.occurrences
            ));
        }
        if self.shared.occurrences > 0 {
            let keys: Vec<String> = self
                .shared_keys
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect();
            out.push(format!(
                "{} @SuppressWarnings with javac/Eclipse keys ({}): every IDE writes them, so they \
                 never propose intellij on their own{}",
                self.shared.occurrences,
                keys.join(", "),
                if self.proposed().contains(&"intellij") {
                    "; intellij honors them too"
                } else {
                    ""
                }
            ));
        }
        out
    }
}

/// What decides which files are counted: the settings of the configuration in force (the
/// starter's for a new one).
pub struct Filter {
    pub max_file_size: u64,
    pub skip_generated: bool,
    pub skip_generated_except: globset::GlobSet,
}

impl Filter {
    /// From a configuration.
    pub fn of(config: &mote_engine::Config) -> Filter {
        Filter {
            max_file_size: config.max_file_size,
            skip_generated: config.skip_generated,
            skip_generated_except: mote_engine::config::compile_path_globs(
                &config.skip_generated_except,
            ),
        }
    }
}

/// Count the markers in `paths` (below `root`) that a plugin of `registry` handles.
pub fn count(root: &Path, paths: &[String], registry: &Registry, filter: &Filter) -> Usage {
    let files: Vec<(&str, usize)> = paths
        .iter()
        .filter_map(|p| Some((p.as_str(), registry.plugin_for_path(p)?)))
        .collect();
    let packages = PackageNames::disk(root);
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 16);
    let chunk = files.len().div_ceil(threads).max(1);
    let mut usage = Usage::default();
    std::thread::scope(|s| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|part| {
                let packages = &packages;
                s.spawn(move || {
                    let mut u = Usage::default();
                    for &(path, plugin) in part {
                        if let Some(text) = read(root, path, filter, packages) {
                            count_file(&mut u, registry, plugin, &text);
                        }
                    }
                    u
                })
            })
            .collect();
        for h in handles {
            if let Ok(u) = h.join() {
                usage.merge(u);
            }
        }
    });
    usage
}

/// The text of a file a scan would analyze (`None` when it would skip it).
fn read(root: &Path, path: &str, filter: &Filter, packages: &PackageNames) -> Option<String> {
    let full = root.join(path);
    let meta = std::fs::metadata(&full).ok()?;
    if !meta.is_file() || meta.len() > filter.max_file_size {
        return None;
    }
    let bytes = std::fs::read(&full).ok()?;
    let (text, _) = analyze::decode_source(&bytes).ok()?;
    if filter.skip_generated && !filter.skip_generated_except.is_match(path) {
        let names = if mote_engine::vendored::applies_to(path) {
            packages.names_for(path).to_vec()
        } else {
            Vec::new()
        };
        let skipped: Option<SkipReason> = if analyze::is_generated_path(path) {
            Some(SkipReason::Generated)
        } else {
            analyze::generated_or_minified_for(path, &text, &names)
        };
        if skipped.is_some() {
            return None;
        }
    }
    Some(text.into_owned())
}

/// Per-file counts, added to `usage` as occurrences (and one file per source with any).
#[derive(Default)]
struct FileCounts {
    sources: BTreeMap<&'static str, usize>,
    intellij_comments: usize,
    intellij_annotations: usize,
    shared: usize,
    shared_keys: BTreeMap<&'static str, usize>,
}

impl FileCounts {
    fn source(&mut self, s: &'static str) {
        *self.sources.entry(s).or_default() += 1;
    }

    fn into_usage(self, u: &mut Usage) {
        for (s, n) in self.sources {
            u.sources.entry(s).or_default().add(n);
        }
        u.intellij_comments.add(self.intellij_comments);
        u.intellij_annotations.add(self.intellij_annotations);
        u.shared.add(self.shared);
        for (k, n) in self.shared_keys {
            *u.shared_keys.entry(k).or_default() += n;
        }
    }
}

fn count_file(u: &mut Usage, registry: &Registry, plugin: usize, text: &str) {
    let mut c = FileCounts::default();
    let p = registry.plugin(plugin);
    if p.id() == "java" {
        scan_java(text, &mut c);
    } else {
        let lex = lexer::lex(text, &p.lexer(), LexOptions::default());
        let eslint = p.id() == "js";
        for comment in &lex.comments {
            if comment.text.contains("NOSONAR") {
                c.source("nosonar");
            }
            if eslint && is_eslint_disable(&comment.text) {
                c.source("eslint");
            }
        }
    }
    c.into_usage(u);
}

/// An `eslint-disable`, `eslint-disable-line` or `eslint-disable-next-line` directive (at the
/// start of the comment's text, as ESLint reads it).
fn is_eslint_disable(comment: &str) -> bool {
    let body = if let Some(b) = comment.strip_prefix("//") {
        b
    } else if let Some(b) = comment.strip_prefix("/*") {
        b.strip_suffix("*/").unwrap_or(b)
    } else {
        return false;
    };
    let body = body.trim_start();
    [
        "eslint-disable-next-line",
        "eslint-disable-line",
        "eslint-disable",
    ]
    .iter()
    .any(|k| {
        body.strip_prefix(k)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    })
}

/// A Java comment's markers.
fn java_comment(text: &str, c: &mut FileCounts) {
    if text.contains("NOSONAR") {
        c.source("nosonar");
    }
    let first = text.split('\n').next().unwrap_or("");
    if has_word(first, "NOPMD") {
        c.source("pmd");
    }
    if text.contains("CHECKSTYLE:OFF") {
        c.source("checkstyle");
    }
    let noinspection = text
        .strip_prefix("//")
        .map(str::trim_start)
        .and_then(|t| t.strip_prefix("noinspection"))
        .is_some_and(|rest| rest.starts_with([' ', '\t']) && !rest.trim().is_empty());
    if noinspection {
        c.source("intellij");
        c.intellij_comments += 1;
    }
}

/// Whether `word` occurs in `text` as a whole word.
fn has_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(ident)
            && !text[at + word.len()..].chars().next().is_some_and(ident)
    })
}

/// A Java annotation's markers: `name` and the string literals of its arguments.
fn java_annotation(name: &str, values: &[&str], c: &mut FileCounts) {
    if SPOTBUGS_ANNOTATIONS.contains(&name) {
        c.source("spotbugs");
        return;
    }
    if name != "SuppressWarnings" && name != "java.lang.SuppressWarnings" {
        return;
    }
    let (mut all, mut pmd, mut intellij, mut shared) = (false, false, false, false);
    for v in values {
        let v = v.trim();
        if v == "all" {
            all = true;
        } else if v == "PMD" || v.starts_with("PMD.") {
            pmd = true;
        } else if let Some(k) = SHARED_KEYS.iter().find(|k| **k == v) {
            shared = true;
            *c.shared_keys.entry(k).or_default() += 1;
        } else if v == "ALL" || is_intellij_inspection(v) {
            intellij = true;
        }
    }
    if all {
        c.source("suppress-warnings-all");
    }
    if pmd {
        c.source("pmd");
    }
    if intellij {
        c.source("intellij");
        c.intellij_annotations += 1;
    } else if shared {
        c.shared += 1;
    }
}

/// Whether `name` is an IntelliJ inspection the `intellij` source maps to Mote rules.
fn is_intellij_inspection(name: &str) -> bool {
    #[cfg(feature = "java")]
    {
        !mote_lang_java::intellij::mote_rules_for_inspection(name).is_empty()
    }
    #[cfg(not(feature = "java"))]
    {
        let _ = name;
        false
    }
}

/// Walk Java source: comments, and annotations in code (strings, characters and text blocks
/// skipped).
fn scan_java(text: &str, c: &mut FileCounts) {
    let b = text.as_bytes();
    let n = b.len();
    let mut i = 0;
    while i < n {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let end = b[i..].iter().position(|&x| x == b'\n').map_or(n, |p| i + p);
                java_comment(&text[i..end], c);
                i = end;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let end = find(b, i + 2, b"*/").map_or(n, |p| p + 2);
                java_comment(&text[i..end], c);
                i = end;
            }
            b'"' if b[i..].starts_with(b"\"\"\"") => {
                let mut j = i + 3;
                i = loop {
                    match find(b, j, b"\"\"\"") {
                        Some(p) if b[p - 1] == b'\\' => j = p + 1,
                        Some(p) => break p + 3,
                        None => break n,
                    }
                };
            }
            b'"' | b'\'' => i = skip_quoted(b, i),
            b'@' => i = annotation(text, i, c),
            _ => i += 1,
        }
    }
}

/// The index of `needle` in `b` at or after `from`.
fn find(b: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    b.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

/// Index after the string or character literal starting at `i` (at the line's end when it is
/// not closed there).
fn skip_quoted(b: &[u8], i: usize) -> usize {
    let q = b[i];
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'\n' => return j,
            x if x == q => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// Read the annotation at `at` (`@`): its name and the string literals of its arguments.
/// Returns the index to continue from.
fn annotation(text: &str, at: usize, c: &mut FileCounts) -> usize {
    let b = text.as_bytes();
    let start = at + 1;
    let mut j = start;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'_' | b'$' | b'.')) {
        j += 1;
    }
    let name = &text[start..j];
    if name.is_empty() || name.starts_with(|ch: char| ch.is_ascii_digit()) {
        return start;
    }
    let mut k = j;
    while k < b.len() && b[k].is_ascii_whitespace() {
        k += 1;
    }
    let mut values: Vec<&str> = Vec::new();
    if b.get(k) == Some(&b'(') {
        // The arguments, up to the matching parenthesis.
        let mut depth = 0usize;
        while k < b.len() {
            match b[k] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'"' => {
                    let end = skip_quoted(b, k);
                    if end > k + 1 && b[end - 1] == b'"' {
                        values.push(&text[k + 1..end - 1]);
                    }
                    k = end;
                    continue;
                }
                b'/' if b.get(k + 1) == Some(&b'/') || b.get(k + 1) == Some(&b'*') => break,
                _ => {}
            }
            k += 1;
        }
    }
    java_annotation(name, &values, c);
    j
}

#[cfg(test)]
mod tests {
    use super::*;

    fn java(src: &str) -> FileCounts {
        let mut c = FileCounts::default();
        scan_java(src, &mut c);
        c
    }

    fn sources(c: &FileCounts) -> Vec<(&'static str, usize)> {
        c.sources.iter().map(|(s, n)| (*s, *n)).collect()
    }

    #[test]
    fn java_markers() {
        let c = java(
            r#"
// CHECKSTYLE:OFF
@SuppressWarnings("PMD.GodClass")
class A { // NOPMD - legacy
    @SuppressWarnings({"unchecked", "PMD", "all"}) int a; // NOSONAR
    @java.lang.SuppressWarnings(value = {"unused", "rawtypes"}) int b;
    @SuppressWarnings("unused") int c;
    @SuppressFBWarnings(value = "DM_GC", justification = "x") void d() {}
    @edu.umd.cs.findbugs.annotations.SuppressFBWarnings void e() {}
    @SuppressWarnings({"OptionalAssignedToNull", "unused"}) void f() {}
    @SuppressWarnings("ALL") void g() {}
    @SuppressWarnings("SpellCheckingInspection") void h() {}
    void i() {
        //noinspection ConstantConditions
        run();
        //noinspection
        run();
        // noinspection unused (spaced: still IntelliJ's)
        String s = "@SuppressWarnings(\"all\") // NOPMD NOSONAR";
        String t = """
            @SuppressWarnings("PMD") /* CHECKSTYLE:OFF */
            """;
        char q = '"';
        /* a NOPMDX, xNOPMD, CHECKSTYLE:ON */
    }
}
"#,
        );
        assert_eq!(
            sources(&c),
            [
                ("checkstyle", 1),
                ("intellij", 4),
                ("nosonar", 1),
                ("pmd", 3),
                ("spotbugs", 2),
                ("suppress-warnings-all", 1),
            ]
        );
        assert_eq!((c.intellij_comments, c.intellij_annotations), (2, 2));
        // `{"unused", "rawtypes"}` and `"unused"` alone; `unused` next to an IntelliJ name
        // counts as that name's annotation.
        assert_eq!(c.shared, 2);
        let keys: Vec<(&str, usize)> = c.shared_keys.iter().map(|(k, n)| (*k, *n)).collect();
        assert_eq!(keys, [("rawtypes", 1), ("unused", 3)]);
    }

    #[test]
    fn odd_java_never_panics() {
        for src in [
            "@",
            "@(",
            "@A(\"",
            "@A(\"x",
            "/*",
            "//",
            "\"\"\"",
            "\"\"\"\\\"\"\"",
            "'",
            "@SuppressWarnings(",
            "@1",
            "@SuppressWarnings(\"PMD\" // x\n)",
        ] {
            let _ = java(src);
        }
    }

    #[test]
    fn eslint_directives() {
        for (comment, want) in [
            ("// eslint-disable-next-line no-console", true),
            ("// eslint-disable-line", true),
            ("/* eslint-disable */", true),
            ("/* eslint-disable no-var, eqeqeq */", true),
            ("//eslint-disable-line react/no-danger -- legacy", true),
            ("/* eslint-enable */", false),
            ("// eslint-disabled", false),
            ("/** eslint-disable */", false),
            ("// see eslint-disable-line", false),
        ] {
            assert_eq!(is_eslint_disable(comment), want, "{comment}");
        }
    }

    fn usage(
        sources: &[(&'static str, usize)],
        comments: usize,
        annotations: usize,
        shared: usize,
    ) -> Usage {
        let t = |n: usize| Tally {
            occurrences: n,
            files: usize::from(n > 0),
        };
        Usage {
            sources: sources.iter().map(|(s, n)| (*s, t(*n))).collect(),
            intellij_comments: t(comments),
            intellij_annotations: t(annotations),
            shared: t(shared),
            shared_keys: if shared > 0 {
                [("unused", shared)].into_iter().collect()
            } else {
                BTreeMap::new()
            },
        }
    }

    #[test]
    fn proposal() {
        assert!(Usage::default().proposed().is_empty());
        assert!(Usage::default().is_empty());
        let u = usage(&[("eslint", 40), ("nosonar", 1)], 0, 0, 0);
        assert_eq!(u.proposed(), ["eslint", "nosonar"]);
        assert!(u.notes().is_empty());
        // javac/Eclipse keys alone never propose intellij.
        let u = usage(&[], 0, 0, 200);
        assert!(!u.is_empty());
        assert!(u.proposed().is_empty());
        assert_eq!(u.notes().len(), 1);
        assert!(
            u.notes()[0].starts_with("200 @SuppressWarnings with javac/Eclipse keys (unused 200)")
        );
        // Two IntelliJ markers are not enough, whatever their kind and however many javac/Eclipse
        // keys there are; three are.
        for (comments, annotations) in [(2, 0), (1, 1), (0, 2)] {
            let u = usage(
                &[("intellij", comments + annotations)],
                comments,
                annotations,
                200,
            );
            assert!(u.proposed().is_empty());
            assert!(
                u.notes()[0].starts_with(&format!(
                    "Not proposed: intellij, with {comments} //noinspection comment"
                )),
                "{:?}",
                u.notes()
            );
            assert!(!u.notes()[1].ends_with("intellij honors them too"));
        }
        for (comments, annotations) in [(3, 0), (2, 1), (0, 3)] {
            let u = usage(
                &[("intellij", comments + annotations)],
                comments,
                annotations,
                9,
            );
            assert_eq!(u.proposed(), ["intellij"]);
            assert_eq!(u.notes().len(), 1);
            assert!(u.notes()[0].ends_with("intellij honors them too"));
        }
        // Every other source: one marker is enough.
        for s in ["checkstyle", "pmd", "spotbugs", "suppress-warnings-all"] {
            assert_eq!(usage(&[(s, 1)], 0, 0, 0).proposed(), [s]);
        }
    }
}
