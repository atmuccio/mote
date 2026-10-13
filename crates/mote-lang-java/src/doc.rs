//! Documentation comments: `/** … */` blocks and Markdown documentation comments (Java 23,
//! JEP 467) — a run of `///` line comments on consecutive lines.
//!
//! Whether a `///` run is documentation depends on the JDK whose javadoc tool reads the code
//! (23 or later reads it as Markdown), not on the release the code is compiled for: a project
//! compiled for Java 17 may well be documented with `///` runs. Older code also uses `///` for
//! ordinary notes and commented-out code. Rules therefore say how they use documentation
//! ([`DocUse`]) and ask [`Ctx::markdown_docs`]: a rule that recognizes documentation to stay
//! silent always reads `///` runs (recognizing them can only remove findings); a rule that
//! reports something about documentation reads them only when the file's release is known to
//! be 23 or later, so an older or unknown release never adds a finding.
//!
//! [`Ctx::markdown_docs`]: crate::Ctx::markdown_docs

use crate::ctx::Ctx;
use crate::kinds::{k, K};
use crate::rule::Span;
use tree_sitter::Node;

/// First Java release whose documentation tool reads `///` runs as documentation comments.
pub const MARKDOWN_DOCS_SINCE: u32 = 23;

/// How a rule uses documentation comments, which decides whether `///` runs count (see
/// [`markdown_docs`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocUse {
    /// Recognizing a doc comment can only remove findings (the declaration is documented, the
    /// import is referenced, the `null` result is documented): `///` runs always count.
    Silences,
    /// Recognizing a doc comment can produce findings (it is misplaced, a tag is empty, the
    /// annotation is missing): `///` runs count only when the release is known to be 23 or
    /// later.
    Reports,
}

/// Whether `///` runs are documentation comments for a rule using them as `usage`, in a file
/// compiled for `release`.
pub fn markdown_docs(release: Option<u32>, usage: DocUse) -> bool {
    match usage {
        DocUse::Silences => true,
        DocUse::Reports => release.is_some_and(|r| r >= MARKDOWN_DOCS_SINCE),
    }
}

/// Whether the text of a `block_comment` is a Javadoc comment: it starts with `/**` and is not
/// the empty comment `/**/`.
pub fn is_javadoc(text: &str) -> bool {
    text.starts_with("/**") && text != "/**/"
}

/// Whether the text of a `line_comment` has the form of a Markdown documentation line: it
/// starts with exactly three slashes (`////` banners and separators are plain comments) and is
/// not a directive of the jbang script runner ([`is_jbang_directive`]).
pub fn is_markdown_text(text: &str) -> bool {
    text.starts_with("///") && !text.starts_with("////") && !is_jbang_directive(&text[3..])
}

/// The directives of the jbang script runner, which also reads them written with three slashes
/// (`///DEPS group:artifact:1.0`, `///SOURCES Other.java`).
const JBANG_DIRECTIVES: &[&str] = &[
    "CDS",
    "COMPILE_OPTIONS",
    "DEPS",
    "DESCRIPTION",
    "DOCS",
    "FILES",
    "GAV",
    "GROOVY",
    "JAVA",
    "JAVAAGENT",
    "JAVAC_OPTIONS",
    "JAVA_OPTIONS",
    "KOTLIN",
    "MAIN",
    "MANIFEST",
    "MODULE",
    "NATIVE_OPTIONS",
    "NOINTEGRATIONS",
    "PREVIEW",
    "REPOS",
    "RUNTIME_OPTIONS",
    "SOURCES",
];

/// Whether the text after the `///` of a line comment is a build instruction for the jbang
/// script runner rather than documentation: its shebang line (`///usr/bin/env jbang …`) or one
/// of its directives, written right after the slashes and followed by a blank or the end of
/// the line (`///DEPS g:a:1.0`, `///PREVIEW`).
fn is_jbang_directive(after_slashes: &str) -> bool {
    let first = after_slashes.as_bytes().first();
    if !first.is_some_and(|b| b.is_ascii_uppercase() || *b == b'u') {
        return false;
    }
    let end = after_slashes
        .find(char::is_whitespace)
        .unwrap_or(after_slashes.len());
    let word = &after_slashes[..end];
    word == "usr/bin/env" || JBANG_DIRECTIVES.contains(&word)
}

/// Whether only blanks precede `node` on its line: it is not a trailing comment after code.
pub fn starts_line(src: &str, node: Node<'_>) -> bool {
    let bytes = src.as_bytes();
    let mut i = node.start_byte().min(bytes.len());
    while i > 0 && matches!(bytes[i - 1], b' ' | b'\t' | 0x0c) {
        i -= 1;
    }
    i == 0 || matches!(bytes[i - 1], b'\n' | b'\r')
}

/// Whether `node` is a line of a Markdown documentation comment: a `line_comment` of the form
/// `///…` that starts its line. (Whether such lines are documentation for a rule depends on
/// how it uses them: [`markdown_docs`].)
pub fn is_markdown_line(src: &str, node: Node<'_>) -> bool {
    k(node) == K::LineComment
        && is_markdown_text(src.get(node.start_byte()..node.end_byte()).unwrap_or(""))
        && starts_line(src, node)
}

/// Whether the Markdown line `next` continues the run of the Markdown line `prev`: it is on
/// the next line, with only blanks in between.
fn continues(src: &str, prev: Node<'_>, next: Node<'_>) -> bool {
    let Some(between) = src.get(prev.end_byte()..next.start_byte()) else {
        return false;
    };
    // The comment's line break may belong to the node, to the gap, or straddle both (a `\r`
    // ending the node, the `\n` starting the gap): count over the whole stretch.
    let Some(stretch) = src.get(prev.start_byte()..next.start_byte()) else {
        return false;
    };
    line_breaks(stretch) == 1 && between.bytes().all(|b| b.is_ascii_whitespace())
}

/// Number of line breaks (`\n`, `\r\n` or a lone `\r`) in `s`.
fn line_breaks(s: &str) -> usize {
    let b = s.as_bytes();
    b.iter()
        .enumerate()
        .filter(|(i, c)| **c == b'\n' || (**c == b'\r' && b.get(i + 1) != Some(&b'\n')))
        .count()
}

/// A cursor on `node`, a child of `parent`. `None` if the cursor cannot reach it.
fn cursor_at<'t>(parent: Node<'t>, node: Node<'t>) -> Option<tree_sitter::TreeCursor<'t>> {
    let mut c = parent.walk();
    c.goto_first_child_for_byte(node.start_byte())?;
    while c.node().id() != node.id() {
        if c.node().start_byte() > node.start_byte() || !c.goto_next_sibling() {
            return None;
        }
    }
    Some(c)
}

/// Whether the Markdown line `line` (a child of `parent`) starts its run: no Markdown line
/// directly above continues into it.
pub fn starts_run<'t>(ctx: &Ctx<'t>, line: Node<'t>) -> bool {
    let src = ctx.source();
    ctx.prev_named_sibling(line)
        .is_none_or(|p| !(is_markdown_line(src, p) && continues(src, p, line)))
}

/// The lines of the `///` run starting at the Markdown line `first`, a child of `parent`, in
/// source order.
pub fn run_from<'t>(src: &str, parent: Node<'t>, first: Node<'t>) -> Vec<Node<'t>> {
    let mut lines = vec![first];
    let Some(mut c) = cursor_at(parent, first) else {
        return lines;
    };
    let mut last = first;
    while c.goto_next_sibling() {
        let n = c.node();
        if !is_markdown_line(src, n) || !continues(src, last, n) {
            break;
        }
        lines.push(n);
        last = n;
    }
    lines
}

/// The lines of the `///` run ending at the Markdown line `last`, a child of `parent`, in
/// source order.
fn run_to<'t>(src: &str, parent: Node<'t>, last: Node<'t>) -> Vec<Node<'t>> {
    let mut lines = vec![last];
    if let Some(mut c) = cursor_at(parent, last) {
        let mut first = last;
        while c.goto_previous_sibling() {
            let n = c.node();
            if !is_markdown_line(src, n) || !continues(src, n, first) {
                break;
            }
            lines.push(n);
            first = n;
        }
    }
    lines.reverse();
    lines
}

/// A documentation comment.
#[derive(Clone, Debug)]
pub enum DocComment<'t> {
    /// A `/** … */` comment.
    Javadoc(Node<'t>),
    /// A run of `///` lines (Java 23), in source order (never empty).
    Markdown(Vec<Node<'t>>),
}

impl<'t> DocComment<'t> {
    /// The source range of the comment (all lines of a run).
    pub fn span(&self) -> Span {
        match self {
            DocComment::Javadoc(n) => Span::from(*n),
            DocComment::Markdown(lines) => match (lines.first(), lines.last()) {
                (Some(f), Some(l)) => Span::cover(*f, *l),
                _ => Span::new(0, 0),
            },
        }
    }

    /// The content lines with the source byte offset where each starts: without `/**`, `*/`
    /// and the leading `*` margin of a Javadoc comment, without the `///` of a Markdown line.
    /// Leading blanks are kept.
    pub fn lines(&self, src: &'t str) -> Vec<(usize, &'t str)> {
        match self {
            DocComment::Javadoc(n) => javadoc_lines(src, *n),
            DocComment::Markdown(lines) => lines
                .iter()
                .map(|l| {
                    let start = l.start_byte() + 3;
                    let text = src.get(start..l.end_byte()).unwrap_or("");
                    (start, text.trim_end_matches(['\r', '\n']))
                })
                .collect(),
        }
    }

    /// The text of the comment normalized for scanning tags and references: its content lines
    /// joined by `\n`; in a Markdown comment, the lines of fenced code blocks (```` ``` ````,
    /// `~~~`) are left empty (they hold code, not tags).
    pub fn text(&self, src: &'t str) -> String {
        let markdown = matches!(self, DocComment::Markdown(_));
        let mut out = String::new();
        let mut fence = Fence::default();
        for (i, (_, line)) in self.lines(src).into_iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            if markdown && fence.code(line) {
                continue;
            }
            out.push_str(line);
        }
        out
    }
}

/// The content lines of a `/** … */` comment with their source offsets.
fn javadoc_lines<'t>(src: &'t str, node: Node<'t>) -> Vec<(usize, &'t str)> {
    let start = node.start_byte();
    let text = src.get(start..node.end_byte()).unwrap_or("");
    let body_start = if text.starts_with("/**") { 3 } else { 0 };
    let body_end = if text.len() >= body_start + 2 && text.ends_with("*/") {
        text.len() - 2
    } else {
        text.len()
    };
    let Some(body) = text.get(body_start..body_end) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut off = start + body_start;
    for line in body.split('\n') {
        let after_ws = line.trim_start();
        let content = after_ws.trim_start_matches('*');
        out.push((
            off + (line.len() - content.len()),
            content.trim_end_matches('\r'),
        ));
        off += line.len() + 1;
    }
    out
}

/// Tracks fenced code blocks (```` ``` ```` or `~~~`) over the content lines of a Markdown
/// comment.
#[derive(Default)]
pub struct Fence {
    open: Option<u8>,
}

impl Fence {
    /// Whether `line` (a content line, read in order) is part of a fenced code block, its
    /// opening and closing fences included.
    pub fn code(&mut self, line: &str) -> bool {
        let t = line.trim_start();
        let marker = if t.starts_with("```") {
            Some(b'`')
        } else if t.starts_with("~~~") {
            Some(b'~')
        } else {
            None
        };
        match (self.open, marker) {
            (None, Some(m)) => {
                self.open = Some(m);
                true
            }
            (Some(o), Some(m)) if o == m => {
                self.open = None;
                true
            }
            (Some(_), _) => true,
            (None, None) => false,
        }
    }
}

/// The documentation comment directly before `decl` (its annotations included): the previous
/// named sibling when it is a `/** … */` comment, or the `///` run ending there when such runs
/// are documentation for `usage` ([`markdown_docs`]). Other comments in between hide it, as do
/// blank lines inside a run (they end it: the run nearest the declaration is its comment).
pub fn doc_comment_before<'t>(
    ctx: &Ctx<'t>,
    decl: Node<'t>,
    usage: DocUse,
) -> Option<DocComment<'t>> {
    let prev = ctx.prev_named_sibling(decl)?;
    match k(prev) {
        K::BlockComment if is_javadoc(ctx.text(prev)) => Some(DocComment::Javadoc(prev)),
        K::LineComment if is_markdown_line(ctx.source(), prev) && ctx.markdown_docs(usage) => {
            let parent = ctx.parent(decl)?;
            Some(DocComment::Markdown(run_to(ctx.source(), parent, prev)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_and_usage() {
        for release in [None, Some(8), Some(21), Some(23), Some(25)] {
            assert!(markdown_docs(release, DocUse::Silences));
        }
        assert!(markdown_docs(Some(23), DocUse::Reports));
        assert!(markdown_docs(Some(25), DocUse::Reports));
        assert!(!markdown_docs(Some(21), DocUse::Reports));
        assert!(!markdown_docs(Some(8), DocUse::Reports));
        assert!(!markdown_docs(None, DocUse::Reports));
    }

    #[test]
    fn comment_forms() {
        assert!(is_markdown_text("/// Doc."));
        assert!(is_markdown_text("///Doc."));
        assert!(is_markdown_text("///"));
        assert!(!is_markdown_text("//// banner"));
        assert!(!is_markdown_text("// plain"));
        // jbang's shebang and directives are build instructions.
        assert!(!is_markdown_text(
            "///usr/bin/env jbang \"$0\" \"$@\" ; exit $?"
        ));
        assert!(!is_markdown_text("///DEPS info.picocli:picocli:4.5.0"));
        assert!(!is_markdown_text("///SOURCES nested/*.java"));
        assert!(!is_markdown_text("///PREVIEW"));
        assert!(!is_markdown_text("///JAVA 21+\r"));
        // Documentation that merely starts like one.
        assert!(is_markdown_text("/// DEPS are resolved first."));
        assert!(is_markdown_text("///DEPSET of the build"));
        assert!(is_markdown_text("///Java version check."));
        assert!(is_markdown_text("///JAVA: the runtime."));
        assert!(is_markdown_text("///usr/bin is where it lives"));
        assert!(is_javadoc("/** Doc. */"));
        assert!(!is_javadoc("/**/"));
        assert!(!is_javadoc("/* plain */"));
    }

    #[test]
    fn line_break_counts() {
        assert_eq!(line_breaks("a\nb"), 1);
        assert_eq!(line_breaks("a\r\nb"), 1);
        assert_eq!(line_breaks("a\rb"), 1);
        assert_eq!(line_breaks("\n\n"), 2);
        assert_eq!(line_breaks("ab"), 0);
    }

    #[test]
    fn fences() {
        let mut f = Fence::default();
        let lines = [
            " Example:",
            " ```java",
            " @param x",
            " ~~~",
            " ```",
            " @return y",
        ];
        let code: Vec<bool> = lines.iter().map(|l| f.code(l)).collect();
        assert_eq!(code, [false, true, true, true, true, false]);
    }
}
