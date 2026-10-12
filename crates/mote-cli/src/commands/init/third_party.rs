//! Third-party code copied into a repository, found for `mote init`: proposed `exclude`
//! globs, never applied by themselves. The generated `.mote.yaml` lists them commented out
//! (the files stay analyzed until someone uncomments a line); `mote init --suggest-excludes`
//! prints them.
//!
//! Two independent heuristics, both tuned to stay quiet when unsure:
//!
//! * **Foreign Java packages.** Java files are grouped by namespace: the organization of a
//!   reverse-domain package (`com.acme`, `org.springframework`), or organization and project
//!   under hosts of many unrelated projects (`org.apache.wink`, `com.google.common`), or the first
//!   segment of other packages (`hudson`). Namespaces holding at least 10% of the Java files are
//!   the project's own. A smaller reverse-domain namespace is proposed when none of its files
//!   imports the project's own packages (code written for the project uses the project; a copied
//!   library does not) or names the project in its package. Never proposed: the Java platform
//!   and specification namespaces (`java`, `javax`, `jakarta`, …), placeholder domains
//!   (`com.example`), Java files below a `resources` directory (test data), and source roots
//!   named after the namespace's project (a module `xmpbox/src/main/java` holding
//!   `org.apache.xmpbox`, a source set `src/withMicrometerTest/java` holding `io.micrometer`).
//! * **Another holder's license header.** Holders are read from the copyright lines of the
//!   header comment (`Copyright (c) 2008 Google Inc.`, `Licensed to the Apache Software
//!   Foundation`). The project's usual holders are those named in at least 10% of the files that
//!   name one, provided they cover at least 70% of those files (a consistent header policy). A
//!   file is third-party when it names only other holders, none of which ever appears next to a
//!   usual holder (that would be a contributor). Such files are proposed by directory when a
//!   directory holds two or more of them, no file of the project's holders, and no more files
//!   without a holder; otherwise one by one.

use mote_engine::source::{DirectorySource, Source};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

/// A proposed exclude glob and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub glob: String,
    pub reason: String,
}

/// Bytes read from the start of each file (header comment, package and imports).
const HEAD_BYTES: u64 = 16 * 1024;

/// Source files looked at: the languages Mote analyzes.
const SOURCE_EXTS: &[&str] = &["java", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

/// Directories the starter configuration already excludes.
const EXCLUDED_DIRS: &[&str] = &["node_modules", "vendor", "generated", ".yarn"];

/// First segments of reverse-domain packages.
const DOMAIN_SEGMENTS: &[&str] = &[
    "com", "org", "net", "io", "edu", "gov", "info", "biz", "me", "co", "dev", "app", "ai", "eu",
    "de", "fr", "uk", "nl", "be", "ch", "at", "es", "se", "no", "dk", "fi", "pl", "cz", "ru", "jp",
    "cn", "kr", "au", "br", "ca", "us", "tech", "cloud",
];

/// Organizations hosting many unrelated projects: their namespaces include the project.
const UMBRELLAS: &[&str] = &[
    "org.apache",
    "org.eclipse",
    "com.google",
    "com.github",
    "io.github",
    "org.codehaus",
    "net.sf",
    "net.java",
    "org.ow2",
    "org.objectweb",
    "org.jboss",
    "com.googlecode",
    "org.kohsuke",
];

/// Namespaces of the Java platform and its specifications, never proposed.
const PLATFORM: &[&str] = &[
    "java", "javax", "jakarta", "sun", "jdk", "com.sun", "org.w3c", "org.xml", "org.omg",
    "org.ietf",
];

/// Placeholder organizations of examples and tests, never proposed.
const PLACEHOLDERS: &[&str] = &[
    "example",
    "examples",
    "sample",
    "samples",
    "demo",
    "test",
    "tests",
    "acme",
    "foo",
    "bar",
    "mycompany",
    "company",
    "yourcompany",
    "domain",
    "mydomain",
];

/// What the heuristics need from one file.
#[derive(Debug, Default)]
struct FileFacts {
    path: String,
    /// Java package.
    package: Option<String>,
    /// Java imports (package or type names, `static` dropped).
    imports: Vec<String>,
    /// Copyright holders named in the header comment, normalized.
    holders: Vec<String>,
}

/// Find third-party code under `root`. Paths below directories the starter excludes
/// (`node_modules`, `vendor`, `generated`, `.yarn`) and paths matching `exclude` (the `exclude`
/// globs of an existing configuration) are not looked at, so nothing they already cover is
/// proposed.
#[cfg(test)]
fn suggest<S: AsRef<str>>(root: &Path, exclude: &[S]) -> Vec<Suggestion> {
    suggest_in(root, &discovered_paths(root, exclude))
}

/// The files `mote init` looks at below `root`, sorted: what a scan of the directory
/// discovers (`.gitignore`d paths left out), except paths below the directories the starter
/// excludes (`node_modules`, `vendor`, `generated`, `.yarn`) and paths matching `exclude` (the
/// `exclude` globs of an existing configuration).
pub fn discovered_paths<S: AsRef<str>>(root: &Path, exclude: &[S]) -> Vec<String> {
    let Ok(discovery) = (DirectorySource {
        root: root.to_path_buf(),
    })
    .discover() else {
        return Vec::new();
    };
    let excluded = mote_engine::config::compile_path_globs(exclude);
    let mut paths: Vec<String> = discovery
        .entries
        .into_iter()
        .map(|e| e.path)
        .filter(|p| !p.split('/').any(|seg| EXCLUDED_DIRS.contains(&seg)) && !excluded.is_match(p))
        .collect();
    paths.sort();
    paths
}

/// [`suggest`] over `paths` ([`discovered_paths`]; only Java, JavaScript and TypeScript files
/// are read).
pub fn suggest_in(root: &Path, paths: &[String]) -> Vec<Suggestion> {
    let paths: Vec<String> = paths
        .iter()
        .filter(|p| {
            let ext = p.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            SOURCE_EXTS.contains(&ext.as_str())
        })
        .cloned()
        .collect();
    let files = read_facts(root, paths);
    let mut out = foreign_packages(&files);
    out.extend(foreign_headers(&files));
    dedupe(out)
}

/// Read the facts of every file, on a few threads.
fn read_facts(root: &Path, paths: Vec<String>) -> Vec<FileFacts> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 16);
    let chunk = paths.len().div_ceil(threads).max(1);
    let mut out: Vec<FileFacts> = Vec::with_capacity(paths.len());
    std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|p| facts(p, &read_head(&root.join(p))))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            if let Ok(v) = h.join() {
                out.extend(v);
            }
        }
    });
    out
}

fn read_head(path: &Path) -> String {
    let mut buf = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(HEAD_BYTES).read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn facts(path: &str, head: &str) -> FileFacts {
    let (header, code) = split_header(head);
    let mut f = FileFacts {
        path: path.to_string(),
        holders: holders(&header),
        ..Default::default()
    };
    if path.ends_with(".java") {
        let (package, imports) = package_and_imports(code);
        f.package = package;
        f.imports = imports;
    }
    f
}

/// The leading comments of `text` (lines without comment punctuation) and the rest.
fn split_header(text: &str) -> (Vec<String>, &str) {
    let mut lines = Vec::new();
    let mut rest = text.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        if rest.starts_with("#!") || rest.starts_with("//") {
            let end = rest.find('\n').unwrap_or(rest.len());
            lines.push(clean_comment_line(&rest[..end]));
            rest = &rest[end..];
        } else if let Some(body) = rest.strip_prefix("/*") {
            let Some(end) = body.find("*/") else {
                lines.extend(body.lines().map(clean_comment_line));
                return (lines, "");
            };
            lines.extend(body[..end].lines().map(clean_comment_line));
            rest = &body[end + 2..];
        } else {
            return (lines, rest);
        }
    }
}

fn clean_comment_line(l: &str) -> String {
    l.trim()
        .trim_start_matches(['/', '*', '!', '#'])
        .trim()
        .to_string()
}

/// The `package` declaration and the imports at the start of Java code.
fn package_and_imports(code: &str) -> (Option<String>, Vec<String>) {
    let mut package = None;
    let mut imports = Vec::new();
    let mut in_block = false;
    for line in code.lines() {
        let mut t = line.trim();
        if in_block {
            match t.find("*/") {
                Some(end) => {
                    in_block = false;
                    t = t[end + 2..].trim();
                }
                None => continue,
            }
        }
        if t.starts_with("/*") {
            if !t.contains("*/") {
                in_block = true;
            }
            continue;
        }
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        if let Some(rest) = t.strip_prefix("package ") {
            if package.is_some() {
                break;
            }
            package = qualified_name(rest);
        } else if let Some(rest) = t.strip_prefix("import ") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix("static ").unwrap_or(rest);
            if let Some(name) = qualified_name(rest) {
                imports.push(name);
            }
        } else if t.starts_with('@') && package.is_none() {
            // annotations of `package-info.java`
            continue;
        } else {
            break;
        }
    }
    (package, imports)
}

/// `a.b.C` from `a.b.C;` / `a.b.*;` (`*` dropped).
fn qualified_name(rest: &str) -> Option<String> {
    let name: String = rest
        .trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.' | ' ' | '\t'))
        .filter(|c| !c.is_whitespace())
        .collect();
    let name = name.trim_end_matches('.');
    (!name.is_empty() && !name.starts_with('.')).then(|| name.to_string())
}

/// The namespace of a package: organization and project under an umbrella organization
/// (`org.apache.wink`), the organization of other reverse-domain packages (`com.acme`), else the
/// first segment (`hudson`).
fn namespace(package: &str) -> String {
    let segs: Vec<&str> = package.split('.').collect();
    let n = if segs.len() >= 3 && UMBRELLAS.contains(&segs[..2].join(".").as_str()) {
        3
    } else if DOMAIN_SEGMENTS.contains(&segs[0]) {
        segs.len().min(2)
    } else {
        1
    };
    segs[..n].join(".")
}

/// Whether `name` (a package or type) lies in namespace `ns`.
fn in_namespace(name: &str, ns: &str) -> bool {
    name == ns || (name.starts_with(ns) && name.as_bytes().get(ns.len()) == Some(&b'.'))
}

/// Java namespaces outside the project's own that never use the project's code.
fn foreign_packages(files: &[FileFacts]) -> Vec<Suggestion> {
    let mut by_ns: BTreeMap<String, Vec<&FileFacts>> = BTreeMap::new();
    for f in files {
        if let Some(p) = &f.package {
            by_ns.entry(namespace(p)).or_default().push(f);
        }
    }
    let total: usize = by_ns.values().map(Vec::len).sum();
    let largest = by_ns.values().map(Vec::len).max().unwrap_or(0);
    let own: Vec<&str> = by_ns
        .iter()
        .filter(|(_, v)| v.len() * 10 >= total || v.len() == largest)
        .map(|(ns, _)| ns.as_str())
        .collect();
    // The project's names: the last segment of each own namespace (`pdfbox`, `acme`).
    let names: Vec<&str> = own.iter().filter_map(|ns| ns.rsplit('.').next()).collect();
    let own_list = own.join(", ");
    let mut out = Vec::new();
    for (ns, members) in &by_ns {
        let segs: Vec<&str> = ns.split('.').collect();
        let candidate = segs.len() >= 2
            && DOMAIN_SEGMENTS.contains(&segs[0])
            && !PLACEHOLDERS.contains(&segs[1])
            && !PLATFORM
                .iter()
                .any(|p| in_namespace(ns, p) || in_namespace(p, ns))
            && !own.contains(&ns.as_str())
            && !own
                .iter()
                .any(|o| in_namespace(ns, o) || in_namespace(o, ns));
        if !candidate {
            continue;
        }
        let uses_project = members.iter().any(|f| {
            f.imports
                .iter()
                .any(|i| own.iter().any(|o| in_namespace(i, o)))
                || f.package
                    .as_deref()
                    .is_some_and(|p| p.split('.').any(|s| names.contains(&s)))
        });
        if uses_project {
            continue;
        }
        // One glob per source root holding the namespace.
        let mut roots: BTreeMap<String, usize> = BTreeMap::new();
        for f in members {
            let Some(pkg) = &f.package else { continue };
            let dir = f.path.rsplit_once('/').map_or("", |(d, _)| d);
            if dir.split('/').any(|s| s == "resources") {
                continue;
            }
            let pkg_path = pkg.replace('.', "/");
            let root = if dir == pkg_path {
                Some("")
            } else {
                dir.strip_suffix(&format!("/{pkg_path}"))
            };
            if let Some(root) = root {
                *roots.entry(root.to_string()).or_default() += 1;
            }
        }
        let ns_path = ns.replace('.', "/");
        // A module or source set named after the namespace's project (`xmpbox/src/main/java`
        // for `org.apache.xmpbox`, `src/withMicrometerTest/java` for `io.micrometer`) is part
        // of the build on purpose.
        let project = segs
            .last()
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        for (root, n) in roots {
            if project.len() >= 3 && root.to_ascii_lowercase().contains(&project) {
                continue;
            }
            let glob = if root.is_empty() {
                format!("{ns_path}/**")
            } else {
                format!("{root}/{ns_path}/**")
            };
            out.push(Suggestion {
                glob,
                reason: format!(
                    "{n} Java file{} in {ns}.*, which does not use the project's packages ({own_list})",
                    plural(n)
                ),
            });
        }
    }
    out
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Copyright holders named in header comment lines, normalized ([`normalize_holder`]): lines
/// starting with `Copyright`, `(c)` or `©`, the text after a `(c)` or `©` elsewhere, and
/// "Licensed to the <holder> under".
fn holders(header: &[String]) -> Vec<String> {
    const MARKERS: [&str; 3] = ["copyright", "(c)", "©"];
    let mut out: Vec<String> = Vec::new();
    for line in header {
        let lower = line.to_lowercase();
        let holder = if let Some(at) = lower.find("licensed to the ") {
            let rest = &lower[at + "licensed to the ".len()..];
            rest.split(" under").next().map(str::to_string)
        } else if MARKERS.iter().any(|m| lower.starts_with(m))
            || lower.contains("(c)")
            || lower.contains('©')
        {
            MARKERS
                .iter()
                .filter_map(|m| lower.rfind(m).map(|p| p + m.len()))
                .max()
                .map(|p| lower[p..].to_string())
        } else {
            None
        };
        let Some(holder) = holder.map(|h| normalize_holder(&h)) else {
            continue;
        };
        if holder.chars().filter(|c| c.is_alphabetic()).count() >= 2 && !out.contains(&holder) {
            out.push(holder);
        }
    }
    out
}

/// A holder as its significant words: lower case, without years, bracketed parts (e-mail
/// addresses, URLs, abbreviations), legal-form and filler words (`the`, `inc`, `corporation`,
/// `and/or its affiliates`, `contributors`), up to the first word about licensing.
fn normalize_holder(h: &str) -> String {
    const FILLER: &[&str] = &[
        "the",
        "and",
        "or",
        "its",
        "of",
        "to",
        "a",
        "an",
        "by",
        "for",
        "other",
        "others",
        "contributors",
        "contributor",
        "affiliates",
        "affiliate",
        "inc",
        "ltd",
        "llc",
        "corp",
        "corporation",
        "co",
        "company",
        "gmbh",
        "limited",
        "incorporated",
        "ag",
        "sa",
        "sas",
        "srl",
        "bv",
        "plc",
        "pty",
        "copyright",
        "c",
        "all",
        "rights",
        "reserved",
        "is",
        "are",
    ];
    const STOP: &[&str] = &[
        "licensed",
        "license",
        "licence",
        "released",
        "under",
        "see",
        "distributed",
        "use",
        "permission",
        "this",
        "you",
    ];
    let h = h.split('|').next().unwrap_or("");
    let mut cleaned = String::new();
    let mut depth = 0u32;
    for c in h.chars() {
        match c {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => cleaned.push(c),
            _ => {}
        }
    }
    let mut words = Vec::new();
    for w in cleaned.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':')) {
        // Paths, URLs, e-mail addresses, build placeholders (`%%build:year%%`).
        if w.contains('/') || w.contains('@') || w.contains('%') || w.starts_with("www.") {
            continue;
        }
        let w = w.trim_matches(|c: char| !c.is_alphanumeric());
        if STOP.contains(&w) {
            break;
        }
        if w.is_empty() || w.chars().all(|c| c.is_ascii_digit() || c == '-') || FILLER.contains(&w)
        {
            continue;
        }
        words.push(w);
        if words.len() == 6 {
            break;
        }
    }
    words.join(" ")
}

/// Whether holder `h` names `house` (all of its words).
fn names_holder(h: &str, house: &str) -> bool {
    let words: BTreeSet<&str> = h.split(' ').collect();
    house.split(' ').all(|w| words.contains(w))
}

/// Files whose header names only holders other than the project's usual ones.
fn foreign_headers(files: &[FileFacts]) -> Vec<Suggestion> {
    let with_holder: Vec<&FileFacts> = files.iter().filter(|f| !f.holders.is_empty()).collect();
    if with_holder.len() < 10 {
        return Vec::new();
    }
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &with_holder {
        for h in &f.holders {
            *count.entry(h.as_str()).or_default() += 1;
        }
    }
    let house: Vec<&str> = count
        .iter()
        .filter(|(_, n)| **n * 10 >= with_holder.len())
        .map(|(h, _)| *h)
        .collect();
    let is_house_holder = |h: &str| house.iter().any(|x| names_holder(h, x));
    let is_house = |f: &FileFacts| f.holders.iter().any(|h| is_house_holder(h));
    let housed = with_holder.iter().filter(|f| is_house(f)).count();
    if house.is_empty() || housed * 10 < with_holder.len() * 7 {
        return Vec::new();
    }
    // Holders that appear next to a usual holder: contributors.
    let associated: BTreeSet<&str> = with_holder
        .iter()
        .filter(|f| is_house(f))
        .flat_map(|f| f.holders.iter().map(String::as_str))
        .collect();
    let is_foreign = |f: &FileFacts| {
        !f.holders.is_empty()
            && !is_house(f)
            && !f.holders.iter().any(|h| associated.contains(h.as_str()))
    };
    // Per directory (and every ancestor): files of other holders, other files with a holder,
    // files without one.
    #[derive(Default)]
    struct Tally {
        house: usize,
        foreign: usize,
        none: usize,
    }
    let mut dirs: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut foreign: Vec<&FileFacts> = Vec::new();
    for f in files {
        let kind = if f.holders.is_empty() {
            0
        } else if is_foreign(f) {
            foreign.push(f);
            2
        } else {
            1
        };
        let mut dir = f.path.as_str();
        while let Some((parent, _)) = dir.rsplit_once('/') {
            let t = dirs.entry(parent).or_default();
            match kind {
                0 => t.none += 1,
                1 => t.house += 1,
                _ => t.foreign += 1,
            }
            dir = parent;
        }
    }
    let third_party = |d: &str| {
        dirs.get(d)
            .is_some_and(|t| t.house == 0 && t.foreign >= 2 && t.none <= t.foreign)
    };
    // Foreign files grouped by the highest third-party directory above them ("" = none).
    let mut groups: BTreeMap<&str, Vec<&FileFacts>> = BTreeMap::new();
    for f in foreign {
        let mut top = "";
        let mut dir = f.path.as_str();
        while let Some((parent, _)) = dir.rsplit_once('/') {
            if !third_party(parent) {
                break;
            }
            top = parent;
            dir = parent;
        }
        groups.entry(top).or_default().push(f);
    }
    let house_list = holder_list(house.iter().copied());
    let mut out = Vec::new();
    for (top, members) in groups {
        if top.is_empty() || members.len() < 2 {
            for f in members {
                out.push(Suggestion {
                    glob: f.path.clone(),
                    reason: format!(
                        "license header of {}, not of the project's {house_list}",
                        holder_list(f.holders.iter().map(String::as_str))
                    ),
                });
            }
            continue;
        }
        // The deepest directory holding all of the group's files.
        let mut common = members[0].path.rsplit_once('/').map_or("", |(d, _)| d);
        for f in &members[1..] {
            while !(f.path.starts_with(common)
                && f.path.as_bytes().get(common.len()) == Some(&b'/'))
            {
                common = common.rsplit_once('/').map_or("", |(d, _)| d);
            }
        }
        if common.len() < top.len() {
            common = top;
        }
        let holders: BTreeSet<&str> = members
            .iter()
            .flat_map(|f| f.holders.iter().map(String::as_str))
            .collect();
        out.push(Suggestion {
            glob: format!("{common}/**"),
            reason: format!(
                "{} files with a license header of {}, not of the project's {house_list}",
                members.len(),
                holder_list(holders.into_iter())
            ),
        });
    }
    out
}

/// At most three holders, quoted.
fn holder_list<'a>(holders: impl Iterator<Item = &'a str>) -> String {
    let all: Vec<&str> = holders.collect();
    let mut v: Vec<String> = all.iter().take(3).map(|h| format!("\"{h}\"")).collect();
    if all.len() > 3 {
        v.push(format!("{} more", all.len() - 3));
    }
    v.join(", ")
}

/// Sort by glob and drop suggestions inside another suggested directory.
fn dedupe(mut v: Vec<Suggestion>) -> Vec<Suggestion> {
    v.sort_by(|a, b| a.glob.cmp(&b.glob));
    v.dedup_by(|a, b| a.glob == b.glob);
    let dirs: Vec<String> = v
        .iter()
        .filter_map(|s| s.glob.strip_suffix("/**").map(|d| format!("{d}/")))
        .collect();
    v.retain(|s| {
        let path = s.glob.strip_suffix("/**").unwrap_or(&s.glob);
        !dirs.iter().any(|d| path.starts_with(d.as_str()))
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    const ASF: &str = "/*\n * Licensed to the Apache Software Foundation (ASF) under one or more\n * contributor license agreements.  See the NOTICE file distributed with\n * this work for additional information regarding copyright ownership.\n */\n";

    fn java(header: &str, package: &str, imports: &[&str], class: &str) -> String {
        let imports: String = imports.iter().map(|i| format!("import {i};\n")).collect();
        format!("{header}package {package};\n\n{imports}\npublic class {class} {{}}\n")
    }

    fn globs(v: &[Suggestion]) -> Vec<&str> {
        v.iter().map(|s| s.glob.as_str()).collect()
    }

    #[test]
    fn foreign_java_package_inside_the_project() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        for i in 0..12 {
            write(
                root,
                &format!("src/main/java/org/apache/pdfbox/pdmodel/P{i}.java"),
                &java(
                    ASF,
                    "org.apache.pdfbox.pdmodel",
                    &["java.util.List"],
                    &format!("P{i}"),
                ),
            );
        }
        write(
            root,
            "examples/src/test/java/org/apache/pdfbox/examples/ExTest.java",
            &java(
                ASF,
                "org.apache.pdfbox.examples",
                &["org.apache.wink.client.MockHttpServer"],
                "ExTest",
            ),
        );
        // A copied helper: the same license header, another project's namespace, and it never
        // uses the project.
        write(
            root,
            "examples/src/test/java/org/apache/wink/client/MockHttpServer.java",
            &java(
                ASF,
                "org.apache.wink.client",
                &["java.net.ServerSocket"],
                "MockHttpServer",
            ),
        );
        // Not proposed: a module of the project that uses it, a specification the project
        // implements, a placeholder domain, test data below `resources`, single-segment
        // packages, and anything below a directory the starter excludes.
        write(
            root,
            "tools/src/main/java/org/apache/fontbox/Font.java",
            &java(
                ASF,
                "org.apache.fontbox",
                &["org.apache.pdfbox.pdmodel.P1"],
                "Font",
            ),
        );
        write(
            root,
            "api/src/main/java/jakarta/servlet/Servlet.java",
            &java(ASF, "jakarta.servlet", &[], "Servlet"),
        );
        write(
            root,
            "e2e/src/main/java/com/example/app/App.java",
            &java(ASF, "com.example.app", &[], "App"),
        );
        write(
            root,
            "it/src/test/resources/project/src/main/java/nl/someone/Sample.java",
            &java(ASF, "nl.someone", &[], "Sample"),
        );
        write(
            root,
            "samples/src/main/java/woolfel/Dummy.java",
            &java(ASF, "woolfel", &[], "Dummy"),
        );
        // A module named after its namespace, which does not use the main one.
        write(
            root,
            "xmpbox/src/main/java/org/apache/xmpbox/Xmp.java",
            &java(ASF, "org.apache.xmpbox", &[], "Xmp"),
        );
        write(
            root,
            "lib/vendor/com/google/common/base/Strings.java",
            &java("", "com.google.common.base", &[], "Strings"),
        );
        let got = suggest(root, &[] as &[&str]);
        assert_eq!(
            globs(&got),
            vec!["examples/src/test/java/org/apache/wink/**"],
            "{got:#?}"
        );
        assert!(got[0].reason.contains("org.apache.wink.*"), "{got:#?}");
        assert!(got[0].reason.contains("org.apache.pdfbox"), "{got:#?}");
    }

    #[test]
    fn modules_of_the_same_organization_are_not_foreign() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        for i in 0..20 {
            write(
                root,
                &format!("core/src/main/java/org/acme/shop/S{i}.java"),
                &java("", "org.acme.shop", &[], &format!("S{i}")),
            );
        }
        // Another module of the organization that does not import the main one.
        write(
            root,
            "cli/src/main/java/org/acme/cli/Main.java",
            &java("", "org.acme.cli", &["java.util.List"], "Main"),
        );
        // Integration tests named after the project, under another root package.
        write(
            root,
            "it/src/test/java/io/acme/shop/ShopIT.java",
            &java("", "io.acme.shop", &[], "ShopIT"),
        );
        // Single-segment root packages of the project.
        for i in 0..6 {
            write(
                root,
                &format!("core/hudson/model/H{i}.java"),
                &java("", "hudson.model", &[], &format!("H{i}")),
            );
        }
        assert_eq!(suggest(root, &[] as &[&str]), Vec::new());
    }

    fn js(header: &str) -> String {
        format!("{header}export function f() {{ return 1; }}\n")
    }

    #[test]
    fn files_with_another_holders_license_header() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let ms = "/*---\n *  Copyright (c) Microsoft Corporation. All rights reserved.\n *  Licensed under the MIT License.\n *---*/\n";
        for i in 0..20 {
            write(root, &format!("src/vs/base/common/m{i}.ts"), &js(ms));
        }
        // Spelled differently, or with a co-holder: still the project's.
        write(
            root,
            "src/vs/base/common/both.ts",
            &js("// Copyright (c) Microsoft Corporation and GitHub. All rights reserved.\n"),
        );
        // A contributor: named next to the project's holder in one file, alone in another.
        write(
            root,
            "src/vs/base/common/contrib1.ts",
            &js(
                "// Copyright (c) Microsoft Corporation.\n// Copyright 2021 Payara Services Ltd.\n",
            ),
        );
        write(
            root,
            "src/vs/base/common/contrib2.ts",
            &js("// Copyright 2021 Payara Services Ltd. and/or its affiliates\n"),
        );
        // A vendored library directory: files of other holders, nothing of the project.
        write(
            root,
            "src/vs/base/common/marked/lib/marked.js",
            &js("/**\n * marked - a markdown parser\n * Copyright (c) 2011-2022, Christopher Jeffrey. (MIT Licensed)\n */\n"),
        );
        write(
            root,
            "src/vs/base/common/marked/lib/marked.esm.js",
            &js("/**\n * marked v4.0.0\n * Copyright (c) [2011] Christopher Jeffrey. (MIT Licensed)\n */\n"),
        );
        write(root, "src/vs/base/common/marked/index.d.ts", &js(""));
        // A single copied file next to the project's own.
        write(
            root,
            "src/vs/base/common/diff/lcs.js",
            &js("// Copyright 2008 Google Inc.\n// Use of this source code is governed by a BSD-style license\n"),
        );
        write(root, "src/vs/base/common/diff/diff.ts", &js(ms));
        let got = suggest(root, &[] as &[&str]);
        assert_eq!(
            globs(&got),
            vec![
                "src/vs/base/common/diff/lcs.js",
                "src/vs/base/common/marked/lib/**"
            ],
            "{got:#?}"
        );
        assert!(
            got[1].reason.contains("\"christopher jeffrey\""),
            "{got:#?}"
        );
        assert!(got[1].reason.contains("\"microsoft\""), "{got:#?}");
        assert!(got[0].reason.contains("\"google\""), "{got:#?}");
    }

    #[test]
    fn no_usual_holder_means_no_header_suggestions() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        // Every file names its own authors: no usual holder to compare with.
        for i in 0..20 {
            write(
                root,
                &format!("src/f{i}.js"),
                &js(&format!("// Copyright (c) 2020 Author Number{i}\n")),
            );
        }
        assert_eq!(suggest(root, &[] as &[&str]), Vec::new());
    }

    #[test]
    fn holder_normalization() {
        let h = |lines: &[&str]| holders(&lines.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        for (line, want) in [
            (
                "Copyright (c) 2004-2010, Sun Microsystems, Inc., Kohsuke Kawaguchi",
                "sun microsystems kohsuke kawaguchi",
            ),
            (
                "Copyright 2002-2024 the original author or authors.",
                "original author authors",
            ),
            (
                "Licensed to the Apache Software Foundation (ASF) under one",
                "apache software foundation",
            ),
            (
                "Copyright 2020 The Apache Software Foundation",
                "apache software foundation",
            ),
            (
                "(c) 2014-2026 Robin Example <robin@example.com>",
                "robin example",
            ),
            (
                "Copyright (C) 2009 The Android Open Source Project. All Rights Reserved.",
                "android open source project",
            ),
            (
                "Copyright (c) 2020, 2023 Oracle and/or its affiliates. All rights reserved.",
                "oracle",
            ),
            (
                "Copyright (c) 2021 Contributors to the Eclipse Foundation",
                "eclipse foundation",
            ),
            ("Copyright (c) 2020 Paul Irish; Licensed MIT", "paul irish"),
            (
                "jQuery v3.7.1 | (c) OpenJS Foundation and other contributors | jquery.org/license",
                "openjs foundation",
            ),
            ("Copyright (c) [build year] Tim Down", "tim down"),
        ] {
            assert_eq!(h(&[line]), vec![want.to_string()], "{line}");
        }
        // Not holder lines.
        assert_eq!(
            h(&[
                "Copyright",
                "Licensed under the MIT License.",
                "this work for additional information regarding copyright ownership."
            ]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn package_and_imports_parsing() {
        let (p, i) = package_and_imports(
            "/* x */\npackage a.b.c ;\n// c\nimport static a.b.C.m;\nimport x.y.*;\n\npublic class A {}\nimport not.this;\n",
        );
        assert_eq!(p.as_deref(), Some("a.b.c"));
        assert_eq!(i, vec!["a.b.C.m".to_string(), "x.y".to_string()]);
        assert_eq!(namespace("org.apache.wink.client"), "org.apache.wink");
        assert_eq!(
            namespace("org.springframework.ldap.core"),
            "org.springframework"
        );
        assert_eq!(namespace("com.acme"), "com.acme");
        assert_eq!(namespace("org.apache"), "org.apache");
        assert_eq!(namespace("hudson.model"), "hudson");
        assert_eq!(namespace("it.org.apache.struts2"), "it");
        assert!(in_namespace("org.apache.pdfbox.x", "org.apache.pdfbox"));
        assert!(!in_namespace("org.apache.pdfboxes", "org.apache.pdfbox"));
    }
}
