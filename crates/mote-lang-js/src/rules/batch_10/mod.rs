//! Rule batch 10: misleading `if` layout and useless regular-expression quantifiers. The author of
//! this batch adds rule modules in this directory and lists them in [`rules`]; nothing outside
//! `src/rules/batch_10/` (except fixtures) needs to change. See `RULE_AUTHORING.md`.

mod misleading_if_layout;
mod useless_regex_quantifier;

use crate::JsRule;

/// The rules of batch 10, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(misleading_if_layout::MisleadingIfLayout),
        Box::new(useless_regex_quantifier::UselessRegexQuantifier),
    ]
}
