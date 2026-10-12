//! Rule batch 11: lazy regular-expression quantifiers, invisible characters and callbacks that
//! receive extra arguments. The author of this batch adds rule modules in this directory and lists
//! them in [`rules`]. See `RULE_AUTHORING.md`.

mod callback_extra_arguments;
mod invisible_character;
mod regex;
mod regex_lazy_quantifier_at_end;

use crate::JsRule;

/// The rules of batch 11, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(regex_lazy_quantifier_at_end::RegexLazyQuantifierAtEnd),
        Box::new(invisible_character::InvisibleCharacter),
        Box::new(callback_extra_arguments::CallbackExtraArguments),
    ]
}
