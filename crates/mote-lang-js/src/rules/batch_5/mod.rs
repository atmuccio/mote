//! Rule batch 5: likely bugs (constructors and `super`, constant and import assignments,
//! unreachable loops, regular expressions, test assertions). The author of this batch adds rule
//! modules in this directory and lists them in [`rules`]; nothing outside `src/rules/batch_5/`
//! (except fixtures) needs to change. See `RULE_AUTHORING.md`.

mod bitwise_on_booleans;
mod constant_length_comparison;
mod constructor_super;
mod empty_collection_access;
mod identical_operands;
mod no_compound_case_label;
mod no_const_assign;
mod no_delete_var;
mod no_empty_character_class;
mod no_import_assign;
mod no_new_native_nonconstructor;
mod no_obj_calls;
mod no_self_import;
mod no_this_before_super;
mod no_unreachable_loop;
mod no_unsafe_optional_chaining;
mod no_use_before_define;
mod require_yield;
mod reversed_compound_operator;
mod self_comparing_assertion;
mod stateful_regex_test;
mod unread_collection;
mod useless_increment;
mod util;
mod valid_expect;

use crate::JsRule;

/// The rules of batch 5, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(constructor_super::ConstructorSuper),
        Box::new(no_this_before_super::NoThisBeforeSuper),
        Box::new(no_const_assign::NoConstAssign),
        Box::new(no_import_assign::NoImportAssign),
        Box::new(no_use_before_define::NoUseBeforeDefine),
        Box::new(no_unsafe_optional_chaining::NoUnsafeOptionalChaining),
        Box::new(require_yield::RequireYield),
        Box::new(no_obj_calls::NoObjCalls),
        Box::new(no_new_native_nonconstructor::NoNewNativeNonconstructor),
        Box::new(no_delete_var::NoDeleteVar),
        Box::new(no_empty_character_class::NoEmptyCharacterClass),
        Box::new(stateful_regex_test::StatefulRegexTest),
        Box::new(no_compound_case_label::NoCompoundCaseLabel),
        Box::new(reversed_compound_operator::ReversedCompoundOperator),
        Box::new(useless_increment::UselessIncrement),
        Box::new(constant_length_comparison::ConstantLengthComparison),
        Box::new(bitwise_on_booleans::BitwiseOnBooleans),
        Box::new(identical_operands::IdenticalOperands),
        Box::new(no_unreachable_loop::NoUnreachableLoop),
        Box::new(unread_collection::UnreadCollection),
        Box::new(empty_collection_access::EmptyCollectionAccess),
        Box::new(no_self_import::NoSelfImport),
        Box::new(valid_expect::ValidExpect),
        Box::new(self_comparing_assertion::SelfComparingAssertion),
    ]
}
