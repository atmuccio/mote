//! The questions the batch-9 regex rules ask of a parsed pattern. Each returns the message of
//! the first problem found, or `None`.

use super::regex_call::Usage;
use super::regex_syntax::{
    ascii_non_word, ascii_word, each_node, first_set, last_set, line_terminators, min_len,
    single_char_set, zero_width, Assert, CharSet, ClassItem, GroupKind, Mode, Regex, Rx, SetKind,
    CASE_INSENSITIVE, K, MULTILINE, UNICODE_CASE,
};

// ------------------------------------------------------------------------------------------
// Parts that can never match
// ------------------------------------------------------------------------------------------

/// What is certainly known around a position of the pattern.
#[derive(Clone)]
struct Around {
    /// A character was certainly consumed right before, and it is in this set.
    prev: Option<CharSet>,
    /// At least one character was certainly consumed before (since the match started).
    consumed: bool,
    /// A character is certainly consumed right after, and it is in this set.
    next: Option<CharSet>,
}

fn union_opt(a: Option<CharSet>, b: &Option<CharSet>) -> Option<CharSet> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        _ => None,
    }
}

/// Cheap text test: whether the pattern has anything [`never_matches`] or the backspace check
/// can report (anchors, boundaries, lookarounds, back-references, a backspace character).
pub(super) fn may_never_match(p: &str) -> bool {
    if p.contains(['^', '$', '\u{8}'])
        || p.contains("*+")
        || p.contains("++")
        || p.contains("}+")
        || p.contains("(?=")
        || p.contains("(?!")
        || p.contains("(?<")
    {
        return true;
    }
    let b = p.as_bytes();
    b.windows(2)
        .any(|w| w[0] == b'\\' && matches!(w[1], b'b' | b'B' | b'A' | b'z' | b'Z' | b'1'..=b'9'))
}

/// Why a part of the pattern can never match, if one cannot.
pub(super) fn never_matches(re: &Regex) -> Option<String> {
    if re.groups < 128 {
        let mut msg = None;
        backrefs(re, &re.root, 0, true, &mut msg);
        if msg.is_some() {
            return msg;
        }
    }
    let mut msg = None;
    let start = Around {
        prev: None,
        consumed: false,
        next: None,
    };
    positions(re, &re.root, &start, &mut msg);
    msg
}

/// Walks the tree tracking which groups may have matched; reports back-references to groups
/// that do not exist or cannot have matched yet. Returns the groups that may have matched
/// after `rx`.
fn backrefs(re: &Regex, rx: &Rx, before: u128, report: bool, msg: &mut Option<String>) -> u128 {
    match &rx.k {
        K::BackRef(n) => {
            if report && msg.is_none() {
                if *n > re.groups {
                    *msg = Some(format!(
                        "Fix this regular expression: the back-reference `\\{n}` refers to group {n}, which does not exist, so it can never match"
                    ));
                } else if before & (1u128 << n) == 0 {
                    *msg = Some(format!(
                        "Fix this regular expression: the back-reference `\\{n}` comes before group {n} can have matched, so it can never match"
                    ));
                }
            }
            before
        }
        K::Group(kind, body) => match kind {
            GroupKind::Capture(k) => backrefs(re, body, before, report, msg) | (1u128 << k),
            GroupKind::NegAhead => {
                backrefs(re, body, before, report, msg);
                before
            }
            // Look-behind bodies with back-references are not modelled.
            GroupKind::Behind => backrefs(re, body, before, false, msg),
            GroupKind::NegBehind => {
                backrefs(re, body, before, false, msg);
                before
            }
            _ => backrefs(re, body, before, report, msg),
        },
        K::Alt(bs) => bs
            .iter()
            .fold(before, |acc, b| acc | backrefs(re, b, before, report, msg)),
        K::Seq(items) => items
            .iter()
            .fold(before, |acc, it| backrefs(re, it, acc, report, msg)),
        K::Repeat { body, max, .. } => match max {
            Some(0) => before,
            Some(1) => before | backrefs(re, body, before, report, msg),
            // Later iterations see the groups of earlier ones.
            _ => backrefs(re, body, before | groups_in(body), report, msg),
        },
        _ => before,
    }
}

fn groups_in(rx: &Rx) -> u128 {
    let mut g = 0u128;
    each_node(rx, &mut |n| {
        if let K::Group(GroupKind::Capture(k), _) = n.k {
            if k < 128 {
                g |= 1u128 << k;
            }
        }
    });
    g
}

/// Walks the tree with what is known before and after each node; reports assertions and
/// lookarounds that can never hold there.
fn positions(re: &Regex, rx: &Rx, at: &Around, msg: &mut Option<String>) {
    if msg.is_some() {
        return;
    }
    match &rx.k {
        K::Assert(a, flags) => *msg = assertion(re, rx, *a, *flags, at),
        K::Group(kind, body) => match kind {
            GroupKind::Capture(_)
            | GroupKind::NonCapture
            | GroupKind::Flags
            | GroupKind::Atomic => {
                positions(re, body, at, msg);
            }
            GroupKind::Ahead => {
                if let (true, Some(a), Some(n)) = (min_len(body) >= 1, first_set(body), &at.next) {
                    if !a.is_empty() && !n.is_empty() && a.disjoint(n) {
                        *msg = Some(format!(
                            "Fix this regular expression: the lookahead `{}` requires a character that what follows it can never start with, so it can never match",
                            re.snippet(rx)
                        ));
                        return;
                    }
                }
                let inner = Around {
                    prev: at.prev.clone(),
                    consumed: at.consumed,
                    next: None,
                };
                positions(re, body, &inner, msg);
            }
            GroupKind::NegAhead => {
                if let (Some(s), Some(n)) = (single_char_set(body), &at.next) {
                    if !n.is_empty() && n.subset_of(&s) {
                        *msg = Some(format!(
                            "Fix this regular expression: the negative lookahead `{}` excludes every character that can follow it, so it can never match",
                            re.snippet(rx)
                        ));
                    }
                }
            }
            GroupKind::Behind => {
                if let (true, Some(a), Some(p)) = (min_len(body) >= 1, last_set(body), &at.prev) {
                    if !a.is_empty() && !p.is_empty() && a.disjoint(p) {
                        *msg = Some(format!(
                            "Fix this regular expression: the lookbehind `{}` requires a character that is never the one just matched before it, so it can never match",
                            re.snippet(rx)
                        ));
                    }
                }
            }
            GroupKind::NegBehind => {
                if let (Some(s), Some(p)) = (single_char_set(body), &at.prev) {
                    if !p.is_empty() && p.subset_of(&s) {
                        *msg = Some(format!(
                            "Fix this regular expression: the negative lookbehind `{}` excludes every character that can precede it, so it can never match",
                            re.snippet(rx)
                        ));
                    }
                }
            }
        },
        K::Alt(bs) => {
            for b in bs {
                positions(re, b, at, msg);
            }
        }
        K::Seq(items) => {
            // next_after[i]: what is certainly consumed after items[i].
            let mut next_after: Vec<Option<CharSet>> = vec![None; items.len()];
            let mut acc = at.next.clone();
            for i in (0..items.len()).rev() {
                next_after[i] = acc.clone();
                let it = &items[i];
                acc = if min_len(it) >= 1 {
                    first_set(it)
                } else {
                    union_opt(first_set(it), &acc)
                };
            }
            let mut prev = at.prev.clone();
            let mut consumed = at.consumed;
            for (i, it) in items.iter().enumerate() {
                let here = Around {
                    prev: prev.clone(),
                    consumed,
                    next: next_after[i].clone(),
                };
                positions(re, it, &here, msg);
                if msg.is_some() {
                    return;
                }
                if min_len(it) >= 1 {
                    prev = last_set(it);
                    consumed = true;
                } else if !zero_width(it) {
                    prev = union_opt(last_set(it), &prev);
                }
            }
        }
        K::Repeat {
            body,
            max: None,
            mode: Mode::Possessive,
            ..
        } if possessive_starves(body, at) => {
            *msg = Some(format!(
                "Fix this regular expression: the possessive `{}` takes every character it can and never gives one back, so what follows it, which must start with one of those characters, can never match",
                re.snippet(rx)
            ));
        }
        K::Repeat { body, max, .. } => match max {
            Some(0) => {}
            Some(1) => positions(re, body, at, msg),
            _ => {
                let inner = Around {
                    prev: union_opt(last_set(body), &at.prev),
                    consumed: at.consumed,
                    next: union_opt(first_set(body), &at.next),
                };
                positions(re, body, &inner, msg);
            }
        },
        _ => {}
    }
}

/// An unbounded possessive repetition of one character set (`\d++`, `[a-z]*+`) followed by a
/// character that is certainly in that set (`\d++\d`): the repetition has already consumed the
/// whole run of such characters and does not backtrack, so the next one is never there.
fn possessive_starves(body: &Rx, at: &Around) -> bool {
    let (Some(s), Some(next)) = (single_char_set(body), &at.next) else {
        return false;
    };
    !next.is_empty() && next.subset_of(&s)
}

fn assertion(re: &Regex, rx: &Rx, a: Assert, flags: u32, at: &Around) -> Option<String> {
    let lt = line_terminators();
    let prev = at.prev.as_ref().filter(|s| !s.is_empty());
    let next = at.next.as_ref().filter(|s| !s.is_empty());
    let what = re.snippet(rx);
    let reason = match a {
        Assert::Caret => {
            if flags & MULTILINE == 0 && at.consumed {
                "it comes after text that must already have been matched, but it only matches at the start of the input"
            } else if prev.is_some_and(|p| p.disjoint(&lt)) {
                "it follows a character that is never a line break"
            } else {
                return None;
            }
        }
        Assert::InputStart if at.consumed => {
            "it comes after text that must already have been matched, but it only matches at the start of the input"
        }
        Assert::Dollar | Assert::InputEndBeforeTerminator
            if next.is_some_and(|n| n.disjoint(&lt)) =>
        {
            "it is followed by a character that is never a line break, but it only matches at a line end"
        }
        Assert::InputEnd if next.is_some() => {
            "it is followed by text that must still be matched, but it only matches at the end of the input"
        }
        Assert::WordBoundary => {
            let (p, n) = (prev?, next?);
            let (w, nw) = (ascii_word(), ascii_non_word());
            if (p.subset_of(&w) && n.subset_of(&w)) || (p.subset_of(&nw) && n.subset_of(&nw)) {
                "the characters on both sides are of the same kind (both word or both non-word characters), so there is never a boundary there"
            } else {
                return None;
            }
        }
        Assert::NotWordBoundary => {
            let (p, n) = (prev?, next?);
            let (w, nw) = (ascii_word(), ascii_non_word());
            if (p.subset_of(&w) && n.subset_of(&nw)) || (p.subset_of(&nw) && n.subset_of(&w)) {
                "one side is always a word character and the other never is, so there is always a boundary there"
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(format!(
        "Fix this regular expression: `{what}` can never match because {reason}"
    ))
}

/// Whether the raw source text of a string literal writes `\b` (a backspace escape).
pub(super) fn has_backspace_escape(raw: &str) -> bool {
    let mut it = raw.chars();
    while let Some(c) = it.next() {
        if c == '\\' && it.next() == Some('b') {
            return true;
        }
    }
    false
}

/// Whether the pattern contains a backspace character outside character classes.
pub(super) fn has_backspace_char(re: &Regex) -> bool {
    let mut found = false;
    each_node(&re.root, &mut |n| {
        if matches!(n.k, K::Char('\u{8}', _)) {
            found = true;
        }
    });
    found
}

// ------------------------------------------------------------------------------------------
// Empty and shadowed alternatives
// ------------------------------------------------------------------------------------------

/// An explicit empty alternative that is a mistake: between two others (`a||b`) anywhere, or
/// making the whole pattern match the empty string where it is searched for.
pub(super) fn empty_alternative(
    re: &Regex,
    usage: Usage,
    split: bool,
    empty_replacement: bool,
) -> Option<String> {
    let mut middle: Option<&Rx> = None;
    each_node(&re.root, &mut |n| {
        if let K::Alt(bs) = &n.k {
            let len = bs.len();
            if middle.is_none()
                && bs
                    .iter()
                    .enumerate()
                    .any(|(i, b)| i > 0 && i + 1 < len && matches!(b.k, K::Empty))
            {
                middle = Some(n);
            }
        }
    });
    if let Some(alt) = middle {
        return Some(format!(
            "Remove the empty alternative in `{}`: `||` adds an alternative that matches the empty string",
            re.snippet(alt)
        ));
    }
    if usage != Usage::Find || (!split && empty_replacement) || min_len(&re.root) != 0 {
        return None;
    }
    let mut top = &re.root;
    while let K::Group(
        GroupKind::Capture(_) | GroupKind::NonCapture | GroupKind::Flags | GroupKind::Atomic,
        body,
    ) = &top.k
    {
        top = body;
    }
    let K::Alt(bs) = &top.k else {
        return None;
    };
    let edge_empty = [bs.first(), bs.last()]
        .into_iter()
        .flatten()
        .any(|b| matches!(b.k, K::Empty));
    if !edge_empty {
        return None;
    }
    let effect = if split {
        "so `split` cuts the input between every pair of characters"
    } else {
        "so the replacement is also inserted between every pair of characters"
    };
    Some(format!(
        "Remove the empty alternative in `{}`: it lets the pattern match the empty string at every position, {effect}",
        re.snippet(top)
    ))
}

/// The literal text of a node made only of literal characters, with their common flags.
fn literal_text(rx: &Rx) -> Option<(String, u32)> {
    match &rx.k {
        K::Char(c, f) => Some((c.to_string(), *f)),
        K::Seq(items) => {
            let mut s = String::new();
            let mut flags = None;
            for it in items {
                let K::Char(c, f) = it.k else {
                    return None;
                };
                if flags.is_some_and(|g| g != f) {
                    return None;
                }
                flags = Some(f);
                s.push(c);
            }
            Some((s, flags?))
        }
        _ => None,
    }
}

/// An alternative that can never be chosen: a duplicate of an earlier one, or — where the
/// pattern is searched for — a literal that starts with an earlier literal alternative at the
/// end of the pattern (`a|ab`: `a` always wins).
pub(super) fn shadowed_alternative(re: &Regex, usage: Usage) -> Option<String> {
    let mut msg = None;
    each_node(&re.root, &mut |n| {
        if msg.is_some() {
            return;
        }
        let K::Alt(bs) = &n.k else {
            return;
        };
        // Inline flags in one alternative carry over to the next ones: equal text may then
        // mean different things.
        let whole = re.text(n);
        if whole
            .match_indices("(?")
            .any(|(i, _)| whole[i + 2..].starts_with(['i', 'd', 'm', 's', 'u', 'U', '-']))
        {
            return;
        }
        let mut seen: Vec<String> = Vec::with_capacity(bs.len());
        for b in bs {
            if matches!(b.k, K::Empty) {
                continue;
            }
            let text = re.text(b);
            let special = super::regex_syntax::any_node(b, &mut |x| {
                matches!(
                    x.k,
                    K::Group(GroupKind::Capture(_) | GroupKind::Flags, _)
                        | K::BackRef(_)
                        | K::Opaque { .. }
                )
            }) || text.contains("(?");
            if !special && seen.contains(&text) {
                msg = Some(format!(
                    "Remove the duplicate alternative `{}`: an identical alternative comes before it, so it is never used",
                    re.snippet(b)
                ));
                return;
            }
            seen.push(text);
        }
    });
    if msg.is_some() || usage != Usage::Find {
        return msg;
    }
    let mut tails = Vec::new();
    tail_alts(&re.root, &mut tails);
    for alt in tails {
        let K::Alt(bs) = &alt.k else {
            continue;
        };
        let lits: Vec<Option<(String, u32)>> = bs.iter().map(literal_text).collect();
        for (j, bj) in bs.iter().enumerate() {
            let prefix = literal_prefix(bj);
            let len_j = match &bj.k {
                K::Seq(items) => items.len(),
                _ => 1,
            };
            for (li, fi) in lits[..j].iter().flatten() {
                let n = li.chars().count();
                if n == 0 || n >= len_j || prefix.len() < n {
                    continue;
                }
                let same = prefix[..n]
                    .iter()
                    .zip(li.chars())
                    .all(|(&(c, f), l)| c == l && f == *fi);
                if same {
                    return Some(format!(
                        "Reorder this alternation: `{}` is never used because the earlier alternative `{}` already matches its start; put the longer alternative first",
                        re.snippet(bj),
                        super::regex_syntax::shorten(li)
                    ));
                }
            }
        }
    }
    None
}

/// The literal characters (with their flags) a node always starts with.
fn literal_prefix(rx: &Rx) -> Vec<(char, u32)> {
    match &rx.k {
        K::Char(c, f) => vec![(*c, *f)],
        K::Seq(items) => items
            .iter()
            .map_while(|it| match it.k {
                K::Char(c, f) => Some((c, f)),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Alternations with nothing after them up to the end of the pattern.
fn tail_alts<'r>(rx: &'r Rx, out: &mut Vec<&'r Rx>) {
    match &rx.k {
        K::Alt(bs) => {
            out.push(rx);
            for b in bs {
                tail_alts(b, out);
            }
        }
        K::Seq(items) => {
            if let Some(last) = items.last() {
                tail_alts(last, out);
            }
        }
        K::Group(
            GroupKind::Capture(_) | GroupKind::NonCapture | GroupKind::Flags | GroupKind::Atomic,
            body,
        ) => tail_alts(body, out),
        _ => {}
    }
}

// ------------------------------------------------------------------------------------------
// Deep recursion
// ------------------------------------------------------------------------------------------

/// Whether a node always matches one character on its own (literal, class, escape).
fn is_single_char(rx: &Rx) -> bool {
    match &rx.k {
        K::Char(..) | K::Set(..) => true,
        K::Group(GroupKind::NonCapture | GroupKind::Capture(_), body) => is_single_char(body),
        _ => false,
    }
}

/// An unbounded, non-possessive repetition of a group that alternates between single
/// characters (`(a|b)*`, `(.|\n)+`).
pub(super) fn overflow_risk(re: &Regex) -> Option<String> {
    let mut msg = None;
    each_node(&re.root, &mut |n| {
        if msg.is_some() {
            return;
        }
        let K::Repeat {
            body,
            max: None,
            mode: Mode::Greedy | Mode::Lazy,
            ..
        } = &n.k
        else {
            return;
        };
        let K::Group(GroupKind::Capture(_) | GroupKind::NonCapture | GroupKind::Flags, inner) =
            &body.k
        else {
            return;
        };
        let K::Alt(bs) = &inner.k else {
            return;
        };
        if bs.len() >= 2 && bs.iter().all(is_single_char) {
            msg = Some(format!(
                "Replace `{}` with a character class: repeating an alternation makes Java recurse once per matched character, and long input overflows the stack",
                re.snippet(n)
            ));
        }
    });
    msg
}

// ------------------------------------------------------------------------------------------
// Case folding and character classes
// ------------------------------------------------------------------------------------------

/// The other case of a non-ASCII letter, when it is a single different character.
fn other_case(c: char) -> Option<char> {
    if c.is_ascii() {
        return None;
    }
    let mut up = c.to_uppercase();
    let mut lo = c.to_lowercase();
    let one_up = (up.len() == 1).then(|| up.next()).flatten();
    let one_lo = (lo.len() == 1).then(|| lo.next()).flatten();
    one_up
        .filter(|u| *u != c)
        .or_else(|| one_lo.filter(|l| *l != c))
}

/// A non-ASCII cased letter matched case-insensitively without `UNICODE_CASE` (and, in a
/// character class, whose other case the class does not list anyway).
pub(super) fn ascii_only_case_folding(re: &Regex) -> Option<String> {
    let mut found: Option<char> = None;
    each_node(&re.root, &mut |n| {
        if found.is_some() {
            return;
        }
        let ascii_ci = |f: u32| f & CASE_INSENSITIVE != 0 && f & UNICODE_CASE == 0;
        match &n.k {
            K::Char(c, f) if ascii_ci(*f) && other_case(*c).is_some() => found = Some(*c),
            K::Set(Some(set), f, SetKind::Class(info)) if ascii_ci(*f) && !info.negated => {
                let missing = |c: char| other_case(c).is_some_and(|o| !set.contains(o));
                for (item, _, _) in &info.items {
                    match item {
                        ClassItem::Char(c) if missing(*c) => {
                            found = Some(*c);
                            return;
                        }
                        ClassItem::Range(a, b) => {
                            let (a, b) = (*a as u32, *b as u32);
                            if b - a > 4096 {
                                continue;
                            }
                            if let Some(c) =
                                (a..=b).filter_map(char::from_u32).find(|c| missing(*c))
                            {
                                found = Some(c);
                                return;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    });
    let c = found?;
    Some(format!(
        "Add `UNICODE_CASE` (inline: `u`) to this case-insensitive pattern: without it only ASCII letters ignore case, so `{c}` never matches its other case"
    ))
}

/// Combining marks and joiners that attach to the preceding character.
fn is_combining(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF
        | 0xFE20..=0xFE2F
        | 0xFE00..=0xFE0F
        | 0x200D
        | 0x1F3FB..=0x1F3FF)
}

/// A character class that does not mean what it seems: a range spanning ASCII punctuation
/// between letters or digits (`A-z`), a character written with a combining mark, or `|` used as
/// if it separated alternatives (`[jpg|png]`).
pub(super) fn class_pitfall(re: &Regex) -> Option<String> {
    let mut msg = None;
    each_node(&re.root, &mut |n| {
        if msg.is_some() {
            return;
        }
        let K::Set(_, _, SetKind::Class(info)) = &n.k else {
            return;
        };
        if !info.simple {
            return;
        }
        let items = &info.items;
        for (i, (item, s, e)) in items.iter().enumerate() {
            match item {
                ClassItem::Range(a, b)
                    if (a.is_ascii_uppercase() && b.is_ascii_lowercase())
                        || (a.is_ascii_digit() && b.is_ascii_alphabetic()) =>
                {
                    msg = Some(format!(
                        "Fix the range `{}` in this character class: it also matches the ASCII punctuation between `{a}` and `{b}`; list the letter and digit ranges separately",
                        re.snippet_range(*s, *e)
                    ));
                    return;
                }
                ClassItem::Char(c) if is_combining(*c) && i > 0 => {
                    if let (ClassItem::Char(base), ..) = &items[i - 1] {
                        if !is_combining(*base) {
                            msg = Some(format!(
                                "Rewrite this character class: `{base}` followed by the combining mark U+{:04X} is one visible character but two class members, each matched on its own",
                                *c as u32
                            ));
                            return;
                        }
                    }
                }
                _ => {}
            }
        }
        let chars: Vec<char> = items
            .iter()
            .filter_map(|(it, ..)| match it {
                ClassItem::Char(c) => Some(*c),
                _ => None,
            })
            .collect();
        // A `|` among other characters, with some character repeated; a class of only pipes
        // is just redundant.
        if chars.contains(&'|') && chars.iter().any(|c| *c != '|') {
            let dup = chars
                .iter()
                .enumerate()
                .any(|(i, c)| chars[..i].contains(c));
            if dup {
                // `[a|b|c]`: every alternative is one member, so the class already chooses;
                // only the pipes are wrong.
                let mut segments = items.split(|(it, ..)| *it == ClassItem::Char('|'));
                let single = segments.clone().count() >= 2 && segments.all(|s| s.len() == 1);
                msg = Some(if single {
                    let members: String = items
                        .iter()
                        .filter(|(it, ..)| *it != ClassItem::Char('|'))
                        .map(|(_, s, e)| re.snippet_range(*s, *e))
                        .collect();
                    let neg = if info.negated { "^" } else { "" };
                    format!(
                        "Drop the `|` from `{}`: a character class already matches any one of its members, and inside it `|` is a literal; write `[{neg}{members}]`",
                        re.snippet(n)
                    )
                } else {
                    format!(
                        "Rewrite `{}` as a group: inside a character class `|` is a literal and every character is matched on its own, so this does not choose between alternatives",
                        re.snippet(n)
                    )
                });
            }
        }
    });
    msg
}

#[cfg(test)]
mod tests {
    use super::super::regex_syntax::{parse, CASE_INSENSITIVE, DOTALL, MULTILINE};
    use super::*;

    fn never(p: &str, flags: u32) -> bool {
        never_matches(&parse(p, flags).unwrap_or_else(|| panic!("parse {p}"))).is_some()
    }

    #[test]
    fn finds_parts_that_never_match() {
        for p in [
            r"a^b",
            r"\d+^",
            r".^",
            r"x$y",
            r"$\.",
            r"a$.",
            r"a\z.",
            r"a\Ab",
            r"\w\b\w",
            r"foo\bbar",
            r", \b,",
            r"a\B ",
            r"(?=\d)[a-z]",
            r"(?!a)a",
            r"(?![a-z])b+",
            r"a(?<=b)",
            r"a(?<!a)",
            r"(a)\2",
            r"\1(a)",
            r"(a)|\1",
            r"(a\1)",
            r"(?:a|b)^c",
            r"(?:x^)*",
            r"(?i)foo\bbar",
            r"(?=a^)",
            r"\n^a",
            r"\d++\d",
            r"[a-z]*+s",
            r".*+x",
            r"\w++(?:foo|bar)",
            r"(?:\d++)\d",
            r"(?i)[a-z]++A",
            r"a{2,}+a",
        ] {
            assert!(never(p, 0), "should never match: {p}");
        }
        assert!(!never(".*+\n", 0), "dot does not take line breaks");
        assert!(never(".*+\n", DOTALL), "dot takes line breaks in DOTALL");
        assert!(never("a^b", MULTILINE), "a^b multiline");
        assert!(never("x$y", MULTILINE), "x$y multiline");
    }

    #[test]
    fn accepts_parts_that_can_match() {
        for p in [
            r"^abc$",
            r"\d++x",
            r"\d*+\.\d",
            r"\d{1,3}+\d",
            r"[a-z]++[A-Z]",
            r"\d++\d?x",
            r"\s*+#",
            r"(?i)[a-z]++\d",
            r"\d++(?=\d)?",
            r"a*+b*+",
            r"^$",
            r"a*^b",
            r"(?:a|^)b",
            r"x|^y",
            r"\bfoo\b",
            r"a\b b",
            r"foo\Bbar",
            r"$\n",
            r"$\s*",
            r"$\r?\n",
            r"abc$|^def",
            r"(?=\d)\w",
            r"(?!a)b",
            r"(?!a)\w",
            r"a(?<=a)",
            r"a(?<!b)",
            r"(a)\1",
            r"(a|b)\1",
            r"(?:(a)|b\1)+",
            r"(a\1?)+",
            r"\w+\b",
            r"(?!^)",
            r"(?<=\G..)",
            r"(?=[A-Z])",
            r"\p{L}\b\p{L}",
            r"(?U)\w\b\w",
            r"a\$b",
            r"[$^]x",
            r"a\Q^\Eb",
            r"(?m)\n^a",
        ] {
            assert!(!never(p, 0), "can match: {p}");
        }
        assert!(!never(r"\n^a", MULTILINE), "newline caret multiline");
        assert!(!never("a$.", DOTALL), "a$. dotall");
        assert!(!never("(?=a)A", CASE_INSENSITIVE), "ci lookahead");
    }

    #[test]
    fn empty_alternatives() {
        let e = |p: &str, u: Usage, split: bool, empty: bool| {
            empty_alternative(&parse(p, 0).unwrap(), u, split, empty).is_some()
        };
        assert!(e("a||b", Usage::Full, false, false));
        assert!(e("(a||b)c", Usage::Unknown, false, false));
        assert!(e(",|", Usage::Find, true, false));
        assert!(e("|,", Usage::Find, true, false));
        assert!(e("(,|;|)", Usage::Find, true, false));
        assert!(e("x|", Usage::Find, false, false));
        assert!(!e("x|", Usage::Find, false, true));
        assert!(!e("x|", Usage::Full, false, false));
        assert!(!e("x|", Usage::Unknown, false, false));
        assert!(!e("a(b|)", Usage::Find, true, false));
        assert!(!e("a|b", Usage::Find, true, false));
    }

    #[test]
    fn shadowed_alternatives() {
        let s = |p: &str, u: Usage| shadowed_alternative(&parse(p, 0).unwrap(), u).is_some();
        assert!(s("jpg|png|jpg", Usage::Full));
        assert!(s("(?:a|b|a)c", Usage::Unknown));
        assert!(s(",|, ", Usage::Find));
        assert!(s("x(a|ab)", Usage::Find));
        assert!(s(r"(?:Mr|Mrs)\.?\s(a|an)", Usage::Find));
        assert!(!s(",|, ", Usage::Full));
        assert!(!s(",|, ", Usage::Unknown));
        assert!(!s("(a|ab)c", Usage::Find));
        assert!(!s("(a|ab)$", Usage::Find));
        assert!(!s("ab|a", Usage::Find));
        assert!(!s("(a)|(a)", Usage::Full));
        assert!(!s("a|x(?i)|a", Usage::Full));
        assert!(!s("a|(?i)ab", Usage::Find));
    }

    #[test]
    fn overflow_risks() {
        let o = |p: &str| overflow_risk(&parse(p, 0).unwrap()).is_some();
        assert!(o("(a|b)*"));
        assert!(o(r"(?:\s|,)+"));
        assert!(o(r"x(.|\n)*?y"));
        assert!(o("([a-z]|_){2,}"));
        assert!(!o("(a|b)*+"));
        assert!(!o("(a|b){1,5}"));
        assert!(!o("(ab|cd)*"));
        assert!(!o("[ab]*"));
        assert!(!o(r"(\r\n|\n)*"));
        assert!(!o("a|b*"));
    }

    #[test]
    fn case_folding_and_classes() {
        let c = |p: &str, f: u32| ascii_only_case_folding(&parse(p, f).unwrap()).is_some();
        assert!(c("café", CASE_INSENSITIVE));
        assert!(c("(?i)[а-я]+", 0));
        assert!(!c("café", 0));
        assert!(!c("(?iu)café", 0));
        assert!(!c("(?i)[äÄ]", 0));
        assert!(!c("(?i)[À-þ]", 0));
        assert!(!c("(?i)straße", 0));
        assert!(!c("(?i)[^é]", 0));
        let k = |p: &str| class_pitfall(&parse(p, 0).unwrap()).is_some();
        assert!(k("[A-z]+"));
        assert!(k("[0-9A-f]"));
        assert!(k(r"\.[jpg|png]"));
        assert!(k("[a|b|c]"));
        assert!(k("[e\u{301}]"));
        assert!(!k("[A-Za-z]"));
        assert!(!k("[!-~]"));
        assert!(!k("[|,;]"));
        assert!(!k("[||]"));
        assert!(!k("[a-z&&[^e]]"));
        assert!(!k("[\u{301}]"));
        let m = |p: &str| class_pitfall(&parse(p, 0).unwrap()).unwrap_or_default();
        assert!(m("[a|b|c]").contains("write `[abc]`"), "{}", m("[a|b|c]"));
        assert!(
            m("[^;|,|:]").contains("write `[^;,:]`"),
            "{}",
            m("[^;|,|:]")
        );
        assert!(
            m("[a-f|0-9|x]").contains("write `[a-f0-9x]`"),
            "{}",
            m("[a-f|0-9|x]")
        );
        assert!(m(r"\.[jpg|png]").contains("as a group"));
        assert!(m("[ab|c|d]").contains("as a group"));
    }
}
