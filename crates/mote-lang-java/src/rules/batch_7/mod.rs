//! Rule batch 7: rare but real bugs (arithmetic and bit operations, assertions, iterators, locks,
//! dates, serialization hooks). The author of this batch adds rule modules in this directory and
//! lists them in [`rules`]; nothing outside `src/rules/batch_7/` (except fixtures) needs to change.
//! See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod abs_of_hash_or_random;
mod accessor_uses_wrong_field;
mod assert_with_side_effect;
mod byte_sign_extension;
mod collection_passed_to_itself;
mod date_field_out_of_range;
mod fail_swallowed_by_catch;
mod has_next_calls_next;
mod hex_string_without_padding;
mod impossible_size_check;
mod incomplete_fluent_assertion;
mod int_constant_overflow;
mod intrinsic_lock_on_lock_object;
mod invalid_range_arguments;
mod jdbc_zero_index;
mod list_remove_in_index_loop;
mod loop_counter_wrong_direction;
mod object_method_name_typo;
mod post_increment_self_assignment;
mod random_cast_to_int;
mod reversed_compound_assignment;
mod scheduled_pool_without_core_threads;
mod self_comparing_assertion;
mod separator_used_as_regex;
mod serialization_hook_signature;
mod shift_out_of_range;
mod sleep_in_synchronized;
mod synchronized_on_get_class;
mod synchronized_on_reassigned_field;
mod tostring_or_clone_returns_null;
mod unretained_annotation_lookup;
mod untyped_to_array_cast;
mod wait_notify_on_thread;

/// The rules of batch 7, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Suspicious logic and typos
        Box::new(loop_counter_wrong_direction::LoopCounterWrongDirection),
        Box::new(post_increment_self_assignment::PostIncrementSelfAssignment),
        Box::new(reversed_compound_assignment::ReversedCompoundAssignment),
        Box::new(impossible_size_check::ImpossibleSizeCheck),
        Box::new(list_remove_in_index_loop::ListRemoveInIndexLoop),
        Box::new(collection_passed_to_itself::CollectionPassedToItself),
        Box::new(accessor_uses_wrong_field::AccessorUsesWrongField::default()),
        Box::new(object_method_name_typo::ObjectMethodNameTypo),
        // Numbers and bits
        Box::new(shift_out_of_range::ShiftOutOfRange),
        Box::new(int_constant_overflow::IntConstantOverflow),
        Box::new(byte_sign_extension::ByteSignExtension::default()),
        Box::new(abs_of_hash_or_random::AbsOfHashOrRandom),
        Box::new(random_cast_to_int::RandomCastToInt),
        Box::new(hex_string_without_padding::HexStringWithoutPadding),
        // API misuse
        Box::new(untyped_to_array_cast::UntypedToArrayCast),
        Box::new(separator_used_as_regex::SeparatorUsedAsRegex),
        Box::new(invalid_range_arguments::InvalidRangeArguments),
        Box::new(date_field_out_of_range::DateFieldOutOfRange::default()),
        Box::new(jdbc_zero_index::JdbcZeroIndex::default()),
        Box::new(has_next_calls_next::HasNextCallsNext),
        Box::new(tostring_or_clone_returns_null::TostringOrCloneReturnsNull),
        Box::new(serialization_hook_signature::SerializationHookSignature),
        Box::new(unretained_annotation_lookup::UnretainedAnnotationLookup::default()),
        Box::new(assert_with_side_effect::AssertWithSideEffect),
        // Concurrency
        Box::new(sleep_in_synchronized::SleepInSynchronized),
        Box::new(intrinsic_lock_on_lock_object::IntrinsicLockOnLockObject::default()),
        Box::new(wait_notify_on_thread::WaitNotifyOnThread),
        Box::new(scheduled_pool_without_core_threads::ScheduledPoolWithoutCoreThreads::default()),
        Box::new(synchronized_on_get_class::SynchronizedOnGetClass),
        Box::new(synchronized_on_reassigned_field::SynchronizedOnReassignedField::default()),
        // Tests
        Box::new(self_comparing_assertion::SelfComparingAssertion),
        Box::new(incomplete_fluent_assertion::IncompleteFluentAssertion),
        Box::new(fail_swallowed_by_catch::FailSwallowedByCatch::default()),
    ]
}
