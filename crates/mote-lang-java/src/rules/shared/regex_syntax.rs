//! A conservative parser for `java.util.regex` patterns, producing a small syntax tree the
//! regex rules (batches 9, 13 and 14) reason about.
//!
//! It models the Java syntax the rules need — possessive and lazy quantifiers, `\Q…\E`, inline
//! flags (`(?i)`, `(?m:…)`), named groups, lookarounds, `\p{…}` classes, nested and intersected
//! character classes, Java's rule for multi-digit back-references — and answers `None` for
//! anything it does not model or that would not compile (comments mode, `LITERAL`, `CANON_EQ`,
//! unknown escapes, unbalanced groups, …). A rule that gets `None` stays silent; mistakes that
//! make the pattern throw belong to `java/invalid-regex-literal`.
//!
//! Positions (`start`, `end`) are indices into the pattern's `char`s, used to quote fragments in
//! messages.

/// `Pattern.UNIX_LINES`.
pub(crate) const UNIX_LINES: u32 = 0x01;
/// `Pattern.CASE_INSENSITIVE`.
pub(crate) const CASE_INSENSITIVE: u32 = 0x02;
/// `Pattern.COMMENTS`.
pub(crate) const COMMENTS: u32 = 0x04;
/// `Pattern.MULTILINE`.
pub(crate) const MULTILINE: u32 = 0x08;
/// `Pattern.LITERAL`.
pub(crate) const LITERAL: u32 = 0x10;
/// `Pattern.DOTALL`.
pub(crate) const DOTALL: u32 = 0x20;
/// `Pattern.UNICODE_CASE`.
pub(crate) const UNICODE_CASE: u32 = 0x40;
/// `Pattern.CANON_EQ`.
pub(crate) const CANON_EQ: u32 = 0x80;
/// `Pattern.UNICODE_CHARACTER_CLASS`.
pub(crate) const UNICODE_CHARACTER_CLASS: u32 = 0x100;

/// Longest pattern (in chars) the parser looks at.
const MAX_LEN: usize = 4000;
/// Deepest group nesting the parser follows.
const MAX_DEPTH: u32 = 64;
const MAX_CHAR: u32 = 0x10_FFFF;

// ------------------------------------------------------------------------------------------
// Character sets
// ------------------------------------------------------------------------------------------

/// A set of Unicode code points as sorted, disjoint, non-adjacent inclusive ranges.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct CharSet(Vec<(u32, u32)>);

impl CharSet {
    pub(crate) fn empty() -> Self {
        CharSet(Vec::new())
    }

    pub(crate) fn full() -> Self {
        CharSet(vec![(0, MAX_CHAR)])
    }

    pub(crate) fn single(c: char) -> Self {
        CharSet(vec![(c as u32, c as u32)])
    }

    pub(crate) fn from_ranges(mut v: Vec<(u32, u32)>) -> Self {
        v.retain(|(a, b)| a <= b);
        v.sort_unstable();
        let mut out: Vec<(u32, u32)> = Vec::with_capacity(v.len());
        for (a, b) in v {
            match out.last_mut() {
                Some(last) if a <= last.1.saturating_add(1) => last.1 = last.1.max(b),
                _ => out.push((a, b)),
            }
        }
        CharSet(out)
    }

    pub(crate) fn chars(s: &str) -> Self {
        Self::from_ranges(s.chars().map(|c| (c as u32, c as u32)).collect())
    }

    pub(crate) fn union(&self, o: &CharSet) -> CharSet {
        let mut v = self.0.clone();
        v.extend_from_slice(&o.0);
        Self::from_ranges(v)
    }

    pub(crate) fn complement(&self) -> CharSet {
        let mut out = Vec::with_capacity(self.0.len() + 1);
        let mut next = 0u32;
        for &(a, b) in &self.0 {
            if a > next {
                out.push((next, a - 1));
            }
            next = b.saturating_add(1);
        }
        if next <= MAX_CHAR {
            out.push((next, MAX_CHAR));
        }
        CharSet(out)
    }

    pub(crate) fn intersect(&self, o: &CharSet) -> CharSet {
        let mut out = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() && j < o.0.len() {
            let (a1, b1) = self.0[i];
            let (a2, b2) = o.0[j];
            let lo = a1.max(a2);
            let hi = b1.min(b2);
            if lo <= hi {
                out.push((lo, hi));
            }
            if b1 < b2 {
                i += 1;
            } else {
                j += 1;
            }
        }
        CharSet(out)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn contains(&self, c: char) -> bool {
        let c = c as u32;
        self.0.iter().any(|&(a, b)| a <= c && c <= b)
    }

    pub(crate) fn disjoint(&self, o: &CharSet) -> bool {
        self.intersect(o).is_empty()
    }

    pub(crate) fn subset_of(&self, o: &CharSet) -> bool {
        self.intersect(&o.complement()).is_empty()
    }

    /// The set extended with the other case of its ASCII letters (Java's case-insensitive
    /// matching without `UNICODE_CASE`).
    fn ascii_fold(&self) -> CharSet {
        let upper = self.intersect(&CharSet(vec![(b'A' as u32, b'Z' as u32)]));
        let lower = self.intersect(&CharSet(vec![(b'a' as u32, b'z' as u32)]));
        let mut v = self.0.clone();
        v.extend(upper.0.iter().map(|(a, b)| (a + 32, b + 32)));
        v.extend(lower.0.iter().map(|(a, b)| (a - 32, b - 32)));
        Self::from_ranges(v)
    }
}

/// Characters Java treats as line terminators (in any mode).
pub(crate) fn line_terminators() -> CharSet {
    CharSet::chars("\n\r\u{85}\u{2028}\u{2029}")
}

/// ASCII word characters: word characters for `\b` in every Java release and mode.
pub(crate) fn ascii_word() -> CharSet {
    CharSet::from_ranges(vec![
        ('0' as u32, '9' as u32),
        ('A' as u32, 'Z' as u32),
        ('_' as u32, '_' as u32),
        ('a' as u32, 'z' as u32),
    ])
}

/// ASCII characters that are not word characters: non-word for `\b` in every release and mode.
pub(crate) fn ascii_non_word() -> CharSet {
    CharSet(vec![(0, 0x7f)]).intersect(&ascii_word().complement())
}

/// The set of a predefined class escape (`\d`, `\s`, …) outside Unicode-class mode.
fn escape_set(e: char, flags: u32) -> Option<CharSet> {
    if flags & UNICODE_CHARACTER_CLASS != 0 {
        return None;
    }
    let base = match e.to_ascii_lowercase() {
        'd' => CharSet(vec![('0' as u32, '9' as u32)]),
        'w' => ascii_word(),
        's' => CharSet::chars(" \t\n\u{b}\u{c}\r"),
        'h' => CharSet::from_ranges(vec![
            (0x20, 0x20),
            (0x09, 0x09),
            (0xa0, 0xa0),
            (0x1680, 0x1680),
            (0x180e, 0x180e),
            (0x2000, 0x200a),
            (0x202f, 0x202f),
            (0x205f, 0x205f),
            (0x3000, 0x3000),
        ]),
        'v' => CharSet::chars("\n\u{b}\u{c}\r\u{85}\u{2028}\u{2029}"),
        _ => return None,
    };
    Some(if e.is_ascii_uppercase() {
        base.complement()
    } else {
        base
    })
}

/// The set `.` matches under `flags`.
fn dot_set(flags: u32) -> CharSet {
    if flags & DOTALL != 0 {
        CharSet::full()
    } else if flags & UNIX_LINES != 0 {
        CharSet::single('\n').complement()
    } else {
        line_terminators().complement()
    }
}

/// The set a single-character matcher with raw set `set` really matches under `flags`
/// (case-insensitivity applied); `None` when unknown.
pub(crate) fn effective(set: &CharSet, flags: u32) -> Option<CharSet> {
    if flags & CASE_INSENSITIVE == 0 {
        return Some(set.clone());
    }
    if flags & UNICODE_CASE != 0 {
        // Unicode case folding is not modelled; only caseless sets are exact.
        let letters = CharSet(vec![('A' as u32, 'Z' as u32), ('a' as u32, 'z' as u32)]);
        return (set.0.iter().all(|&(_, b)| b < 0x80) && set.disjoint(&letters))
            .then(|| set.clone());
    }
    Some(set.ascii_fold())
}

// ------------------------------------------------------------------------------------------
// Syntax tree
// ------------------------------------------------------------------------------------------

/// A zero-width assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Assert {
    /// `^`
    Caret,
    /// `$`
    Dollar,
    /// `\b`
    WordBoundary,
    /// `\B`
    NotWordBoundary,
    /// `\A`
    InputStart,
    /// `\z`
    InputEnd,
    /// `\Z`
    InputEndBeforeTerminator,
    /// `\G`, `\b{g}`
    Other,
}

/// The kind of a parenthesized group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupKind {
    /// `(…)` or `(?<name>…)`, with its number.
    Capture(u32),
    /// `(?:…)`
    NonCapture,
    /// `(?flags:…)`
    Flags,
    /// `(?>…)`
    Atomic,
    /// `(?=…)`
    Ahead,
    /// `(?!…)`
    NegAhead,
    /// `(?<=…)`
    Behind,
    /// `(?<!…)`
    NegBehind,
}

impl GroupKind {
    pub(crate) fn is_lookaround(self) -> bool {
        matches!(
            self,
            GroupKind::Ahead | GroupKind::NegAhead | GroupKind::Behind | GroupKind::NegBehind
        )
    }
}

/// How a quantifier backtracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Greedy,
    Lazy,
    Possessive,
}

/// Where a single-character matcher comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SetKind {
    /// `.`
    Dot,
    /// `\d`, `\p{L}`, `\x{…}`, …
    Escape,
    /// `[…]`
    Class(Box<ClassInfo>),
}

/// One item of a simple character class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClassItem {
    /// A single character, written literally or as an escape.
    Char(char),
    /// `lo-hi`
    Range(char, char),
    /// A predefined class escape (`\d`, `\p{L}`); its set when known.
    Escape(Option<CharSet>),
}

/// The items of a character class, recorded when the class is *simple*: no nesting and no
/// intersection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClassInfo {
    pub(crate) negated: bool,
    /// `(item, start, end)` for simple classes; empty otherwise.
    pub(crate) items: Vec<(ClassItem, usize, usize)>,
    /// No nested class and no `&&`.
    pub(crate) simple: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum K {
    /// Matches the empty string (an empty alternative or group).
    Empty,
    /// A literal character (and the flags in effect).
    Char(char, u32),
    /// A single-character matcher: its raw set when known, flags in effect, and its origin.
    Set(Option<CharSet>, u32, SetKind),
    /// A zero-width assertion and the flags in effect.
    Assert(Assert, u32),
    Group(GroupKind, Box<Rx>),
    Alt(Vec<Rx>),
    Seq(Vec<Rx>),
    Repeat {
        body: Box<Rx>,
        min: u32,
        max: Option<u32>,
        mode: Mode,
    },
    /// A numbered back-reference.
    BackRef(u32),
    /// Something with the given length bounds whose content is not modelled (`\R`, `\X`,
    /// named back-references).
    Opaque {
        min: u32,
        max: Option<u32>,
    },
}

/// A node of the syntax tree and its position in the pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rx {
    pub(crate) k: K,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// A parsed pattern.
pub(crate) struct Regex {
    pub(crate) root: Rx,
    pub(crate) chars: Vec<char>,
    /// Number of capturing groups.
    pub(crate) groups: u32,
}

impl Regex {
    /// The pattern text of a node, shortened for messages.
    pub(crate) fn snippet(&self, rx: &Rx) -> String {
        self.snippet_range(rx.start, rx.end)
    }

    pub(crate) fn snippet_range(&self, start: usize, end: usize) -> String {
        let end = end.min(self.chars.len());
        let start = start.min(end);
        let s: String = self.chars[start..end].iter().collect();
        shorten(&s)
    }

    /// The raw pattern text of a node.
    pub(crate) fn text(&self, rx: &Rx) -> String {
        let end = rx.end.min(self.chars.len());
        self.chars[rx.start.min(end)..end].iter().collect()
    }
}

/// Shortens a pattern fragment for a message, escaping control characters and backticks.
pub(crate) fn shorten(s: &str) -> String {
    let mut out = String::new();
    for (n, c) in s.chars().enumerate() {
        if n >= 40 {
            out.push('…');
            break;
        }
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '`' => out.push('\''),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Parses `pattern` compiled with `flags`. `None` when the pattern uses something not
/// modelled or does not compile.
pub(crate) fn parse(pattern: &str, flags: u32) -> Option<Regex> {
    if flags & (COMMENTS | LITERAL | CANON_EQ) != 0 {
        return None;
    }
    let chars: Vec<char> = pattern.chars().collect();
    if chars.len() > MAX_LEN {
        return None;
    }
    let mut flags = flags;
    if flags & UNICODE_CHARACTER_CLASS != 0 {
        flags |= UNICODE_CASE;
    }
    let mut p = Parser {
        c: &chars,
        i: 0,
        flags,
        groups: 0,
        depth: 0,
    };
    let root = p.alt()?;
    if p.i != chars.len() {
        return None;
    }
    let groups = p.groups;
    Some(Regex {
        root,
        chars,
        groups,
    })
}

struct Parser<'a> {
    c: &'a [char],
    i: usize,
    flags: u32,
    groups: u32,
    depth: u32,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn at(&self, k: usize) -> Option<char> {
        self.c.get(k).copied()
    }

    fn alt(&mut self) -> Option<Rx> {
        let start = self.i;
        let first = self.seq()?;
        if self.peek() != Some('|') {
            return Some(first);
        }
        let mut branches = vec![first];
        while self.peek() == Some('|') {
            self.i += 1;
            branches.push(self.seq()?);
        }
        Some(Rx {
            k: K::Alt(branches),
            start,
            end: self.i,
        })
    }

    fn seq(&mut self) -> Option<Rx> {
        let start = self.i;
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let quoted = c == '\\' && self.at(self.i + 1) == Some('Q');
            let Some(atom) = self.atom()? else {
                // Inline flags: no node; a quantifier right after them does not compile.
                if matches!(self.peek(), Some('*' | '+' | '?' | '{')) {
                    return None;
                }
                continue;
            };
            if quoted {
                // Java unquotes `\Q…\E` before parsing, so a quantifier after it applies to
                // the last quoted character only: not modelled.
                if matches!(self.peek(), Some('*' | '+' | '?' | '{')) {
                    return None;
                }
                match atom.k {
                    K::Seq(inner) => items.extend(inner),
                    K::Empty => {}
                    _ => items.push(atom),
                }
                continue;
            }
            items.push(self.quantify(atom)?);
        }
        Some(match items.len() {
            0 => Rx {
                k: K::Empty,
                start,
                end: start,
            },
            1 => items.pop()?,
            _ => Rx {
                k: K::Seq(items),
                start,
                end: self.i,
            },
        })
    }

    /// `Some(None)` for inline flags, `None` when not modelled / invalid.
    fn atom(&mut self) -> Option<Option<Rx>> {
        let start = self.i;
        let c = self.peek()?;
        let k = match c {
            '(' => return self.group(),
            '[' => {
                let (set, info) = self.class()?;
                K::Set(set, self.flags, SetKind::Class(Box::new(info)))
            }
            '\\' => return self.escape().map(Some),
            '.' => {
                self.i += 1;
                K::Set(Some(dot_set(self.flags)), self.flags, SetKind::Dot)
            }
            '^' => {
                self.i += 1;
                K::Assert(Assert::Caret, self.flags)
            }
            '$' => {
                self.i += 1;
                K::Assert(Assert::Dollar, self.flags)
            }
            '*' | '+' | '?' | '{' => return None,
            c => {
                self.i += 1;
                K::Char(c, self.flags)
            }
        };
        Some(Some(Rx {
            k,
            start,
            end: self.i,
        }))
    }

    fn quantify(&mut self, atom: Rx) -> Option<Rx> {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.i += 1;
                (0, None)
            }
            Some('+') => {
                self.i += 1;
                (1, None)
            }
            Some('?') => {
                self.i += 1;
                (0, Some(1))
            }
            Some('{') => {
                let mut j = self.i + 1;
                let lo = self.number(&mut j)?;
                let hi = match self.at(j)? {
                    '}' => Some(lo),
                    ',' => {
                        j += 1;
                        if self.at(j)? == '}' {
                            None
                        } else {
                            let hi = self.number(&mut j)?;
                            if self.at(j)? != '}' || hi < lo {
                                return None;
                            }
                            Some(hi)
                        }
                    }
                    _ => return None,
                };
                self.i = j + 1;
                (lo, hi)
            }
            _ => return Some(atom),
        };
        let mode = match self.peek() {
            Some('?') => {
                self.i += 1;
                Mode::Lazy
            }
            Some('+') => {
                self.i += 1;
                Mode::Possessive
            }
            _ => Mode::Greedy,
        };
        let start = atom.start;
        Some(Rx {
            k: K::Repeat {
                body: Box::new(atom),
                min,
                max,
                mode,
            },
            start,
            end: self.i,
        })
    }

    fn number(&self, j: &mut usize) -> Option<u32> {
        let from = *j;
        let mut v: u32 = 0;
        while let Some(d) = self.at(*j).and_then(|c| c.to_digit(10)) {
            v = v.checked_mul(10)?.checked_add(d)?;
            *j += 1;
        }
        (*j > from).then_some(v)
    }

    fn group(&mut self) -> Option<Option<Rx>> {
        let start = self.i;
        self.i += 1; // (
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let saved = self.flags;
        let kind = if self.peek() == Some('?') {
            self.i += 1;
            match self.peek()? {
                ':' => {
                    self.i += 1;
                    GroupKind::NonCapture
                }
                '=' => {
                    self.i += 1;
                    GroupKind::Ahead
                }
                '!' => {
                    self.i += 1;
                    GroupKind::NegAhead
                }
                '>' => {
                    self.i += 1;
                    GroupKind::Atomic
                }
                '<' => match self.at(self.i + 1)? {
                    '=' => {
                        self.i += 2;
                        GroupKind::Behind
                    }
                    '!' => {
                        self.i += 2;
                        GroupKind::NegBehind
                    }
                    c if c.is_ascii_alphabetic() => {
                        let mut j = self.i + 1;
                        while self.at(j)?.is_ascii_alphanumeric() {
                            j += 1;
                        }
                        if self.at(j)? != '>' {
                            return None;
                        }
                        self.i = j + 1;
                        self.groups += 1;
                        GroupKind::Capture(self.groups)
                    }
                    _ => return None,
                },
                _ => {
                    // Inline flags: (?idmsuU-idmsuU) or (?flags:X).
                    let mut on = true;
                    let mut any = false;
                    loop {
                        let c = self.peek()?;
                        let bit = match c {
                            'i' => CASE_INSENSITIVE,
                            'd' => UNIX_LINES,
                            'm' => MULTILINE,
                            's' => DOTALL,
                            'u' => UNICODE_CASE,
                            'U' => UNICODE_CHARACTER_CLASS | UNICODE_CASE,
                            '-' if on => {
                                on = false;
                                self.i += 1;
                                continue;
                            }
                            ')' if any => {
                                self.i += 1;
                                self.depth -= 1;
                                // The flags stay in effect until the enclosing group ends.
                                return Some(None);
                            }
                            ':' if any => {
                                self.i += 1;
                                break;
                            }
                            // Comments mode, CANON_EQ, unknown letters: not modelled.
                            _ => return None,
                        };
                        any = true;
                        if on {
                            self.flags |= bit;
                        } else {
                            self.flags &= !bit;
                        }
                        self.i += 1;
                    }
                    GroupKind::Flags
                }
            }
        } else {
            self.groups += 1;
            GroupKind::Capture(self.groups)
        };
        let body = self.alt()?;
        if self.peek() != Some(')') {
            return None;
        }
        self.i += 1;
        self.flags = saved;
        self.depth -= 1;
        Some(Some(Rx {
            k: K::Group(kind, Box::new(body)),
            start,
            end: self.i,
        }))
    }

    /// Parses `\x…` outside a class; the backslash is at `self.i`.
    fn escape(&mut self) -> Option<Rx> {
        let start = self.i;
        let e = self.at(self.i + 1)?;
        self.i += 2;
        let flags = self.flags;
        let k = match e {
            'd' | 'D' | 's' | 'S' | 'w' | 'W' | 'h' | 'H' | 'v' | 'V' => {
                K::Set(escape_set(e, flags), flags, SetKind::Escape)
            }
            'p' | 'P' => {
                self.property()?;
                K::Set(None, flags, SetKind::Escape)
            }
            'N' => {
                self.braced()?;
                K::Set(None, flags, SetKind::Escape)
            }
            'b' => {
                if self.peek() == Some('{') {
                    self.braced()?;
                    K::Assert(Assert::Other, flags)
                } else {
                    K::Assert(Assert::WordBoundary, flags)
                }
            }
            'B' => K::Assert(Assert::NotWordBoundary, flags),
            'A' => K::Assert(Assert::InputStart, flags),
            'G' => K::Assert(Assert::Other, flags),
            'Z' => K::Assert(Assert::InputEndBeforeTerminator, flags),
            'z' => K::Assert(Assert::InputEnd, flags),
            'R' => K::Opaque {
                min: 1,
                max: Some(2),
            },
            'X' => K::Opaque { min: 1, max: None },
            '1'..='9' => {
                // Java: take further digits only while they name a group opened so far.
                let mut n = e.to_digit(10)?;
                while let Some(d) = self.peek().and_then(|c| c.to_digit(10)) {
                    let next = n.checked_mul(10)?.checked_add(d)?;
                    if next > self.groups {
                        break;
                    }
                    n = next;
                    self.i += 1;
                }
                K::BackRef(n)
            }
            'k' => {
                if self.peek() != Some('<') {
                    return None;
                }
                let mut j = self.i + 1;
                while self.at(j)?.is_ascii_alphanumeric() {
                    j += 1;
                }
                if self.at(j)? != '>' {
                    return None;
                }
                self.i = j + 1;
                K::Opaque { min: 0, max: None }
            }
            'Q' => {
                let mut items = Vec::new();
                while let Some(c) = self.peek() {
                    if c == '\\' && self.at(self.i + 1) == Some('E') {
                        self.i += 2;
                        break;
                    }
                    items.push(Rx {
                        k: K::Char(c, flags),
                        start: self.i,
                        end: self.i + 1,
                    });
                    self.i += 1;
                }
                return Some(match items.len() {
                    0 => Rx {
                        k: K::Empty,
                        start,
                        end: self.i,
                    },
                    _ => Rx {
                        k: K::Seq(items),
                        start,
                        end: self.i,
                    },
                });
            }
            _ => K::Char(self.escaped_char(e)?, flags),
        };
        Some(Rx {
            k,
            start,
            end: self.i,
        })
    }

    /// The character denoted by an escape `\e` whose letter was just consumed (literal
    /// escapes only: `\t`, `\x41`, `\.`, …).
    fn escaped_char(&mut self, e: char) -> Option<char> {
        Some(match e {
            't' => '\t',
            'n' => '\n',
            'r' => '\r',
            'f' => '\u{c}',
            'a' => '\u{7}',
            'e' => '\u{1b}',
            '0' => {
                // \0n, \0nn, \0mnn (m <= 3)
                let d1 = self.peek()?.to_digit(8)?;
                self.i += 1;
                let mut v = d1;
                if let Some(d2) = self.peek().and_then(|c| c.to_digit(8)) {
                    self.i += 1;
                    v = v * 8 + d2;
                    if d1 <= 3 {
                        if let Some(d3) = self.peek().and_then(|c| c.to_digit(8)) {
                            self.i += 1;
                            v = v * 8 + d3;
                        }
                    }
                }
                char::from_u32(v)?
            }
            'x' => {
                if self.peek() == Some('{') {
                    let (a, b) = self.braced()?;
                    let hex: String = self.c[a..b].iter().collect();
                    char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?
                } else {
                    let h: String = self.c.get(self.i..self.i + 2)?.iter().collect();
                    self.i += 2;
                    char::from_u32(u32::from_str_radix(&h, 16).ok()?)?
                }
            }
            'u' => {
                let h: String = self.c.get(self.i..self.i + 4)?.iter().collect();
                if !h.chars().all(|c| c.is_ascii_hexdigit()) {
                    return None;
                }
                self.i += 4;
                // Lone or paired surrogates: not modelled.
                char::from_u32(u32::from_str_radix(&h, 16).ok()?)?
            }
            'c' => {
                let c = self.peek()?;
                self.i += 1;
                char::from_u32((c as u32) ^ 64)?
            }
            c if c.is_ascii_alphanumeric() => return None,
            c => c,
        })
    }

    /// Skips `{…}` at `self.i`; returns the content's bounds.
    fn braced(&mut self) -> Option<(usize, usize)> {
        if self.peek() != Some('{') {
            return None;
        }
        let from = self.i + 1;
        let close = from + self.c.get(from..)?.iter().position(|c| *c == '}')?;
        self.i = close + 1;
        Some((from, close))
    }

    /// Skips the argument of `\p` / `\P`: `{Name}` or one letter.
    fn property(&mut self) -> Option<()> {
        if self.peek() == Some('{') {
            self.braced().map(|_| ())
        } else if self.peek()?.is_ascii_alphabetic() {
            self.i += 1;
            Some(())
        } else {
            None
        }
    }

    /// Parses a character class at `self.i` (`[`). Returns its raw set (when known) and items.
    fn class(&mut self) -> Option<(Option<CharSet>, ClassInfo)> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        self.i += 1; // [
        let mut info = ClassInfo {
            negated: false,
            items: Vec::new(),
            simple: true,
        };
        if self.peek() == Some('^') {
            info.negated = true;
            self.i += 1;
        }
        // Operands separated by `&&`; each is a union of items.
        let mut operands: Vec<Option<CharSet>> = Vec::new();
        let mut cur: Option<CharSet> = Some(CharSet::empty());
        let mut cur_items = 0usize;
        let mut first = true;
        loop {
            let c = self.peek()?;
            let item_start = self.i;
            if c != ']' || first {
                cur_items += 1;
            }
            match c {
                ']' if !first => {
                    if !operands.is_empty() && cur_items == 0 {
                        return None;
                    }
                    self.i += 1;
                    break;
                }
                '[' => {
                    info.simple = false;
                    let (set, _) = self.class()?;
                    cur = union_opt(cur, set);
                }
                '&' if self.at(self.i + 1) == Some('&') => {
                    info.simple = false;
                    // An empty operand (`[&&a]`, `[a&&&&b]`) has a special meaning: not
                    // modelled.
                    if cur_items <= 1 {
                        return None;
                    }
                    self.i += 2;
                    operands.push(cur.take());
                    cur = Some(CharSet::empty());
                    cur_items = 0;
                    first = false;
                    continue;
                }
                _ => {
                    let item = self.class_atom()?;
                    let item = match item {
                        ClassAtom::Char(lo)
                            if self.peek() == Some('-')
                                && !matches!(self.at(self.i + 1), Some(']' | '[') | None) =>
                        {
                            // A range; the upper bound must be a single character.
                            self.i += 1;
                            if self.peek() == Some('&') && self.at(self.i + 1) == Some('&') {
                                return None;
                            }
                            match self.class_atom()? {
                                ClassAtom::Char(hi) if hi >= lo => ClassItem::Range(lo, hi),
                                _ => return None,
                            }
                        }
                        ClassAtom::Char(c) => ClassItem::Char(c),
                        ClassAtom::Set(s) => {
                            // `\d-x`: Java's reading of such a dash is not modelled.
                            if self.peek() == Some('-') && !matches!(self.at(self.i + 1), Some(']'))
                            {
                                return None;
                            }
                            ClassItem::Escape(s)
                        }
                        ClassAtom::Quoted(chars) => {
                            for ch in chars {
                                cur = union_opt(cur, Some(CharSet::single(ch)));
                                info.items.push((ClassItem::Char(ch), item_start, self.i));
                            }
                            first = false;
                            continue;
                        }
                    };
                    let set = match &item {
                        ClassItem::Char(c) => Some(CharSet::single(*c)),
                        ClassItem::Range(a, b) => Some(CharSet(vec![(*a as u32, *b as u32)])),
                        ClassItem::Escape(s) => s.clone(),
                    };
                    cur = union_opt(cur, set);
                    info.items.push((item, item_start, self.i));
                }
            }
            first = false;
        }
        operands.push(cur);
        let mut set: Option<CharSet> = Some(CharSet::full());
        for op in operands {
            set = match (set, op) {
                (Some(a), Some(b)) => Some(a.intersect(&b)),
                _ => None,
            };
        }
        if info.negated {
            // Negation combined with nesting or intersection changed meaning across Java
            // releases: not modelled.
            set = if info.simple {
                set.map(|s| s.complement())
            } else {
                None
            };
        }
        if !info.simple {
            info.items.clear();
        }
        self.depth -= 1;
        Some((set, info))
    }

    /// One character or predefined class inside a character class.
    fn class_atom(&mut self) -> Option<ClassAtom> {
        let c = self.peek()?;
        if c != '\\' {
            self.i += 1;
            return Some(ClassAtom::Char(c));
        }
        let e = self.at(self.i + 1)?;
        self.i += 2;
        Some(match e {
            'd' | 'D' | 's' | 'S' | 'w' | 'W' | 'h' | 'H' | 'v' | 'V' => {
                ClassAtom::Set(escape_set(e, self.flags))
            }
            'p' | 'P' => {
                self.property()?;
                ClassAtom::Set(None)
            }
            'N' => {
                self.braced()?;
                ClassAtom::Set(None)
            }
            'Q' => {
                let mut chars = Vec::new();
                while let Some(c) = self.peek() {
                    if c == '\\' && self.at(self.i + 1) == Some('E') {
                        self.i += 2;
                        break;
                    }
                    chars.push(c);
                    self.i += 1;
                }
                ClassAtom::Quoted(chars)
            }
            // Word boundaries, back-references, \R, \X … inside a class: not modelled.
            'b' | 'B' | 'A' | 'G' | 'Z' | 'z' | 'R' | 'X' | 'k' | 'E' | '1'..='9' => return None,
            _ => ClassAtom::Char(self.escaped_char(e)?),
        })
    }
}

enum ClassAtom {
    Char(char),
    Set(Option<CharSet>),
    Quoted(Vec<char>),
}

fn union_opt(a: Option<CharSet>, b: Option<CharSet>) -> Option<CharSet> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(&b)),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------
// Queries
// ------------------------------------------------------------------------------------------

/// Shortest length a node can match.
pub(crate) fn min_len(rx: &Rx) -> u32 {
    match &rx.k {
        K::Empty | K::Assert(..) | K::BackRef(_) => 0,
        K::Char(..) | K::Set(..) => 1,
        K::Opaque { min, .. } => *min,
        K::Group(kind, body) => {
            if kind.is_lookaround() {
                0
            } else {
                min_len(body)
            }
        }
        K::Alt(bs) => bs.iter().map(min_len).min().unwrap_or(0),
        K::Seq(items) => items
            .iter()
            .map(min_len)
            .fold(0u32, |a, b| a.saturating_add(b)),
        K::Repeat { body, min, .. } => min_len(body).saturating_mul(*min),
    }
}

/// Whether a node never consumes input (assertions and lookarounds only).
pub(crate) fn zero_width(rx: &Rx) -> bool {
    match &rx.k {
        K::Empty | K::Assert(..) => true,
        K::Char(..) | K::Set(..) | K::BackRef(_) | K::Opaque { .. } => false,
        K::Group(kind, body) => kind.is_lookaround() || zero_width(body),
        K::Alt(bs) => bs.iter().all(zero_width),
        K::Seq(items) => items.iter().all(zero_width),
        K::Repeat { body, max, .. } => *max == Some(0) || zero_width(body),
    }
}

/// A superset of the characters a node can consume first (when it consumes any); `None` when
/// unknown.
pub(crate) fn first_set(rx: &Rx) -> Option<CharSet> {
    match &rx.k {
        K::Empty | K::Assert(..) => Some(CharSet::empty()),
        K::Char(c, f) => effective(&CharSet::single(*c), *f),
        K::Set(s, f, _) => effective(s.as_ref()?, *f),
        K::BackRef(_) | K::Opaque { .. } => None,
        K::Group(kind, body) => {
            if kind.is_lookaround() {
                Some(CharSet::empty())
            } else {
                first_set(body)
            }
        }
        K::Alt(bs) => bs
            .iter()
            .try_fold(CharSet::empty(), |acc, b| Some(acc.union(&first_set(b)?))),
        K::Seq(items) => seq_first(items),
        K::Repeat { body, max, .. } => {
            if *max == Some(0) {
                Some(CharSet::empty())
            } else {
                first_set(body)
            }
        }
    }
}

fn seq_first(items: &[Rx]) -> Option<CharSet> {
    let mut acc = CharSet::empty();
    for it in items {
        acc = acc.union(&first_set(it)?);
        if min_len(it) >= 1 {
            break;
        }
    }
    Some(acc)
}

/// A superset of the characters a node can consume last (when it consumes any).
pub(crate) fn last_set(rx: &Rx) -> Option<CharSet> {
    match &rx.k {
        K::Seq(items) => seq_last(items),
        K::Alt(bs) => bs
            .iter()
            .try_fold(CharSet::empty(), |acc, b| Some(acc.union(&last_set(b)?))),
        K::Group(kind, body) if !kind.is_lookaround() => last_set(body),
        K::Repeat { body, max, .. } if *max != Some(0) => last_set(body),
        _ => first_set(rx),
    }
}

fn seq_last(items: &[Rx]) -> Option<CharSet> {
    let mut acc = CharSet::empty();
    for it in items.iter().rev() {
        acc = acc.union(&last_set(it)?);
        if min_len(it) >= 1 {
            break;
        }
    }
    Some(acc)
}

/// The exact set of a node that always matches exactly one character from it (a literal, a
/// class, or such a node in a plain group).
pub(crate) fn single_char_set(rx: &Rx) -> Option<CharSet> {
    match &rx.k {
        K::Char(c, f) => effective(&CharSet::single(*c), *f),
        K::Set(s, f, _) => effective(s.as_ref()?, *f),
        K::Group(GroupKind::Capture(_) | GroupKind::NonCapture | GroupKind::Atomic, body) => {
            single_char_set(body)
        }
        _ => None,
    }
}

/// Whether the node or anything inside it satisfies `pred`.
pub(crate) fn any_node(rx: &Rx, pred: &mut impl FnMut(&Rx) -> bool) -> bool {
    if pred(rx) {
        return true;
    }
    match &rx.k {
        K::Group(_, body) | K::Repeat { body, .. } => any_node(body, pred),
        K::Alt(v) | K::Seq(v) => v.iter().any(|r| any_node(r, pred)),
        _ => false,
    }
}

/// Visits every node, parents first.
pub(crate) fn each_node<'r>(rx: &'r Rx, f: &mut impl FnMut(&'r Rx)) {
    f(rx);
    match &rx.k {
        K::Group(_, body) | K::Repeat { body, .. } => each_node(body, f),
        K::Alt(v) | K::Seq(v) => v.iter().for_each(|r| each_node(r, f)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Regex {
        parse(s, 0).unwrap_or_else(|| panic!("should parse: {s}"))
    }

    #[test]
    fn parses_java_syntax() {
        for ok in [
            "",
            "abc",
            "a*b+c?",
            "a*?b+?c??",
            "a*+b++c?+",
            "(a|b)*",
            "(?:x)+",
            "(?i)abc",
            "(?i:abc)",
            "(?<name>\\d+)\\k<name>",
            "(?<=a)b(?!c)",
            "(?>ab)",
            "[a-z0-9_]+",
            "[]a]",
            "[^]a]",
            "\\p{Alpha}{2,3}",
            "\\pL",
            "a{2}",
            "a{2,}",
            "a{2,5}?",
            "\\{name\\}",
            "\\Qa(b\\E",
            "\\Qa(b",
            "\\x{41}\\x41\\u0041\\0101\\cA\\t\\e",
            "^$",
            "[-a]",
            "[a-]",
            "a|",
            "|a",
            "[a-z&&[^aeiou]]",
            "[a[b]]",
            "\\R\\X\\b{g}",
            "(?m)^a$",
            "(?-i)a",
            "[\\Q]\\E]",
        ] {
            assert!(parse(ok, 0).is_some(), "{ok}");
        }
    }

    #[test]
    fn rejects_unmodelled_or_invalid() {
        for bad in [
            "(?x)a b", "(a", "a)", "*a", "a**", "{2}", "a{2,1}", "\\q", "[a", "(?#c)", "[\\b]",
            "[a-\\d]", "[\\d-z]", "\\",
        ] {
            assert!(parse(bad, 0).is_none(), "{bad}");
        }
        assert!(parse("a", COMMENTS).is_none());
        assert!(parse("a", LITERAL).is_none());
    }

    #[test]
    fn numbers_groups_and_backrefs_like_java() {
        let r = p("(a)(?<n>b)(?:c)\\2");
        assert_eq!(r.groups, 2);
        // With one group, `\10` is a back-reference to 1 followed by `0`.
        let r = p("(a)\\10");
        let K::Seq(items) = &r.root.k else { panic!() };
        assert_eq!(items[1].k, K::BackRef(1));
        assert!(matches!(items[2].k, K::Char('0', _)));
    }

    #[test]
    fn inline_flags_are_scoped_to_their_group() {
        let r = p("(a(?i)b)c");
        let K::Seq(items) = &r.root.k else { panic!() };
        let K::Char('c', f) = items[1].k else {
            panic!()
        };
        assert_eq!(f & CASE_INSENSITIVE, 0);
        let r = p("(?i)a|b");
        let K::Alt(bs) = &r.root.k else { panic!() };
        let K::Char('b', f) = bs[1].k else { panic!() };
        assert_ne!(f & CASE_INSENSITIVE, 0);
    }

    #[test]
    fn computes_lengths_and_sets() {
        let r = p("ab?c*");
        assert_eq!(min_len(&r.root), 1);
        assert_eq!(first_set(&p("a?b").root), Some(CharSet::chars("ab")));
        assert_eq!(last_set(&p("ab?").root), Some(CharSet::chars("ab")));
        assert_eq!(
            first_set(&p("[a-c&&[^b]]").root),
            Some(CharSet::chars("ac"))
        );
        assert_eq!(first_set(&p("(?i)a").root), Some(CharSet::chars("aA")));
        assert!(zero_width(&p("^(?=a)\\b").root));
        assert!(CharSet::chars("ab").subset_of(&ascii_word()));
        assert!(CharSet::chars(" ,").subset_of(&ascii_non_word()));
    }
}
