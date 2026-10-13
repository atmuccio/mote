//! Text → [`Node`] tree, with positions where libyaml put them (so configuration problems keep
//! their columns) and the limits that keep hostile documents cheap.
//!
//! The parser places some events elsewhere than libyaml did; the loader moves them:
//!
//! * a node with an anchor or a tag starts at its first property (`&x {…}` at `&`);
//! * a block scalar starts at its indicator (`|`, `>`), not at its first content line;
//! * a block sequence starts at its first `-` (an indentless one too);
//! * an empty value is just after the `:` (or `-`) in block style, at the next token (`,` or
//!   `}`) in flow style;
//! * a block mapping starts at its first key, properties included.

use super::{
    Error, ErrorKind, Mark, Node, Scalar, Style, Value, MAX_ALIAS_NODES, MAX_BYTES, MAX_DEPTH,
    MAX_NODES,
};
use granit_parser::{Event, Marker, Parser, ScalarStyle, ScanError, Span, StructureStyle, Tag};
use std::borrow::Cow;
use std::collections::HashMap;

/// The document of `text`.
#[cfg(test)]
pub(crate) fn load(text: &str) -> Result<Node, Error> {
    match load_partial(text) {
        (node, None) => Ok(node),
        (_, Some(e)) => Err(e),
    }
}

/// The document of `text` and, when the text stops being YAML (or breaks a limit), the error
/// with what was read before it: open sequences and mappings closed, a key without its value
/// left out.
pub(crate) fn load_partial(text: &str) -> (Node, Option<Error>) {
    let refused = |e: Error| (missing(&e), Some(e));
    if text.len() > MAX_BYTES {
        return refused(Error::new(ErrorKind::TooLarge(text.len())));
    }
    if let Some(offset) = disallowed_character(text) {
        return refused(Error::new(ErrorKind::ControlCharacter(offset)));
    }
    if let Some(e) = unknown_directive(text) {
        return refused(e);
    }
    let mut loader = Loader::new(text);
    let error = loader.run().err();
    (loader.finish(error.as_ref()), error)
}

/// Byte offset of the first character YAML does not allow (C0 and C1 controls other than tab,
/// line feed, carriage return and next line; DEL; U+FFFE, U+FFFF), as libyaml checked.
fn disallowed_character(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|&(_, c)| {
            !matches!(c,
                '\t' | '\n' | '\r' | '\u{20}'..='\u{7e}' | '\u{85}'
                | '\u{a0}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..)
        })
        .map(|(i, _)| i)
}

/// A directive other than `%YAML` and `%TAG` before the document, which libyaml refused (the
/// YAML specification lets a parser ignore it).
fn unknown_directive(text: &str) -> Option<Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let name: String = line
            .strip_prefix('%')?
            .chars()
            .take_while(|c| !c.is_whitespace())
            .collect();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
        if name != "YAML" && name != "TAG" {
            let line = i + 1;
            return Some(Error::new(ErrorKind::Syntax(
                "found unknown directive name".to_string(),
                Mark {
                    line,
                    column: 2 + name.chars().count(),
                },
                Some((
                    "while scanning a directive".to_string(),
                    Some(Mark { line, column: 1 }),
                )),
            )));
        }
    }
    None
}

/// An open sequence or mapping.
struct Frame {
    mark: Mark,
    tag: Option<String>,
    anchor: usize,
    /// `[…]` or `{…}`.
    flow: bool,
    /// The mark comes from the first key (a block mapping without properties).
    mark_from_first_key: bool,
    /// Nodes in the document when the frame opened.
    nodes_before: usize,
    /// Height of the tallest child.
    height: usize,
    kind: FrameKind,
}

enum FrameKind {
    Seq(Vec<Node>),
    Map(Vec<(Node, Node)>, Option<Node>),
}

/// A completed anchored node, for its aliases.
struct Anchored {
    node: Node,
    nodes: usize,
    height: usize,
}

struct Loader<'a> {
    parser: Parser<'a, granit_parser::StrInput<'a>>,
    src: Source<'a>,
    stack: Vec<Frame>,
    root: Option<Node>,
    anchors: HashMap<usize, Anchored>,
    documents: usize,
    nodes: usize,
    alias_nodes: usize,
}

impl<'a> Loader<'a> {
    fn new(text: &'a str) -> Loader<'a> {
        let options = granit_parser::options! {
            block_nesting_limit: MAX_DEPTH,
            flow_nesting_limit: MAX_DEPTH,
        };
        Loader {
            parser: Parser::new_from_str_with_options(text, options),
            src: Source::new(text),
            stack: Vec::new(),
            root: None,
            anchors: HashMap::new(),
            documents: 0,
            nodes: 0,
            alias_nodes: 0,
        }
    }

    fn run(&mut self) -> Result<(), Error> {
        while let Some(next) = self.parser.next_event() {
            let (event, span) = match next {
                Ok(e) => e,
                // Whatever follows the first document makes a second one.
                Err(_) if self.documents > 0 => {
                    return Err(Error::new(ErrorKind::MoreThanOneDocument))
                }
                Err(e) => return Err(self.syntax_error(&e)),
            };
            if self.documents > 0 && !matches!(event, Event::StreamEnd) {
                return Err(Error::new(ErrorKind::MoreThanOneDocument));
            }
            match event {
                Event::StreamStart | Event::DocumentStart(..) => {}
                Event::DocumentEnd => self.documents += 1,
                Event::StreamEnd => {
                    if self.root.is_none() {
                        self.root = Some(Node {
                            value: Value::Void,
                            tag: None,
                            mark: mark(span.start),
                        });
                    }
                    break;
                }
                Event::Alias(id) => self.alias(id, span)?,
                Event::Scalar(text, style, anchor, tag) => {
                    self.scalar(text, style, anchor, tag, span)?
                }
                Event::SequenceStart(style, anchor, tag) => {
                    self.open(true, style, anchor, tag, span)?
                }
                Event::MappingStart(style, anchor, tag) => {
                    self.open(false, style, anchor, tag, span)?
                }
                Event::SequenceEnd | Event::MappingEnd => self.close(),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
        Ok(())
    }

    /// The root, with any open sequences and mappings closed: after an error, at a
    /// [`Value::Missing`] node (as a last item, the value of a key without one, or a last key).
    fn finish(mut self, error: Option<&Error>) -> Node {
        if let Some(e) = error.filter(|_| self.root.is_none()) {
            match self.stack.last_mut().map(|f| &mut f.kind) {
                Some(FrameKind::Seq(items)) => items.push(missing(e)),
                Some(FrameKind::Map(entries, key)) => {
                    let key = key.take().unwrap_or_else(|| missing(e));
                    entries.push((key, missing(e)));
                }
                None => self.root = Some(missing(e)),
            }
        }
        while !self.stack.is_empty() {
            self.close();
        }
        self.root.unwrap_or_else(|| Node::new(Value::Void))
    }

    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.nodes += n;
        if self.nodes > MAX_NODES {
            return Err(Error::new(ErrorKind::TooManyNodes));
        }
        Ok(())
    }

    fn scalar(
        &mut self,
        text: Cow<'_, str>,
        style: ScalarStyle,
        anchor: usize,
        tag: Option<Cow<'_, Tag>>,
        span: Span,
    ) -> Result<(), Error> {
        self.count(1)?;
        let style = match style {
            ScalarStyle::Plain => Style::Plain,
            ScalarStyle::SingleQuoted => Style::SingleQuoted,
            ScalarStyle::DoubleQuoted => Style::DoubleQuoted,
            ScalarStyle::Literal => Style::Literal,
            ScalarStyle::Folded => Style::Folded,
        };
        // The parser writes an empty node as `~`, with nothing in the text.
        let empty = style == Style::Plain && span.start == span.end;
        let mut at = self.src.pos(span.start);
        if matches!(style, Style::Literal | Style::Folded) {
            if let Some(header) = self.src.block_header(at) {
                at = header;
            }
        }
        if anchor != 0 || tag.is_some() {
            at = self.src.properties(at);
        } else if empty {
            at = self.empty_value(at);
        }
        let node = Node {
            value: Value::Scalar(Scalar {
                text: if empty {
                    String::new()
                } else {
                    text.into_owned()
                },
                style,
            }),
            tag: tag.map(|t| tag_text(&t)),
            mark: self.src.mark(at),
        };
        self.complete(node, anchor, 1, 0);
        Ok(())
    }

    /// Where libyaml put an empty value: after the `:` or `-` in block style, at the next token
    /// in a flow mapping.
    fn empty_value(&self, at: Pos) -> Pos {
        match self.stack.last() {
            Some(Frame {
                flow: true,
                kind: FrameKind::Map(_, Some(_)),
                ..
            }) => match self.src.char_at(at) {
                Some((':', after)) => self.src.skip_blank_forward(after),
                _ => at,
            },
            Some(Frame {
                flow: false,
                kind: FrameKind::Map(_, Some(_)),
                ..
            }) => match self.src.char_at(at) {
                Some((':', after)) => after,
                _ => at,
            },
            Some(Frame {
                flow: false,
                kind: FrameKind::Seq(_),
                ..
            }) => self.src.after_dash(at).unwrap_or(at),
            _ => at,
        }
    }

    fn open(
        &mut self,
        seq: bool,
        style: StructureStyle,
        anchor: usize,
        tag: Option<Cow<'_, Tag>>,
        span: Span,
    ) -> Result<(), Error> {
        let mut at = self.src.pos(span.start);
        if self.stack.len() >= MAX_DEPTH {
            return Err(Error::new(ErrorKind::RecursionLimitExceeded(
                self.src.mark(at),
            )));
        }
        let nodes_before = self.nodes;
        self.count(1)?;
        let flow = matches!(style, StructureStyle::Flow);
        if seq && !flow && self.src.char_at(at).map(|(c, _)| c) != Some('-') {
            // An indentless sequence starts at its `-`, not at its first item.
            if let Some(dash) = self.src.dash_before(at) {
                at = dash;
            }
        }
        let has_properties = anchor != 0 || tag.is_some();
        if has_properties {
            at = self.src.properties(at);
        }
        self.stack.push(Frame {
            mark: self.src.mark(at),
            tag: tag.map(|t| tag_text(&t)),
            anchor,
            flow,
            mark_from_first_key: !seq && !flow && !has_properties,
            nodes_before,
            height: 0,
            kind: if seq {
                FrameKind::Seq(Vec::new())
            } else {
                FrameKind::Map(Vec::new(), None)
            },
        });
        Ok(())
    }

    fn close(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        let value = match frame.kind {
            FrameKind::Seq(items) => Value::Seq(items),
            // A key without its value (the text ended) is left out.
            FrameKind::Map(entries, _) => Value::Map(entries),
        };
        let node = Node {
            value,
            tag: frame.tag,
            mark: frame.mark,
        };
        let nodes = self.nodes - frame.nodes_before;
        self.complete(node, frame.anchor, nodes, frame.height + 1);
    }

    fn alias(&mut self, id: usize, span: Span) -> Result<(), Error> {
        let at = self.src.pos(span.start);
        let at = self.src.mark(at);
        // An alias of a node still open (`&a [*a]`) would expand forever.
        let Some(target) = self.anchors.get(&id) else {
            return Err(Error::new(ErrorKind::RecursionLimitExceeded(at)));
        };
        let (nodes, height) = (target.nodes, target.height);
        if self.alias_nodes + nodes > MAX_ALIAS_NODES {
            return Err(Error::new(ErrorKind::RepetitionLimitExceeded(at)));
        }
        if self.stack.len() + height > MAX_DEPTH {
            return Err(Error::new(ErrorKind::RecursionLimitExceeded(at)));
        }
        let node = target.node.clone();
        self.alias_nodes += nodes;
        self.count(nodes)?;
        self.complete(node, 0, nodes, height);
        Ok(())
    }

    /// Add a completed node to the open sequence or mapping (or make it the root).
    fn complete(&mut self, node: Node, anchor: usize, nodes: usize, height: usize) {
        if anchor != 0 {
            self.anchors.insert(
                anchor,
                Anchored {
                    node: node.clone(),
                    nodes,
                    height,
                },
            );
        }
        let Some(frame) = self.stack.last_mut() else {
            if self.root.is_none() {
                self.root = Some(node);
            }
            return;
        };
        frame.height = frame.height.max(height);
        match &mut frame.kind {
            FrameKind::Seq(items) => items.push(node),
            FrameKind::Map(entries, key) => match key.take() {
                Some(k) => entries.push((k, node)),
                None => {
                    if frame.mark_from_first_key && entries.is_empty() {
                        frame.mark = node.mark;
                    }
                    *key = Some(node);
                }
            },
        }
    }
}

fn missing(e: &Error) -> Node {
    Node::new(Value::Missing(Box::new(e.clone())))
}

fn tag_text(t: &Tag) -> String {
    format!("{}{}", t.handle(), t.suffix())
}

fn mark(m: Marker) -> Mark {
    Mark {
        line: m.line(),
        column: m.col() + 1,
    }
}

impl Loader<'_> {
    /// The parser's error, in libyaml's terms where they differ (the old parser's messages): an
    /// unclosed collection or quote is reported where the text ends, with where it opened.
    fn syntax_error(&self, e: &ScanError) -> Error {
        use granit_parser::ErrorKind as Kind;
        let at = mark(*e.marker());
        let open = self.stack.last();
        // The innermost open collection of a kind, for libyaml's "while parsing …" position.
        let start_of = |flow: bool, seq: bool| {
            open.filter(|f| f.flow == flow && matches!(f.kind, FrameKind::Seq(_)) == seq)
                .map(|f| f.mark)
        };
        let context = |what: &str, at: Option<Mark>| Some((what.to_string(), at));
        let (problem, at, context) = match e.kind() {
            Kind::UnknownAnchor => match self.bad_alias_name(e.marker()) {
                // libyaml read only letters, digits, `_` and `-` as an alias name: `*.js` is not
                // an alias of an unknown anchor but a bad alias.
                Some(bad) => (
                    "did not find expected alphabetic or numeric character".to_string(),
                    bad,
                    context("while scanning an alias", Some(at)),
                ),
                None => ("unknown anchor".to_string(), at, None),
            },
            Kind::UnclosedFlowCollection { open }
            | Kind::MismatchedFlowCollectionEnd { open, .. } => {
                let (close, what) = if *open == '{' {
                    ('}', "mapping")
                } else {
                    (']', "sequence")
                };
                // An unclosed collection is reported where the text ends; a wrong closing
                // bracket where it is.
                let (problem_at, opened) = match e.kind() {
                    Kind::UnclosedFlowCollection { .. } => (self.src.end(), Some(at)),
                    _ => (at, start_of(true, *open == '[')),
                };
                (
                    format!("did not find expected ',' or '{close}'"),
                    problem_at,
                    context(&format!("while parsing a flow {what}"), opened),
                )
            }
            Kind::MisplacedFlowCollectionEnd => {
                // A closing bracket in block context: what libyaml was looking for there.
                let (problem, what, opened) = match open.map(|f| (&f.kind, f.flow)) {
                    Some((FrameKind::Map(_, None), false)) => (
                        "did not find expected key",
                        "while parsing a block mapping",
                        start_of(false, false),
                    ),
                    Some((FrameKind::Seq(_), false)) => (
                        "did not find expected '-' indicator",
                        "while parsing a block collection",
                        start_of(false, true),
                    ),
                    _ => (
                        "did not find expected node content",
                        "while parsing a block node",
                        None,
                    ),
                };
                (problem.to_string(), at, context(what, opened))
            }
            Kind::SimpleKeyExpected => (
                // libyaml gave up on the key at the next token after its line.
                "could not find expected ':'".to_string(),
                self.src.after_line(e.marker()),
                context("while scanning a simple key", Some(at)),
            ),
            Kind::UnsupportedYamlMajorVersion => {
                ("found incompatible YAML document".to_string(), at, None)
            }
            Kind::UnexpectedCharacter { .. } => (
                "found character that cannot start any token".to_string(),
                at,
                context("while scanning for the next token", None),
            ),
            Kind::TabInBlockIndentation => (
                "found character that cannot start any token".to_string(),
                // At the tab.
                self.src.tab_before(e.marker()).unwrap_or(at),
                context("while scanning for the next token", None),
            ),
            Kind::UnclosedQuotedScalar => (
                "found unexpected end of stream".to_string(),
                self.src.end(),
                context("while scanning a quoted scalar", Some(at)),
            ),
            // A quote still open where the text ends.
            Kind::InvalidQuotedScalarIndent if self.src.only_blanks_after(e.marker()) => (
                "found unexpected end of stream".to_string(),
                self.src.end(),
                context(
                    "while scanning a quoted scalar",
                    self.src.opening_quote_before(e.marker()),
                ),
            ),
            _ => {
                let info = e.info();
                // `while parsing a block mapping, did not find expected key`: libyaml put what
                // was being read last.
                match info.strip_prefix("while ").and_then(|r| r.split_once(", ")) {
                    Some((what, problem)) => {
                        let moved = self.libyaml_position(problem, what, e.marker());
                        let opened = match what {
                            // libyaml named where the block scalar, anchor or alias starts.
                            "scanning a block scalar" if moved.is_some() => Some(at),
                            "scanning an anchor or alias" => Some(at),
                            "parsing a block mapping" => start_of(false, false),
                            _ => None,
                        };
                        let what = match (what, self.src.char_at(self.src.pos(*e.marker()))) {
                            ("scanning an anchor or alias", Some(('&', _))) => "scanning an anchor",
                            ("scanning an anchor or alias", Some(('*', _))) => "scanning an alias",
                            (what, _) => what,
                        };
                        let problem = match problem {
                            "found a tab" => "found a tab character that violates indentation",
                            p => p,
                        };
                        let at = moved.unwrap_or(at);
                        (
                            problem.to_string(),
                            at,
                            context(&format!("while {what}"), opened),
                        )
                    }
                    None => (info, at, None),
                }
            }
        };
        Error::new(ErrorKind::Syntax(problem, at, context))
    }
}

impl Loader<'_> {
    /// For an alias `*name` at `m` that libyaml would not have read (its names are letters,
    /// digits, `_` and `-`, followed by a blank or an indicator): where it stopped.
    fn bad_alias_name(&self, m: &Marker) -> Option<Mark> {
        let p = self.src.pos(*m);
        let ('*', mut q) = self.src.char_at(p)? else {
            return None;
        };
        let mut length = 0;
        while let Some((c, r)) = self.src.char_at(q) {
            if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                break;
            }
            q = r;
            length += 1;
        }
        let ends_well = match self.src.char_at(q) {
            None => true,
            Some((c, _)) => {
                c.is_whitespace() || matches!(c, '?' | ':' | ',' | ']' | '}' | '%' | '@' | '`')
            }
        };
        (length == 0 || !ends_well).then(|| self.src.mark(q))
    }

    /// Where libyaml reported two common errors: an entry `- …` where a mapping key is due at
    /// its `-`; an invalid block scalar header (`mote: >=0.1`) at the character after the
    /// indicator.
    fn libyaml_position(&self, problem: &str, context: &str, m: &Marker) -> Option<Mark> {
        let p = self.src.pos(*m);
        match (problem, context) {
            ("did not find expected key", "parsing a block mapping") => {
                self.src.dash_before(p).map(|d| self.src.mark(d))
            }
            // At the character after `&` or `*`.
            (
                "did not find expected alphabetic or numeric character",
                "scanning an anchor or alias",
            ) => {
                let (c, after) = self.src.char_at(p)?;
                matches!(c, '&' | '*').then(|| self.src.mark(after))
            }
            // At the tab.
            ("found a tab", "scanning a plain scalar") => {
                let mut q = p;
                while let Some((c, r)) = self.src.char_at(q) {
                    match c {
                        '\t' => return Some(self.src.mark(q)),
                        ' ' | '\n' | '\r' => q = r,
                        _ => return None,
                    }
                }
                None
            }
            ("did not find expected comment or line break", "scanning a block scalar") => {
                let (c, mut q) = self.src.char_at(p)?;
                if !matches!(c, '|' | '>') {
                    return None;
                }
                while let Some((c, r)) = self.src.char_at(q) {
                    if !matches!(c, '+' | '-' | '0'..='9' | ' ' | '\t') {
                        break;
                    }
                    q = r;
                }
                Some(self.src.mark(q))
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Positions in the text.

/// A position: 1-based line, 0-based column in characters, byte offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pos {
    line: usize,
    col: usize,
    byte: usize,
}

/// The text with its line starts, for the small scans that move event positions. Line breaks
/// are those of the parser: `\n`, `\r\n` and `\r`; a byte order mark is not on line 1.
struct Source<'a> {
    text: &'a str,
    /// Byte offset of each line's start (line 1 first).
    line_starts: Vec<usize>,
}

impl<'a> Source<'a> {
    fn new(text: &'a str) -> Source<'a> {
        let bom = if text.starts_with('\u{feff}') { 3 } else { 0 };
        let mut line_starts = vec![bom];
        let bytes = text.as_bytes();
        let mut i = bom;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                    i += 2;
                    line_starts.push(i);
                }
                b'\n' | b'\r' => {
                    i += 1;
                    line_starts.push(i);
                }
                _ => i += 1,
            }
        }
        Source { text, line_starts }
    }

    /// The end of the text, where libyaml put it: at the start of a line (after a last line
    /// without a line break, on the next one).
    fn end(&self) -> Mark {
        let last = self.line_starts.len();
        Mark {
            line: if self.line_start(last) < self.text.len() {
                last + 1
            } else {
                last
            },
            column: 1,
        }
    }

    fn line_start(&self, line: usize) -> usize {
        self.line_starts
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or(self.text.len())
    }

    fn mark(&self, p: Pos) -> Mark {
        Mark {
            line: p.line,
            column: p.col + 1,
        }
    }

    /// The position of a parser marker.
    fn pos(&self, m: Marker) -> Pos {
        let (line, col) = (m.line().max(1), m.col());
        let byte = match m.byte_offset() {
            Some(b) if b <= self.text.len() && self.text.is_char_boundary(b) => b,
            // Not given: count the characters from the start of the line.
            _ => {
                let start = self.line_start(line);
                self.text[start..]
                    .char_indices()
                    .nth(col)
                    .map_or(self.text.len(), |(i, _)| start + i)
            }
        };
        Pos { line, col, byte }
    }

    /// The character at `p` and the position after it (on the next line after a break).
    fn char_at(&self, p: Pos) -> Option<(char, Pos)> {
        let ch = self.text.get(p.byte..)?.chars().next()?;
        let next = match ch {
            '\r' if self.text[p.byte + 1..].starts_with('\n') => Pos {
                line: p.line + 1,
                col: 0,
                byte: p.byte + 2,
            },
            '\n' | '\r' => Pos {
                line: p.line + 1,
                col: 0,
                byte: p.byte + 1,
            },
            _ => Pos {
                line: p.line,
                col: p.col + 1,
                byte: p.byte + ch.len_utf8(),
            },
        };
        Some((ch, next))
    }

    /// The character before `p` and its position (a line break, `\r\n` as one, at the end of
    /// the line before).
    fn char_before(&self, p: Pos) -> Option<(char, Pos)> {
        if p.byte <= self.line_start(1) {
            return None;
        }
        let ch = self.text.get(..p.byte)?.chars().next_back()?;
        if p.col > 0 {
            return Some((
                ch,
                Pos {
                    line: p.line,
                    col: p.col - 1,
                    byte: p.byte - ch.len_utf8(),
                },
            ));
        }
        let line = p.line.checked_sub(1).filter(|l| *l >= 1)?;
        let start = self.line_start(line);
        let mut end = p.byte - 1;
        if ch == '\n' && end > start && self.text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        Some((
            ch,
            Pos {
                line,
                col: self.text[start..end].chars().count(),
                byte: end,
            },
        ))
    }

    /// Back over blanks, line breaks and comments.
    fn skip_blank_back(&self, mut p: Pos) -> Pos {
        while let Some((ch, q)) = self.char_before(p) {
            match ch {
                ' ' | '\t' => p = q,
                '\n' | '\r' => p = self.before_comment(q),
                _ => break,
            }
        }
        p
    }

    /// `p` at the end of a line: the start of the line's comment, if it has one.
    fn before_comment(&self, p: Pos) -> Pos {
        let start = self.line_start(p.line);
        let line = &self.text[start..p.byte];
        let mut prev_blank = true;
        for (i, ch) in line.char_indices() {
            if ch == '#' && prev_blank {
                return Pos {
                    line: p.line,
                    col: line[..i].chars().count(),
                    byte: start + i,
                };
            }
            prev_blank = ch == ' ' || ch == '\t';
        }
        p
    }

    /// Forward over blanks, line breaks and comments.
    fn skip_blank_forward(&self, mut p: Pos) -> Pos {
        while let Some((ch, q)) = self.char_at(p) {
            match ch {
                ' ' | '\t' | '\n' | '\r' => p = q,
                '#' => {
                    // To the end of the line.
                    p = q;
                    while let Some((c, r)) = self.char_at(p) {
                        if c == '\n' || c == '\r' {
                            break;
                        }
                        p = r;
                    }
                }
                _ => break,
            }
        }
        p
    }

    /// The start of the node properties (`&anchor`, `!tag`) written before `p`, or `p`.
    fn properties(&self, p: Pos) -> Pos {
        let mut start = p;
        // A node has at most one anchor and one tag.
        for _ in 0..2 {
            let end = self.skip_blank_back(start);
            let mut token = end;
            while let Some((ch, q)) = self.char_before(token) {
                if ch.is_whitespace() || matches!(ch, ',' | '[' | ']' | '{' | '}') {
                    break;
                }
                token = q;
            }
            if token != end && matches!(self.text.as_bytes()[token.byte], b'&' | b'!') {
                start = token;
            } else {
                break;
            }
        }
        start
    }

    /// The indicator (`|`, `>`) of the block scalar whose content starts at `p`.
    fn block_header(&self, p: Pos) -> Option<Pos> {
        let mut q = self.skip_blank_back(p);
        while let Some((ch, r)) = self.char_before(q) {
            match ch {
                '+' | '-' | '0'..='9' => q = r,
                '|' | '>' => return Some(r),
                _ => return None,
            }
        }
        None
    }

    /// The `-` before `p` on its line, past blanks.
    fn dash_before(&self, p: Pos) -> Option<Pos> {
        let mut q = p;
        while let Some((ch, r)) = self.char_before(q) {
            match ch {
                ' ' | '\t' if r.line == p.line => q = r,
                '-' if r.line == p.line => return Some(r),
                _ => return None,
            }
        }
        None
    }

    /// The next token after the line of a parser marker (or the end of the text).
    fn after_line(&self, m: &Marker) -> Mark {
        let mut p = self.pos(*m);
        while let Some((c, q)) = self.char_at(p) {
            if c == '\n' || c == '\r' {
                break;
            }
            p = q;
        }
        let p = self.skip_blank_forward(p);
        if p.byte >= self.text.len() {
            return self.end();
        }
        self.mark(p)
    }

    /// Whether only blanks and line breaks follow a parser marker.
    fn only_blanks_after(&self, m: &Marker) -> bool {
        let p = self.pos(*m);
        self.text[p.byte..].trim().is_empty()
    }

    /// The nearest quote before a parser marker that can open a quoted scalar (after a blank,
    /// an indicator or the start of a line).
    fn opening_quote_before(&self, m: &Marker) -> Option<Mark> {
        let mut p = self.pos(*m);
        while let Some((c, q)) = self.char_before(p) {
            if c == '\'' || c == '"' {
                let opens = match self.char_before(q) {
                    None => true,
                    Some((b, _)) => {
                        b.is_whitespace() || matches!(b, ':' | ',' | '[' | '{' | '-' | '?')
                    }
                };
                if opens {
                    return Some(self.mark(q));
                }
            }
            p = q;
        }
        None
    }

    /// The tab just before a parser marker.
    fn tab_before(&self, m: &Marker) -> Option<Mark> {
        let (ch, p) = self.char_before(self.pos(*m))?;
        (ch == '\t').then(|| self.mark(p))
    }

    /// Just after the `-` before `p` on its line.
    fn after_dash(&self, p: Pos) -> Option<Pos> {
        let dash = self.dash_before(p)?;
        self.char_at(dash).map(|(_, after)| after)
    }
}
