//! Vendored and bundled JavaScript: copies of libraries and bundler output committed to a
//! repository (`public/js/jquery-3.7.1.js`, `client-dist/socket.io.js`, a webpack build in
//! `static/`). With `skip_generated`, such files are skipped as `vendored`, like generated and
//! minified files: their findings belong to the library or the build, not to the code the team
//! writes.
//!
//! Only strong signals count — never a directory name alone:
//!
//! * a **library banner**: the file starts with a comment that minifiers preserve (`/*! … */`,
//!   `//! …` lines, or `/** @license … */`) naming a release (`v1.2`, `4.8.4`) and a copyright
//!   or license (`/*! jQuery v3.7.1 | (c) OpenJS Foundation | jquery.org/license */`), or with a
//!   plain block comment naming all three — a library release that is not a license's version
//!   (`version 1.7.3`, `Lib v1.2`, `Lib 1.2.3`, but not `Apache License, Version 2.0`), a
//!   copyright and a license (`/*  Prototype JavaScript framework, version 1.7.3 (c) 2005-2010
//!   Sam Stephenson … MIT-style license */`);
//! * webpack's **license pointer** `/*! For license information please see main.js.LICENSE.txt */`;
//! * a **bundler runtime**: webpack's bootstrap (`/******/` lines with `webpackBootstrap` or
//!   `__webpack_…` names) or a webpack chunk (`(self["webpackChunk…"] = …).push(…)`), Parcel's
//!   prelude (`$parcel$…` helpers, `parcelRequire = (function (modules, …`), Browserify's
//!   prelude, or two or more esbuild module markers (`// node_modules/pkg/index.js` lines);
//! * a **source-mapped bundle**: a trailing `//# sourceMappingURL=` comment on a file of at least
//!   2 KB whose code is a single wrapped expression — an IIFE or UMD wrapper (`(function (global,
//!   factory) { … }(this, …));`, `!function(e){…}(…)`, `(() => { … })();`) that spans at least
//!   90% of the code. Compiled first-party modules (`tsc` output, which also ends with a
//!   source-map comment) consist of many top-level statements and are not matched.
//!
//! **The project's own banner.** A library banner whose library name is the project's own — the
//! `name` of the nearest `package.json` above the file or of the root one ([`crate::packages`];
//! compared case-insensitively, punctuation and a trailing `js` ignored, a scoped
//! `@scope/name` also by `name`) — is the project's release banner on its own source, so it does
//! not count: the file is judged by the other signals alone, exactly like a file without a
//! banner. Build output stays skipped on its own evidence: socket.io's committed
//! `packages/socket.io/client-dist/socket.io.js` names "Socket.IO" in its banner and its nearest
//! `package.json` is `socket.io`, but it is one UMD wrapper ending with a source-map comment (a
//! source-mapped bundle).
//!
//! Applies to `.js`, `.mjs` and `.cjs` files. Every check reads the first few kilobytes and the
//! last line, except the esbuild markers (one substring search) and the wrapper span (only for
//! files ending with a source-map comment).

/// What marked a file as vendored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VendoredSignal {
    /// A preserved library banner naming a release and a copyright or license.
    LibraryBanner,
    /// webpack's `/*! For license information please see … */` pointer.
    WebpackLicense,
    /// webpack's bootstrap or a webpack chunk.
    Webpack,
    /// Parcel's prelude.
    Parcel,
    /// Browserify's prelude.
    Browserify,
    /// esbuild's `// node_modules/…` module markers.
    Esbuild,
    /// A trailing source-map comment on a single wrapped expression.
    SourceMappedBundle,
}

impl VendoredSignal {
    /// Short kebab-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            VendoredSignal::LibraryBanner => "library-banner",
            VendoredSignal::WebpackLicense => "webpack-license",
            VendoredSignal::Webpack => "webpack",
            VendoredSignal::Parcel => "parcel",
            VendoredSignal::Browserify => "browserify",
            VendoredSignal::Esbuild => "esbuild",
            VendoredSignal::SourceMappedBundle => "source-mapped-bundle",
        }
    }
}

/// File extensions the check applies to.
const JS_EXTS: &[&str] = &["js", "mjs", "cjs"];

/// Bytes of the file's start searched for bundler preludes.
const HEAD_BYTES: usize = 4096;

/// Longest banner comment read.
const BANNER_BYTES: usize = 2048;

/// Smallest source-mapped bundle (smaller files are test fixtures and stubs).
const MIN_BUNDLE_BYTES: usize = 2048;

/// Whether the vendored check applies to files at `path` (`.js`, `.mjs`, `.cjs`).
pub fn applies_to(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    JS_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e))
}

/// Why `text` (at `path`) is vendored or bundled JavaScript, if it is (without knowing the
/// project's package names).
pub fn vendored_signal(path: &str, text: &str) -> Option<VendoredSignal> {
    vendored_signal_for::<&str>(path, text, &[])
}

/// Why `text` (at `path`) is vendored or bundled JavaScript, if it is; `packages` are the names
/// of the npm packages the file belongs to ([`crate::packages`]): a library banner naming one of
/// them does not count.
pub fn vendored_signal_for<S: AsRef<str>>(
    path: &str,
    text: &str,
    packages: &[S],
) -> Option<VendoredSignal> {
    if !applies_to(path) {
        return None;
    }
    let bytes = text.as_bytes();
    let first = first_comment(bytes);
    if first.starts_with(b"/*! For license information please see") {
        return Some(VendoredSignal::WebpackLicense);
    }
    if (is_library_banner(first) || is_plain_library_banner(first))
        && !names_a_package(first, packages)
    {
        return Some(VendoredSignal::LibraryBanner);
    }
    let head = &bytes[..floor_boundary(text, HEAD_BYTES)];
    let start = code_start(bytes);
    let code = &bytes[start..];
    if is_webpack(head, code) {
        return Some(VendoredSignal::Webpack);
    }
    if is_parcel(head) {
        return Some(VendoredSignal::Parcel);
    }
    if is_browserify(code) {
        return Some(VendoredSignal::Browserify);
    }
    if esbuild_markers(text) >= 2 {
        return Some(VendoredSignal::Esbuild);
    }
    if is_source_mapped_bundle(bytes, start) {
        return Some(VendoredSignal::SourceMappedBundle);
    }
    None
}

/// The largest char boundary of `text` at or below `n`.
fn floor_boundary(text: &str, n: usize) -> usize {
    let mut n = n.min(text.len());
    while !text.is_char_boundary(n) {
        n -= 1;
    }
    n
}

fn skip_ws(t: &[u8], mut i: usize) -> usize {
    while i < t.len() && t[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn line_end(t: &[u8], i: usize) -> usize {
    t[i..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(t.len(), |p| i + p)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The file's first comment (after a `#!` line and blank lines): a whole `/* … */` block (empty
/// when it does not close within [`BANNER_BYTES`]), or a run of `//!` lines. Empty otherwise.
fn first_comment(t: &[u8]) -> &[u8] {
    let mut i = 0;
    if t.starts_with(b"#!") {
        i = line_end(t, 0);
    }
    i = skip_ws(t, i);
    let rest = &t[i..];
    if rest.starts_with(b"/*") {
        let window = &rest[..rest.len().min(BANNER_BYTES)];
        return match find(&window[2..], b"*/") {
            Some(end) => &rest[..end + 4],
            None => &[],
        };
    }
    if rest.starts_with(b"//!") {
        let mut end = 0;
        while end < rest.len().min(BANNER_BYTES) && rest[end..].starts_with(b"//!") {
            end = skip_ws(rest, line_end(rest, end));
        }
        return &rest[..end];
    }
    &[]
}

/// A comment minifiers preserve that names a release and a copyright or license.
fn is_library_banner(c: &[u8]) -> bool {
    let preserved = c.starts_with(b"/*!")
        || c.starts_with(b"//!")
        || ((c.starts_with(b"/**") || c.starts_with(b"/*"))
            && c[..c.len().min(40)]
                .windows(8)
                .any(|w| w.eq_ignore_ascii_case(b"@license")));
    if !preserved {
        return false;
    }
    let lower = c.to_ascii_lowercase();
    let legal = find(&lower, b"(c)").is_some()
        || find(&lower, "©".as_bytes()).is_some()
        || find(&lower, b"copyright").is_some()
        || find(&lower, b"licen").is_some()
        || find(&lower, b"released under").is_some()
        || has_word(&lower, b"mit");
    legal && has_release(c)
}

/// A plain block comment (`/* … */`, `/** … */`) at the top of the file that names a library
/// release (`Prototype JavaScript framework, version 1.7.3`, `Lib v1.2`, `Lib 1.2.3`), a
/// copyright (`(c)`, `©`, `Copyright`) and a license. Stricter than a preserved banner: all
/// three, and a license's own version (`Apache License, Version 2.0`, `GPL version 2.1`) is not
/// a release.
fn is_plain_library_banner(c: &[u8]) -> bool {
    if !c.starts_with(b"/*") {
        return false;
    }
    let lower = c.to_ascii_lowercase();
    let copyright = find(&lower, b"(c)").is_some()
        || find(&lower, "©".as_bytes()).is_some()
        || find(&lower, b"copyright").is_some();
    let license = find(&lower, b"licen").is_some()
        || find(&lower, b"released under").is_some()
        || find(&lower, b"distributable under").is_some()
        || has_word(&lower, b"mit")
        || has_word(&lower, b"bsd")
        || has_word(&lower, b"gpl");
    copyright && license && has_library_release(&lower)
}

/// Whether the banner comment `c` names one of `packages` (the project's own npm packages).
fn names_a_package<S: AsRef<str>>(c: &[u8], packages: &[S]) -> bool {
    if packages.is_empty() {
        return false;
    }
    let name = name_key(&banner_name(c));
    !name.is_empty()
        && packages.iter().any(|p| {
            let p = p.as_ref();
            name_key(p) == name
                || p.rsplit_once('/')
                    .is_some_and(|(_, bare)| name_key(bare) == name)
        })
}

/// A library or package name for comparison: its letters and digits, lower case, without a
/// trailing `js` (`Socket.IO` and `socket.io` → `socketio`, `Chart.js` and `chart.js` → `chart`,
/// `@acme/widgets` → `acmewidgets`).
fn name_key(name: &str) -> String {
    let mut k: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if k.len() > 3 && k.ends_with("js") {
        k.truncate(k.len() - 2);
    }
    k
}

/// The library name a banner starts with: its first line of text up to the release (`jQuery` in
/// `/*! jQuery v3.7.1 | … */`, `Socket.IO` in `Socket.IO v4.8.4`, `Prototype JavaScript
/// framework` in `Prototype JavaScript framework, version 1.7.3`), or that whole line when the
/// release is on a later one (`moment.js` above `//! version : 2.29.4`); cut at `|`, `(`, `,` and
/// ` - `, without a leading `@license` / `@preserve`.
fn banner_name(c: &[u8]) -> String {
    let text = String::from_utf8_lossy(c);
    for line in text.lines() {
        let mut l = line
            .trim()
            .trim_start_matches(['/', '*', '!'])
            .trim()
            .trim_end_matches("*/")
            .trim();
        for tag in ["@license", "@preserve"] {
            if l.get(..tag.len())
                .is_some_and(|p| p.eq_ignore_ascii_case(tag))
            {
                l = l[tag.len()..].trim();
            }
        }
        if l.chars().any(char::is_alphanumeric) {
            return library_name(l);
        }
    }
    String::new()
}

/// The words of a banner line before its release number (see [`banner_name`]).
fn library_name(line: &str) -> String {
    let cut = [" - ", " – ", " — ", "|", "(", ","]
        .iter()
        .filter_map(|sep| line.find(sep))
        .min()
        .unwrap_or(line.len());
    let mut words: Vec<&str> = Vec::new();
    for w in line[..cut].split_whitespace() {
        let bare = w.trim_end_matches([':', ';']);
        let number = bare.strip_prefix(['v', 'V']).unwrap_or(bare);
        if bare.eq_ignore_ascii_case("version")
            || (number.starts_with(|c: char| c.is_ascii_digit()) && number.contains('.'))
        {
            break;
        }
        // `name@1.2.3`
        if let Some((name, version)) = w.split_once('@').filter(|(n, _)| !n.is_empty()) {
            if version.starts_with(|c: char| c.is_ascii_digit()) {
                words.push(name);
                break;
            }
        }
        words.push(w);
    }
    words
        .join(" ")
        .trim_end_matches([':', '-', ' '])
        .to_string()
}

/// Words that make a number before or after them a license's or standard's version.
const LICENSE_CONTEXT: &[&[u8]] = &[
    b"licen",
    b"gpl",
    b"mpl",
    b"epl",
    b"apache",
    b"public",
    b"bsd",
    b"creative",
    b"cc-by",
    b"cc by",
    b"cddl",
    b"(the",
];

/// Whether the lower-case comment `c` names a release that is not a license's version: `vX.Y`
/// or `version X.Y` (also `version: X.Y`), or a bare `X.Y.Z`; with no license word in the 48
/// bytes before it or the 16 after it.
fn has_library_release(c: &[u8]) -> bool {
    let mut i = 0;
    while i < c.len() {
        let at_word = i == 0 || !(c[i - 1].is_ascii_alphanumeric() || c[i - 1] == b'.');
        if !at_word || !(c[i].is_ascii_digit() || c[i] == b'v') {
            i += 1;
            continue;
        }
        let v = c[i] == b'v';
        let mut j = i + usize::from(v);
        let mut parts = 0;
        loop {
            let digits = c[j..].iter().take_while(|b| b.is_ascii_digit()).count();
            if digits == 0 {
                break;
            }
            parts += 1;
            j += digits;
            if c.get(j) == Some(&b'.') && c.get(j + 1).is_some_and(u8::is_ascii_digit) {
                j += 1;
            } else {
                break;
            }
        }
        if parts == 0 {
            i += 1;
            continue;
        }
        let ends_word = !c.get(j).is_some_and(|b| b.is_ascii_alphabetic());
        let before = &c[i.saturating_sub(48)..i];
        let after = &c[j..c.len().min(j + 16)];
        let after_version = {
            let t = before.trim_ascii_end();
            let t = t
                .strip_suffix(b":")
                .or_else(|| t.strip_suffix(b"="))
                .unwrap_or(t);
            t.trim_ascii_end().ends_with(b"version")
        };
        let release = (v && parts >= 2) || (after_version && parts >= 2) || parts >= 3;
        let license_context = LICENSE_CONTEXT
            .iter()
            .any(|w| find(before, w).is_some() || find(after, w).is_some())
            || has_word(before, b"mit");
        if ends_word && release && !license_context {
            return true;
        }
        i = j.max(i + 1);
    }
    false
}

/// Whether `word` occurs in `t` as a whole word.
fn has_word(t: &[u8], word: &[u8]) -> bool {
    let ident = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
    let mut from = 0;
    while let Some(p) = find(&t[from..], word) {
        let at = from + p;
        let end = at + word.len();
        if !(at > 0 && ident(&t[at - 1])) && !t.get(end).is_some_and(ident) {
            return true;
        }
        from = at + 1;
    }
    false
}

/// Whether `c` holds a release number: `v1.2`, `v1.2.3`, or `1.2.3` (two-part numbers without a
/// `v`, such as "License 2.0", do not count).
fn has_release(c: &[u8]) -> bool {
    let mut i = 0;
    while i < c.len() {
        let at_word = i == 0 || !(c[i - 1].is_ascii_alphanumeric() || c[i - 1] == b'.');
        if !at_word {
            i += 1;
            continue;
        }
        let v = c[i] == b'v' || c[i] == b'V';
        let mut j = i + usize::from(v);
        let mut parts = 0;
        loop {
            let digits = c[j..].iter().take_while(|b| b.is_ascii_digit()).count();
            if digits == 0 {
                break;
            }
            parts += 1;
            j += digits;
            if c.get(j) == Some(&b'.') && c.get(j + 1).is_some_and(u8::is_ascii_digit) {
                j += 1;
            } else {
                break;
            }
        }
        let ends_word = !c.get(j).is_some_and(|b| b.is_ascii_alphabetic());
        if ends_word && (parts >= 3 || (v && parts >= 2)) {
            return true;
        }
        i = j.max(i + 1);
    }
    false
}

/// Offset of the first code byte: after a `#!` line, comments, blank lines and directive
/// prologues (`"use strict";`).
fn code_start(t: &[u8]) -> usize {
    let mut i = 0;
    if t.starts_with(b"#!") {
        i = line_end(t, 0);
    }
    loop {
        i = skip_ws(t, i);
        let rest = &t[i..];
        if rest.starts_with(b"//") {
            i = line_end(t, i);
        } else if rest.starts_with(b"/*") {
            match find(&rest[2..], b"*/") {
                Some(end) => i += end + 4,
                None => return t.len(),
            }
        } else if rest.starts_with(b"\"use strict\"") || rest.starts_with(b"'use strict'") {
            i += "'use strict'".len();
            let j = skip_ws(t, i);
            if t.get(j) == Some(&b';') {
                i = j + 1;
            }
        } else {
            return i;
        }
    }
}

/// webpack's bootstrap in the first lines, or a webpack chunk.
fn is_webpack(head: &[u8], code: &[u8]) -> bool {
    let bootstrap = head
        .split(|&b| b == b'\n')
        .any(|l| l.starts_with(b"/******/"))
        && (find(head, b"webpackBootstrap").is_some() || find(head, b"__webpack_").is_some());
    bootstrap
        || [
            &b"(self[\"webpackChunk"[..],
            b"(self.webpackChunk",
            b"(globalThis[\"webpackChunk",
            b"(globalThis.webpackChunk",
            b"(window[\"webpackJsonp",
            b"(window.webpackJsonp",
            b"(this[\"webpackJsonp",
        ]
        .iter()
        .any(|p| code.starts_with(p))
}

/// Parcel's prelude: its `$parcel$` helpers, or Parcel 1's module loader.
fn is_parcel(head: &[u8]) -> bool {
    head.split(|&b| b == b'\n').any(|l| {
        let l = &l[skip_ws(l, 0)..];
        l.starts_with(b"function $parcel$")
            || l.starts_with(b"var $parcel$")
            || l.starts_with(b"parcelRequire = (function (modules, cache, entry, globalName)")
    })
}

/// Browserify's prelude at the start of the code.
fn is_browserify(code: &[u8]) -> bool {
    [
        &b"(function(){function r(e,n,t){function o(i,f){"[..],
        b"require=(function e(t,n,r){function s(o,u){",
        b"(function e(t,n,r){function s(o,u){",
    ]
    .iter()
    .any(|p| code.starts_with(p))
}

/// Lines that are only an esbuild module marker: `// node_modules/<path>.<js-like extension>`.
fn esbuild_markers(text: &str) -> usize {
    const EXTS: &[&str] = &[
        ".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts", ".tsx", ".json",
    ];
    let mut n = 0;
    for (at, _) in text.match_indices("\n// node_modules/") {
        let line = &text[at + 4..];
        let line = &line[..line.find('\n').unwrap_or(line.len())];
        let line = line.trim_end_matches('\r');
        if !line.contains(char::is_whitespace) && EXTS.iter().any(|e| line.ends_with(e)) {
            n += 1;
            if n >= 2 {
                break;
            }
        }
    }
    n
}

/// A trailing source-map comment on code that is one wrapped expression spanning at least 90%
/// of it.
fn is_source_mapped_bundle(t: &[u8], start: usize) -> bool {
    let trimmed = t.trim_ascii_end();
    let last = trimmed
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    let last_line = &trimmed[last..];
    let source_map = [
        &b"//# sourceMappingURL="[..],
        b"//@ sourceMappingURL=",
        b"/*# sourceMappingURL=",
    ]
    .iter()
    .any(|p| last_line.starts_with(p));
    if !source_map || last <= start {
        return false;
    }
    let end = start + t[start..last].trim_ascii_end().len();
    if end - start < MIN_BUNDLE_BYTES {
        return false;
    }
    let code = &t[start..end];
    let wrapper = [
        &b"("[..],
        b"!function",
        b";(",
        b";!function",
        b"+function",
        b"~function",
        b"void function",
    ]
    .iter()
    .any(|p| code.starts_with(p));
    wrapper && first_statement_len(code) * 10 >= code.len() * 9
}

/// Length of the first statement of `code`: up to and including the first `;` outside brackets,
/// strings, template literals, regular expressions and comments (all of `code` without one).
/// A lexical approximation, good enough to tell one wrapped expression from a list of
/// statements.
fn first_statement_len(code: &[u8]) -> usize {
    #[derive(Clone, Copy)]
    enum Mode {
        /// Inside a template literal.
        Template,
        /// Inside `${ … }` of a template literal, with the braces opened since.
        Interp(u32),
    }
    let mut modes: Vec<Mode> = Vec::new();
    let mut depth: u32 = 0;
    // The last significant byte in code, to tell a regex literal from a division.
    let mut prev: u8 = b'(';
    let mut i = 0;
    let n = code.len();
    while i < n {
        let c = code[i];
        if let Some(Mode::Template) = modes.last() {
            match c {
                b'\\' => i += 1,
                b'`' => {
                    modes.pop();
                    prev = b'`';
                }
                b'$' if code.get(i + 1) == Some(&b'{') => {
                    modes.push(Mode::Interp(0));
                    i += 1;
                }
                _ => {}
            }
            i += 1;
            continue;
        }
        match c {
            b'/' if code.get(i + 1) == Some(&b'/') => {
                i = line_end(code, i);
                continue;
            }
            b'/' if code.get(i + 1) == Some(&b'*') => {
                i = find(&code[i + 2..], b"*/").map_or(n, |e| i + 2 + e + 2);
                continue;
            }
            b'/' if b"(,=:[!&|?{};+-*%<>~^".contains(&prev) => {
                i = skip_regex(code, i);
                prev = b'/';
                continue;
            }
            b'\'' | b'"' => {
                i = skip_string(code, i);
                prev = c;
                continue;
            }
            b'`' => modes.push(Mode::Template),
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'{' => {
                depth += 1;
                if let Some(Mode::Interp(open)) = modes.last_mut() {
                    *open += 1;
                }
            }
            b'}' => match modes.last_mut() {
                Some(Mode::Interp(0)) => {
                    modes.pop();
                }
                Some(Mode::Interp(open)) => {
                    *open -= 1;
                    depth = depth.saturating_sub(1);
                }
                _ => depth = depth.saturating_sub(1),
            },
            b';' if depth == 0 && modes.is_empty() => return i + 1,
            _ => {}
        }
        if !c.is_ascii_whitespace() {
            prev = c;
        }
        i += 1;
    }
    n
}

/// Index after the string literal starting at `i` (or at the line's end when unterminated).
fn skip_string(t: &[u8], i: usize) -> usize {
    let q = t[i];
    let mut j = i + 1;
    while j < t.len() {
        match t[j] {
            b'\\' => j += 2,
            b'\n' => return j,
            b if b == q => return j + 1,
            _ => j += 1,
        }
    }
    t.len()
}

/// Index after the regular expression literal starting at `i`; a `/` without a closing one on
/// its line is taken as a division (index after the `/`).
fn skip_regex(t: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    let mut class = false;
    while j < t.len() {
        match t[j] {
            b'\\' => j += 1,
            b'\n' => return i + 1,
            b'[' => class = true,
            b']' => class = false,
            b'/' if !class => return j + 1,
            _ => {}
        }
        j += 1;
    }
    i + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(path: &str, text: &str) -> Option<&'static str> {
        vendored_signal(path, text).map(VendoredSignal::as_str)
    }

    fn signal_for(path: &str, text: &str, packages: &[&str]) -> Option<&'static str> {
        vendored_signal_for(path, text, packages).map(VendoredSignal::as_str)
    }

    #[test]
    fn banner_names() {
        for (c, want) in [
            ("/*! jQuery v3.7.1 | (c) OpenJS Foundation */", "jQuery"),
            ("/*!\n * Socket.IO v4.8.4\n * (c) x\n */", "Socket.IO"),
            ("/*! @license DOMPurify 3.4.15 | (c) Cure53 */", "DOMPurify"),
            ("/** @license React v16.14.0\n * react.production.min.js\n */", "React"),
            ("//! moment.js\n//! version : 2.29.4\n", "moment.js"),
            (
                "/*  Prototype JavaScript framework, version 1.7.3\n *  (c) 2005-2010 Sam Stephenson */",
                "Prototype JavaScript framework",
            ),
            (
                "/*\n * jQuery Cycle Plugin (with Transition Definitions)\n * Version: 2.88\n */",
                "jQuery Cycle Plugin",
            ),
            ("/**\n * Canteen v1.0.4\n */", "Canteen"),
            ("/*! mylib - v1.2.3 - 2020-01-01 */", "mylib"),
            ("/*! lodash@4.17.21 | MIT */", "lodash"),
            ("/*! vue.js v2.6.14 (c) Evan You */", "vue.js"),
            ("/*! @acme/widgets 1.0.0 | MIT */", "@acme/widgets"),
            ("/*! */", ""),
            ("", ""),
        ] {
            assert_eq!(banner_name(c.as_bytes()), want, "{c}");
        }
        for (a, b) in [
            ("Socket.IO", "socket.io"),
            ("Chart.js", "chart.js"),
            ("Chart.js", "chartjs"),
            ("moment.js", "moment"),
            (
                "Prototype JavaScript framework",
                "prototype-javascript-framework",
            ),
        ] {
            assert_eq!(name_key(a), name_key(b), "{a} / {b}");
        }
        assert_ne!(name_key("Socket.IO"), name_key("socket.io-client"));
        assert_eq!(name_key("cjs"), "cjs");
    }

    #[test]
    fn the_projects_own_banner_does_not_count() {
        let own = "/*! mylib v1.2.3 | (c) Me | MIT */\nexport function a() {}\n";
        assert_eq!(signal_for("src/mylib.js", own, &["mylib"]), None);
        assert_eq!(signal_for("src/mylib.js", own, &["app", "mylib"]), None);
        assert_eq!(
            signal_for("src/mylib.js", own, &["other"]),
            Some("library-banner")
        );
        assert_eq!(signal_for("src/mylib.js", own, &[]), Some("library-banner"));
        // Case, punctuation, a trailing `js`, scoped names.
        for (banner, package) in [
            ("/*!\n * Socket.IO v4.8.4\n * (c) 2014-2026 Guillermo Rauch\n * Released under the MIT License.\n */\n", "socket.io"),
            ("/*! Chart.js v4.4.0 | (c) Chart.js Contributors | MIT */\n", "chart.js"),
            ("/*! @license Chart.js v4.4.0 | MIT */\n", "chartjs"),
            ("/*! @acme/widgets v1.0.0 | (c) Acme | MIT */\n", "@acme/widgets"),
            ("/*! Widgets v1.0.0 | (c) Acme | MIT */\n", "@acme/widgets"),
            ("//! moment.js\n//! version : 2.29.4\n//! license : MIT\n", "moment"),
            ("/*  Prototype JavaScript framework, version 1.7.3\n *  (c) 2005-2010 Sam Stephenson\n *  Prototype is freely distributable under the terms of an MIT-style license.\n */\n", "prototype-javascript-framework"),
        ] {
            let text = format!("{banner}export const a = 1;\n");
            assert_eq!(signal("lib/x.js", &text), Some("library-banner"), "{banner}");
            assert_eq!(signal_for("lib/x.js", &text, &[package]), None, "{banner}");
        }
        // Another library's banner in the project: still vendored.
        for (banner, package) in [
            ("/*! jQuery Browser Plugin v0.1.0 | (c) Gabriel Cebrian | MIT */\n", "jquery"),
            ("/*! jQuery v3.7.1 | (c) OpenJS Foundation | jquery.org/license */\n", "jquery-ui"),
            ("/*!\n * Socket.IO v4.8.4\n * (c) 2014-2026 Guillermo Rauch\n * Released under the MIT License.\n */\n", "socket.io-client"),
            ("/*! @license DOMPurify 3.4.15 | (c) Cure53 and other contributors | Released under the Apache license 2.0 */\n", "code-oss-dev"),
        ] {
            let text = format!("{banner}export const a = 1;\n");
            assert_eq!(signal_for("lib/x.js", &text, &[package]), Some("library-banner"), "{banner}");
        }
    }

    #[test]
    fn the_projects_own_build_output_stays_skipped() {
        // Shaped like socket.io's committed `packages/socket.io/client-dist/socket.io.js`: the
        // banner names the package, but the file is one UMD wrapper with a source map.
        let bundle = format!(
            "/*!\n * Socket.IO v4.8.4\n * (c) 2014-2026 Guillermo Rauch\n * Released under the MIT License.\n */\n(function (global, factory) {{\n  typeof exports === 'object' && typeof module !== 'undefined' ? module.exports = factory() :\n  (global = typeof globalThis !== 'undefined' ? globalThis : global || self, global.io = factory());\n}})(this, (function () {{ 'use strict';\n{}  return lookup;\n}}));\n//# sourceMappingURL=socket.io.js.map\n",
            body(60)
        );
        let path = "packages/socket.io/client-dist/socket.io.js";
        assert_eq!(signal(path, &bundle), Some("library-banner"));
        assert_eq!(
            signal_for(path, &bundle, &["socket.io"]),
            Some("source-mapped-bundle")
        );
        // A webpack bundle under the project's own banner.
        let webpack = "/*! app v1.0.0 | MIT */\n/******/ (() => { // webpackBootstrap\n/******/ \tvar __webpack_modules__ = ({});\n/******/ })();\n";
        assert_eq!(
            signal_for("dist/app.js", webpack, &["app"]),
            Some("webpack")
        );
        // The project's hand-written source under the same banner: analyzed, like a file
        // without a banner (compiled modules with many statements included).
        let source = format!(
            "/*!\n * Socket.IO v4.8.4\n * (c) 2014-2026 Guillermo Rauch\n * Released under the MIT License.\n */\nimport {{ Manager }} from './manager.js';\n{}export {{ Manager }};\n//# sourceMappingURL=index.js.map\n",
            body(60).replace("    ", "")
        );
        assert_eq!(
            signal_for("packages/socket.io/lib/index.js", &source, &["socket.io"]),
            None
        );
    }

    /// `n` lines of ordinary statements.
    fn body(n: usize) -> String {
        (0..n)
            .map(|i| format!("    var value{i} = compute({i}, \"text {{ ; }}\") / 2; // ; ) }}\n"))
            .collect()
    }

    #[test]
    fn library_banners() {
        for text in [
            "/*! jQuery v3.7.1 | (c) OpenJS Foundation and other contributors | jquery.org/license */\n(function(){})();\n",
            "/*!\n * Socket.IO v4.8.4\n * (c) 2014-2026 Guillermo Rauch\n * Released under the MIT License.\n */\n(function () {})();\n",
            "/*! @license DOMPurify 3.4.15 | (c) Cure53 and other contributors | Released under the Apache license 2.0 */\nfunction a() {}\n",
            "/** @license React v16.14.0\n * react.production.min.js\n *\n * Copyright (c) Facebook, Inc. and its affiliates.\n */\n'use strict';\n",
            "//! moment.js\n//! version : 2.29.4\n//! license : MIT\n;(function (global, factory) {})(this, function () {});\n",
            "#!/usr/bin/env node\n/*! tool 1.0.0 | MIT */\nrun();\n",
        ] {
            assert_eq!(signal("lib/x.js", text), Some("library-banner"), "{text}");
        }
        for text in [
            // first-party headers: no release, a two-part number without `v`, no legal words,
            // not a preserved comment, or not at the top
            "/*!\n * Module dependencies.\n */\n\nconst a = require('a');\n",
            "/*! Copyright (c) 2020 Example Corp. Licensed under the Apache License 2.0 */\nexport const a = 1;\n",
            "/*! app v1.2.3 */\nexport const a = 1;\n",
            "// jQuery v3.7.1 | (c) OpenJS Foundation | jquery.org/license\nexport const a = 1;\n",
            "'use strict';\n/*! lib v1.2.3 | MIT */\nmodule.exports = 1;\n",
            "/*! version v1.2.3a | (c) x */\nx();\n",
        ] {
            assert_eq!(signal("lib/x.js", text), None, "{text}");
        }
        // Plain block comments: a library release, a copyright and a license, all three.
        for text in [
            "/*  Prototype JavaScript framework, version 1.7.3\n *  (c) 2005-2010 Sam Stephenson\n *\n *  Prototype is freely distributable under the terms of an MIT-style license.\n *--------------------------------------------------------------------------*/\nvar Prototype = {};\n",
            "/**\n * Canteen v1.0.4\n * August 19th, 2015\n *\n * Copyright 2015 Platfora, Inc.\n *\n * Licensed under the Apache License, Version 2.0 (the \"License\");\n */\n(function () {})();\n",
            "/*\n * jQuery Cycle Plugin (with Transition Definitions)\n * Copyright (c) 2007-2010 M. Alsup\n * Version: 2.88 (08-JUN-2010)\n * Dual licensed under the MIT and GPL licenses.\n */\n;(function($) {})(jQuery);\n",
            "/*\n * MyLib 1.2.3\n * Copyright 2020 Someone. Released under the BSD license.\n */\nx();\n",
        ] {
            assert_eq!(signal("lib/x.js", text), Some("library-banner"), "{text}");
        }
        for text in [
            // first-party license headers: the only version is the license's
            "/*\n * Licensed to the Apache Software Foundation (ASF) under one\n * or more contributor license agreements.  See the NOTICE file\n * distributed with this work for additional information\n * regarding copyright ownership.  The ASF licenses this file\n * to you under the Apache License, Version 2.0 (the\n * \"License\"); you may not use this file except in compliance\n */\nexport const a = 1;\n",
            "/*\n * Copyright 2002-2024 the original author or authors.\n *\n * Licensed under the Apache License,\n * Version 2.0 (the \"License\");\n */\nexport const a = 1;\n",
            "/*\n * Copyright (C) 2020 Acme\n * This library is free software; you can redistribute it under the terms of the GNU\n * Lesser General Public License version 2.1.\n */\nexport const a = 1;\n",
            "/*\n * Copyright (c) 2020 Acme. Licensed under the Mozilla Public License, v. 2.0.\n */\nexport const a = 1;\n",
            "/*\n * Copyright (c) 2020 Acme. Licensed under the MIT License v2.0 (yes).\n */\nexport const a = 1;\n",
            // two of the three
            "/*\n * MyApp version 2.3.1\n * Copyright (c) 2021 Acme Corp. All rights reserved.\n */\nexport const a = 1;\n",
            "/*\n * MyApp version 2.3.1, MIT license\n */\nexport const a = 1;\n",
            "/*\n * Copyright (c) Microsoft Corporation. All rights reserved.\n * Licensed under the MIT License. Requires TypeScript 4.9.\n */\nexport const a = 1;\n",
            // not the first comment of the file
            "'use strict';\n/*  Lib, version 1.7.3\n *  (c) 2005 Someone, MIT license */\nx();\n",
        ] {
            assert_eq!(signal("lib/x.js", text), None, "{text}");
        }
        // JavaScript only.
        let banner = "/*! jQuery v3.7.1 | (c) OpenJS Foundation | jquery.org/license */\nx();\n";
        assert_eq!(signal("lib/x.ts", banner), None);
        assert_eq!(signal("lib/x.java", banner), None);
        assert_eq!(signal("lib/x.MJS", banner), Some("library-banner"));
        assert_eq!(signal("lib/x.cjs", banner), Some("library-banner"));
    }

    #[test]
    fn release_numbers() {
        for (s, want) in [
            ("v1.2", true),
            ("V10.20", true),
            ("1.2.3", true),
            ("1.2.3-beta.1", true),
            ("version 4.17.21.", true),
            ("1.2", false),
            ("License 2.0", false),
            ("2014-2026", false),
            ("x1.2.3", false),
            ("1.2.3a", false),
            ("v1", false),
        ] {
            assert_eq!(has_release(s.as_bytes()), want, "{s}");
        }
    }

    #[test]
    fn bundler_runtimes() {
        let cases = [
            ("/******/ (() => { // webpackBootstrap\n/******/ \tvar __webpack_modules__ = ({});\n/******/ })();\n", "webpack"),
            ("/******/ (function(modules) { // webpackBootstrap\n/******/ })([]);\n", "webpack"),
            ("\"use strict\";\n(self[\"webpackChunkapp\"] = self[\"webpackChunkapp\"] || []).push([[179],{}]);\n", "webpack"),
            ("(window.webpackJsonp=window.webpackJsonp||[]).push([[0],[]]);\n", "webpack"),
            ("/*! For license information please see main.js.LICENSE.txt */\n(()=>{})();\n", "webpack-license"),
            ("function $parcel$export(e, n, v, s) {\n  Object.defineProperty(e, n, {get: v, set: s});\n}\n", "parcel"),
            ("parcelRequire = (function (modules, cache, entry, globalName) {\n})({}, {}, []);\n", "parcel"),
            ("(function(){function r(e,n,t){function o(i,f){if(!n[i]){}}}})()({1:[function(require,module,exports){\n}]});\n", "browserify"),
            ("var __defProp = Object.defineProperty;\n\n// node_modules/left-pad/index.js\nvar a = 1;\n\n// node_modules/@scope/pkg/dist/index.mjs\nvar b = 2;\n\n// src/index.ts\nconsole.log(a, b);\n", "esbuild"),
        ];
        for (text, want) in cases {
            assert_eq!(signal("dist/x.js", text), Some(want), "{text}");
        }
        for text in [
            // a webpack plugin's own source, a comment about node_modules, one esbuild marker
            "// Replaces __webpack_require__ calls.\nmodule.exports = function plugin() {};\n",
            "/******/\nfunction banner() {}\n",
            "// node_modules/foo is patched by postinstall\n// node_modules/bar/index.js\nrun();\n",
            "// generated\n\n// node_modules/left-pad/index.js\nvar a = 1;\n",
            "const parcelRequire = load();\nfunction $parcel() {}\n",
        ] {
            assert_eq!(signal("src/x.js", text), None, "{text}");
        }
    }

    #[test]
    fn source_mapped_bundles() {
        let umd = format!(
            "(function (global, factory) {{\n  typeof exports === 'object' ? module.exports = factory() : global.lib = factory();\n}})(this, (function () {{ 'use strict';\n{}  var re = /[;)}}]/g, s = `a ${{ {{ b: 1 }}.b }} ; ) }}`;\n  return {{}};\n}}));\n//# sourceMappingURL=lib.js.map\n",
            body(60)
        );
        assert_eq!(
            signal("test/lib/lib.js", &umd),
            Some("source-mapped-bundle")
        );
        let iife = format!(
            "\"use strict\";\n(() => {{\n{}}})();\n//# sourceMappingURL=out.js.map\n",
            body(60)
        );
        assert_eq!(signal("out/x.js", &iife), Some("source-mapped-bundle"));
        let bang = format!(
            "!function(e){{\n{}}}([]);\n//# sourceMappingURL=b.js.map",
            body(60)
        );
        assert_eq!(signal("b.js", &bang), Some("source-mapped-bundle"));

        // tsc output of a first-party module: many top-level statements.
        let tsc = format!(
            "\"use strict\";\nObject.defineProperty(exports, \"__esModule\", {{ value: true }});\nexports.install = install;\n{}async function install() {{\n}}\n//# sourceMappingURL=install.js.map\n",
            body(60).replace("    ", "")
        );
        assert_eq!(signal("build/install.js", &tsc), None);
        // A wrapper that is followed by more statements, a small bundle, no source map, a
        // source map that is not the last line.
        let followed = format!(
            "(function () {{\n{}}})();\n{}\n//# sourceMappingURL=x.js.map\n",
            body(30),
            body(30).replace("    ", "")
        );
        assert_eq!(signal("x.js", &followed), None);
        let small = "(function () {\n  run();\n})();\n//# sourceMappingURL=x.js.map\n";
        assert_eq!(signal("x.js", small), None);
        assert_eq!(
            signal(
                "x.js",
                &umd.replace("//# sourceMappingURL=lib.js.map\n", "")
            ),
            None
        );
        assert_eq!(signal("x.js", &format!("{umd}run();\n")), None);
    }

    #[test]
    fn never_panics_on_odd_input() {
        for text in [
            "",
            "/*",
            "/*!",
            "//!",
            "#!",
            "\"use strict\"",
            "(`${",
            "(`${'",
            "(/[/",
            "é/*! ü v1.2.3 (c) */",
            "/*! (c) v1.2.3",
        ] {
            let _ = signal("x.js", text);
            let padded = format!("{text}{}\n//# sourceMappingURL=x", "é".repeat(2000));
            let _ = signal("x.js", &padded);
            let _ = first_statement_len(text.as_bytes());
        }
    }
}
