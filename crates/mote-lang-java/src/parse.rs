//! Parsing with one reusable `Parser` per thread and a deterministic work budget.
//!
//! tree-sitter calls the progress callback once every 100 parse operations (shifts, reductions,
//! recovery steps, tree balancing). The number of calls depends only on the input, so counting
//! them bounds the work on pathological inputs without making the result depend on machine load
//! or thread count (a wall-clock deadline would, and the engine caches the result).

#[path = "case_patterns.rs"]
mod case_patterns;

use crate::kinds::LANGUAGE;
use std::cell::RefCell;
use std::ops::ControlFlow;
use std::time::{Duration, Instant};
use tree_sitter::{ParseOptions, Parser, Tree};

/// How much parser work a file may use before it is reported as a syntax error instead.
///
/// The unit is one tree-sitter progress check, about 100 parse operations. A file of `len` bytes
/// may use `base + len * per_kib / 1024` checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseBudget {
    /// Checks allowed for any file, however small.
    pub base: u64,
    /// Additional checks per KiB of input.
    pub per_kib: u64,
}

impl ParseBudget {
    /// 10 000 checks plus 256 per KiB: about 25 operations per byte. Real code uses at most ~1
    /// operation per byte (SymmetricDS: 0.53 on average, 1.1 for the densest file); the densest
    /// synthetic inputs found (`.b.b.b…`, `;;;…`, ambiguous generics, error recovery on
    /// punctuation soup) use 5–6. Only runaway parses reach the budget, and they do so after the
    /// same amount of work on every machine.
    pub const DEFAULT: ParseBudget = ParseBudget {
        base: 10_000,
        per_kib: 256,
    };

    /// Progress checks allowed for an input of `len` bytes.
    pub fn checks_for(&self, len: usize) -> u64 {
        let per_len = (len as u64).saturating_mul(self.per_kib) / 1024;
        self.base.saturating_add(per_len)
    }
}

impl Default for ParseBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

thread_local! {
    static PARSER: RefCell<Option<Parser>> = const { RefCell::new(None) };
}

/// Parse `text` within `budget` (and within `timeout` of wall-clock time, when one is given).
/// Returns `None` when a limit was hit (or the parser could not be created); the caller reports
/// that as a syntax error.
pub(crate) fn parse(text: &str, budget: ParseBudget, timeout: Option<Duration>) -> Option<Tree> {
    PARSER.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        if slot.is_none() {
            let mut p = Parser::new();
            p.set_language(&LANGUAGE).ok()?;
            *slot = Some(p);
        }
        let parser = slot.as_mut()?;
        // Same length and offsets as `text`; see `varargs` and `case_patterns`.
        let text = crate::varargs::blank_varargs_annotations(text);
        let text = case_patterns::blank_case_pattern_modifiers(text);
        let bytes = text.as_bytes();
        let len = bytes.len();
        let max_checks = budget.checks_for(len);
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut checks = 0u64;
        let mut progress = |_: &tree_sitter::ParseState| {
            checks += 1;
            if checks > max_checks || deadline.is_some_and(|d| Instant::now() > d) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let options = ParseOptions::new().progress_callback(&mut progress);
        let tree = parser.parse_with_options(
            &mut |i, _| if i < len { &bytes[i..] } else { &[] as &[u8] },
            None,
            Some(options),
        );
        if tree.is_none() {
            // A cancelled parse leaves state behind that the next parse would resume.
            parser.reset();
        }
        tree
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY: ParseBudget = ParseBudget {
        base: 1,
        per_kib: 0,
    };

    #[test]
    fn parses_and_cancels() {
        let t = parse(
            "class A { void m() { int x = 1; } }",
            ParseBudget::DEFAULT,
            None,
        )
        .expect("tree");
        assert!(!t.root_node().has_error());
        // A budget of one check cancels a non-trivial input; the next parse must still work.
        let big = "class A { void m() { int x = 1; } }\n".repeat(20_000);
        assert!(parse(&big, TINY, None).is_none());
        let t2 = parse("class B {}", ParseBudget::DEFAULT, None).expect("tree after cancel");
        assert_eq!(t2.root_node().kind(), "program");
        // A zero timeout cancels too.
        assert!(parse(&big, ParseBudget::DEFAULT, Some(Duration::ZERO)).is_none());
        assert!(parse("class C {}", ParseBudget::DEFAULT, None).is_some());
    }

    #[test]
    fn budget_is_proportional_to_size() {
        assert_eq!(ParseBudget::DEFAULT.checks_for(0), 10_000);
        assert_eq!(
            ParseBudget::DEFAULT.checks_for(1 << 20),
            10_000 + 256 * 1024
        );
        assert_eq!(
            ParseBudget {
                base: u64::MAX,
                per_kib: u64::MAX
            }
            .checks_for(usize::MAX),
            u64::MAX
        );
    }

    /// The budget is a count of parser operations: the cut-off point is the same on every run.
    #[test]
    fn budget_cutoff_is_exact() {
        let text = "class A { void m() { int x = 1; } }\n".repeat(2_000);
        let mut needed = 0u64;
        {
            let mut p = Parser::new();
            p.set_language(&LANGUAGE).unwrap();
            let mut count = |_: &tree_sitter::ParseState| {
                needed += 1;
                ControlFlow::Continue(())
            };
            let opts = ParseOptions::new().progress_callback(&mut count);
            let bytes = text.as_bytes();
            p.parse_with_options(
                &mut |i, _| if i < bytes.len() { &bytes[i..] } else { &[] },
                None,
                Some(opts),
            )
            .expect("tree");
        }
        assert!(needed > 10, "{needed}");
        for _ in 0..3 {
            let exact = ParseBudget {
                base: needed,
                per_kib: 0,
            };
            assert!(parse(&text, exact, None).is_some());
            let short = ParseBudget {
                base: needed - 1,
                per_kib: 0,
            };
            assert!(parse(&text, short, None).is_none());
        }
        // Real code stays far below the default budget.
        assert!(needed * 10 < ParseBudget::DEFAULT.checks_for(text.len()));
    }
}
