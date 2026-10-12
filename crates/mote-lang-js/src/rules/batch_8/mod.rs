//! Rule batch 8: maintainability smells (tests, return values, regular expressions, types,
//! nesting). The author of this batch adds rule modules in this directory and lists them in
//! [`rules`]; nothing outside `src/rules/batch_8/` (except fixtures) needs to change. See
//! `RULE_AUTHORING.md`.

mod assert_argument_order;
mod assignment_in_operand;
mod async_in_constructor;
mod consistent_return;
mod default_param_last;
mod enum_initializers;
mod for_loop_counter_misuse;
mod immediately_returned_variable;
mod invariant_return;
mod max_nested_functions;
mod negated_comparison;
mod no_extend_native;
mod no_labels;
mod no_redundant_type_constituents;
mod no_unused_private_class_members;
mod no_useless_catch;
mod prefer_type_predicate;
mod redundant_regex_syntax;
mod regex_too_complex;
mod skipped_test_without_reason;
mod test_file_without_tests;
mod test_retries;
mod test_without_assertion;
mod unspecific_throw_assertion;
mod util;

use crate::JsRule;

/// The rules of batch 8, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(no_useless_catch::NoUselessCatch),
        Box::new(no_unused_private_class_members::NoUnusedPrivateClassMembers),
        Box::new(invariant_return::InvariantReturn),
        Box::new(for_loop_counter_misuse::ForLoopCounterMisuse),
        Box::new(consistent_return::ConsistentReturn),
        Box::new(no_extend_native::NoExtendNative),
        Box::new(async_in_constructor::AsyncInConstructor),
        Box::new(test_without_assertion::TestWithoutAssertion),
        Box::new(unspecific_throw_assertion::UnspecificThrowAssertion),
        Box::new(negated_comparison::NegatedComparison),
        Box::new(no_redundant_type_constituents::NoRedundantTypeConstituents),
        Box::new(immediately_returned_variable::ImmediatelyReturnedVariable),
        Box::new(no_labels::NoLabels),
        Box::new(assignment_in_operand::AssignmentInOperand),
        Box::new(max_nested_functions::MaxNestedFunctions),
        Box::new(regex_too_complex::RegexTooComplex),
        Box::new(default_param_last::DefaultParamLast),
        Box::new(redundant_regex_syntax::RedundantRegexSyntax),
        Box::new(prefer_type_predicate::PreferTypePredicate),
        Box::new(enum_initializers::EnumInitializers),
        Box::new(assert_argument_order::AssertArgumentOrder),
        Box::new(skipped_test_without_reason::SkippedTestWithoutReason),
        Box::new(test_retries::TestRetries),
        Box::new(test_file_without_tests::TestFileWithoutTests),
    ]
}
