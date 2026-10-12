//! Rule batch 10: narrower shapes split off from existing rules (overly broad expected exceptions,
//! `if`/`else` that only returns booleans). The author of this batch adds rule modules in this
//! directory and lists them in [`rules`]; nothing outside `src/rules/batch_10/` (except fixtures)
//! needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod broad_expected_exception;
mod simplifiable_boolean_if_else;

/// The rules of batch 10, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        Box::new(simplifiable_boolean_if_else::SimplifiableBooleanIfElse),
        Box::new(broad_expected_exception::BroadExpectedException),
    ]
}
