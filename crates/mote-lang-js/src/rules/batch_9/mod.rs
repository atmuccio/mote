//! Rule batch 9: maintainability smells (accessor pairs, magic numbers, mutable public statics,
//! top-level `this`) and `Number.isNaN`. The author of this batch adds rule modules in this
//! directory and lists them in [`rules`]; nothing outside `src/rules/batch_9/` (except fixtures)
//! needs to change. See `RULE_AUTHORING.md`.

mod accessor_pairs;
mod magic_number;
mod mutable_public_static_field;
mod prefer_number_isnan;
mod top_level_this;

use crate::JsRule;

/// The rules of batch 9, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(prefer_number_isnan::PreferNumberIsnan),
        Box::new(mutable_public_static_field::MutablePublicStaticField),
        Box::new(top_level_this::TopLevelThis),
        Box::new(accessor_pairs::AccessorPairs),
        Box::new(magic_number::MagicNumber),
    ]
}
