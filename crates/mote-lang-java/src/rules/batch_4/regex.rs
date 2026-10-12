//! A small, deliberately conservative checker for `java.util.regex` pattern syntax.
//!
//! [`definite_error`] only reports mistakes that make `Pattern.compile` throw for certain;
//! whenever the pattern uses a construct this checker does not model (quoting with `\Q`,
//! nested or intersected character classes, comments mode, unknown `(?…)` groups), it answers
//! `None` ("not sure") rather than guessing.

/// Characters that have a meaning in a Java regular expression outside character classes.
pub(super) const META_CHARS: &[char] = &[
    '\\', '^', '$', '.', '|', '?', '*', '+', '(', ')', '[', ']', '{', '}',
];

/// Whether something before this point can take a quantifier.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quant {
    /// An atom precedes: a quantifier is fine.
    Yes,
    /// Start of the pattern, of a group, or of an alternative: a quantifier is dangling.
    No,
    /// Not modelled (after anchors, quantifiers, flags): stay silent.
    Unknown,
}

/// Position of the `}` (or `>`) closing a braced escape argument starting at `open`.
fn closing(chars: &[char], open: usize, close: char) -> Option<usize> {
    chars
        .get(open + 1..)?
        .iter()
        .position(|c| *c == close)
        .map(|p| open + 1 + p)
}

/// The reason `re` cannot compile, when that is certain; `None` when it compiles or when the
/// pattern is outside what this checker understands.
pub(super) fn definite_error(re: &str) -> Option<&'static str> {
    let chars: Vec<char> = re.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut depth = 0u32;
    let mut quant = Quant::No;
    while i < n {
        let c = chars[i];
        match c {
            '\\' => {
                let Some(&e) = chars.get(i + 1) else {
                    return Some("it ends with a lone backslash");
                };
                if e == 'Q' {
                    return None;
                }
                i += 2;
                // Braced arguments: \p{Alpha}, \x{263A}, \N{NAME}, \b{g}, \k<name>.
                if matches!(e, 'p' | 'P' | 'x' | 'N' | 'b') && chars.get(i) == Some(&'{') {
                    i = closing(&chars, i, '}')? + 1;
                } else if e == 'k' && chars.get(i) == Some(&'<') {
                    i = closing(&chars, i, '>')? + 1;
                }
                quant = if matches!(e, 'b' | 'B' | 'A' | 'G' | 'Z' | 'z') {
                    Quant::Unknown
                } else {
                    Quant::Yes
                };
                continue;
            }
            '[' => {
                match class_end(&chars, i + 1)? {
                    Ok(after) => i = after,
                    Err(reason) => return Some(reason),
                }
                quant = Quant::Yes;
                continue;
            }
            '(' => {
                if chars.get(i + 1) == Some(&'?') {
                    match group_construct(&chars, i + 2)? {
                        Construct::Group(after) => {
                            depth += 1;
                            i = after;
                            quant = Quant::No;
                        }
                        Construct::Flags(after) => {
                            i = after;
                            quant = Quant::Unknown;
                        }
                    }
                    continue;
                }
                depth += 1;
                quant = Quant::No;
            }
            ')' => {
                if depth == 0 {
                    return Some("it has an unmatched closing parenthesis");
                }
                depth -= 1;
                quant = Quant::Yes;
            }
            '|' => quant = Quant::No,
            '*' | '+' | '?' => {
                if quant == Quant::No {
                    return Some("a quantifier (`*`, `+`, `?`) has nothing to repeat");
                }
                // Lazy / possessive suffix.
                if matches!(chars.get(i + 1), Some('?' | '+')) {
                    i += 1;
                }
                quant = Quant::Unknown;
            }
            '{' => {
                if !chars.get(i + 1).is_some_and(char::is_ascii_digit) {
                    return Some(
                        "a `{` does not start a repetition like `{2}` (escape it as `\\\\{`)",
                    );
                }
                // A counted repetition: skip to its end; malformed ones are left alone.
                i = closing(&chars, i, '}')? + 1;
                quant = Quant::Unknown;
                continue;
            }
            '^' | '$' => quant = Quant::Unknown,
            _ => quant = Quant::Yes,
        }
        i += 1;
    }
    if depth > 0 {
        return Some("a group is not closed");
    }
    None
}

enum Construct {
    /// A group opener; index after the construct's prefix.
    Group(usize),
    /// Inline flags `(?i)`; index after the `)`.
    Flags(usize),
}

/// Parses what follows `(?` at `at`; `None` when unknown or in comments mode.
fn group_construct(chars: &[char], at: usize) -> Option<Construct> {
    match chars.get(at)? {
        ':' | '=' | '!' | '>' => Some(Construct::Group(at + 1)),
        '<' => match chars.get(at + 1)? {
            '=' | '!' => Some(Construct::Group(at + 2)),
            c if c.is_ascii_alphabetic() => {
                let end = closing(chars, at, '>')?;
                let name_ok = chars[at + 1..end].iter().all(char::is_ascii_alphanumeric);
                name_ok.then_some(Construct::Group(end + 1))
            }
            _ => None,
        },
        _ => {
            // Inline flags: (?idmsuxU-idmsuxU) or (?flags:X).
            let mut j = at;
            while let Some(c) = chars.get(j) {
                match c {
                    'i' | 'd' | 'm' | 's' | 'u' | 'U' | '-' => j += 1,
                    // Comments mode changes the syntax: not modelled.
                    'x' => return None,
                    ')' => return (j > at).then_some(Construct::Flags(j + 1)),
                    ':' => return (j > at).then_some(Construct::Group(j + 1)),
                    _ => return None,
                }
            }
            None
        }
    }
}

/// Scans a character class whose content starts at `start` (just after `[`). Returns
/// `Some(Ok(index after ']'))`, `Some(Err(reason))` for a definite error, or `None` when the
/// class uses constructs this checker does not model.
fn class_end(chars: &[char], start: usize) -> Option<Result<usize, &'static str>> {
    let mut j = start;
    if chars.get(j) == Some(&'^') {
        j += 1;
    }
    // A `]` right after `[` or `[^` is a literal.
    if chars.get(j) == Some(&']') {
        j += 1;
    }
    loop {
        let Some(&c) = chars.get(j) else {
            return Some(Err("a character class `[` is not closed"));
        };
        match c {
            ']' => return Some(Ok(j + 1)),
            '[' => return None,
            '&' if chars.get(j + 1) == Some(&'&') => return None,
            '\\' => {
                let Some(&e) = chars.get(j + 1) else {
                    return Some(Err("it ends with a lone backslash"));
                };
                if e == 'Q' {
                    return None;
                }
                j += 2;
                if matches!(e, 'p' | 'P' | 'x' | 'N') && chars.get(j) == Some(&'{') {
                    j = closing(chars, j, '}')? + 1;
                }
            }
            _ => {
                // A simple literal range `a-z`.
                if chars.get(j + 1) == Some(&'-') {
                    if let Some(&hi) = chars.get(j + 2) {
                        if !matches!(hi, ']' | '[' | '\\' | '&') {
                            if hi < c {
                                return Some(Err("a character range is in the wrong order"));
                            }
                            j += 3;
                            continue;
                        }
                    }
                }
                j += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::definite_error as e;

    #[test]
    fn accepts_valid_patterns() {
        for ok in [
            "",
            "abc",
            "a*b+c?",
            "(a|b)*",
            "(?:x)+",
            "(?i)abc",
            "(?i:abc)",
            "(?<name>\\d+)\\k<name>",
            "(?<=a)b(?!c)",
            "[a-z0-9_]+",
            "[]a]",
            "[^]a]",
            "\\p{Alpha}{2,3}",
            "a{2}",
            "a{2,}",
            "\\{name\\}",
            "\\s*,\\s*",
            "\\.",
            "[.$|]",
            "x*?y++z?+",
            "\\Qa(b\\E",
            "\\x{41}",
            "^$",
            "[-a]",
            "[a-]",
            "a|",
            "|a",
        ] {
            assert_eq!(e(ok), None, "{ok}");
        }
    }

    #[test]
    fn rejects_definite_errors() {
        for bad in [
            "(abc", "abc)", "a)(", "[abc", "*abc", "+", "?x", "(*a)", "a|*b", "\\", "abc\\",
            "{name}", "${user}", "a{b}", "a{}", "[z-a]", "[abc\\",
        ] {
            assert!(e(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn stays_silent_when_unsure() {
        for unsure in ["[a[b]]", "[a-z&&[^e]]", "(?x) ( a", "(?#c)", "{2}"] {
            assert_eq!(e(unsure), None, "{unsure}");
        }
    }
}
