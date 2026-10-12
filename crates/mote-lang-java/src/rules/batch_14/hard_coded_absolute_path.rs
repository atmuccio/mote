//! `java/hard-coded-absolute-path` — an absolute file-system path of one machine (a drive
//! letter, a personal home folder, the Unix temporary folder) written into production code.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["portability", "configuration"],
    remediation_minutes: 10,
    description: r#"Detects string literals in production code that are absolute file-system paths
tied to one machine or one operating system:

* a Windows drive path to data or a user folder, in any use: `"C:\\data\\input.csv"`,
  `"D:/exports"`, `"c:/users/alice/report.pdf"`;
* a Windows system folder (`"C:\\Windows\\Fonts\\arial.ttf"`, `"C:\\Program Files\\…"`) or
  the Unix temporary folder (`"/tmp/cache"`, `"/var/tmp/x"`) given straight to a file API:
  `new File(..)`, `Paths.get(..)`, `Path.of(..)`, `Files.*(..)`, file streams, readers and
  writers;
* a macOS home folder (`"/Users/bob/work"`) given to a file API or stored in a variable, field
  or constant named like a path (`path`, `dir`, `file`, `folder`, `home`, `location`).

**Why it matters.** The path exists on the developer's machine, or on one operating system,
and nowhere else: the program fails on another workstation, in a container, on a build
server, or on Windows versus Unix. A hard-coded temporary folder also bypasses the platform's
setting and can collide with other users of a shared machine.

**How to fix.** Read the location from configuration (a property, an environment variable, a
command-line argument), or derive it: `System.getProperty("java.io.tmpdir")`,
`Files.createTempDirectory(..)`, `System.getProperty("user.home")`.

```java
// Noncompliant
File out = new File("C:\\reports\\daily.pdf");
Path cache = Paths.get("/tmp/app-cache");

// Compliant
File out = new File(config.reportDirectory(), "daily.pdf");
Path cache = Files.createTempDirectory("app-cache");
```

Not reported: test code; annotation values (documentation examples, defaults of injected
properties); the fallback value of a configuration lookup (`getProperty(key, "/tmp/x")`,
`orElse(..)`); literals compared with or searched in another string (`startsWith("C:\\")`,
path-parsing code); `case` labels; bare roots (`"C:\\"`); system folders and `/tmp` kept in
constants, which usually list the known locations of each platform on purpose, or used as a
path on a remote Unix host; `/home/…` and `/root/…` (the fixed layout of containers and
service accounts); and URIs (`file:///C:/…`), which do not start with the path."#,
    ..RuleMeta::new(
        "java/hard-coded-absolute-path",
        "java",
        "Hard-coded absolute file-system path",
    )
};

pub struct HardCodedAbsolutePath;

/// Methods whose string arguments are compared or searched, not used as a location.
const TEXT_METHODS: &[&str] = &[
    "startsWith",
    "endsWith",
    "equals",
    "equalsIgnoreCase",
    "contains",
    "indexOf",
    "lastIndexOf",
    "matches",
    "replace",
    "replaceAll",
    "replaceFirst",
    "split",
    "regionMatches",
    "compareTo",
    "compareToIgnoreCase",
    "contentEquals",
    "removeStart",
    "removeEnd",
];

/// Classes whose constructors take a file location.
const FILE_TYPES: &[&str] = &[
    "File",
    "FileInputStream",
    "FileOutputStream",
    "FileReader",
    "FileWriter",
    "RandomAccessFile",
];

/// Classes whose static methods take a file location.
const FILE_HELPERS: &[&str] = &["Paths", "Path", "Files"];

/// Which kind of path a literal's content is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PathKind {
    /// A drive path to data or a user folder: machine-bound in any use.
    Drive,
    /// A Windows system folder or the Unix temporary folder: only given to a file API.
    Platform,
    /// A macOS home folder: given to a file API or kept in a path-named variable.
    Home,
}

/// Classifies the raw (source) content of a string literal.
fn classify(raw: &str) -> Option<PathKind> {
    let b = raw.as_bytes();
    // `C:\\x` (source text of `C:\x`) or `C:/x`, with a folder or file name after the root.
    if b.len() >= 4 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        let rest = if raw[2..].starts_with("\\\\") {
            &raw[4..]
        } else if raw[2..].starts_with('/') {
            &raw[3..]
        } else {
            return None;
        };
        let first = rest.chars().next()?;
        if !(first.is_alphanumeric() || matches!(first, '_' | '.' | '$' | '-')) {
            return None;
        }
        let top = rest
            .split(['\\', '/'])
            .next()
            .unwrap_or(rest)
            .to_ascii_lowercase();
        let system = top == "windows" || top.starts_with("program files") || top == "programdata";
        return Some(if system {
            PathKind::Platform
        } else {
            PathKind::Drive
        });
    }
    for tmp in ["/tmp", "/var/tmp"] {
        if let Some(rest) = raw.strip_prefix(tmp) {
            if rest.is_empty() || (rest.starts_with('/') && !rest.contains(char::is_whitespace)) {
                return Some(PathKind::Platform);
            }
        }
    }
    if let Some(rest) = raw.strip_prefix("/Users/") {
        let first = rest.chars().next()?;
        if first.is_alphanumeric() && !rest.contains(char::is_whitespace) {
            return Some(PathKind::Home);
        }
    }
    None
}

/// Whether a variable or field name suggests a file-system location.
fn path_like_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    ["path", "dir", "file", "folder", "home", "location"]
        .iter()
        .any(|w| l.contains(w))
}

/// Whether an argument of a call is a fallback value (`getProperty(key, "/tmp")`,
/// `orElse("/tmp")`).
fn is_fallback(name: &str, first_arg: bool) -> bool {
    name.starts_with("orElse")
        || (!first_arg && (name.starts_with("get") || name.starts_with("default")))
}

/// How a literal is used.
enum Use {
    /// Compared, searched, a label, an annotation value, a fallback: never reported.
    Ignored,
    /// Given to a file API.
    File,
    /// Stored in a variable, field or constant named like a path.
    PathName,
    /// Anything else.
    Other,
}

fn usage<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Use {
    // Climb through concatenations and parentheses to the literal's use.
    let mut cur = node;
    let mut user = None;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "binary_expression" if syntax::operator_str(a) == "+" => cur = a,
            "parenthesized_expression" => cur = a,
            _ => {
                user = Some(a);
                break;
            }
        }
    }
    let Some(user) = user else {
        return Use::Ignored;
    };
    match kind(user) {
        "annotation_argument_list"
        | "element_value_pair"
        | "element_value_array_initializer"
        | "switch_label"
        | "binary_expression" => Use::Ignored,
        "argument_list" => {
            let Some(call) = ctx.parent(user) else {
                return Use::Ignored;
            };
            let first_arg = named_children(user).next().map(|a| a.id()) == Some(cur.id());
            match kind(call) {
                "method_invocation" => {
                    let Some(name) = ctx.call_name(call) else {
                        return Use::Ignored;
                    };
                    if TEXT_METHODS.contains(&name) || is_fallback(name, first_arg) {
                        return Use::Ignored;
                    }
                    let recv = call.field(Field::Object).map(|o| ctx.text(o));
                    let recv = recv.map(|r| r.rsplit('.').next().unwrap_or(r));
                    if recv.is_some_and(|r| FILE_HELPERS.contains(&r)) {
                        Use::File
                    } else {
                        Use::Other
                    }
                }
                "object_creation_expression" => {
                    let t = call.field(Field::Type).map(|t| ctx.text(t)).unwrap_or("");
                    let simple = t.rsplit('.').next().unwrap_or(t);
                    if FILE_TYPES.contains(&simple) {
                        Use::File
                    } else {
                        Use::Other
                    }
                }
                _ => Use::Other,
            }
        }
        // The literal is the receiver: `"C:\\x".equals(p)`.
        "method_invocation" if user.field(Field::Object).map(|o| o.id()) == Some(cur.id()) => {
            Use::Ignored
        }
        "variable_declarator" => {
            if user
                .field(Field::Name)
                .is_some_and(|n| path_like_name(ctx.text(n)))
            {
                Use::PathName
            } else {
                Use::Other
            }
        }
        "assignment_expression" => {
            let named = user.field(Field::Left).is_some_and(|l| {
                let t = ctx.text(l);
                path_like_name(t.rsplit('.').next().unwrap_or(t))
            });
            if named {
                Use::PathName
            } else {
                Use::Other
            }
        }
        _ => Use::Other,
    }
}

impl<'t> JavaRule<'t> for HardCodedAbsolutePath {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["string_literal"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let text = ctx.text(node);
        // Cheapest test first: the content must start with `/` or a drive letter.
        let Some(second) = text.as_bytes().get(1) else {
            return;
        };
        if *second != b'/' && !second.is_ascii_alphabetic() {
            return;
        }
        if text.starts_with("\"\"\"") || ctx.is_test() {
            return;
        }
        let Some(raw) = syntax::string_literal_content(node, ctx.source()) else {
            return;
        };
        let Some(path_kind) = classify(raw) else {
            return;
        };
        let reported = match (usage(ctx, node), path_kind) {
            (Use::Ignored, _) => false,
            (_, PathKind::Drive) => true,
            (Use::File, _) => true,
            (Use::PathName, PathKind::Home) => true,
            _ => false,
        };
        if !reported || ctx.is_inside(node, "annotation", false) {
            return;
        }
        let shown: String = raw.chars().take(40).collect();
        ctx.emit(
            node,
            format!(
                "Replace the hard-coded path `{shown}`: it only exists on some machines; read it from configuration or derive it"
            ),
        );
    }
}
