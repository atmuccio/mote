//! Source comments and ESLint-compatible suppression directives.
//!
//! Recognised directives (in line or block comments, at the start of the comment text):
//!
//! * `eslint-disable-next-line [rules]` — the line after the comment,
//! * `eslint-disable-line [rules]` — the comment's own line,
//! * `eslint-disable [rules]` … `eslint-enable [rules]` — a region (to end of file if unclosed).
//!
//! `rules` is a comma-separated list; no list means all rules. ESLint names map to Mote ids:
//! core rules `name` → `js/name`, `@typescript-eslint/name` → `js/name`. Rules of other ESLint
//! plugins have no Mote equivalent and are ignored (a directive naming only such rules
//! suppresses nothing). Text after ` -- ` is kept as the justification.

use mote_core::{Comment, LineIndex, ScopedSuppression};
use std::collections::BTreeMap;

/// The kind of these suppressions: the engine applies them with the `eslint` source of
/// `suppressions.compat`.
const KIND: &str = "compat:eslint";

pub(crate) fn convert(
    comments: &[oxc_ast::Comment],
    source: &str,
    lines: &LineIndex<'_>,
) -> Vec<Comment> {
    let mut out = Vec::with_capacity(comments.len());
    for c in comments {
        let (start, end) = (c.span.start, c.span.end);
        let Some(text) = source.get(start as usize..end as usize) else {
            continue;
        };
        let (start_line, start_col) = lines.line_col(start);
        let line_start = source
            .get(..start as usize)
            .and_then(|s| s.rfind('\n'))
            .map_or(0, |i| i + 1);
        let trailing = source
            .get(line_start..start as usize)
            .is_some_and(|prefix| prefix.chars().any(|ch| !ch.is_whitespace()));
        out.push(Comment {
            start_line,
            end_line: lines.line_of(end),
            start_col,
            text: text.to_string(),
            trailing,
        });
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Directive {
    NextLine,
    Line,
    Disable,
    Enable,
}

struct Open {
    start_line: u32,
    justification: Option<String>,
}

pub(crate) fn suppressions(
    comments: &[oxc_ast::Comment],
    source: &str,
    lines: &LineIndex<'_>,
) -> Vec<ScopedSuppression> {
    let mut out = Vec::new();
    let mut open_all: Option<Open> = None;
    let mut open_rules: BTreeMap<String, Open> = BTreeMap::new();

    for c in comments {
        let Some(body) = source.get(c.content_span().start as usize..c.content_span().end as usize)
        else {
            continue;
        };
        let Some((directive, rest)) = parse_directive(body) else {
            continue;
        };
        let (list, justification) = split_justification(rest);
        // `None` = all rules; `Some(empty)` = only unknown rules were named.
        let rules = parse_rules(list);
        if rules.as_ref().is_some_and(Vec::is_empty) {
            continue;
        }
        let start_line = lines.line_of(c.span.start);
        match directive {
            Directive::NextLine | Directive::Line => {
                let line = if directive == Directive::NextLine {
                    lines.line_of(c.span.end) + 1
                } else {
                    start_line
                };
                out.push(ScopedSuppression {
                    start_line: line,
                    end_line: line,
                    rules,
                    kind: KIND.to_string(),
                    justification,
                });
            }
            Directive::Disable => match rules {
                None => {
                    if open_all.is_none() {
                        open_all = Some(Open {
                            start_line,
                            justification,
                        });
                    }
                }
                Some(ids) => {
                    for id in ids {
                        open_rules.entry(id).or_insert_with(|| Open {
                            start_line,
                            justification: justification.clone(),
                        });
                    }
                }
            },
            Directive::Enable => match rules {
                None => {
                    if let Some(open) = open_all.take() {
                        out.push(region(open, start_line, None));
                    }
                    for (id, open) in std::mem::take(&mut open_rules) {
                        out.push(region(open, start_line, Some(vec![id])));
                    }
                }
                Some(ids) => {
                    for id in ids {
                        if let Some(open) = open_rules.remove(&id) {
                            out.push(region(open, start_line, Some(vec![id])));
                        }
                    }
                }
            },
        }
    }

    let last = lines.line_count().max(1);
    if let Some(open) = open_all.take() {
        out.push(region(open, last, None));
    }
    for (id, open) in open_rules {
        out.push(region(open, last, Some(vec![id])));
    }
    out.sort_by(|a, b| {
        (a.start_line, a.end_line, &a.rules).cmp(&(b.start_line, b.end_line, &b.rules))
    });
    out
}

fn region(open: Open, end_line: u32, rules: Option<Vec<String>>) -> ScopedSuppression {
    ScopedSuppression {
        start_line: open.start_line,
        end_line: end_line.max(open.start_line),
        rules,
        kind: KIND.to_string(),
        justification: open.justification,
    }
}

fn parse_directive(body: &str) -> Option<(Directive, &str)> {
    let body = body.trim_start();
    for (keyword, directive) in [
        ("eslint-disable-next-line", Directive::NextLine),
        ("eslint-disable-line", Directive::Line),
        ("eslint-disable", Directive::Disable),
        ("eslint-enable", Directive::Enable),
    ] {
        if let Some(rest) = body.strip_prefix(keyword) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return Some((directive, rest));
            }
        }
    }
    None
}

/// Split `rule-a, rule-b -- because reasons` into the list and the justification.
fn split_justification(rest: &str) -> (&str, Option<String>) {
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'-'
            && bytes[i + 1] == b'-'
            && (i == 0 || bytes[i - 1].is_ascii_whitespace())
        {
            let reason = rest[i..].trim_start_matches('-').trim();
            let reason = (!reason.is_empty()).then(|| reason.to_string());
            return (&rest[..i], reason);
        }
        i += 1;
    }
    (rest, None)
}

fn parse_rules(list: &str) -> Option<Vec<String>> {
    let names: Vec<&str> = list
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }
    let mut ids: Vec<String> = names.into_iter().filter_map(map_rule).collect();
    ids.sort();
    ids.dedup();
    Some(ids)
}

/// ESLint rule name → Mote rule id.
fn map_rule(name: &str) -> Option<String> {
    let base = match name.strip_prefix("@typescript-eslint/") {
        Some(rest) => rest,
        None if name.contains('/') => return None,
        None => name,
    };
    if base.is_empty() || !base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    Some(format!("js/{base}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn run(source: &str) -> (Vec<Comment>, Vec<ScopedSuppression>) {
        let alloc = Allocator::default();
        let ret = Parser::new(&alloc, source, SourceType::ts()).parse();
        let lines = LineIndex::new(source);
        (
            convert(&ret.program.comments, source, &lines),
            suppressions(&ret.program.comments, source, &lines),
        )
    }

    fn ids(s: &ScopedSuppression) -> Option<Vec<&str>> {
        s.rules
            .as_ref()
            .map(|r| r.iter().map(String::as_str).collect())
    }

    #[test]
    fn comments_with_trailing_flag() {
        let (comments, _) = run("// head\nlet a = 1; // tail\n/* block\n spans */ let b;\n");
        assert_eq!(comments.len(), 3);
        assert_eq!(
            (
                comments[0].start_line,
                comments[0].trailing,
                comments[0].text.as_str()
            ),
            (1, false, "// head")
        );
        assert_eq!(
            (
                comments[1].start_line,
                comments[1].start_col,
                comments[1].trailing
            ),
            (2, 12, true)
        );
        assert_eq!(
            (
                comments[2].start_line,
                comments[2].end_line,
                comments[2].trailing
            ),
            (3, 4, false)
        );
    }

    #[test]
    fn line_directives() {
        let src = "// eslint-disable-next-line no-console, @typescript-eslint/no-explicit-any -- legacy\nconsole.log(1 as any);\nfoo(); // eslint-disable-line\nbar(); // eslint-disable-line react/no-danger\n";
        let (_, s) = run(src);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].start_line, s[0].end_line), (2, 2));
        assert_eq!(
            ids(&s[0]),
            Some(vec!["js/no-console", "js/no-explicit-any"])
        );
        assert_eq!(s[0].justification.as_deref(), Some("legacy"));
        assert_eq!(s[0].kind, "compat:eslint");
        assert_eq!(
            (s[1].start_line, s[1].end_line, s[1].rules.clone()),
            (3, 3, None)
        );
    }

    #[test]
    fn regions() {
        let src = "/* eslint-disable no-var, eqeqeq */\nvar a;\n/* eslint-enable no-var */\nvar b;\n/* eslint-disable */\nx;\n";
        let (_, s) = run(src);
        assert_eq!(s.len(), 3);
        assert_eq!(
            (s[0].start_line, s[0].end_line, ids(&s[0])),
            (1, 3, Some(vec!["js/no-var"]))
        );
        assert_eq!(
            (s[1].start_line, s[1].end_line, ids(&s[1])),
            (1, 6, Some(vec!["js/eqeqeq"]))
        );
        assert_eq!(
            (s[2].start_line, s[2].end_line, s[2].rules.clone()),
            (5, 6, None)
        );
    }

    #[test]
    fn not_directives() {
        let (_, s) =
            run("// eslint-disabled\n/** eslint-disable */\n// see eslint-disable-line\nx;\n");
        assert!(s.is_empty(), "{s:?}");
    }
}
