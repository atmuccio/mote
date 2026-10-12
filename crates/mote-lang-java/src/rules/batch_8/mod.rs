//! Rule batch 8: mistakes in tests (assertions, mocks), constructor pitfalls, loop bounds, null
//! checks and equality. The author of this batch adds rule modules in this directory and lists them
//! in [`rules`]; nothing outside `src/rules/batch_8/` (except fixtures and the registration line in
//! `src/rules/mod.rs`) needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod assertion_description_after_check;
mod assertion_in_thread;
mod compare_to_without_equals;
mod conditional_mixed_boxed_numbers;
mod constructor_calls_overridable;
mod equals_unrelated_types;
mod field_injection;
mod inner_class_could_be_static;
mod loop_exits_first_pass;
mod loop_may_skip_bound;
mod loop_never_entered;
mod map_callback_returns_null;
mod mockito_mixed_matchers;
mod mocks_not_initialized;
mod null_check_after_dereference;
mod overload_instead_of_override;
mod soft_assertions_not_verified;
mod static_field_set_in_constructor;
mod swapped_arguments;
mod test_assertion_in_production;
mod test_catch_only_fails;
mod this_escape_in_constructor;
mod unsynchronized_accessor;

/// The rules of batch 8, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Equality
        Box::new(equals_unrelated_types::EqualsUnrelatedTypes::default()),
        Box::new(conditional_mixed_boxed_numbers::ConditionalMixedBoxedNumbers),
        // Calls
        Box::new(swapped_arguments::SwappedArguments::default()),
        Box::new(compare_to_without_equals::CompareToWithoutEquals),
        Box::new(overload_instead_of_override::OverloadInsteadOfOverride::default()),
        // Loops
        Box::new(loop_exits_first_pass::LoopExitsFirstPass),
        Box::new(loop_never_entered::LoopNeverEntered),
        Box::new(loop_may_skip_bound::LoopMaySkipBound),
        // Null handling
        Box::new(null_check_after_dereference::NullCheckAfterDereference),
        Box::new(map_callback_returns_null::MapCallbackReturnsNull),
        // Concurrency
        Box::new(unsynchronized_accessor::UnsynchronizedAccessor),
        // Design
        Box::new(static_field_set_in_constructor::StaticFieldSetInConstructor::default()),
        Box::new(constructor_calls_overridable::ConstructorCallsOverridable::default()),
        Box::new(this_escape_in_constructor::ThisEscapeInConstructor),
        Box::new(field_injection::FieldInjection::default()),
        Box::new(inner_class_could_be_static::InnerClassCouldBeStatic),
        // Tests
        Box::new(assertion_in_thread::AssertionInThread::default()),
        Box::new(test_catch_only_fails::TestCatchOnlyFails::default()),
        Box::new(soft_assertions_not_verified::SoftAssertionsNotVerified),
        Box::new(assertion_description_after_check::AssertionDescriptionAfterCheck),
        Box::new(mockito_mixed_matchers::MockitoMixedMatchers::default()),
        Box::new(mocks_not_initialized::MocksNotInitialized::default()),
        Box::new(test_assertion_in_production::TestAssertionInProduction::default()),
    ]
}
