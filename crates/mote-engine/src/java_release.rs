//! The Java language release a file is compiled for.
//!
//! `java.release` in the configuration wins. Otherwise the release is read, for each file, from
//! the nearest build file up the directory tree — `pom.xml`, `build.gradle.kts`, `build.gradle`
//! in the file's directory, then its parent, up to the scan root. A build file that declares
//! nothing (a Maven module inheriting from `../pom.xml`, a Gradle subproject configured by the
//! root project) passes the search on to the next one up. No build file declaring it: unknown.
//!
//! Build files are data: poms are parsed as XML and Gradle scripts are scanned as text. Nothing
//! is resolved beyond the file itself (no parent download, no script evaluation).
//!
//! * Maven (`pom.xml`), in this order: the `maven-compiler-plugin` `<release>`, the
//!   `maven.compiler.release` property, the plugin's `<source>`, `maven.compiler.source`,
//!   `java.version` (the Spring Boot convention), the plugin's `<target>`,
//!   `maven.compiler.target`. The plugin is read from `<build><plugins>` (then
//!   `<pluginManagement>`); executions and profiles are ignored. A value `${name}` is resolved
//!   from the pom's own `<properties>`. The first of these the pom declares decides: if it cannot
//!   be resolved, the pom yields nothing.
//! * Gradle (Groovy or Kotlin DSL), in this order: `options.release = N` / `.set(N)`,
//!   `sourceCompatibility`, a toolchain `languageVersion = JavaLanguageVersion.of(N)`,
//!   `jvmToolchain(N)`, `targetCompatibility`. Values may be written `17`, `'17'`, `"1.8"`,
//!   `JavaVersion.VERSION_17`, `JavaVersion.VERSION_1_8`, `JavaVersion.toVersion(17)` or
//!   `JavaLanguageVersion.of(17)`; a variable is unknown. When a setting appears several times
//!   (e.g. per source set), the lowest value is taken. Comments are ignored.
//!
//! `1.8` is normalized to `8`.

use crate::source::{BlobLoader, SourceEntry};
use quick_xml::events::Event;
use quick_xml::Reader;
use rustc_hash::FxHashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Id of the Java plugin, whose rules receive the release.
pub const JAVA: &str = "java";

/// Name of the parameter carrying the release to every enabled rule of the Java plugin
/// (`ParamValue::Int`). Absent when the release is unknown. Set by the engine only: the
/// configuration cannot set it per rule. The Java plugin reads the same name.
pub const JAVA_RELEASE_PARAM: &str = "java_release";

/// Build files looked up in each directory, in order.
pub const BUILD_FILES: [&str; 3] = ["pom.xml", "build.gradle.kts", "build.gradle"];

/// Larger build files are ignored.
const MAX_BUILD_FILE: usize = 4 << 20;

/// Where a release came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseOrigin {
    /// `java.release` in the configuration.
    Config,
    /// A build file: its path (relative to the scan root, `/`-separated) and the setting read.
    BuildFile { path: String, setting: &'static str },
}

impl fmt::Display for ReleaseOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseOrigin::Config => write!(f, "java.release in the configuration"),
            ReleaseOrigin::BuildFile { path, setting } => write!(f, "{path}: {setting}"),
        }
    }
}

/// The Java release of a file and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaRelease {
    pub release: u32,
    pub origin: ReleaseOrigin,
}

/// Where build files are looked up.
enum BuildFiles {
    /// The build files among a discovery's entries, read through its loader (so a scan of a
    /// revision reads the revision's build files).
    Listed {
        entries: FxHashMap<String, SourceEntry>,
        loader: Arc<dyn BlobLoader>,
    },
    /// Files on disk below this root.
    Disk(PathBuf),
}

/// Per-scan lookup of the Java release of files, memoized per directory and per build file.
pub struct JavaReleases {
    configured: Option<Arc<JavaRelease>>,
    files: BuildFiles,
    by_dir: Mutex<FxHashMap<String, Option<Arc<JavaRelease>>>>,
    parsed: Mutex<FxHashMap<String, Option<(u32, &'static str)>>>,
}

impl JavaReleases {
    fn new(configured: Option<u32>, files: BuildFiles) -> Self {
        JavaReleases {
            configured: configured.map(|release| {
                Arc::new(JavaRelease {
                    release,
                    origin: ReleaseOrigin::Config,
                })
            }),
            files,
            by_dir: Mutex::new(FxHashMap::default()),
            parsed: Mutex::new(FxHashMap::default()),
        }
    }

    /// Build files taken from discovered `entries` (paths relative to the scan root), read with
    /// `loader`. `configured` is `java.release`.
    pub fn listed(
        configured: Option<u32>,
        entries: &[SourceEntry],
        loader: Arc<dyn BlobLoader>,
    ) -> Self {
        let entries = if configured.is_some() {
            FxHashMap::default()
        } else {
            entries
                .iter()
                .filter(|e| BUILD_FILES.contains(&file_name(&e.path)))
                .map(|e| (e.path.clone(), e.clone()))
                .collect()
        };
        Self::new(configured, BuildFiles::Listed { entries, loader })
    }

    /// Build files read from disk below `root`.
    pub fn disk(configured: Option<u32>, root: &Path) -> Self {
        Self::new(configured, BuildFiles::Disk(root.to_path_buf()))
    }

    /// The release of the file at `path` (relative to the scan root, `/`-separated).
    pub fn release_for(&self, path: &str) -> Option<Arc<JavaRelease>> {
        if let Some(c) = &self.configured {
            return Some(c.clone());
        }
        // Climb until a directory whose answer is known or whose build file declares a release;
        // every directory passed on the way shares that answer.
        let mut visited: Vec<&str> = Vec::new();
        let mut d = path.rsplit_once('/').map_or("", |(d, _)| d);
        let found = loop {
            if let Some(r) = lock(&self.by_dir).get(d) {
                break r.clone();
            }
            visited.push(d);
            if let Some(r) = self.declared_in(d) {
                break Some(Arc::new(r));
            }
            if d.is_empty() {
                break None;
            }
            d = d.rsplit_once('/').map_or("", |(p, _)| p);
        };
        let mut memo = lock(&self.by_dir);
        for v in visited {
            memo.entry(v.to_string()).or_insert_with(|| found.clone());
        }
        found
    }

    /// The release declared by a build file in directory `dir` itself.
    fn declared_in(&self, dir: &str) -> Option<JavaRelease> {
        BUILD_FILES.iter().find_map(|name| {
            let path = if dir.is_empty() {
                (*name).to_string()
            } else {
                format!("{dir}/{name}")
            };
            let (release, setting) = self.build_file(&path)?;
            Some(JavaRelease {
                release,
                origin: ReleaseOrigin::BuildFile { path, setting },
            })
        })
    }

    /// What the build file at `path` declares (memoized; `None` when absent or silent).
    fn build_file(&self, path: &str) -> Option<(u32, &'static str)> {
        if let Some(r) = lock(&self.parsed).get(path) {
            return *r;
        }
        let found = self.read(path).and_then(|bytes| {
            let text = String::from_utf8_lossy(&bytes);
            if file_name(path) == "pom.xml" {
                from_pom(&text)
            } else {
                from_gradle(&text)
            }
        });
        lock(&self.parsed).insert(path.to_string(), found);
        found
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let bytes = match &self.files {
            BuildFiles::Listed { entries, loader } => {
                let e = entries.get(path)?;
                if e.size.is_some_and(|s| s > MAX_BUILD_FILE as u64) {
                    return None;
                }
                loader.load(e).ok()?
            }
            BuildFiles::Disk(root) => {
                let full = root.join(path);
                let meta = std::fs::metadata(&full).ok()?;
                if !meta.is_file() || meta.len() > MAX_BUILD_FILE as u64 {
                    return None;
                }
                std::fs::read(full).ok()?
            }
        };
        (bytes.len() <= MAX_BUILD_FILE).then_some(bytes)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A release number from a version string: `17`, `1.8` (→ 8), `11.0`; quotes are ignored.
pub fn normalize(v: &str) -> Option<u32> {
    let v = v.trim().trim_matches(['\'', '"']).trim();
    let v = match v.strip_prefix("1.") {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => v,
    };
    let mut parts = v.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    if !parts.all(|p| !p.is_empty() && p.bytes().all(|b| b == b'0')) {
        return None;
    }
    (1..=99).contains(&major).then_some(major)
}

// ---------------------------------------------------------------------------------------------
// Maven

#[derive(Default)]
struct CompilerPlugin {
    artifact_id: String,
    release: Option<String>,
    source: Option<String>,
    target: Option<String>,
}

const PLUGINS: [&str; 4] = ["project", "build", "plugins", "plugin"];
const MANAGED_PLUGINS: [&str; 5] = ["project", "build", "pluginManagement", "plugins", "plugin"];

fn is_at(stack: &[String], path: &[&str]) -> bool {
    stack.len() == path.len() && stack.iter().zip(path).all(|(a, b)| a == b)
}

/// The release a `pom.xml` declares, with the setting it came from.
pub fn from_pom(text: &str) -> Option<(u32, &'static str)> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<String> = Vec::new();
    let mut props: FxHashMap<String, String> = FxHashMap::default();
    // The plugin being read, whether it is managed, and the compiler plugins found.
    let mut current: Option<(CompilerPlugin, bool)> = None;
    let mut plugin: Option<CompilerPlugin> = None;
    let mut managed: Option<CompilerPlugin> = None;
    loop {
        let text = match reader.read_event() {
            // A malformed pom keeps what was read before the problem.
            Err(_) | Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                stack.push(e.local_name().as_ref().to_string());
                if is_at(&stack, &PLUGINS) {
                    current = Some((CompilerPlugin::default(), false));
                } else if is_at(&stack, &MANAGED_PLUGINS) {
                    current = Some((CompilerPlugin::default(), true));
                }
                continue;
            }
            Ok(Event::End(_)) => {
                if is_at(&stack, &PLUGINS) || is_at(&stack, &MANAGED_PLUGINS) {
                    if let Some((p, is_managed)) = current.take() {
                        if p.artifact_id == "maven-compiler-plugin" {
                            let slot = if is_managed {
                                &mut managed
                            } else {
                                &mut plugin
                            };
                            slot.get_or_insert(p);
                        }
                    }
                }
                stack.pop();
                continue;
            }
            Ok(Event::Text(t)) => t.xml10_content().trim().to_string(),
            Ok(Event::CData(t)) => t.into_inner().trim().to_string(),
            Ok(_) => continue,
        };
        let names: Vec<&str> = stack.iter().map(String::as_str).collect();
        match names.as_slice() {
            ["project", "properties", name] => {
                props.insert((*name).to_string(), text);
            }
            [.., "plugin", "artifactId"] => {
                if let Some((p, _)) = &mut current {
                    p.artifact_id = text;
                }
            }
            [.., "plugin", "configuration", key] => {
                let plugin_depth = names.len() - 2;
                let at_plugin = is_at(&stack[..plugin_depth], &PLUGINS)
                    || is_at(&stack[..plugin_depth], &MANAGED_PLUGINS);
                if let (true, Some((p, _))) = (at_plugin, &mut current) {
                    match *key {
                        "release" => p.release = Some(text),
                        "source" => p.source = Some(text),
                        "target" => p.target = Some(text),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let resolve = |v: &str| -> Option<u32> {
        let mut v = v.trim();
        for _ in 0..4 {
            match v.strip_prefix("${").and_then(|r| r.strip_suffix('}')) {
                Some(name) => v = props.get(name.trim())?.trim(),
                None => break,
            }
        }
        normalize(v)
    };
    let prop = |name: &str| props.get(name).map(String::as_str);
    let from_plugins = |get: fn(&CompilerPlugin) -> Option<&String>| {
        plugin
            .as_ref()
            .and_then(get)
            .or_else(|| managed.as_ref().and_then(get))
            .map(String::as_str)
    };
    let candidates: [(Option<&str>, &'static str); 7] = [
        (
            from_plugins(|p| p.release.as_ref()),
            "maven-compiler-plugin <release>",
        ),
        (prop("maven.compiler.release"), "maven.compiler.release"),
        (
            from_plugins(|p| p.source.as_ref()),
            "maven-compiler-plugin <source>",
        ),
        (prop("maven.compiler.source"), "maven.compiler.source"),
        (prop("java.version"), "java.version"),
        (
            from_plugins(|p| p.target.as_ref()),
            "maven-compiler-plugin <target>",
        ),
        (prop("maven.compiler.target"), "maven.compiler.target"),
    ];
    // The first declared setting decides; unresolvable → nothing from this pom.
    let (value, setting) = candidates.into_iter().find_map(|(v, s)| Some((v?, s)))?;
    resolve(value).map(|r| (r, setting))
}

// ---------------------------------------------------------------------------------------------
// Gradle

/// Gradle settings in priority order: the keyword, the setting's display name.
const GRADLE_SETTINGS: [(&str, &str); 5] = [
    ("release", "options.release"),
    ("sourceCompatibility", "sourceCompatibility"),
    ("languageVersion", "toolchain languageVersion"),
    ("jvmToolchain", "jvmToolchain"),
    ("targetCompatibility", "targetCompatibility"),
];

/// The release a Gradle build script (Groovy or Kotlin DSL) declares, with the setting.
pub fn from_gradle(text: &str) -> Option<(u32, &'static str)> {
    let code = strip_comments(text);
    GRADLE_SETTINGS
        .iter()
        .find_map(|(keyword, setting)| lowest(&code, keyword).map(|v| (v, *setting)))
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// The lowest value assigned to `keyword` in `code`.
fn lowest(code: &str, keyword: &str) -> Option<u32> {
    let bytes = code.as_bytes();
    let mut best: Option<u32> = None;
    let mut from = 0;
    while let Some(i) = code[from..].find(keyword).map(|i| i + from) {
        from = i + keyword.len();
        let end = i + keyword.len();
        if i > 0 && is_ident_byte(bytes[i - 1]) {
            continue;
        }
        if bytes.get(end).is_some_and(|b| is_ident_byte(*b)) {
            continue;
        }
        if keyword == "release" && !code[..i].ends_with("options.") {
            continue;
        }
        if let Some(v) = assigned_value(&code[end..]) {
            best = Some(best.map_or(v, |b| b.min(v)));
        }
    }
    best
}

/// The value after a setting name: `= v`, `.set(v)`, `(v)` or Groovy's `name v`.
fn assigned_value(rest: &str) -> Option<u32> {
    let r = rest.trim_start();
    if let Some(v) = r.strip_prefix(".set(") {
        return value(v);
    }
    if let Some(v) = r.strip_prefix('=') {
        return (!v.starts_with('=')).then(|| value(v)).flatten();
    }
    if let Some(v) = r.strip_prefix('(') {
        return value(v);
    }
    // Groovy command syntax (`sourceCompatibility 1.8`) on the same line.
    if rest.starts_with([' ', '\t'])
        && r.starts_with(|c: char| c.is_ascii_digit() || c == '\'' || c == '"' || c == 'J')
    {
        return value(r);
    }
    None
}

/// A version value: a number, a quoted string, `JavaVersion.VERSION_1_8`,
/// `JavaVersion.toVersion(v)` or `JavaLanguageVersion.of(v)`.
fn value(s: &str) -> Option<u32> {
    let s = s.trim_start();
    let first = s.chars().next()?;
    if first == '\'' || first == '"' {
        let body = &s[1..];
        let end = body.find(first)?;
        return normalize(&body[..end]);
    }
    if first.is_ascii_digit() {
        let end = s
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '_'))
            .unwrap_or(s.len());
        let lit = s[..end].trim_end_matches('.').replace('_', "");
        return normalize(&lit);
    }
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$'))
        .unwrap_or(s.len());
    let token = &s[..end];
    if let Some((_, v)) = token.rsplit_once("JavaVersion.VERSION_") {
        return normalize(&v.replace('_', "."));
    }
    if token.ends_with("JavaVersion.toVersion") || token.ends_with("JavaLanguageVersion.of") {
        return s[end..].trim_start().strip_prefix('(').and_then(value);
    }
    None
}

/// `code` with `//` and `/* */` comments blanked out (string literals kept, so a `/*` inside
/// `'**/*.java'` is not a comment).
fn strip_comments(code: &str) -> String {
    let b = code.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let end = code[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
                out.push(b' ');
                i = end;
            }
            q @ (b'\'' | b'"') => {
                let triple = b.get(i + 1) == Some(&q) && b.get(i + 2) == Some(&q);
                let start = i;
                i += if triple { 3 } else { 1 };
                while i < b.len() {
                    if b[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if triple {
                        if b[i] == q && b.get(i + 1) == Some(&q) && b.get(i + 2) == Some(&q) {
                            i += 3;
                            break;
                        }
                    } else if b[i] == q || b[i] == b'\n' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let end = i.min(b.len());
                out.extend_from_slice(&b[start..end]);
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::EntryLocation;

    #[test]
    fn normalizes_versions() {
        assert_eq!(normalize("17"), Some(17));
        assert_eq!(normalize(" 1.8 "), Some(8));
        assert_eq!(normalize("'11'"), Some(11));
        assert_eq!(normalize("\"21\""), Some(21));
        assert_eq!(normalize("11.0"), Some(11));
        assert_eq!(normalize("1.10"), Some(10));
        assert_eq!(normalize("${java.version}"), None);
        assert_eq!(normalize("17-ea"), None);
        assert_eq!(normalize("0"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("11.0.2"), None);
    }

    fn pom(body: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n\
             <modelVersion>4.0.0</modelVersion>\n{body}\n</project>\n"
        )
    }

    #[test]
    fn pom_properties() {
        let p = pom("<properties><maven.compiler.release>17</maven.compiler.release></properties>");
        assert_eq!(from_pom(&p), Some((17, "maven.compiler.release")));
        let p = pom(
            "<properties><maven.compiler.source>1.8</maven.compiler.source>\
             <maven.compiler.target>1.8</maven.compiler.target></properties>",
        );
        assert_eq!(from_pom(&p), Some((8, "maven.compiler.source")));
        let p = pom("<properties><maven.compiler.target>11</maven.compiler.target></properties>");
        assert_eq!(from_pom(&p), Some((11, "maven.compiler.target")));
        // Spring Boot style, with an external parent.
        let p = pom(
            "<parent><groupId>org.springframework.boot</groupId><relativePath/></parent>\
             <properties><java.version>21</java.version></properties>",
        );
        assert_eq!(from_pom(&p), Some((21, "java.version")));
        // release wins over source.
        let p = pom(
            "<properties><maven.compiler.source>8</maven.compiler.source>\
             <maven.compiler.release>11</maven.compiler.release></properties>",
        );
        assert_eq!(from_pom(&p), Some((11, "maven.compiler.release")));
    }

    #[test]
    fn pom_property_references() {
        let p = pom(
            "<properties><jdk>11</jdk><maven.compiler.source>${jdk}</maven.compiler.source>\
             </properties>",
        );
        assert_eq!(from_pom(&p), Some((11, "maven.compiler.source")));
        // Chained references.
        let p = pom(
            "<properties><a>${b}</a><b>1.8</b><maven.compiler.release>${a}</maven.compiler.release>\
             </properties>",
        );
        assert_eq!(from_pom(&p), Some((8, "maven.compiler.release")));
        // Defined elsewhere (a parent pom): this pom yields nothing, even with a lower-priority
        // setting present.
        let p = pom(
            "<properties><maven.compiler.release>${java.release}</maven.compiler.release>\
             <maven.compiler.target>8</maven.compiler.target></properties>",
        );
        assert_eq!(from_pom(&p), None);
    }

    #[test]
    fn pom_compiler_plugin() {
        let plugin = |cfg: &str| {
            pom(&format!(
                "<properties><java.level>17</java.level></properties>\
                 <build><plugins>\
                   <plugin><groupId>org.apache.maven.plugins</groupId>\
                     <artifactId>maven-surefire-plugin</artifactId>\
                     <configuration><release>9</release></configuration></plugin>\
                   <plugin><groupId>org.apache.maven.plugins</groupId>\
                     <artifactId>maven-compiler-plugin</artifactId>\
                     <version>3.13.0</version>\
                     <configuration>{cfg}</configuration>\
                     <executions><execution><id>java9</id>\
                       <configuration><release>9</release></configuration></execution></executions>\
                   </plugin>\
                 </plugins></build>"
            ))
        };
        assert_eq!(
            from_pom(&plugin("<release>${java.level}</release>")),
            Some((17, "maven-compiler-plugin <release>"))
        );
        assert_eq!(
            from_pom(&plugin("<source>1.8</source><target>1.8</target>")),
            Some((8, "maven-compiler-plugin <source>"))
        );
        // pluginManagement counts too.
        let p = pom("<build><pluginManagement><plugins><plugin>\
               <artifactId>maven-compiler-plugin</artifactId>\
               <configuration><release>11</release></configuration>\
             </plugin></plugins></pluginManagement></build>");
        assert_eq!(from_pom(&p), Some((11, "maven-compiler-plugin <release>")));
        // Profiles are ignored.
        let p = pom(
            "<profiles><profile><properties><maven.compiler.release>21</maven.compiler.release>\
             </properties></profile></profiles>",
        );
        assert_eq!(from_pom(&p), None);
        // Nothing declared, and malformed input.
        assert_eq!(from_pom(&pom("<artifactId>x</artifactId>")), None);
        // A truncated pom keeps what was read.
        assert_eq!(
            from_pom("<project><properties><maven.compiler.release>17"),
            Some((17, "maven.compiler.release"))
        );
        assert_eq!(from_pom("not xml at all"), None);
    }

    #[test]
    fn gradle_groovy() {
        assert_eq!(
            from_gradle("apply plugin: 'java'\nsourceCompatibility = '17'\n"),
            Some((17, "sourceCompatibility"))
        );
        assert_eq!(
            from_gradle("sourceCompatibility = 1.8\ntargetCompatibility = 1.8\n"),
            Some((8, "sourceCompatibility"))
        );
        assert_eq!(
            from_gradle("java {\n  sourceCompatibility = JavaVersion.VERSION_1_8\n}\n"),
            Some((8, "sourceCompatibility"))
        );
        assert_eq!(
            from_gradle("sourceCompatibility = JavaVersion.toVersion('11')"),
            Some((11, "sourceCompatibility"))
        );
        assert_eq!(
            from_gradle("tasks.withType(JavaCompile) {\n  options.release = 17\n}\n"),
            Some((17, "options.release"))
        );
        assert_eq!(
            from_gradle(
                "java { toolchain { languageVersion = JavaLanguageVersion.of(21) } }\n\
                 targetCompatibility = 17\n"
            ),
            Some((21, "toolchain languageVersion"))
        );
        assert_eq!(
            from_gradle("sourceCompatibility 1.8\n"),
            Some((8, "sourceCompatibility"))
        );
        // Several source sets: the lowest.
        assert_eq!(
            from_gradle(
                "compileJava { options.release = 11 }\ncompileTestJava { options.release = 17 }"
            ),
            Some((11, "options.release"))
        );
        // Variables and comments are not values; `/*` inside a string is not a comment.
        assert_eq!(from_gradle("sourceCompatibility = javaVersion\n"), None);
        assert_eq!(
            from_gradle(
                "// sourceCompatibility = 1.8\n/* targetCompatibility = 8 */\n\
                 exclude '**/*.class'\nsourceCompatibility = 17 // was 11\n"
            ),
            Some((17, "sourceCompatibility"))
        );
        assert_eq!(from_gradle("release = 17\nversion = '1.0'\n"), None);
        assert_eq!(from_gradle("if (sourceCompatibility == 8) {}"), None);
        assert_eq!(from_gradle("mySourceCompatibility = 8"), None);
    }

    #[test]
    fn gradle_kotlin_dsl() {
        assert_eq!(
            from_gradle("java {\n    sourceCompatibility = JavaVersion.VERSION_17\n}\n"),
            Some((17, "sourceCompatibility"))
        );
        assert_eq!(
            from_gradle(
                "java {\n    toolchain {\n        languageVersion.set(JavaLanguageVersion.of(\"17\"))\n    }\n}\n"
            ),
            Some((17, "toolchain languageVersion"))
        );
        assert_eq!(
            from_gradle("tasks.withType<JavaCompile> {\n    options.release.set(11)\n}\n"),
            Some((11, "options.release"))
        );
        assert_eq!(
            from_gradle("kotlin {\n    jvmToolchain(21)\n}\n"),
            Some((21, "jvmToolchain"))
        );
        assert_eq!(
            from_gradle("java.sourceCompatibility = org.gradle.api.JavaVersion.VERSION_11"),
            Some((11, "sourceCompatibility"))
        );
        assert_eq!(
            from_gradle("val raw = \"\"\"\n// not a comment sourceCompatibility\n\"\"\"\n"),
            None
        );
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn found(r: Option<Arc<JavaRelease>>) -> Option<(u32, String)> {
        r.map(|r| (r.release, r.origin.to_string()))
    }

    #[test]
    fn nearest_build_file_with_inheritance() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        // Parent pom declares the release; the module pom inherits (declares nothing).
        write(
            root,
            "pom.xml",
            &pom("<properties><maven.compiler.release>17</maven.compiler.release></properties>"),
        );
        write(
            root,
            "core/pom.xml",
            &pom("<parent><artifactId>app</artifactId></parent><artifactId>core</artifactId>"),
        );
        // A module overriding it.
        write(
            root,
            "legacy/pom.xml",
            &pom("<properties><maven.compiler.source>1.8</maven.compiler.source></properties>"),
        );
        let r = JavaReleases::disk(None, root);
        assert_eq!(
            found(r.release_for("core/src/main/java/a/A.java")),
            Some((17, "pom.xml: maven.compiler.release".into()))
        );
        assert_eq!(
            found(r.release_for("legacy/src/main/java/a/B.java")),
            Some((8, "legacy/pom.xml: maven.compiler.source".into()))
        );
        assert_eq!(
            found(r.release_for("Top.java")),
            Some((17, "pom.xml: maven.compiler.release".into()))
        );
        // The configuration wins.
        let c = JavaReleases::disk(Some(21), root);
        assert_eq!(
            found(c.release_for("legacy/src/main/java/a/B.java")),
            Some((21, "java.release in the configuration".into()))
        );
    }

    #[test]
    fn parentless_multi_module_layout() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        // No build file at the root; each module stands alone.
        write(
            root,
            "service-a/pom.xml",
            &pom("<properties><maven.compiler.release>11</maven.compiler.release></properties>"),
        );
        write(
            root,
            "service-b/build.gradle.kts",
            "plugins { java }\njava { sourceCompatibility = JavaVersion.VERSION_21 }\n",
        );
        write(root, "service-c/build.gradle", "apply plugin: 'java'\n");
        let r = JavaReleases::disk(None, root);
        assert_eq!(
            found(r.release_for("service-a/src/main/java/A.java")),
            Some((11, "service-a/pom.xml: maven.compiler.release".into()))
        );
        assert_eq!(
            found(r.release_for("service-b/src/main/java/B.java")),
            Some((21, "service-b/build.gradle.kts: sourceCompatibility".into()))
        );
        assert_eq!(found(r.release_for("service-c/src/main/java/C.java")), None);
        assert_eq!(found(r.release_for("tools/D.java")), None);
    }

    #[test]
    fn gradle_root_configures_subprojects() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        write(
            root,
            "build.gradle",
            "subprojects {\n  apply plugin: 'java'\n  sourceCompatibility = '11'\n}\n",
        );
        write(root, "settings.gradle", "include 'api'\n");
        write(
            root,
            "api/build.gradle",
            "dependencies { implementation 'x:y:1' }\n",
        );
        let r = JavaReleases::disk(None, root);
        assert_eq!(
            found(r.release_for("api/src/main/java/A.java")),
            Some((11, "build.gradle: sourceCompatibility".into()))
        );
    }

    struct MapLoader(FxHashMap<String, String>);

    impl BlobLoader for MapLoader {
        fn load(&self, entry: &SourceEntry) -> std::io::Result<Vec<u8>> {
            self.0
                .get(&entry.path)
                .map(|t| t.clone().into_bytes())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
        fn load_blob(&self, _: &str) -> std::io::Result<Option<Vec<u8>>> {
            Ok(None)
        }
    }

    #[test]
    fn listed_build_files_come_from_the_loader() {
        let files: FxHashMap<String, String> = [
            (
                "m/pom.xml".to_string(),
                pom("<properties><java.version>17</java.version></properties>"),
            ),
            ("m/src/A.java".to_string(), "class A {}".to_string()),
        ]
        .into_iter()
        .collect();
        let entries: Vec<SourceEntry> = files
            .keys()
            .map(|p| SourceEntry {
                path: p.clone(),
                blob_id: None,
                size: None,
                location: EntryLocation::Object,
            })
            .collect();
        let r = JavaReleases::listed(None, &entries, Arc::new(MapLoader(files)));
        assert_eq!(
            found(r.release_for("m/src/A.java")),
            Some((17, "m/pom.xml: java.version".into()))
        );
        assert_eq!(found(r.release_for("other/B.java")), None);
    }
}
