//! `@SuppressWarnings`, other tools' annotations and comments → [`ScopedSuppression`].
//!
//! Annotations, on class-like, method, constructor, field, and local-variable declarations (the
//! suppression covers the declaration's lines, annotations included):
//!
//! * `@SuppressWarnings("mote")` — every rule (kind `annotation`);
//! * `@SuppressWarnings("mote:java/rule")` / `("mote:rule")` — one rule (kind
//!   `annotation`);
//! * `@SuppressWarnings("all")` — every rule (source `suppress-warnings-all`);
//! * `@SuppressWarnings("PMD")` — every rule; `("PMD.RuleName")` — the Mote rule
//!   [`pmd`](crate::pmd) maps the PMD rule to (source `pmd`);
//! * `@SuppressWarnings("InspectionName")` — the Mote rules [`intellij`](crate::intellij)
//!   maps the IntelliJ inspection to; `("ALL")` every rule of that table (source `intellij`);
//! * `@SuppressFBWarnings("PATTERN")` / `(value = {"A", "B"}, justification = "…")` — the
//!   Mote rules [`spotbugs`](crate::spotbugs) maps the patterns to; without a value, every
//!   rule of that table (source `spotbugs`; the justification is kept).
//!
//! A tool's catch-all form silences only the rules with an equivalent in that tool, except where
//! the form already silenced every rule before sources could be chosen (`"all"`, `"PMD"`,
//! `NOPMD`, `CHECKSTYLE:OFF`). Values are compared case-sensitively: `"all"` belongs to the
//! source `suppress-warnings-all`, `"ALL"` to `intellij`.
//!
//! Comments, the forms other tools use:
//!
//! * `// NOPMD` (optionally `// NOPMD - reason`) — every rule on the comment's line (source
//!   `pmd`, [`from_comments`]);
//! * `// CHECKSTYLE:OFF` … `// CHECKSTYLE:ON` — every rule on the lines from the one to the other
//!   (to the end of the file when there is no `CHECKSTYLE:ON`; source `checkstyle`,
//!   [`from_comments`]);
//! * `//noinspection Name[,Name…]` — the rules mapped from the inspections (`ALL`: every rule of
//!   the IntelliJ table) on the statement or declaration that follows the comment (source
//!   `intellij`, [`on_comment`]).
//!
//! A name without a Mote equivalent suppresses nothing. A source's suppressions have the kind
//! `compat:<source>`; the engine applies them only when `suppressions.compat` selects the source.

use crate::ctx::Ctx;
use crate::kinds::{k, kind, K};
use crate::syntax::{self, named_children};
use crate::syntax::{Field, NodeExt};
use mote_core::{Comment, ScopedSuppression};
use tree_sitter::Node;

const PMD: &str = "compat:pmd";
const CHECKSTYLE: &str = "compat:checkstyle";
const SUPPRESS_WARNINGS_ALL: &str = "compat:suppress-warnings-all";
const SPOTBUGS: &str = "compat:spotbugs";
const INTELLIJ: &str = "compat:intellij";

/// Names of SpotBugs' suppression annotation.
const SPOTBUGS_ANNOTATIONS: &[&str] = &[
    "SuppressFBWarnings",
    "edu.umd.cs.findbugs.annotations.SuppressFBWarnings",
    "com.github.spotbugs.annotations.SuppressFBWarnings",
    "edu.umd.cs.findbugs.annotations.SuppressWarnings",
];

/// String literal values of the annotation element `key` (`value` also takes a positional
/// argument), iteratively: arrays may nest arbitrarily.
fn element_strings<'t>(args: Node<'t>, src: &'t str, key: &str) -> Vec<&'t str> {
    let mut stack: Vec<Node<'t>> = Vec::new();
    for arg in named_children(args) {
        if kind(arg) == "element_value_pair" {
            let matches = arg
                .field(Field::Key)
                .is_some_and(|k| syntax::node_text(k, src) == key);
            if let (true, Some(v)) = (matches, arg.field(Field::Value)) {
                stack.push(v);
            }
        } else if key == "value" {
            stack.push(arg);
        }
    }
    stack.reverse();
    let mut out = Vec::new();
    while let Some(node) = stack.pop() {
        if out.len() > 1_000 {
            break;
        }
        match kind(node) {
            "string_literal" => {
                if let Some(s) = syntax::string_literal_content(node, src) {
                    out.push(s);
                }
            }
            "element_value_array_initializer" => {
                let kids: Vec<Node<'t>> = named_children(node).collect();
                stack.extend(kids.into_iter().rev());
            }
            _ => {}
        }
    }
    out
}

/// Handle an `annotation` node; `ctx.stack` holds its ancestors.
pub(crate) fn on_annotation(ctx: &Ctx<'_>, ann: Node<'_>, out: &mut Vec<ScopedSuppression>) {
    let name = ann.field(Field::Name).map(|n| ctx.text(n)).unwrap_or("");
    let spotbugs = SPOTBUGS_ANNOTATIONS.contains(&name);
    if !spotbugs && name != "SuppressWarnings" && name != "java.lang.SuppressWarnings" {
        return;
    }
    let n = ctx.stack.len();
    let (Some(parent), Some(decl)) = (
        ctx.stack.get(n.wrapping_sub(1)),
        ctx.stack.get(n.wrapping_sub(2)),
    ) else {
        return;
    };
    if k(*parent) != K::Modifiers {
        return;
    }
    let decl_ok = k(*decl).is_type_decl()
        || matches!(
            k(*decl),
            K::MethodDecl
                | K::ConstructorDecl
                | K::CompactConstructorDecl
                | K::FieldDecl
                | K::ConstantDecl
                | K::LocalVarDecl
        );
    if !decl_ok {
        return;
    }
    let args = ann.field(Field::Arguments);
    let values = args.map_or_else(Vec::new, |a| element_strings(a, ctx.src, "value"));
    let start_line = decl.start_position().row as u32 + 1;
    let end_line = decl.end_position().row as u32 + 1;
    let mk =
        |rules: Option<Vec<String>>, kind: &str, justification: Option<String>| ScopedSuppression {
            start_line,
            end_line,
            rules,
            kind: kind.to_string(),
            justification,
        };
    if spotbugs {
        let justification = args
            .and_then(|a| {
                element_strings(a, ctx.src, "justification")
                    .first()
                    .copied()
            })
            .map(str::trim)
            .filter(|j| !j.is_empty())
            .map(str::to_string);
        if values.is_empty() {
            // Every SpotBugs warning: every rule with a SpotBugs equivalent, nothing else.
            let all = crate::spotbugs::all_spotbugs_rules();
            out.push(mk(
                Some(all.into_iter().map(str::to_string).collect()),
                SPOTBUGS,
                justification,
            ));
        } else {
            let mut rules: Vec<String> = values
                .iter()
                .flat_map(|v| crate::spotbugs::mote_rules_for_spotbugs(v))
                .map(str::to_string)
                .collect();
            rules.sort();
            rules.dedup();
            if !rules.is_empty() {
                out.push(mk(Some(rules), SPOTBUGS, justification));
            }
        }
        return;
    }
    let mut all_mote = false;
    let mut all_warnings = false;
    let mut all_pmd = false;
    let mut rules: Vec<String> = Vec::new();
    let mut pmd_rules: Vec<String> = Vec::new();
    let mut intellij_rules: Vec<String> = Vec::new();
    // Names are compared case-sensitively: `"all"` (javac, Eclipse) and IntelliJ's `"ALL"` are
    // different sources.
    for v in values {
        let v = v.trim();
        if v == "mote" {
            all_mote = true;
        } else if v == "all" {
            all_warnings = true;
        } else if v == "PMD" {
            all_pmd = true;
        } else if v == crate::intellij::ALL_INSPECTIONS {
            // Every inspection: every rule with an IntelliJ equivalent, nothing else.
            intellij_rules.extend(
                crate::intellij::all_inspection_rules()
                    .into_iter()
                    .map(str::to_string),
            );
        } else if let Some(name) = v.strip_prefix("PMD.") {
            if let Some(id) = crate::pmd::mote_rule_for_pmd(name) {
                pmd_rules.push(id.to_string());
            }
        } else if let Some(id) = v.strip_prefix("mote:") {
            let id = id.trim();
            if !id.is_empty() {
                rules.push(if id.contains('/') {
                    id.to_string()
                } else {
                    format!("java/{id}")
                });
            }
        } else {
            intellij_rules.extend(
                crate::intellij::mote_rules_for_inspection(v)
                    .iter()
                    .map(|id| id.to_string()),
            );
        }
    }
    if all_mote {
        out.push(mk(None, "annotation", None));
    } else if !rules.is_empty() {
        out.push(mk(Some(rules), "annotation", None));
    }
    if all_warnings {
        out.push(mk(None, SUPPRESS_WARNINGS_ALL, None));
    }
    for (all, mut ids, kind) in [(all_pmd, pmd_rules, PMD), (false, intellij_rules, INTELLIJ)] {
        if all {
            out.push(mk(None, kind, None));
        } else if !ids.is_empty() {
            ids.sort();
            ids.dedup();
            out.push(mk(Some(ids), kind, None));
        }
    }
}

/// IntelliJ's `//noinspection Name[,Name…] [comment]` on a line comment: the rules of the named
/// inspections on the statement or declaration that follows (comments in between are skipped).
pub(crate) fn on_comment(ctx: &Ctx<'_>, comment: Node<'_>, out: &mut Vec<ScopedSuppression>) {
    let text = ctx.text(comment);
    let Some(rest) = text
        .strip_prefix("//")
        .map(str::trim_start)
        .and_then(|t| t.strip_prefix("noinspection"))
    else {
        return;
    };
    if !rest.starts_with([' ', '\t']) {
        return;
    }
    let (names, remark) = inspection_names(rest);
    if names.is_empty() {
        return;
    }
    let mut target = comment.next_named_sibling();
    while let Some(t) = target {
        if !matches!(k(t), K::LineComment | K::BlockComment) {
            break;
        }
        target = t.next_named_sibling();
    }
    let Some(target) = target else { return };
    // `ALL` (case-sensitive): every rule with an IntelliJ equivalent, nothing else.
    let mut ids: Vec<String> = if names.contains(&crate::intellij::ALL_INSPECTIONS) {
        crate::intellij::all_inspection_rules()
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        names
            .iter()
            .flat_map(|n| crate::intellij::mote_rules_for_inspection(n))
            .map(|id| id.to_string())
            .collect()
    };
    if ids.is_empty() {
        return;
    }
    ids.sort();
    ids.dedup();
    out.push(ScopedSuppression {
        start_line: target.start_position().row as u32 + 1,
        end_line: target.end_position().row as u32 + 1,
        rules: Some(ids),
        kind: INTELLIJ.to_string(),
        justification: remark,
    });
}

/// `Name1, Name2 remark` → the names and the remark.
fn inspection_names(rest: &str) -> (Vec<&str>, Option<String>) {
    let is_id = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-');
    let mut names = Vec::new();
    let mut rest = rest.trim_start();
    loop {
        let end = rest.find(|c: char| !is_id(c)).unwrap_or(rest.len());
        if end == 0 {
            break;
        }
        names.push(&rest[..end]);
        let after = rest[end..].trim_start();
        match after.strip_prefix(',') {
            Some(more) => rest = more.trim_start(),
            None => {
                rest = after;
                break;
            }
        }
    }
    let remark = rest.trim();
    (names, (!remark.is_empty()).then(|| remark.to_string()))
}

/// The text after a marker as a reason: `NOPMD - reason`, `NOPMD: reason`, `NOPMD reason`.
fn marker_reason(rest: &str) -> Option<String> {
    let rest = rest.split("*/").next().unwrap_or("");
    let r = rest
        .trim_start()
        .trim_start_matches(['-', ':'])
        .trim()
        .to_string();
    (!r.is_empty()).then_some(r)
}

/// Whether `word` occurs in `text` as a whole word (not inside a longer identifier).
fn find_word(text: &str, word: &str) -> Option<usize> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(pos) = text[from..].find(word) {
        let at = from + pos;
        let end = at + word.len();
        let before_ok = !text[..at].chars().next_back().is_some_and(ident);
        let after_ok = !text[end..].chars().next().is_some_and(ident);
        if before_ok && after_ok {
            return Some(at);
        }
        from = end;
    }
    None
}

/// Compatibility suppressions from comments: `NOPMD` on a line, `CHECKSTYLE:OFF` …
/// `CHECKSTYLE:ON` regions. `comments` are in source order.
pub(crate) fn from_comments(comments: &[Comment], out: &mut Vec<ScopedSuppression>) {
    let mut off_since: Option<u32> = None;
    for c in comments {
        let first = c.text.split('\n').next().unwrap_or("");
        if let Some(at) = find_word(first, "NOPMD") {
            out.push(ScopedSuppression {
                start_line: c.start_line,
                end_line: c.start_line,
                rules: None,
                kind: PMD.to_string(),
                justification: marker_reason(&first[at + "NOPMD".len()..]),
            });
        }
        if off_since.is_none() && c.text.contains("CHECKSTYLE:OFF") {
            off_since = Some(c.start_line);
        } else if let (Some(start), true) = (off_since, c.text.contains("CHECKSTYLE:ON")) {
            out.push(ScopedSuppression {
                start_line: start,
                end_line: c.end_line.max(start),
                rules: None,
                kind: CHECKSTYLE.to_string(),
                justification: None,
            });
            off_since = None;
        }
    }
    if let Some(start) = off_since {
        out.push(ScopedSuppression {
            start_line: start,
            end_line: u32::MAX,
            rules: None,
            kind: CHECKSTYLE.to_string(),
            justification: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::JavaPlugin;
    use mote_core::{LanguagePlugin, RuleSet, SourceFile};

    /// (start, end, rules, kind, justification) of each suppression.
    type Scoped = (u32, u32, Option<Vec<String>>, String, Option<String>);

    fn scoped(src: &str) -> Vec<Scoped> {
        JavaPlugin::new()
            .analyze(
                &SourceFile {
                    path: "A.java",
                    text: src,
                    is_test: false,
                },
                &RuleSet::default(),
            )
            .suppressions
            .into_iter()
            .map(|s| (s.start_line, s.end_line, s.rules, s.kind, s.justification))
            .collect()
    }

    fn ids(v: &[&str]) -> Option<Vec<String>> {
        Some(v.iter().map(|s| s.to_string()).collect())
    }

    /// What a bare `@SuppressFBWarnings` silences: the SpotBugs table's rules.
    fn every_spotbugs() -> Option<Vec<String>> {
        ids(&crate::spotbugs::all_spotbugs_rules())
    }

    /// What IntelliJ's `ALL` silences: the IntelliJ table's rules.
    fn every_intellij() -> Option<Vec<String>> {
        ids(&crate::intellij::all_inspection_rules())
    }

    #[test]
    fn catch_all_forms_cover_only_the_tools_equivalents() {
        let spotbugs = crate::spotbugs::all_spotbugs_rules();
        let intellij = crate::intellij::all_inspection_rules();
        for (tool, all) in [("spotbugs", &spotbugs), ("intellij", &intellij)] {
            assert!(
                all.windows(2).all(|w| w[0] < w[1]),
                "{tool}: sorted, unique"
            );
            // Rules no SpotBugs pattern / IntelliJ inspection maps to stay reported.
            for not_covered in ["java/cognitive-complexity", "java/member-order"] {
                assert!(!all.contains(&not_covered), "{tool}: {not_covered}");
            }
        }
        assert!(spotbugs.contains(&"java/explicit-gc") && !spotbugs.contains(&"java/magic-number"));
        assert!(
            intellij.contains(&"java/magic-number") && intellij.contains(&"java/optional-null")
        );
    }

    #[test]
    fn all_and_capital_all_belong_to_different_sources() {
        let src = r#"
class A {
    @SuppressWarnings("all")
    void a() {}
    @SuppressWarnings("ALL")
    void b() {}
    @SuppressWarnings({"All", "aLL"})
    void c() {}
    void d() {
        //noinspection all
        int x = 1;
        //noinspection All
        int y = 2;
    }
}
"#;
        assert_eq!(
            scoped(src),
            vec![
                (3, 4, None, "compat:suppress-warnings-all".into(), None),
                (5, 6, every_intellij(), "compat:intellij".into(), None),
            ],
            "`all` is javac's (suppress-warnings-all), `ALL` IntelliJ's; other spellings are neither"
        );
    }

    #[test]
    fn intellij_data_flow_and_resource_names() {
        let src = r#"
class A {
    void a(String s) {
        //noinspection ConstantConditions
        if (s != null) {}
        //noinspection ConstantValue, DataFlowIssue
        if (s != null) {}
        //noinspection resource
        java.io.InputStream in = open();
    }
    @SuppressWarnings({"deprecation", "unchecked"})
    void b() {}
}
"#;
        let null_check = ids(&["java/null-check-after-dereference"]);
        assert_eq!(
            scoped(src),
            vec![
                (5, 5, null_check.clone(), "compat:intellij".into(), None),
                (7, 7, null_check, "compat:intellij".into(), None),
                (
                    9,
                    9,
                    ids(&["java/resource-not-closed-with-try"]),
                    "compat:intellij".into(),
                    None
                ),
            ],
            "`deprecation` and `unchecked` have no equivalent"
        );
    }

    #[test]
    fn suppressions_from_annotations() {
        let src = r#"
@SuppressWarnings("mote")
class A {
    @SuppressWarnings({"unchecked", "mote:empty-catch-block", "mote:java/unused-import"})
    void m() {
        @SuppressWarnings("all") int x = 1;
    }
    @SuppressWarnings(value = "mote:java/x")
    private int f;
}
"#;
        assert_eq!(
            scoped(src),
            vec![
                (2, 10, None, "annotation".into(), None),
                (
                    4,
                    7,
                    ids(&["java/empty-catch-block", "java/unused-import"]),
                    "annotation".into(),
                    None
                ),
                (6, 6, None, "compat:suppress-warnings-all".into(), None),
                (8, 9, ids(&["java/x"]), "annotation".into(), None),
            ]
        );
    }

    #[test]
    fn pmd_annotations_map_to_mote_rules() {
        let src = r#"
class A {
    @SuppressWarnings("PMD")
    void a() {}
    @SuppressWarnings({"PMD.EmptyCatchBlock", "PMD.UnusedImports", "PMD.NoSuchRule"})
    void b() {}
    @SuppressWarnings("PMD.NoSuchRule")
    void c() {}
    @SuppressWarnings("PMDX")
    void d() {}
    @SuppressWarnings("PMD.SystemPrintln")
    void e() {}
}
"#;
        assert_eq!(
            scoped(src),
            vec![
                (3, 4, None, "compat:pmd".into(), None),
                (
                    5,
                    6,
                    ids(&["java/empty-catch-block", "java/unused-import"]),
                    "compat:pmd".into(),
                    None
                ),
                // A rule outside PMD's quickstart ruleset (PMD_MIGRATION_EQUIVALENTS).
                (
                    11,
                    12,
                    ids(&["java/system-out-print"]),
                    "compat:pmd".into(),
                    None
                ),
            ],
            "an unknown PMD name suppresses nothing"
        );
    }

    #[test]
    fn spotbugs_annotations_map_to_mote_rules() {
        let src = r#"
import edu.umd.cs.findbugs.annotations.SuppressFBWarnings;
class A {
    @SuppressFBWarnings("DM_GC")
    void a() { System.gc(); }
    @SuppressFBWarnings(value = {"ES_COMPARING_STRINGS_WITH_EQ", "NOT_A_PATTERN"}, justification = "interned")
    boolean b(String s) { return s == "x"; }
    @edu.umd.cs.findbugs.annotations.SuppressFBWarnings(justification = "legacy")
    void c() {}
    @SuppressFBWarnings({"NOT_A_PATTERN", "CORRECTNESS"})
    void d() {}
    @SuppressFBWarnings(value = "SA_LOCAL_SELF", justification = "")
    private int e;
}
"#;
        assert_eq!(
            scoped(src),
            vec![
                (
                    4,
                    5,
                    ids(&["java/explicit-gc"]),
                    "compat:spotbugs".into(),
                    None
                ),
                (
                    6,
                    7,
                    ids(&["java/string-reference-equality"]),
                    "compat:spotbugs".into(),
                    Some("interned".into())
                ),
                (
                    8,
                    9,
                    every_spotbugs(),
                    "compat:spotbugs".into(),
                    Some("legacy".into())
                ),
                (
                    12,
                    13,
                    ids(&["java/identical-operands", "java/self-assignment"]),
                    "compat:spotbugs".into(),
                    None
                ),
            ],
            "names without an equivalent suppress nothing; no value suppresses every SpotBugs rule"
        );
    }

    #[test]
    fn intellij_annotations_and_comments() {
        let src = r#"
class A {
    @SuppressWarnings({"unchecked", "OptionalAssignedToNull", "SpellCheckingInspection"})
    void a() { java.util.Optional<String> o = null; }
    @SuppressWarnings("ALL")
    void b() {}

    //noinspection OptionalAssignedToNull
    void c() {
        // first a comment, then the inspection's
        //noinspection CallToSystemGC, StringEquality because the test needs it
        // another comment
        System.gc(
        );
        //noinspection SpellCheckingInspection
        int unrelated = 1;
        //noinspection ALL
        int all = 2;
        //noinspectionX StringEquality
        int notOne = 3;
        // noinspection unused
        int spaced = 4;
    }
    //noinspection UnusedAssignment
}
"#;
        assert_eq!(
            scoped(src),
            vec![
                (
                    3,
                    4,
                    ids(&["java/optional-null"]),
                    "compat:intellij".into(),
                    None
                ),
                (5, 6, every_intellij(), "compat:intellij".into(), None),
                (
                    9,
                    23,
                    ids(&["java/optional-null"]),
                    "compat:intellij".into(),
                    None
                ),
                (
                    13,
                    14,
                    ids(&["java/explicit-gc", "java/string-reference-equality"]),
                    "compat:intellij".into(),
                    Some("because the test needs it".into())
                ),
                (18, 18, every_intellij(), "compat:intellij".into(), None),
                (
                    22,
                    22,
                    ids(&[
                        "java/unused-local-variable",
                        "java/unused-private-class",
                        "java/unused-private-field",
                        "java/unused-private-method",
                        "java/unused-private-method-parameter"
                    ]),
                    "compat:intellij".into(),
                    None
                ),
            ],
            "an inspection without an equivalent, or no following statement, suppresses nothing"
        );
    }

    #[test]
    fn nopmd_and_checkstyle_comments() {
        let src = "\
class A {
    int a = 1; // NOPMD - legacy constant
    int b = 2; // NOPMD
    int c = 3; /* NOPMD: kept for the wire format */
    // CHECKSTYLE:OFF
    int d = 4;
    int e = 5;
    // CHECKSTYLE:ON
    int f = 6; // NOPMDX is not the marker, nor is xNOPMD
    // CHECKSTYLE:OFF
    int g = 7;
}
";
        assert_eq!(
            scoped(src),
            vec![
                (
                    2,
                    2,
                    None,
                    "compat:pmd".into(),
                    Some("legacy constant".into())
                ),
                (3, 3, None, "compat:pmd".into(), None),
                (
                    4,
                    4,
                    None,
                    "compat:pmd".into(),
                    Some("kept for the wire format".into())
                ),
                (5, 8, None, "compat:checkstyle".into(), None),
                (10, u32::MAX, None, "compat:checkstyle".into(), None),
            ]
        );
    }
}
