//! Rule batch 3 — exceptions, concurrency, tests, misc. The author of this batch adds rule
//! modules in this directory and lists them in [`rules`]; nothing outside `src/rules/batch_3/`
//! (except fixtures) needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod assert_boolean_literal;
mod catch_generic_exception;
mod catch_throwable_or_error;
mod clone_override;
mod disabled_test_without_reason;
mod double_checked_locking;
mod exception_naming;
mod finalize_override;
mod generic_exception_thrown;
mod instanceof_pattern_candidate;
mod labeled_statement;
mod lost_exception_cause;
mod prefer_try_with_resources;
mod resource_not_closed_with_try;
mod sleep_in_test;
mod static_date_format;
mod synchronized_on_value;
mod test_without_assertion;
mod throws_generic_exception;
mod useless_catch;
mod util;
mod wait_notify_outside_synchronized;

/// The rules of batch 3, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // exceptions
        Box::new(catch_throwable_or_error::CatchThrowableOrError),
        Box::new(catch_generic_exception::CatchGenericException),
        Box::new(throws_generic_exception::ThrowsGenericException),
        Box::new(useless_catch::UselessCatch),
        Box::new(lost_exception_cause::LostExceptionCause),
        Box::new(exception_naming::ExceptionNaming),
        Box::new(generic_exception_thrown::GenericExceptionThrown),
        // concurrency
        Box::new(double_checked_locking::DoubleCheckedLocking),
        Box::new(static_date_format::StaticDateFormat::default()),
        Box::new(synchronized_on_value::SynchronizedOnValue),
        Box::new(wait_notify_outside_synchronized::WaitNotifyOutsideSynchronized),
        // object lifecycle
        Box::new(finalize_override::FinalizeOverride),
        Box::new(clone_override::CloneOverride),
        // tests
        Box::new(sleep_in_test::SleepInTest),
        Box::new(test_without_assertion::TestWithoutAssertion::default()),
        Box::new(disabled_test_without_reason::DisabledTestWithoutReason),
        Box::new(assert_boolean_literal::AssertBooleanLiteral),
        // misc
        Box::new(instanceof_pattern_candidate::InstanceofPatternCandidate),
        Box::new(labeled_statement::LabeledStatement),
        Box::new(resource_not_closed_with_try::ResourceNotClosedWithTry),
        Box::new(prefer_try_with_resources::PreferTryWithResources),
    ]
}
