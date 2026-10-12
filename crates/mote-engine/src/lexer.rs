//! A fast, byte-level tokenizer for C-like languages, driven by a plugin's [`LexerSpec`].
//!
//! The lexer is deliberately language-light: it only needs to know comment delimiters and a few
//! literal forms. From one pass over the text it produces
//!
//! * the set of lines that contain code (for NCLOC and "next code line" lookups),
//! * the number of comment lines,
//! * the comments themselves (used when a plugin does not report comments),
//! * the token stream used for duplication detection: `(hash, line)` pairs, with literal values
//!   optionally normalized and tokens on "skip" lines (imports, package declarations) dropped.
//!
//! Robustness: the input is any `&str`; the lexer only ever indexes bytes and hashes byte slices,
//! so it cannot panic on odd input. Unterminated `"…"` / `'…'` literals end at the newline, so a
//! stray quote desynchronizes at most one line.

use mote_core::{Comment, LexerSpec};
use xxhash_rust::xxh3::xxh3_64_with_seed;

/// Options that change the token stream (part of the cache key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LexOptions {
    /// Replace string/number/regex literal values by their kind when hashing tokens.
    pub ignore_literals: bool,
}

impl Default for LexOptions {
    fn default() -> Self {
        LexOptions {
            ignore_literals: true,
        }
    }
}

/// Everything the lexer extracts from one file.
#[derive(Clone, Debug, Default)]
pub struct LexOutput {
    /// Number of lines (a trailing newline does not start a new line; empty text has 0 lines).
    pub lines: u32,
    /// Sorted, de-duplicated 1-based lines that contain at least one code token.
    pub code_lines: Vec<u32>,
    /// Number of lines containing comment text (at least one alphanumeric character).
    pub comment_lines: u32,
    /// Comments found in the text, in source order.
    pub comments: Vec<Comment>,
    /// Token hashes for duplication detection.
    pub token_hashes: Vec<u32>,
    /// 1-based line of each token in [`token_hashes`](Self::token_hashes).
    pub token_lines: Vec<u32>,
}

impl LexOutput {
    /// Number of code lines (NCLOC).
    pub fn ncloc(&self) -> u32 {
        self.code_lines.len() as u32
    }

    /// Whether `line` contains code.
    pub fn is_code_line(&self, line: u32) -> bool {
        self.code_lines.binary_search(&line).is_ok()
    }

    /// The first code line strictly after `line`, if any.
    pub fn next_code_line_after(&self, line: u32) -> Option<u32> {
        let idx = match self.code_lines.binary_search(&(line + 1)) {
            Ok(i) | Err(i) => i,
        };
        self.code_lines.get(idx).copied()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Ident,
    Number,
    Str,
    Regex,
    Punct,
}

const SEED_IDENT: u64 = 0x9E37_79B9_7F4A_7C15;
const SEED_PUNCT: u64 = 0xC2B2_AE3D_27D4_EB4F;
const SEED_LIT: u64 = 0x1656_67B1_9E37_79F9;

/// Operators recognised as single tokens (longest first within each length class).
const PUNCT4: &[&[u8]] = &[b">>>="];
const PUNCT3: &[&[u8]] = &[
    b"===", b"!==", b"**=", b"...", b"<<=", b">>=", b">>>", b"&&=", b"||=", b"??=", b"<=>",
];
const PUNCT2: &[&[u8]] = &[
    b"==", b"!=", b"<=", b">=", b"&&", b"||", b"++", b"--", b"+=", b"-=", b"*=", b"/=", b"%=",
    b"&=", b"|=", b"^=", b"<<", b">>", b"->", b"=>", b"::", b"??", b"?.", b"**",
];

/// Keywords after which a `/` starts a regular expression (JS family).
const REGEX_PRECEDERS: &[&[u8]] = &[
    b"return",
    b"typeof",
    b"instanceof",
    b"in",
    b"of",
    b"new",
    b"delete",
    b"void",
    b"throw",
    b"case",
    b"do",
    b"else",
    b"yield",
    b"await",
];

struct Lexer<'a> {
    b: &'a [u8],
    spec: &'a LexerSpec,
    opts: LexOptions,
    i: usize,
    line: u32,
    line_start: usize,
    /// Whether a code token has been seen on the current line (for trailing comments).
    code_on_line: bool,
    code_lines: Vec<u32>,
    comment_line_marks: Vec<u32>,
    comments: Vec<Comment>,
    hashes: Vec<u32>,
    lines: Vec<u32>,
    /// Cached "skip this line for CPD" decision: (line, skip).
    skip_cache: (u32, bool),
    /// Previous significant token (kind, start, end) for the regex heuristic.
    prev: Option<(Kind, usize, usize)>,
    /// Brace depth in code.
    brace_depth: u32,
    /// Brace depths at which template-literal `${` substitutions were opened.
    template_stack: Vec<u32>,
}

/// Tokenize `text` according to `spec`.
pub fn lex(text: &str, spec: &LexerSpec, opts: LexOptions) -> LexOutput {
    let b = text.as_bytes();
    let mut lx = Lexer {
        b,
        spec,
        opts,
        i: 0,
        line: 1,
        line_start: 0,
        code_on_line: false,
        code_lines: Vec::with_capacity(b.len() / 30 + 1),
        comment_line_marks: Vec::new(),
        comments: Vec::new(),
        hashes: Vec::with_capacity(b.len() / 5 + 1),
        lines: Vec::with_capacity(b.len() / 5 + 1),
        skip_cache: (0, false),
        prev: None,
        brace_depth: 0,
        template_stack: Vec::new(),
    };
    lx.run();

    let lines = if b.is_empty() {
        0
    } else {
        let nl = memchr_count(b'\n', b) as u32;
        if b.last() == Some(&b'\n') {
            nl
        } else {
            nl + 1
        }
    };
    let mut code_lines = lx.code_lines;
    // Multi-line tokens mark their lines after the lines of earlier tokens; restore order.
    if !code_lines.is_sorted() {
        code_lines.sort_unstable();
    }
    code_lines.dedup();
    let mut comment_marks = lx.comment_line_marks;
    comment_marks.sort_unstable();
    comment_marks.dedup();
    LexOutput {
        lines,
        code_lines,
        comment_lines: comment_marks.len() as u32,
        comments: lx.comments,
        token_hashes: lx.hashes,
        token_lines: lx.lines,
    }
}

fn memchr_count(needle: u8, hay: &[u8]) -> usize {
    hay.iter().filter(|&&c| c == needle).count()
}

#[inline]
fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c >= 0x80
}

#[inline]
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

impl<'a> Lexer<'a> {
    #[inline]
    fn peek(&self, off: usize) -> u8 {
        self.b.get(self.i + off).copied().unwrap_or(0)
    }

    /// Whether the text at `self.i` starts with `pat` (never for an empty `pat`). Called for
    /// every token and every byte of a block comment: the first byte is compared inline, so the
    /// slice comparison only runs on a likely match.
    #[inline]
    fn starts_with(&self, pat: &str) -> bool {
        let p = pat.as_bytes();
        p.first()
            .is_some_and(|&first| self.b.get(self.i) == Some(&first))
            && self.b[self.i..].starts_with(p)
    }

    fn newline(&mut self) {
        self.line += 1;
        self.line_start = self.i + 1;
        self.code_on_line = false;
    }

    #[inline]
    fn mark_code(&mut self, line: u32) {
        if self.code_lines.last() != Some(&line) {
            self.code_lines.push(line);
        }
    }

    fn run(&mut self) {
        let n = self.b.len();
        while self.i < n {
            let c = self.b[self.i];
            match c {
                b'\n' => {
                    self.newline();
                    self.i += 1;
                }
                b' ' | b'\t' | b'\r' | 0x0B | 0x0C => self.i += 1,
                _ => {
                    if self.starts_with(self.spec.line_comment) {
                        self.line_comment();
                    } else if self.starts_with(self.spec.block_comment.0)
                        && !self.spec.block_comment.1.is_empty()
                    {
                        self.block_comment();
                    } else if c == b'"' {
                        if self.spec.text_blocks && self.peek(1) == b'"' && self.peek(2) == b'"' {
                            self.text_block();
                        } else {
                            self.quoted(b'"');
                        }
                    } else if c == b'\'' {
                        self.quoted(b'\'');
                    } else if c == b'`' && self.spec.template_literals {
                        self.i += 1;
                        self.template_continue(self.i - 1, self.line, self.line_start);
                    } else if c.is_ascii_digit() || (c == b'.' && self.peek(1).is_ascii_digit()) {
                        self.number();
                    } else if is_ident_start(c) {
                        self.ident();
                    } else if c == b'/' && self.spec.template_literals && self.regex_allowed() {
                        if !self.regex() {
                            self.punct();
                        }
                    } else {
                        self.punct();
                    }
                }
            }
        }
    }

    fn line_comment(&mut self) {
        let start = self.i;
        let start_line = self.line;
        let start_col = self.col_of(start);
        let trailing = self.code_on_line;
        let mut end = start;
        while end < self.b.len() && self.b[end] != b'\n' {
            end += 1;
        }
        let body = &self.b[start + self.spec.line_comment.len()..end];
        if body.iter().any(u8::is_ascii_alphanumeric) {
            self.comment_line_marks.push(start_line);
        }
        self.push_comment(start, end, start_line, start_line, start_col, trailing);
        self.i = end;
    }

    fn block_comment(&mut self) {
        let (open, close) = self.spec.block_comment;
        let start = self.i;
        let start_line = self.line;
        let start_col = self.col_of(start);
        let trailing = self.code_on_line;
        self.i += open.len();
        let mut has_alnum = false;
        let close_b = close.as_bytes();
        loop {
            if self.i >= self.b.len() {
                break;
            }
            if self.starts_with(close) {
                self.i += close_b.len();
                break;
            }
            let c = self.b[self.i];
            if c == b'\n' {
                if has_alnum {
                    self.comment_line_marks.push(self.line);
                }
                has_alnum = false;
                self.line += 1;
                self.line_start = self.i + 1;
                // Code after the comment on a later line is not "on the comment's first line".
                self.code_on_line = false;
            } else if c.is_ascii_alphanumeric() {
                has_alnum = true;
            }
            self.i += 1;
        }
        if has_alnum {
            self.comment_line_marks.push(self.line);
        }
        let end_line = self.line;
        self.push_comment(start, self.i, start_line, end_line, start_col, trailing);
    }

    fn push_comment(
        &mut self,
        start: usize,
        end: usize,
        start_line: u32,
        end_line: u32,
        start_col: u32,
        trailing: bool,
    ) {
        let text = String::from_utf8_lossy(&self.b[start..end]).into_owned();
        self.comments.push(Comment {
            start_line,
            end_line,
            start_col,
            text,
            trailing,
        });
    }

    fn col_of(&self, offset: usize) -> u32 {
        let seg = &self.b[self.line_start.min(offset)..offset];
        seg.iter().filter(|&&c| (c & 0xC0) != 0x80).count() as u32 + 1
    }

    /// `"…"` or `'…'`: ends at the matching quote, a newline, or EOF.
    fn quoted(&mut self, q: u8) {
        let start = self.i;
        self.i += 1;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'\\' {
                // Do not skip over a newline: an escaped newline still ends the line count right.
                if self.peek(1) == b'\n' {
                    self.i += 1;
                    continue;
                }
                self.i += 2;
                continue;
            }
            if c == q {
                self.i += 1;
                break;
            }
            if c == b'\n' {
                break;
            }
            self.i += 1;
        }
        let end = self.i.min(self.b.len());
        self.i = end;
        self.emit(Kind::Str, start, end, self.line);
    }

    /// Java `"""` text block, possibly spanning many lines.
    fn text_block(&mut self) {
        let start = self.i;
        let start_line = self.line;
        let start_line_start = self.line_start;
        self.i += 3;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'\\' {
                if self.peek(1) == b'\n' {
                    self.i += 1;
                    continue;
                }
                self.i += 2;
                continue;
            }
            if c == b'"' && self.peek(1) == b'"' && self.peek(2) == b'"' {
                self.i += 3;
                break;
            }
            if c == b'\n' {
                self.line += 1;
                self.line_start = self.i + 1;
            }
            self.i += 1;
        }
        self.i = self.i.min(self.b.len());
        let end_line = self.line;
        self.emit_multiline(
            Kind::Str,
            start,
            self.i,
            start_line,
            start_line_start,
            end_line,
        );
    }

    /// Continue scanning a template literal whose opening backtick (or closing `}` of a
    /// substitution) ends right before `self.i`. Returns when the template ends or a `${`
    /// substitution opens (in which case code lexing resumes).
    fn template_continue(&mut self, start: usize, start_line: u32, start_line_start: usize) {
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'\\' {
                if self.peek(1) == b'\n' {
                    self.i += 1;
                    continue;
                }
                self.i += 2;
                continue;
            }
            if c == b'`' {
                self.i += 1;
                break;
            }
            if c == b'$' && self.peek(1) == b'{' {
                self.i += 2;
                let end_line = self.line;
                self.emit_multiline(
                    Kind::Str,
                    start,
                    self.i,
                    start_line,
                    start_line_start,
                    end_line,
                );
                self.template_stack.push(self.brace_depth);
                self.brace_depth += 1;
                return;
            }
            if c == b'\n' {
                self.line += 1;
                self.line_start = self.i + 1;
            }
            self.i += 1;
        }
        self.i = self.i.min(self.b.len());
        let end_line = self.line;
        self.emit_multiline(
            Kind::Str,
            start,
            self.i,
            start_line,
            start_line_start,
            end_line,
        );
    }

    fn number(&mut self) {
        let start = self.i;
        let hex = self.peek(0) == b'0' && matches!(self.peek(1), b'x' | b'X');
        self.i += 1;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            let part_of_number = c.is_ascii_alphanumeric()
                || c == b'_'
                || (c == b'.' && self.peek(1).is_ascii_digit())
                || ((c == b'+' || c == b'-') && !hex && matches!(self.b[self.i - 1], b'e' | b'E'));
            if !part_of_number {
                break;
            }
            self.i += 1;
        }
        self.emit(Kind::Number, start, self.i, self.line);
    }

    fn ident(&mut self) {
        let start = self.i;
        self.i += 1;
        while self.i < self.b.len() && is_ident_continue(self.b[self.i]) {
            self.i += 1;
        }
        self.emit(Kind::Ident, start, self.i, self.line);
    }

    fn punct(&mut self) {
        let start = self.i;
        let rest = &self.b[self.i..];
        let len = if PUNCT4.iter().any(|p| rest.starts_with(p)) {
            4
        } else if PUNCT3.iter().any(|p| rest.starts_with(p)) {
            3
        } else if PUNCT2.iter().any(|p| rest.starts_with(p)) {
            2
        } else {
            // One whole UTF-8 scalar so we never split a character.
            let c = rest[0];
            if c < 0x80 {
                1
            } else {
                1 + rest[1..]
                    .iter()
                    .take_while(|&&x| (x & 0xC0) == 0x80)
                    .count()
            }
        };
        self.i += len;
        let c = self.b[start];
        if len == 1 && self.spec.template_literals {
            if c == b'{' {
                self.brace_depth += 1;
            } else if c == b'}' {
                if self.brace_depth > 0 {
                    self.brace_depth -= 1;
                }
                if self.template_stack.last() == Some(&self.brace_depth) {
                    self.template_stack.pop();
                    self.emit(Kind::Punct, start, self.i, self.line);
                    let (l, ls) = (self.line, self.line_start);
                    self.template_continue(self.i - 1, l, ls);
                    return;
                }
            }
        }
        self.emit(Kind::Punct, start, self.i, self.line);
    }

    /// JS heuristic: a `/` starts a regex when the previous token cannot end an expression.
    fn regex_allowed(&self) -> bool {
        if matches!(self.peek(1), b'/' | b'*') {
            return false;
        }
        match self.prev {
            None => true,
            Some((Kind::Ident, s, e)) => REGEX_PRECEDERS.contains(&&self.b[s..e]),
            Some((Kind::Number | Kind::Str | Kind::Regex, _, _)) => false,
            Some((Kind::Punct, s, e)) => {
                !matches!(&self.b[s..e], b")" | b"]" | b"}" | b"++" | b"--")
            }
        }
    }

    /// Try to lex a regex literal at `self.i`; returns false (without consuming) when the line
    /// ends first.
    fn regex(&mut self) -> bool {
        let start = self.i;
        let mut j = self.i + 1;
        let mut in_class = false;
        while j < self.b.len() {
            match self.b[j] {
                b'\n' => return false,
                b'\\' => {
                    if self.b.get(j + 1) == Some(&b'\n') {
                        return false;
                    }
                    j += 2;
                    continue;
                }
                b'[' => in_class = true,
                b']' => in_class = false,
                b'/' if !in_class => {
                    j += 1;
                    while j < self.b.len() && self.b[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    self.i = j;
                    self.emit(Kind::Regex, start, j, self.line);
                    return true;
                }
                _ => {}
            }
            j += 1;
        }
        false
    }

    fn skip_line(&mut self, line: u32, line_start: usize) -> bool {
        if self.spec.cpd_skip_line_prefixes.is_empty() {
            return false;
        }
        if self.skip_cache.0 == line {
            return self.skip_cache.1;
        }
        let mut k = line_start;
        while k < self.b.len() && matches!(self.b[k], b' ' | b'\t') {
            k += 1;
        }
        let rest = &self.b[k.min(self.b.len())..];
        let skip = self
            .spec
            .cpd_skip_line_prefixes
            .iter()
            .any(|p| !p.is_empty() && rest.starts_with(p.as_bytes()));
        self.skip_cache = (line, skip);
        skip
    }

    fn hash(&self, kind: Kind, start: usize, end: usize) -> u32 {
        let bytes = &self.b[start..end.min(self.b.len())];
        let h = match kind {
            Kind::Ident => xxh3_64_with_seed(bytes, SEED_IDENT),
            Kind::Punct => xxh3_64_with_seed(bytes, SEED_PUNCT),
            Kind::Number | Kind::Str | Kind::Regex => {
                if self.opts.ignore_literals {
                    return match kind {
                        Kind::Number => 0x4E55_4D31,
                        Kind::Regex => 0x5245_4731,
                        _ => 0x5354_5231,
                    };
                }
                xxh3_64_with_seed(bytes, SEED_LIT)
            }
        };
        (h ^ (h >> 32)) as u32
    }

    fn emit(&mut self, kind: Kind, start: usize, end: usize, line: u32) {
        self.code_on_line = true;
        self.mark_code(line);
        self.prev = Some((kind, start, end));
        let ls = self.line_start;
        if !self.skip_line(line, ls) {
            let h = self.hash(kind, start, end);
            self.hashes.push(h);
            self.lines.push(line);
        }
    }

    fn emit_multiline(
        &mut self,
        kind: Kind,
        start: usize,
        end: usize,
        start_line: u32,
        start_line_start: usize,
        end_line: u32,
    ) {
        self.code_on_line = true;
        for l in start_line..=end_line {
            self.mark_code(l);
        }
        self.prev = Some((kind, start, end));
        if !self.skip_line(start_line, start_line_start) {
            let h = self.hash(kind, start, end);
            self.hashes.push(h);
            self.lines.push(start_line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn java() -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: true,
            template_literals: false,
            cpd_skip_line_prefixes: &["import ", "package "],
        }
    }

    fn js() -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: false,
            template_literals: true,
            cpd_skip_line_prefixes: &["import "],
        }
    }

    #[test]
    fn counts_code_and_comment_lines() {
        let src = "package a;\n\n// hello\nclass A { // trailing\n  /* block\n   * more\n   */\n  int x = 1;\n}\n";
        let out = lex(src, &java(), LexOptions::default());
        assert_eq!(out.lines, 9);
        assert_eq!(out.code_lines, vec![1, 4, 8, 9]);
        assert_eq!(out.comment_lines, 4); // lines 3, 4, 5, 6 (line 7 has only "*/")
        assert_eq!(out.comments.len(), 3);
        assert!(!out.comments[0].trailing);
        assert!(out.comments[1].trailing);
        assert_eq!(out.comments[1].start_col, 11);
        assert_eq!(out.comments[2].start_line, 5);
        assert_eq!(out.comments[2].end_line, 7);
        // package line tokens are skipped for CPD
        assert!(!out.token_lines.contains(&1));
        assert_eq!(out.next_code_line_after(4), Some(8));
    }

    #[test]
    fn strings_hide_comment_markers() {
        let src = "String s = \"// not a comment /* nor this\"; char c = '\\''; int y; // real\n";
        let out = lex(src, &java(), LexOptions::default());
        assert_eq!(out.comments.len(), 1);
        assert_eq!(out.comments[0].text, "// real");
    }

    #[test]
    fn unterminated_string_ends_at_newline() {
        let src = "a = \"oops\nb = 1; // c\n";
        let out = lex(src, &java(), LexOptions::default());
        assert_eq!(out.comments.len(), 1);
        assert_eq!(out.code_lines, vec![1, 2]);
    }

    #[test]
    fn text_blocks_span_lines() {
        let src = "String s = \"\"\"\n  hello \"quoted\" // no\n  \\\"\"\" still inside\n  \"\"\";\nint x;\n";
        let out = lex(src, &java(), LexOptions::default());
        assert!(out.comments.is_empty());
        assert_eq!(out.code_lines, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn template_literals_with_nested_substitutions() {
        let src =
            "const a = `x ${ f({ b: `inner ${c}` }) } // not comment\n y`; // yes\nlet z = 1;\n";
        let out = lex(src, &js(), LexOptions::default());
        assert_eq!(out.comments.len(), 1, "{:?}", out.comments);
        assert_eq!(out.comments[0].text, "// yes");
        assert_eq!(out.code_lines, vec![1, 2, 3]);
    }

    #[test]
    fn regex_literals_do_not_desync() {
        let src =
            "const r = /\"[/]*/g; // c1\nconst d = a / b / c; // c2\nif (x) return /'/.test(y);\n";
        let out = lex(src, &js(), LexOptions::default());
        let texts: Vec<&str> = out.comments.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, vec!["// c1", "// c2"]);
    }

    #[test]
    fn literal_normalization() {
        let a = lex(
            "f(\"a\", 1);",
            &java(),
            LexOptions {
                ignore_literals: true,
            },
        );
        let b = lex(
            "f(\"b\", 2);",
            &java(),
            LexOptions {
                ignore_literals: true,
            },
        );
        assert_eq!(a.token_hashes, b.token_hashes);
        let a = lex(
            "f(\"a\", 1);",
            &java(),
            LexOptions {
                ignore_literals: false,
            },
        );
        let b = lex(
            "f(\"b\", 2);",
            &java(),
            LexOptions {
                ignore_literals: false,
            },
        );
        assert_ne!(a.token_hashes, b.token_hashes);
        // identifiers are always significant
        let c = lex(
            "g(\"a\", 1);",
            &java(),
            LexOptions {
                ignore_literals: true,
            },
        );
        assert_ne!(a.token_hashes[0], c.token_hashes[0]);
    }

    #[test]
    fn operators_are_single_tokens() {
        let out = lex("a >>>= b === c;", &js(), LexOptions::default());
        assert_eq!(out.token_hashes.len(), 6);
    }

    #[test]
    fn never_panics_on_odd_input() {
        let inputs = [
            "",
            "\"",
            "'",
            "`",
            "`${",
            "`${`${",
            "/*",
            "/* unterminated",
            "\"\"\"",
            "\"\"\"\n",
            "\\",
            "a\\",
            "}}}}",
            "`a${b}c${",
            "/",
            "x = /",
            "x = /[",
            "é\u{0}\u{FFFF}😀 /* é */ \"é",
            "0x",
            "1e+",
            ".5e-3",
            "\r\n\r\n",
            "return /abc\\",
            "`\\",
            "'\\",
            "\"\\",
        ];
        for s in inputs {
            for spec in [java(), js()] {
                let out = lex(s, &spec, LexOptions::default());
                assert_eq!(out.token_hashes.len(), out.token_lines.len());
            }
        }
        // every prefix of a tricky text
        let text = "const a = `x ${ f({ b: `inner ${c}` }) }` /* é */ + /re[/]/g + \"s\\\"\" + '\\'' // end\n";
        for end in 0..=text.len() {
            if text.is_char_boundary(end) {
                lex(&text[..end], &js(), LexOptions::default());
                lex(&text[..end], &java(), LexOptions::default());
            }
        }
    }

    #[test]
    fn line_count_semantics() {
        assert_eq!(lex("", &java(), LexOptions::default()).lines, 0);
        assert_eq!(lex("a", &java(), LexOptions::default()).lines, 1);
        assert_eq!(lex("a\n", &java(), LexOptions::default()).lines, 1);
        assert_eq!(lex("a\nb", &java(), LexOptions::default()).lines, 2);
        assert_eq!(lex("a\r\nb\r\n", &java(), LexOptions::default()).lines, 2);
    }
}
