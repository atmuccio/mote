//! A cheap linear pre-scan that bounds how deeply the parser and the semantic model will recurse
//! on a file.
//!
//! Oxc parses recursively (every bracket, prefix operator, right-associative operator and nested
//! statement is a few Rust frames) and the semantic builder visits the AST recursively (so a
//! left-deep chain such as `a.b().c()…` or `a + b + c + …` costs a frame per link even though it
//! is parsed in a loop). On a deeply nested input either one overflows the thread's stack, which
//! aborts the whole process. The plugin therefore estimates the stack an input will need from its
//! tokens, before parsing, and reports a file whose estimate exceeds half of the analysis
//! thread's stack ([`limit_for_stack`]) as one syntax error instead of analysing it.
//!
//! The estimate walks the source once, skipping comments, strings, regular expressions and
//! template text (approximately: this is not a full lexer), and keeps a stack of open brackets.
//! Every token is charged the stack its construct needs (measured per nesting level on 2 MiB
//! threads, in release and debug builds, with every rule enabled, and rounded up). Within one
//! bracket level, the charges of a statement or list element add up (a chain `a.b.c` or
//! `a ? b : c ? d : e` nests); `,`, `;` and statement-ending newlines start a new sum; nested
//! brackets add their own cost on top of the level that contains them. The charges are generous:
//! real code stays far below the limit on a 2 MiB stack (VS Code: at most ~19% of it).
//!
//! The scan runs at roughly 3–4× the speed of Oxc parsing plus semantic analysis, so it is only
//! run when it could matter: [`too_deep`] first checks two upper bounds of the estimate that cost
//! (next to) nothing, the file length times the largest charge per byte and a weighted count of
//! bytes ([`byte_bound`]). On the engine's large analysis stacks they clear all but the largest
//! files.

/// The estimate limit for analysis threads with `stack` bytes of stack: half of it. The charges
/// are upper bounds per nesting level; the other half leaves room for the caller's own frames and
/// keeps every construct rejected at no more than half the depth that would overflow.
pub(crate) fn limit_for_stack(stack: usize) -> u64 {
    stack as u64 / 2
}

/// An opening bracket: `(`, `[`, `{`, `${`, or a type-argument/comparison `<`. The worst
/// measured level (`{a:{a:…}}` type literals, `Array<Array<…>>`, `[...[...]]`) needs ~3.7 KB.
const BRACKET: u64 = 4_000;
/// One level of JSX element nesting (~0.9 KB measured).
const JSX: u64 = 1_200;
/// A statement that nests the next one without braces: `if`, `for`, `while`, `do`, `with`
/// (`for (;;) for (;;) …` ~2.5 KB per level).
const STATEMENT: u64 = 2_500;
/// Prefix and right-associative operators and other recursive links: unary operators, `new`,
/// `typeof`/`await`/…, assignments, `=>`, `?`, `:` (ternaries, labels), `**`, `function`,
/// `class`, `extends` (~0.7–1.4 KB per level).
const HEAVY: u64 = 1_500;
/// A link of a left-deep chain: `.`/`?.` member access, call, index, binary operator (~0.35–0.6
/// KB per link: parsed in a loop, visited recursively).
const LINK: u64 = 600;
/// A postfix `!` (non-null assertion), `++` or `--` (~0.1 KB in release, ~0.3 KB in debug).
const POSTFIX: u64 = LINK;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Top,
    Paren,
    Bracket,
    Brace,
    /// `${ … }` inside a template literal.
    Template,
    /// `<` after an operand: type arguments `Map<K, V>` or a comparison (closed by `>`, by the
    /// end of the statement, or by the enclosing bracket).
    Angle,
    /// A JSX opening tag up to its `>`.
    JsxTag,
    /// The children of a JSX element, up to its closing tag.
    JsxChildren,
}

impl Kind {
    fn base(self) -> u64 {
        match self {
            Kind::Top => 0,
            Kind::JsxTag | Kind::JsxChildren => JSX,
            _ => BRACKET,
        }
    }
}

#[derive(Debug)]
struct Frame {
    kind: Kind,
    /// Charges of the current statement / list element at this level.
    links: u64,
    /// Largest cost of a nested bracket closed in the current statement / list element.
    child: u64,
    /// Largest `links + child` of the finished statements / list elements at this level.
    best: u64,
    /// The `( … )` of an `if`/`for`/`while`/`with` header: a statement follows it, not an
    /// operand (so `if (x) /re/` starts a regular expression).
    header: bool,
    /// An [`Kind::Angle`] opened where no operand precedes it (TypeScript): a prefix type
    /// assertion `<T>x`, so an operand follows its `>`.
    assertion: bool,
    /// A [`Kind::Paren`] holding a plain `=` at its own level (see [`MAX_ASSIGN_PARENS`]).
    assign: bool,
    /// Statements nested without braces in the current statement at this level (`if (a) if
    /// (b) …`, an `else if` chain): levels of [`MAX_NESTING`].
    stmts: usize,
}

/// Most parentheses holding a plain assignment (`(b = …)`, `({a} = …)`, `(a, b = …)`) that may
/// nest. Each could be the parameters of an arrow function with a default value, which the
/// parser finds out by parsing what follows as parameters and, when no `=>` comes, parsing it
/// again as an expression: nested, that is quadratic (2 000 levels took 0.6 s and 20 000 levels
/// gigabytes). Real code nests a handful.
pub(crate) const MAX_ASSIGN_PARENS: u32 = 32;

/// Deepest nesting analysed whatever the stack: brackets (parentheses, brackets, braces,
/// template substitutions, JSX) plus statements nested without braces (`if (a) if (b) …`,
/// `else if` chains). Past it, time grows faster than the input rather than the stack: the
/// semantic builder looks names up scope by scope (20 000 nested `for … of` blocks took a
/// second, 20 000 nested `with` statements a third of one), and so do rules that look at
/// ancestors. Real code nests a few dozen levels.
pub(crate) const MAX_NESTING: usize = 4_000;

impl Frame {
    fn new(kind: Kind) -> Self {
        Frame {
            kind,
            links: 0,
            child: 0,
            best: 0,
            header: false,
            assertion: false,
            assign: false,
            stmts: 0,
        }
    }
    fn cost(&self) -> u64 {
        self.kind
            .base()
            .saturating_add(self.best.max(self.links.saturating_add(self.child)))
    }
}

struct Scan<'a> {
    b: &'a [u8],
    i: usize,
    jsx: bool,
    frames: Vec<Frame>,
    /// Kind of the innermost open frame (cached).
    kind: Kind,
    /// Sum of `base + links` over the open frames: a lower bound of the final estimate.
    path: u64,
    /// The previous token ends an operand (identifier, literal, closing bracket), so `/` is a
    /// division, `(` a call, `<` a comparison or type arguments, and `+`/`-`/`!` binary/postfix.
    operand: bool,
    /// A line break since the previous token.
    newline: bool,
    /// The previous token is `if`, `for`, `while` or `with` (its `(` is a header).
    header_kw: bool,
    /// Open parentheses holding a plain assignment (see [`MAX_ASSIGN_PARENS`]).
    assign_parens: u32,
    /// Sum of `stmts` over the open frames.
    stmt_levels: usize,
}

/// The estimated stack an analysis of `src` needs, in bytes (saturating; the scan stops early
/// once `limit` is exceeded and then returns a value above it). `jsx` enables JSX handling of
/// `<` (JavaScript and `.tsx` files).
pub(crate) fn estimate(src: &str, jsx: bool, limit: u64) -> u64 {
    let mut s = Scan {
        b: src.as_bytes(),
        i: 0,
        jsx,
        frames: vec![Frame::new(Kind::Top)],
        kind: Kind::Top,
        path: 0,
        operand: false,
        newline: false,
        header_kw: false,
        assign_parens: 0,
        stmt_levels: 0,
    };
    if s.run(limit) {
        return limit.saturating_add(1);
    }
    while s.frames.len() > 1 {
        s.close_top();
    }
    s.frames[0].cost()
}

/// Whether `src` is too deeply nested to analyse on a stack for which [`limit_for_stack`] gave
/// `limit`.
pub(crate) fn too_deep(src: &str, jsx: bool, limit: u64) -> bool {
    let b = src.as_bytes();
    // The estimate never exceeds the sum of the charges of all tokens (a frame's cost is at most
    // its own charges plus the costs of all frames nested in it). No byte carries more than
    // `MAX_PER_BYTE`, so short files need no look at all; for longer ones, `byte_bound` usually
    // settles it without tokenizing.
    if (b.len() as u64).saturating_mul(MAX_PER_BYTE) <= limit || byte_bound_within(b, limit) {
        // The stack is safe. Only nesting past [`MAX_NESTING`] or [`MAX_ASSIGN_PARENS`] can
        // still reject the file, and only if the raw bytes nest that deep at all.
        let (parens, brackets) = raw_nesting(b);
        if parens <= MAX_ASSIGN_PARENS && brackets <= MAX_NESTING {
            return false;
        }
    }
    estimate(src, jsx, limit) > limit
}

/// The deepest nesting of `(`, and of any of `(`, `[`, `{`, in `b`, counting every byte (strings
/// and comments included: upper bounds of what the scan sees; a `<` or JSX tag opens no level
/// here, but the scan only runs when one of these says so or the stack needs it).
fn raw_nesting(b: &[u8]) -> (u32, usize) {
    let (mut parens, mut max_parens) = (0u32, 0u32);
    let (mut all, mut max_all) = (0usize, 0usize);
    let mut step = |c: u8| match c {
        b'(' => {
            parens += 1;
            max_parens = max_parens.max(parens);
            all += 1;
            max_all = max_all.max(all);
        }
        b'[' | b'{' => {
            all += 1;
            max_all = max_all.max(all);
        }
        b')' => {
            parens = parens.saturating_sub(1);
            all = all.saturating_sub(1);
        }
        b']' | b'}' => all = all.saturating_sub(1),
        _ => {}
    };
    // Every file is scanned: read eight bytes at a time and visit only the brackets among them.
    let mut words = b.chunks_exact(8);
    for w in words.by_ref() {
        let mut word = [0u8; 8];
        word.copy_from_slice(w);
        let mut brackets = bracket_bytes(u64::from_le_bytes(word));
        while brackets != 0 {
            let at = (brackets.trailing_zeros() / 8) as usize;
            if let Some(&c) = w.get(at) {
                step(c);
            }
            brackets &= brackets - 1;
        }
    }
    words.remainder().iter().for_each(|&c| step(c));
    (max_parens, max_all)
}

/// A mask with the high bit set for each byte of the little-endian word `x` that is a bracket
/// (`(` `)` `[` `]` `{` `}`), and possibly for a byte right above a marked one: the scan
/// looks at every marked byte and skips those that are not brackets, so only a bracket left
/// unmarked would matter, and none is.
#[inline]
fn bracket_bytes(x: u64) -> u64 {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;
    // The high bit of every zero byte of `t` (and maybe of some bytes above one): a zero byte
    // borrows through the subtraction and keeps its high bit clear in `t`.
    let zero_bytes = |t: u64| t.wrapping_sub(ONES) & !t & HIGHS;
    // `(` 0x28 / `)` 0x29: equal to 0x28 once the low bit is cleared.
    let parens = (x ^ (ONES * 0x28)) & (ONES * 0xFE);
    // `[` 0x5B / `{` 0x7B and `]` 0x5D / `}` 0x7D: equal to 0x7B / 0x7D once bit 0x20 is set
    // (no other byte value maps there).
    let lowered = x | (ONES * 0x20);
    zero_bytes(parens) | zero_bytes(lowered ^ (ONES * 0x7B)) | zero_bytes(lowered ^ (ONES * 0x7D))
}

/// [`raw_nesting`] byte by byte (the reference its word-at-a-time scan must agree with).
#[cfg(test)]
fn raw_nesting_bytewise(b: &[u8]) -> (u32, usize) {
    let (mut parens, mut max_parens) = (0u32, 0u32);
    let (mut all, mut max_all) = (0usize, 0usize);
    for &c in b {
        match c {
            b'(' => {
                parens += 1;
                max_parens = max_parens.max(parens);
                all += 1;
                max_all = max_all.max(all);
            }
            b'[' | b'{' => {
                all += 1;
                max_all = max_all.max(all);
            }
            b')' => {
                parens = parens.saturating_sub(1);
                all = all.saturating_sub(1);
            }
            b']' | b'}' => all = all.saturating_sub(1),
            _ => {}
        }
    }
    (max_parens, max_all)
}

/// The most any single byte adds to the estimate: a `<` where no operand precedes it in
/// TypeScript (a prefix type assertion `<T>x` plus a bracket); `(` or `[` after an operand (a
/// call or index link plus a bracket) add less.
const MAX_PER_BYTE: u64 = HEAVY + BRACKET;

/// An upper bound of [`estimate`] from the bytes alone, without tokenizing (~2 GB/s): every byte
/// is weighted with the most a token starting there can add.
///
/// * `(` `[`: a call/index link plus a bracket; `<`: a prefix type assertion plus a bracket; `{`:
///   a bracket (`${` and JSX tags included).
/// * `.` and `` ` ``: a link; other operator bytes (`= ! ~ + - * / % & | ^ ? : >`): at most
///   [`HEAVY`].
/// * a lowercase letter that starts a word (the byte before it cannot continue an identifier):
///   the largest charge of a keyword with that initial ([`keyword_initial_weight`]).
/// * anything else (other letters, digits, whitespace, quotes, closing brackets): nothing.
// The reference definition (tests); `too_deep` decides with `byte_bound_within`.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn byte_bound(b: &[u8]) -> u64 {
    let Some(&first) = b.first() else { return 0 };
    // The first byte starts a word if it is a letter.
    let head = u64::from(BYTE_WEIGHT[0][usize::from(first)]);
    (head + weight_units(&b[..b.len() - 1], &b[1..])) * WEIGHT_UNIT
}

/// Whether [`byte_bound`]`(b) <= limit`, deciding early where it can: once the sum so far plus
/// the most the rest could add is within the limit, or once the sum so far exceeds it.
fn byte_bound_within(b: &[u8], limit: u64) -> bool {
    const CHUNK: usize = 16 << 10;
    let Some(&first) = b.first() else { return true };
    let mut sum = u64::from(BYTE_WEIGHT[0][usize::from(first)]) * WEIGHT_UNIT;
    let mut rest = b.len() as u64 - 1;
    for (prev, cur) in b.chunks(CHUNK).zip(b[1..].chunks(CHUNK)) {
        sum += weight_units(prev, cur) * WEIGHT_UNIT;
        rest -= cur.len() as u64;
        if sum > limit {
            return false;
        }
        if sum.saturating_add(rest.saturating_mul(MAX_PER_BYTE)) <= limit {
            return true;
        }
    }
    sum <= limit
}

/// Sum of [`BYTE_WEIGHT`] over the bytes of `cur`, each preceded by the byte at the same index
/// of `prev`, in [`WEIGHT_UNIT`]s.
fn weight_units(prev: &[u8], cur: &[u8]) -> u64 {
    // Per chunk, the sum fits in u32 (at most 55 per byte).
    prev.chunks(1 << 20)
        .zip(cur.chunks(1 << 20))
        .map(|(prev, cur)| {
            let sum: u32 = prev
                .iter()
                .zip(cur)
                .map(|(&p, &c)| {
                    u32::from(BYTE_WEIGHT[usize::from(is_ident_byte(p))][usize::from(c)])
                })
                .sum();
            u64::from(sum)
        })
        .sum()
}

/// The largest charge of a keyword starting with `c` (see `Scan::word`, which charges at most
/// once per word): `if`/`in`/`instanceof`/`infer`, `for`/`function`, `while`/`with`,
/// `do`/`delete`; `typeof`/`throw`, `void`, `new`, `class`, `extends`, `await`/`as`, `yield`,
/// `keyof`, `readonly`, `unique`; `satisfies`, `of`. Other initials start no charged keyword.
const fn keyword_initial_weight(c: u8) -> u64 {
    match c {
        b'i' | b'f' | b'w' | b'd' => STATEMENT,
        b't' | b'v' | b'n' | b'c' | b'e' | b'a' | b'y' | b'k' | b'r' | b'u' => HEAVY,
        b's' | b'o' => LINK,
        _ => 0,
    }
}

/// Unit of [`BYTE_WEIGHT`]: every charge is a multiple of it.
const WEIGHT_UNIT: u64 = 100;

/// `BYTE_WEIGHT[prev_is_ident][byte]`: the weight of `byte` in [`byte_bound`], in
/// [`WEIGHT_UNIT`]s.
static BYTE_WEIGHT: [[u8; 256]; 2] = {
    const fn units(charge: u64) -> u8 {
        assert!(charge % WEIGHT_UNIT == 0 && charge / WEIGHT_UNIT <= 255);
        (charge / WEIGHT_UNIT) as u8
    }
    let mut t = [[0u8; 256]; 2];
    let mut c = 0;
    while c < 256 {
        let byte = c as u8;
        let w = match byte {
            b'(' | b'[' => units(LINK + BRACKET),
            b'<' => units(MAX_PER_BYTE),
            b'{' => units(BRACKET),
            b'.' | b'`' => units(LINK),
            b'=' | b'!' | b'~' | b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'?'
            | b':' | b'>' => units(HEAVY),
            _ => 0,
        };
        t[1][c] = w;
        t[0][c] = if byte.is_ascii_lowercase() {
            units(keyword_initial_weight(byte))
        } else {
            w
        };
        c += 1;
    }
    t
};

/// Bytes that continue an identifier or a number (non-ASCII bytes are taken as identifier
/// characters).
static IDENT: [bool; 256] = {
    let mut t = [false; 256];
    let mut c = 0;
    while c < 256 {
        let b = c as u8;
        t[c] = b.is_ascii_alphanumeric()
            || b == b'_'
            || b == b'$'
            || b == b'\\'
            || b == b'#'
            || b >= 0x80;
        c += 1;
    }
    t
};

fn is_ident_byte(c: u8) -> bool {
    IDENT[c as usize]
}

/// Frames a stray `)` or `]` may not close.
fn is_block(k: Kind) -> bool {
    matches!(k, Kind::Brace | Kind::Template)
}

impl Scan<'_> {
    fn peek(&self, k: usize) -> u8 {
        self.b.get(self.i + k).copied().unwrap_or(0)
    }

    fn top(&mut self) -> &mut Frame {
        // The Top frame is never popped.
        let last = self.frames.len() - 1;
        &mut self.frames[last]
    }

    fn top_kind(&self) -> Kind {
        self.kind
    }

    fn set_kind(&mut self, kind: Kind) {
        self.top().kind = kind;
        self.kind = kind;
    }

    /// Exceeded: the lower bound `path + child` of the innermost level is over the limit.
    fn over(&self, limit: u64) -> bool {
        let child = self.frames.last().map_or(0, |f| f.child);
        self.path.saturating_add(child) > limit
    }

    fn charge(&mut self, w: u64) {
        let f = self.top();
        f.links = f.links.saturating_add(w);
        self.path = self.path.saturating_add(w);
    }

    fn push(&mut self, kind: Kind) {
        if self.frames.len() + self.stmt_levels > MAX_NESTING {
            // Too deep to analyse in reasonable time (see `MAX_NESTING`): reject the file like
            // one too deep for the stack.
            self.path = u64::MAX;
        }
        self.frames.push(Frame::new(kind));
        self.kind = kind;
        self.path = self.path.saturating_add(kind.base());
    }

    fn close_top(&mut self) {
        if self.frames.len() <= 1 {
            return;
        }
        let Some(f) = self.frames.pop() else { return };
        if f.assign {
            self.assign_parens = self.assign_parens.saturating_sub(1);
        }
        self.stmt_levels = self.stmt_levels.saturating_sub(f.stmts);
        self.path = self
            .path
            .saturating_sub(f.kind.base().saturating_add(f.links));
        let cost = f.cost();
        let parent = self.top();
        parent.child = parent.child.max(cost);
        self.kind = parent.kind;
    }

    /// Close frames down to (and including) the innermost one matching `pred`, without
    /// crossing a frame matching `stop`; nothing when there is none (a stray closing bracket,
    /// which then cannot close brackets outside the current block). Returns the closed frame's
    /// kind and header flag.
    fn close_until(
        &mut self,
        pred: impl Fn(Kind) -> bool,
        stop: impl Fn(Kind) -> bool,
    ) -> Option<(Kind, bool)> {
        let pos = self
            .frames
            .iter()
            .rposition(|f| pred(f.kind) || stop(f.kind))?;
        if pos == 0 || !pred(self.frames[pos].kind) {
            return None;
        }
        while self.frames.len() > pos + 1 {
            self.close_top();
        }
        let closed = (self.top_kind(), self.top().header);
        self.close_top();
        Some(closed)
    }

    /// Start a new statement / list element at the current level.
    fn reset(&mut self) {
        let f = self.top();
        f.best = f.best.max(f.links.saturating_add(f.child));
        let links = f.links;
        let stmts = f.stmts;
        f.links = 0;
        f.child = 0;
        f.stmts = 0;
        self.path = self.path.saturating_sub(links);
        self.stmt_levels = self.stmt_levels.saturating_sub(stmts);
    }

    /// A statement nested in the current one without braces (see [`MAX_NESTING`]).
    fn nested_statement(&mut self) {
        self.top().stmts += 1;
        self.stmt_levels += 1;
        if self.frames.len() + self.stmt_levels > MAX_NESTING {
            self.path = u64::MAX;
        }
    }

    /// Close `<…` frames that cannot span the current separator.
    fn close_open(&mut self, kinds: &[Kind]) {
        while self.frames.len() > 1 && kinds.contains(&self.top_kind()) {
            self.close_top();
        }
    }

    /// Before a token that can start a statement: a line break after a complete operand ends
    /// the previous statement (automatic semicolon insertion, approximately).
    fn statement_start(&mut self) {
        if self.newline && self.operand {
            // `x < y` comparisons do not span statements (type arguments never end a line with
            // an operand followed by a statement).
            self.close_open(&[Kind::Angle]);
            if matches!(self.top_kind(), Kind::Top | Kind::Brace) {
                self.reset();
            }
        }
    }

    /// Returns `true` when the limit was exceeded.
    fn run(&mut self, limit: u64) -> bool {
        let b = self.b;
        while self.i < b.len() {
            if self.kind == Kind::JsxChildren {
                // JSX text: quotes, slashes and punctuation are literal characters.
                match b[self.i..].iter().position(|&c| c == b'{' || c == b'<') {
                    Some(k) => self.i += k,
                    None => break,
                }
            }
            let c = b[self.i];
            if is_ident_byte(c) && !c.is_ascii_digit() {
                // Fast path for the most common token.
                let start = self.i;
                self.skip_ident();
                self.header_kw = self.word(start);
                self.newline = false;
                if self.over(limit) {
                    return true;
                }
                continue;
            }
            match c {
                b'\n' => {
                    self.newline = true;
                    self.i += 1;
                    continue;
                }
                b' ' | b'\t' | b'\r' | 0x0B | 0x0C => {
                    self.i += 1;
                    continue;
                }
                b'/' if self.peek(1) == b'/' => {
                    self.skip_line();
                    continue;
                }
                b'/' if self.peek(1) == b'*' => {
                    self.skip_block_comment();
                    continue;
                }
                _ => {}
            }
            self.token(c);
            self.header_kw = false;
            self.newline = false;
            if self.over(limit) {
                return true;
            }
        }
        false
    }

    fn token(&mut self, c: u8) {
        match c {
            b'\'' | b'"' => {
                self.statement_start();
                self.skip_string(c);
                self.operand = true;
            }
            b'`' => {
                self.statement_start();
                if self.operand {
                    self.charge(LINK); // tagged template
                }
                self.i += 1;
                self.template_text();
            }
            b'0'..=b'9' => {
                self.statement_start();
                self.skip_number();
                self.operand = true;
            }
            b'.' if self.peek(1).is_ascii_digit() && !self.operand => {
                self.skip_number();
                self.operand = true;
            }
            b'.' => {
                // `.`, `...`
                self.i += if self.peek(1) == b'.' && self.peek(2) == b'.' {
                    3
                } else {
                    1
                };
                self.charge(LINK);
                self.operand = false;
            }
            b'(' | b'[' => {
                if self.operand {
                    self.charge(LINK); // call or index
                }
                self.i += 1;
                if c == b'(' && self.assign_parens >= MAX_ASSIGN_PARENS {
                    // Too many nested parenthesized assignments: reject (see
                    // `MAX_ASSIGN_PARENS`) like a file nested too deep.
                    self.path = u64::MAX;
                }
                self.push(if c == b'(' {
                    Kind::Paren
                } else {
                    Kind::Bracket
                });
                if c == b'(' && self.header_kw {
                    self.top().header = true;
                }
                self.operand = false;
            }
            b'{' => {
                self.statement_start();
                self.i += 1;
                self.push(Kind::Brace);
                self.operand = false;
            }
            b')' => {
                self.i += 1;
                let closed = self.close_until(|k| k == Kind::Paren, is_block);
                // After an `if (…)` header comes a statement, not an operand.
                self.operand = !closed.is_some_and(|(_, header)| header);
            }
            b']' => {
                self.i += 1;
                self.close_until(|k| k == Kind::Bracket, is_block);
                self.operand = true;
            }
            b'}' => {
                self.i += 1;
                let closed =
                    self.close_until(|k| matches!(k, Kind::Brace | Kind::Template), |_| false);
                self.operand = true;
                if closed.is_some_and(|(k, _)| k == Kind::Template) {
                    self.template_text();
                }
            }
            b',' => {
                self.i += 1;
                // `<T,>(x) => …` is a type parameter list, not a JSX tag.
                self.close_open(&[Kind::JsxTag]);
                self.reset();
                self.operand = false;
            }
            b';' => {
                self.i += 1;
                self.close_open(&[Kind::Angle, Kind::JsxTag, Kind::JsxChildren]);
                self.reset();
                self.operand = false;
            }
            b'<' => self.less_than(),
            b'>' => self.greater_than(),
            b'/' => {
                if self.peek(1) == b'>' && self.top_kind() == Kind::JsxTag {
                    // Self-closing JSX element.
                    self.i += 2;
                    self.close_top();
                    self.operand = true;
                } else if self.operand {
                    self.operator(b'/');
                } else {
                    self.statement_start();
                    self.skip_regex();
                    self.operand = true;
                }
            }
            b'=' => {
                if self.peek(1) == b'>' {
                    self.i += 2;
                    // A `<T>(x) => …` generic arrow was taken for JSX.
                    self.close_open(&[Kind::JsxTag, Kind::JsxChildren]);
                    self.charge(HEAVY);
                } else if self.peek(1) == b'=' {
                    self.i += if self.peek(2) == b'=' { 3 } else { 2 };
                    self.charge(LINK);
                } else {
                    self.i += 1;
                    if self.top_kind() != Kind::JsxTag {
                        self.charge(HEAVY); // not a JSX attribute
                    }
                    if self.top_kind() == Kind::Paren && !self.top().assign {
                        self.top().assign = true;
                        self.assign_parens += 1;
                    }
                }
                self.operand = false;
            }
            b'!' => {
                if self.peek(1) == b'=' {
                    self.i += if self.peek(2) == b'=' { 3 } else { 2 };
                    self.charge(LINK);
                    self.operand = false;
                } else if self.operand {
                    self.i += 1;
                    self.charge(POSTFIX); // non-null assertion
                } else {
                    self.statement_start();
                    self.i += 1;
                    self.charge(HEAVY);
                }
            }
            b'~' => {
                self.statement_start();
                self.i += 1;
                self.charge(HEAVY);
                self.operand = false;
            }
            b'+' | b'-' => {
                if self.peek(1) == c {
                    self.i += 2;
                    if self.operand {
                        self.charge(POSTFIX);
                    } else {
                        self.statement_start();
                        self.charge(HEAVY);
                    }
                } else if self.peek(1) == b'=' {
                    self.i += 2;
                    self.charge(HEAVY);
                    self.operand = false;
                } else {
                    self.i += 1;
                    self.charge(if self.operand { LINK } else { HEAVY });
                    self.operand = false;
                }
            }
            b'?' => {
                if self.peek(1) == b'.' && !self.peek(2).is_ascii_digit() {
                    self.i += 2;
                    self.charge(LINK);
                } else if self.peek(1) == b'?' {
                    let assign = self.peek(2) == b'=';
                    self.i += if assign { 3 } else { 2 };
                    self.charge(if assign { HEAVY } else { LINK });
                } else if self.peek(1) == b':' {
                    self.i += 2; // `x?: T` optional member
                } else {
                    self.i += 1;
                    self.charge(HEAVY);
                }
                self.operand = false;
            }
            b':' => {
                self.i += 1;
                self.charge(HEAVY);
                self.operand = false;
            }
            b'*' | b'%' | b'&' | b'|' | b'^' => self.operator(c),
            b'@' => {
                self.statement_start();
                self.i += 1;
                self.operand = false;
            }
            _ => {
                // Anything else (stray bytes, ` `, …): skip.
                self.i += 1;
            }
        }
    }

    /// A binary operator starting with `c` (and its assignment form).
    fn operator(&mut self, c: u8) {
        self.i += 1;
        let mut w = LINK;
        if self.peek(0) == c && c != b'%' && c != b'^' {
            // `**`, `&&`, `||` (a `//` comment never gets here)
            self.i += 1;
            if c == b'*' {
                w = HEAVY; // right-associative
            }
        }
        if self.peek(0) == b'=' {
            self.i += 1;
            w = HEAVY; // assignment
        }
        self.charge(w);
        self.operand = false;
    }

    fn less_than(&mut self) {
        match self.peek(1) {
            b'/' if self.jsx => {
                // `</tag>`: closes the innermost JSX element.
                self.i += 2;
                while self.i < self.b.len() && self.b[self.i] != b'>' && self.b[self.i] != b'\n' {
                    self.i += 1;
                }
                if self.peek(0) == b'>' {
                    self.i += 1;
                }
                self.close_until(|k| k == Kind::JsxChildren, is_block);
                self.operand = true;
            }
            b'=' => {
                self.i += 2;
                self.charge(LINK);
                self.operand = false;
            }
            b'<' => {
                self.i += 2;
                let w = if self.peek(0) == b'=' {
                    self.i += 1;
                    HEAVY
                } else {
                    LINK
                };
                self.charge(w);
                self.operand = false;
            }
            _ => {
                self.i += 1;
                // An element starts where an operand cannot: after an operator, or among the
                // children of another element (`<a><b/><c>…`, where `operand` is set).
                if self.jsx && (!self.operand || self.kind == Kind::JsxChildren) {
                    self.statement_start();
                    self.push(Kind::JsxTag);
                } else {
                    // Where no operand precedes it: a prefix type assertion `<T>x` (or the type
                    // parameters of a generic arrow function). The expression after it nests,
                    // like after a unary operator (`<A><A>…x` recursed past any limit before).
                    let assertion = !self.operand;
                    if assertion {
                        self.charge(HEAVY);
                    }
                    self.push(Kind::Angle);
                    self.top().assertion = assertion;
                }
                self.operand = false;
            }
        }
    }

    fn greater_than(&mut self) {
        match self.top_kind() {
            Kind::Angle => {
                self.i += 1;
                let assertion = self.top().assertion;
                self.close_top();
                // After type arguments (`Map<K, V>`, `f<T>(…)`) the operand is complete; after a
                // type assertion it starts.
                self.operand = !assertion;
            }
            Kind::JsxTag => {
                self.i += 1;
                self.set_kind(Kind::JsxChildren);
                self.operand = false;
            }
            _ => {
                // `>`, `>=`, `>>`, `>>>`; `>>=` and `>>>=` assign.
                let mut n = 0;
                while self.peek(0) == b'>' {
                    self.i += 1;
                    n += 1;
                }
                let mut w = LINK;
                if self.peek(0) == b'=' {
                    self.i += 1;
                    if n > 1 {
                        w = HEAVY;
                    }
                }
                self.charge(w);
                self.operand = false;
            }
        }
    }

    /// An identifier or keyword spanning `start..self.i`. Returns whether it is `if`, `for`,
    /// `while` or `with` (whose parenthesis is a statement header).
    ///
    /// Reserved words set `operand = false` (a `/` after them starts a regular expression).
    /// Contextual keywords (`await`, `yield`, `let`, `of`, `as`, `keyof`, …) may also be plain
    /// identifiers, so they leave `operand = true`: a `/` after them is then read as a division,
    /// the reading that scans more of the line as code.
    ///
    /// Keep [`keyword_initial_weight`] in sync: `byte_bound` must cover every keyword charge made
    /// here by the keyword's initial (tested by `cheap_bounds_are_upper_bounds`).
    fn word(&mut self, start: usize) -> bool {
        let word = &self.b[start..self.i];
        // Keywords are 2–10 lowercase ASCII letters.
        if !(2..=10).contains(&word.len()) || !word[0].is_ascii_lowercase() {
            self.statement_start();
            self.operand = true;
            return false;
        }
        let (charge, operand) = match word {
            b"if" | b"for" | b"while" | b"with" => {
                self.statement_start();
                self.charge(STATEMENT);
                self.nested_statement();
                self.operand = false;
                return true;
            }
            b"extends" if self.kind == Kind::JsxTag => {
                // `<T extends U>(x) => …`: type parameters, not a JSX tag.
                self.set_kind(Kind::Angle);
                (HEAVY, false)
            }
            b"case" | b"default" => {
                // Switch cases are a flat list, not a chain.
                self.reset();
                (0, false)
            }
            b"do" => {
                self.nested_statement();
                (STATEMENT, false)
            }
            b"typeof" | b"void" | b"delete" | b"new" | b"throw" | b"function" | b"class"
            | b"extends" => (HEAVY, false),
            b"await" | b"yield" | b"keyof" | b"infer" | b"readonly" | b"unique" => (HEAVY, true),
            b"instanceof" | b"in" => (LINK, false),
            b"as" | b"satisfies" | b"of" => (LINK, true),
            b"else" | b"catch" | b"finally" | b"return" | b"export" | b"import" | b"const"
            | b"var" | b"break" | b"continue" | b"try" | b"switch" => (0, false),
            _ => (0, true),
        };
        if !matches!(
            word,
            b"else" | b"catch" | b"finally" | b"instanceof" | b"in" | b"as" | b"satisfies" | b"of"
        ) {
            self.statement_start();
        }
        if charge > 0 {
            self.charge(charge);
        }
        self.operand = operand;
        false
    }

    fn skip_ident(&mut self) {
        self.i = self.find_from(self.i, |c| !is_ident_byte(c));
    }

    /// Index of the first byte at or after `from` matching `pred` (or the end).
    fn find_from(&self, from: usize, pred: impl Fn(u8) -> bool) -> usize {
        let rest = self.b.get(from..).unwrap_or_default();
        from + rest.iter().position(|&c| pred(c)).unwrap_or(rest.len())
    }

    /// A numeric literal (`1`, `1.5`, `.5`, `0x1F`, `1_000n`; an exponent sign is left to the
    /// operator branch, which only over-counts).
    fn skip_number(&mut self) {
        self.i += 1;
        while self.i < self.b.len() && (is_ident_byte(self.b[self.i]) || self.b[self.i] == b'.') {
            self.i += 1;
        }
    }

    fn skip_line(&mut self) {
        self.i = self.find_from(self.i, |c| c == b'\n');
    }

    fn skip_block_comment(&mut self) {
        self.i += 2;
        loop {
            let at = self.find_from(self.i, |c| c == b'*' || c == b'\n');
            self.i = at + 1;
            match self.b.get(at) {
                None => return,
                Some(b'\n') => self.newline = true,
                Some(_) if self.peek(0) == b'/' => {
                    self.i += 1;
                    return;
                }
                Some(_) => {}
            }
        }
    }

    /// A `'…'` / `"…"` string (ends at the closing quote or, unterminated, at the line end).
    fn skip_string(&mut self, quote: u8) {
        self.i += 1;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'\n' => return,
                q if q == quote => {
                    self.i += 1;
                    return;
                }
                _ => self.i += 1,
            }
        }
    }

    /// A regular expression literal: up to the closing `/` outside a class, then the flags.
    fn skip_regex(&mut self) {
        self.i += 1;
        let mut class = false;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'\n' => return,
                b'[' => {
                    class = true;
                    self.i += 1;
                }
                b']' => {
                    class = false;
                    self.i += 1;
                }
                b'/' if !class => {
                    self.i += 1;
                    self.skip_ident();
                    return;
                }
                _ => self.i += 1,
            }
        }
    }

    /// Template literal text after `` ` `` or after the `}` closing a `${`: up to the closing
    /// backtick (an operand) or the next `${` (opens a [`Kind::Template`] frame).
    fn template_text(&mut self) {
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'`' => {
                    self.i += 1;
                    self.operand = true;
                    return;
                }
                b'$' if self.peek(1) == b'{' => {
                    self.i += 2;
                    self.push(Kind::Template);
                    self.operand = false;
                    return;
                }
                _ => self.i += 1,
            }
        }
        self.operand = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The word-at-a-time bracket scan agrees with the byte-by-byte one, on every byte value at
    /// every position of a word and on pseudo-random bracket soups of every length.
    #[test]
    fn raw_nesting_matches_the_bytewise_scan() {
        for len in 0..40 {
            for pos in 0..len {
                for v in 0..=255u8 {
                    let mut b = vec![b'('; len];
                    b[pos] = v;
                    assert_eq!(raw_nesting(&b), raw_nesting_bytewise(&b), "{len} {pos} {v}");
                    b.iter_mut().for_each(|c| *c = b'x');
                    b[pos] = v;
                    assert_eq!(raw_nesting(&b), raw_nesting_bytewise(&b), "{len} {pos} {v}");
                }
            }
        }
        let alphabet = b"()[]{}ab x\n\"'\x7f\xff";
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for len in 0..600 {
            let b: Vec<u8> = (0..len)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    alphabet[(seed % alphabet.len() as u64) as usize]
                })
                .collect();
            assert_eq!(raw_nesting(&b), raw_nesting_bytewise(&b), "{len}");
        }
    }

    fn est(src: &str) -> u64 {
        estimate(src, true, u64::MAX)
    }

    #[test]
    fn brackets_nest_and_lists_do_not_add_up() {
        assert_eq!(est(""), 0);
        assert_eq!(est("f"), 0);
        assert_eq!(est("((1))"), 2 * BRACKET);
        assert_eq!(est("[[1], [2], [[3]]]"), 3 * BRACKET);
        // Elements of a list are alternatives, not a chain.
        assert_eq!(est("f(a.b, c.d, e.f)"), LINK + BRACKET + LINK);
        assert_eq!(est("x = 1; y = 2;\nz = 3"), HEAVY);
        // A chain within one statement adds up.
        assert_eq!(est("a.b.c.d"), 3 * LINK);
        assert_eq!(est("a + b + c"), 2 * LINK);
        // Links after a nested bracket count on top of it (`a(…).b.c` visits through b, c).
        assert_eq!(est("a((1)).b.c"), 3 * LINK + 2 * BRACKET);
    }

    #[test]
    fn statements_end_at_semicolons_and_line_breaks() {
        let chain = "a.b.c.d.e\n".repeat(1_000);
        assert_eq!(est(&chain), 4 * LINK);
        let continued = "x = a\n  .b()\n  .c()\n  + d\n  + e\n";
        assert_eq!(est(continued), HEAVY + 6 * LINK + BRACKET);
        let blocks = "if (a) {\n  b();\n}\nif (c) {\n  d();\n}\n";
        assert_eq!(est(blocks), STATEMENT + BRACKET + LINK + BRACKET);
        // `else if` chains nest.
        let chain = format!("if (a) {{}}{}", " else if (a) {}".repeat(10));
        assert_eq!(est(&chain), 11 * STATEMENT + BRACKET);
    }

    #[test]
    fn strings_comments_regexes_and_templates_are_skipped() {
        assert_eq!(est("'((((' + \"[[[\" // ((((\n/* {{{{ */"), LINK);
        assert_eq!(est("x = /[(]\\//g.test(s)"), HEAVY + 2 * LINK + BRACKET);
        assert_eq!(est("a / b / c"), 2 * LINK);
        assert_eq!(est("`((( ${ a } ((( ${ `${b}` } )))`"), 2 * BRACKET);
        assert_eq!(est("'unterminated ((((\nx"), 0);
    }

    #[test]
    fn angle_brackets() {
        // Type arguments nest; comparisons end with the statement.
        assert_eq!(est("let x: A<B<C>>;"), HEAVY + 2 * BRACKET);
        // `(`, then `<` inside it; the block is a sibling of the parenthesis, not nested.
        assert_eq!(est("if (a < b) {}\nif (c < d) {}"), STATEMENT + 2 * BRACKET);
        let many = "x = a < b;\n".repeat(100);
        assert_eq!(est(&many), HEAVY + BRACKET);
    }

    #[test]
    fn jsx_elements_nest() {
        let flat = format!("x = <div>{}</div>;", "<p>Don't, stop.</p>".repeat(100));
        assert!(est(&flat) < 5 * JSX + 10 * HEAVY, "{}", est(&flat));
        let deep = format!("x = {}{};", "<a>".repeat(50), "</a>".repeat(50));
        assert!(est(&deep) >= 50 * JSX);
        // In TypeScript, `<T>x` is a type assertion, not an element: a prefix operator.
        let casts = "const a = <Foo>b;\n".repeat(100);
        assert_eq!(estimate(&casts, false, u64::MAX), 2 * HEAVY + BRACKET);
        // Chained assertions nest (regression: only the first was charged).
        let chain = format!("x = {}b;", "<A>".repeat(10));
        assert_eq!(estimate(&chain, false, u64::MAX), 11 * HEAVY + BRACKET);
        // Type arguments after an operand are not assertions.
        assert_eq!(
            estimate("x = f<A>(b) + g<B>(c);", false, u64::MAX),
            HEAVY + 3 * LINK + BRACKET
        );
    }

    #[test]
    fn early_exit_and_stray_closers() {
        let deep = "(".repeat(1_000_000);
        assert!(too_deep(&deep, true, limit_for_stack(2 << 20)));
        assert_eq!(est(")))]]]}}}"), 0);
        assert_eq!(est("(((]"), 3 * BRACKET);
        // A stray `)` (say, inside a regular expression taken for a division) cannot close
        // brackets outside its block.
        // f( function {  x = a / b  )(ignored) / ;  }  )
        assert_eq!(
            est("f(function () { x = a / b /)/; })"),
            LINK + BRACKET + HEAVY + BRACKET + HEAVY + 3 * LINK
        );
    }

    #[test]
    fn jsx_siblings_do_not_hide_nesting() {
        let n = 60;
        let deep = format!("x = {}{};", "<a><br/>".repeat(n), "</a>".repeat(n));
        assert!(est(&deep) >= n as u64 * JSX, "{}", est(&deep));
        let deep = format!("x = {}{};", "<a><b>t</b>".repeat(n), "</a>".repeat(n));
        assert!(est(&deep) >= n as u64 * JSX, "{}", est(&deep));
    }

    #[test]
    fn comparisons_end_at_statement_breaks() {
        let lines = "const a = x < y\n".repeat(1_000);
        assert!(est(&lines) < 10 * BRACKET, "{}", est(&lines));
    }

    #[test]
    fn statement_headers_are_followed_by_statements() {
        // `if (x) /re/…`: a regular expression, whose `(` is not a bracket.
        assert_eq!(
            est("if (x) /((((/.test(s);"),
            STATEMENT + BRACKET + LINK + LINK
        );
        // After other parentheses, `/` divides.
        assert_eq!(est("y = (a) / (b);"), HEAVY + LINK + BRACKET);
    }

    #[test]
    fn contextual_keywords_may_be_identifiers() {
        // `of` as a variable: `/` divides, so the parentheses are counted.
        assert_eq!(est("x = of / (((1))) / 2;"), HEAVY + 3 * LINK + 3 * BRACKET);
    }

    /// The cheap bounds that let `too_deep` skip the scan must never be below the estimate.
    #[test]
    fn cheap_bounds_are_upper_bounds() {
        const TOKENS: &[&str] = &[
            "(",
            ")",
            "[",
            "]",
            "{",
            "}",
            "<",
            ">",
            "${",
            "`",
            "'",
            "\"",
            "/",
            "//",
            "/*",
            "*/",
            "=",
            "=>",
            "==",
            "!",
            "~",
            "+",
            "++",
            "-",
            "*",
            "**=",
            "%",
            "&&",
            "|",
            "^",
            "?",
            "?.",
            "??=",
            ":",
            ".",
            "...",
            ",",
            ";",
            "\n",
            " ",
            "a",
            "B",
            "1",
            ".5",
            "if",
            "do",
            "for",
            "while",
            "with",
            "new",
            "typeof",
            "await",
            "extends",
            "in",
            "of",
            "as",
            "case",
            "else",
            "return",
            "void",
            "delete",
            "throw",
            "function",
            "class",
            "yield",
            "keyof",
            "infer",
            "readonly",
            "unique",
            "instanceof",
            "satisfies",
            "default",
            "try",
            "switch",
            "let",
            "</",
            "/>",
            "#x",
            "\\",
            "é",
        ];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for round in 0..20_000 {
            let mut src = String::new();
            for _ in 0..(round % 200) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                src.push_str(TOKENS[(seed % TOKENS.len() as u64) as usize]);
            }
            for jsx in [false, true] {
                let e = estimate(&src, jsx, u64::MAX);
                let b = byte_bound(src.as_bytes());
                assert!(e <= b, "estimate {e} > bound {b} for {src:?}");
                assert!(b <= src.len() as u64 * MAX_PER_BYTE, "{src:?}");
            }
        }
    }

    #[test]
    fn early_decisions_agree_with_the_full_bound() {
        let sources = [
            "x = a.b(c, d) + e;\nif (f) { g = [h, i]; }\n".repeat(5_000),
            "((((".repeat(20_000),
            "abc def ghi\n".repeat(20_000),
            format!("{}{}", "if (a) f(x.y);\n".repeat(3_000), "(".repeat(30_000)),
        ];
        for src in &sources {
            let b = src.as_bytes();
            let full = byte_bound(b);
            for limit in [0, 1, full / 3, full - 1, full, full + 1, full * 2, u64::MAX] {
                assert_eq!(byte_bound_within(b, limit), full <= limit, "limit {limit}");
            }
        }
        assert!(byte_bound_within(b"", 0));
    }

    #[test]
    fn limit_scales_with_the_stack() {
        let deep = format!("x = {}1{};", "(".repeat(300), ")".repeat(300));
        assert!(too_deep(&deep, false, limit_for_stack(2 << 20)));
        assert!(!too_deep(&deep, false, limit_for_stack(8 << 20)));
        // On a large stack, a typical file is cleared by the length check alone.
        assert!(!too_deep(
            &"x = f(a, b);
"
            .repeat(1_000),
            false,
            limit_for_stack(256 << 20)
        ));
        assert!(too_deep(
            &"(".repeat(1 << 20),
            false,
            limit_for_stack(256 << 20)
        ));
    }
}
