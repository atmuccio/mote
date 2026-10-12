//! Rule batch 7: likely bugs (regular expressions, loops, JSX rendering, array mutation, misleading
//! layout). The author of this batch adds rule modules in this directory and lists them in
//! [`rules`]; nothing outside `src/rules/batch_7/` (except fixtures) needs to change. See
//! `RULE_AUTHORING.md`.

mod accessor_field_mismatch;
mod for_equality_termination;
mod in_operator_misuse;
mod jsx_no_leaked_render;
mod misleading_array_mutation;
mod misleading_indentation;
mod no_code_after_done;
mod no_dupe_disjunctions;
mod no_empty_alternative;
mod no_for_in_array;
mod no_infinite_loop;
mod no_misleading_character_class;
mod no_self_state_update;
mod no_unexpected_multiline;
mod no_unknown_property;
mod reduce_without_initial_value;
mod regex;
mod regex_control_character;
mod unicode_escape_needs_flag;
mod util;
mod void_result_used;

use crate::JsRule;

/// The rules of batch 7, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(no_unexpected_multiline::NoUnexpectedMultiline),
        Box::new(misleading_indentation::MisleadingIndentation),
        Box::new(no_infinite_loop::NoInfiniteLoop),
        Box::new(misleading_array_mutation::MisleadingArrayMutation),
        Box::new(reduce_without_initial_value::ReduceWithoutInitialValue),
        Box::new(void_result_used::VoidResultUsed),
        Box::new(no_dupe_disjunctions::NoDupeDisjunctions),
        Box::new(no_empty_alternative::NoEmptyAlternative),
        Box::new(accessor_field_mismatch::AccessorFieldMismatch),
        Box::new(in_operator_misuse::InOperatorMisuse),
        Box::new(for_equality_termination::ForEqualityTermination),
        Box::new(no_unknown_property::NoUnknownProperty),
        Box::new(jsx_no_leaked_render::JsxNoLeakedRender),
        Box::new(no_for_in_array::NoForInArray),
        Box::new(no_misleading_character_class::NoMisleadingCharacterClass),
        Box::new(unicode_escape_needs_flag::UnicodeEscapeNeedsFlag),
        Box::new(regex_control_character::RegexControlCharacter),
        Box::new(no_self_state_update::NoSelfStateUpdate),
        Box::new(no_code_after_done::NoCodeAfterDone),
    ]
}
