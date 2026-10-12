//! `java/redundant-regex-syntax` — a constant regular expression that spells something the long
//! way.

use super::regex_call::{self, CALL_NAMES};
use super::regex_syntax::{self as rx, CharSet, ClassItem, GroupKind, Regex, Rx, SetKind, K};
use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["regex", "clarity"],
    remediation_minutes: 2,
    description: r#"Detects constant regular expressions (given to `Pattern.compile`,
`Pattern.matches`, `String.matches`, `replaceAll`, `replaceFirst`, `split`, or Bean Validation's
`@Pattern`) that contain syntax with a shorter or plainer equivalent:

* a quantifier written with braces where a symbol or nothing says the same: `{1}` (remove it),
  `{0,}` (`*`), `{1,}` (`+`), `{0,1}` (`?`);
* an empty group: `(?:)`, `(?i:)`, `(?>)`, and `()` when the groups of the match cannot be read
  (`matches`, `split`, `@Pattern`, a replacement without `$`);
* a character class holding a single plain character or a single predefined class: `[a]`,
  `[,]`, `[\d]`;
* a member of a character class that another member already covers: `[\w_]`, `[a-z0-9a]`,
  `[\s\n]`;
* an alternation of single characters or predefined classes, which a character class states
  in one step: `a|b|c` is `[abc]`, `\s|,` is `[\s,]`;
* two or more spaces in a row, which are hard to count: ` {3}` states the number;
* a named group whose name is never used: the pattern is only tested (`matches`, `split`,
  `@Pattern`) or the replacement does not refer to `${name}`, and the pattern has no
  `\k<name>`.

**Why it matters.** Regular expressions are dense; every redundant construct is one more thing
the reader must decode and check, and a redundant class member suggests the author expected
it to add something (`[\w_]` reads as if `\w` did not include `_`).

**How to fix.** Use the shorter form shown in the message.

```java
// Noncompliant
String[] words = line.split("[,]|[;]");
boolean ok = code.matches("[A-Z]{1}[\\w_]{0,}");

// Compliant
String[] words = line.split("[,;]");
boolean ok = code.matches("[A-Z]\\w*");
```

Not reported: single-character classes used to quote a metacharacter or a space (`[.]`, `[$]`,
`[ ]`), classes whose repetition is a `|` misread as an alternation (see
`java/regex-suspicious-character-class`), duplicates that only appear under case-insensitive
matching, empty lookarounds (`(?!)` is a deliberate "never matches"), named groups of a pattern
compiled with `Pattern.compile` (the matcher may read them), patterns that are not constant, and
patterns the parser does not model (comments mode, `LITERAL`)."#,
    ..RuleMeta::new(
        "java/redundant-regex-syntax",
        "java",
        "Regular expression written the long way",
    )
};

pub struct RedundantRegexSyntax;

/// Characters that a one-character class may be quoting: regex metacharacters, class syntax,
/// whitespace, and `#` (a comment in comments mode).
fn quotes_something(c: char) -> bool {
    c.is_whitespace() || "\\^$.|?*+()[]{}-&#".contains(c)
}

/// The set of a class item, when known.
fn item_set(item: &ClassItem) -> Option<CharSet> {
    match item {
        ClassItem::Char(c) => Some(CharSet::single(*c)),
        ClassItem::Range(a, b) => Some(CharSet::from_ranges(vec![(*a as u32, *b as u32)])),
        ClassItem::Escape(s) => s.clone(),
    }
}

/// How the groups of the match can be read at this call site.
struct Access<'a> {
    /// No group of the match is ever read.
    none: bool,
    /// The replacement text of `replaceAll` / `replaceFirst`, when constant.
    replacement: Option<&'a str>,
}

fn find_issue(re: &Regex, pattern: &str, access: &Access<'_>) -> Option<String> {
    let mut quantifier = None;
    let mut empty_group = None;
    let mut single_class = None;
    let mut covered = None;
    let mut alternation = None;
    let mut spaces = None;
    let mut named = None;
    rx::each_node(&re.root, &mut |n| match &n.k {
        K::Repeat { body, min, max, .. } if quantifier.is_none() => {
            let q: String = re.chars[body.end.min(n.end)..n.end].iter().collect();
            let q = q.trim_end_matches(['?', '+']);
            if !q.starts_with('{') {
                return;
            }
            let better = match (min, max) {
                (1, Some(1)) => "nothing: the element is matched once anyway",
                (0, None) => "`*`",
                (1, None) => "`+`",
                (0, Some(1)) => "`?`",
                _ => return,
            };
            quantifier = Some(format!("Replace the quantifier `{q}` with {better}"));
        }
        K::Group(kind, body) if empty_group.is_none() && matches!(body.k, K::Empty) => {
            let redundant = match kind {
                GroupKind::NonCapture | GroupKind::Flags | GroupKind::Atomic => true,
                GroupKind::Capture(_) => access.none,
                _ => false,
            };
            if redundant {
                empty_group = Some(format!("Remove the empty group `{}`", re.snippet(n)));
            }
        }
        K::Set(_, _, SetKind::Class(info)) if info.simple => {
            if single_class.is_none() && !info.negated && info.items.len() == 1 {
                let (item, start, end) = &info.items[0];
                let inner: String = re.chars[*start..*end].iter().collect();
                let plain = match item {
                    ClassItem::Char(c) => {
                        inner.chars().count() == 1 && !quotes_something(*c) && !c.is_control()
                    }
                    ClassItem::Escape(_) => inner.starts_with('\\'),
                    ClassItem::Range(..) => false,
                };
                // The class text must be exactly `[` + the item + `]`.
                if plain && n.end == *end + 1 && n.start + 1 == *start {
                    single_class = Some(format!(
                        "Write `{}` without the brackets: a class of one member matches just that",
                        rx::shorten(&inner)
                    ));
                }
            }
            if covered.is_none() && info.items.len() > 1 {
                covered = covered_member(re, &info.items);
            }
        }
        K::Alt(branches) if alternation.is_none() => {
            if let Some(msg) = single_char_alternation(re, n, branches) {
                alternation = Some(msg);
            }
        }
        K::Seq(items) if spaces.is_none() && !pattern.contains("\\Q") => {
            // Literal spaces only (not `\x20`).
            let space =
                |it: &Rx| matches!(it.k, K::Char(' ', _)) && re.chars.get(it.start) == Some(&' ');
            let mut run = 0;
            for it in items {
                if space(it) {
                    run += 1;
                    if run >= 2 {
                        break;
                    }
                } else {
                    run = 0;
                }
            }
            if run >= 2 {
                let mut total = 0;
                let mut best = 0;
                for it in items {
                    if space(it) {
                        total += 1;
                        best = best.max(total);
                    } else {
                        total = 0;
                    }
                }
                spaces = Some(format!(
                    "Write the {best} spaces in a row as ` {{{best}}}`: repeated spaces are hard to count"
                ));
            }
        }
        K::Group(GroupKind::Capture(_), _) if named.is_none() => {
            let text = re.text(n);
            let Some(rest) = text.strip_prefix("(?<") else {
                return;
            };
            let Some(name) = rest.split('>').next() else {
                return;
            };
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
                return;
            }
            if pattern.contains(&format!("\\k<{name}>")) {
                return;
            }
            let unused = access.none
                || access
                    .replacement
                    .is_some_and(|r| !r.contains(&format!("${{{name}}}")));
            if unused {
                named = Some(format!(
                    "Remove the name of the group `{name}`: nothing refers to it"
                ));
            }
        }
        _ => {}
    });
    quantifier
        .or(empty_group)
        .or(single_class)
        .or(covered)
        .or(alternation)
        .or(spaces)
        .or(named)
}

/// A member of a simple class that another member already contains.
fn covered_member(re: &Regex, items: &[(ClassItem, usize, usize)]) -> Option<String> {
    // A `|` among repeated characters is `java/regex-suspicious-character-class`'s case.
    let chars: Vec<char> = items
        .iter()
        .filter_map(|(it, ..)| match it {
            ClassItem::Char(c) => Some(*c),
            _ => None,
        })
        .collect();
    if chars.contains(&'|') {
        return None;
    }
    let sets: Vec<Option<CharSet>> = items.iter().map(|(it, ..)| item_set(it)).collect();
    for (i, (_, si, ei)) in items.iter().enumerate() {
        let Some(a) = &sets[i] else { continue };
        for (j, (_, sj, ej)) in items.iter().enumerate() {
            if i == j {
                continue;
            }
            let Some(b) = &sets[j] else { continue };
            // Equal members: report the later one; otherwise the contained one.
            if a == b {
                if i > j {
                    return Some(format!(
                        "Remove the repeated `{}` from this character class",
                        re.snippet_range(*si, *ei)
                    ));
                }
                continue;
            }
            if a.subset_of(b) {
                return Some(format!(
                    "Remove `{}` from this character class: `{}` already matches it",
                    re.snippet_range(*si, *ei),
                    re.snippet_range(*sj, *ej)
                ));
            }
        }
    }
    None
}

/// An alternation whose branches are all single characters or predefined classes.
fn single_char_alternation(re: &Regex, alt: &Rx, branches: &[Rx]) -> Option<String> {
    if branches.len() < 2 {
        return None;
    }
    let mut flags = None;
    let mut texts: Vec<String> = Vec::with_capacity(branches.len());
    for b in branches {
        let f = match &b.k {
            K::Char(_, f) => *f,
            K::Set(_, f, SetKind::Escape) => *f,
            _ => return None,
        };
        if *flags.get_or_insert(f) != f {
            return None;
        }
        let t = re.text(b);
        // Duplicate branches are `java/regex-shadowed-alternative`'s case.
        if texts.contains(&t) {
            return None;
        }
        texts.push(t);
    }
    Some(format!(
        "Replace the alternation `{}` with a character class: each branch is a single character",
        re.snippet(alt)
    ))
}

impl<'t> JavaRule<'t> for RedundantRegexSyntax {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "annotation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        CALL_NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(u) = regex_call::regex_use(ctx, node, |p| {
            p.contains(['{', '(', '[', '|', '<']) || p.contains("  ")
        }) else {
            return;
        };
        let replacement = if matches!(ctx.call_name(node), Some("replaceAll" | "replaceFirst")) {
            util::arg(node, 1).and_then(|r| util::string_constant(ctx, r))
        } else {
            None
        };
        let none = u.split
            || match kind(node) {
                "annotation" => true,
                _ => match ctx.call_name(node) {
                    Some("matches" | "split") => true,
                    Some("replaceAll" | "replaceFirst") => {
                        replacement.as_deref().is_some_and(|r| !r.contains('$'))
                    }
                    _ => false,
                },
            };
        let Some(re) = u.parse() else {
            return;
        };
        let access = Access {
            none,
            replacement: replacement.as_deref(),
        };
        if let Some(msg) = find_issue(&re, &u.pattern, &access) {
            ctx.emit(u.node, msg);
        }
    }
}
