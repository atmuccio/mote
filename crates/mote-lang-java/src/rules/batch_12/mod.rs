//! Rule batch 12: Java bug patterns (comparisons, overflow, concurrency, iterators, serialization,
//! test lifecycle). The author of this batch adds rule modules in this directory and lists them in
//! [`rules`]; nothing outside `src/rules/batch_12/` (except fixtures) needs to change. See
//! `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod assert_public_argument;
mod compare_to_without_comparable;
mod default_charset;
mod double_checked_publish_early;
mod duplicate_branch_body;
mod equals_asymmetric_subclass;
mod int_multiplication_overflow;
mod interrupt_status_lost;
mod iterator_contract;
mod misleading_if_layout;
mod nested_test_class_not_run;
mod optional_null;
mod parameter_overwritten_before_read;
mod serialization_constructor_missing;
mod test_lifecycle_method_not_run;
mod wait_notify_wrong_monitor;
mod wait_with_nested_lock;

/// The rules of batch 12, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        Box::new(duplicate_branch_body::DuplicateBranchBody),
        Box::new(interrupt_status_lost::InterruptStatusLost),
        Box::new(int_multiplication_overflow::IntMultiplicationOverflow),
        Box::new(nested_test_class_not_run::NestedTestClassNotRun::default()),
        Box::new(test_lifecycle_method_not_run::TestLifecycleMethodNotRun::default()),
        Box::new(parameter_overwritten_before_read::ParameterOverwrittenBeforeRead),
        Box::new(misleading_if_layout::MisleadingIfLayout),
        Box::new(wait_notify_wrong_monitor::WaitNotifyWrongMonitor),
        Box::new(double_checked_publish_early::DoubleCheckedPublishEarly),
        Box::new(equals_asymmetric_subclass::EqualsAsymmetricSubclass::default()),
        Box::new(optional_null::OptionalNull::default()),
        Box::new(iterator_contract::IteratorContract),
        Box::new(default_charset::DefaultCharset),
        Box::new(compare_to_without_comparable::CompareToWithoutComparable),
        Box::new(wait_with_nested_lock::WaitWithNestedLock::default()),
        Box::new(assert_public_argument::AssertPublicArgument),
        Box::new(serialization_constructor_missing::SerializationConstructorMissing),
    ]
}
