//! Rule batch 4: bug patterns (equality and hashing, ignored return values, regular expressions,
//! format strings, concurrency) and test methods that never run. The author of this batch adds rule
//! modules in this directory and lists them in [`rules`]; nothing outside `src/rules/batch_4/`
//! (except fixtures) needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod array_hash_code;
mod atomic_equals;
mod boxed_boolean_condition;
mod class_name_string_comparison;
mod compare_to_min_value;
mod equals_overload;
mod equals_without_type_check;
mod exception_not_thrown;
mod format_argument_mismatch;
mod ignored_failure_return;
mod ignored_return_value;
mod incompatible_collection_lookup;
mod index_of_positive_check;
mod invalid_regex_literal;
mod misleading_indentation;
mod non_atomic_volatile_update;
mod notify_instead_of_notify_all;
mod overwritten_before_read;
mod record_array_component;
mod regex;
mod replace_all_without_regex;
mod string_builder_char_constructor;
mod test_method_not_run;
mod url_in_hash_collection;
mod util;
mod wait_without_loop;
mod week_year_pattern;

/// The rules of batch 4, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // api misuse: formats, return values, regexes, dates
        Box::new(format_argument_mismatch::FormatArgumentMismatch::default()),
        Box::new(ignored_return_value::IgnoredReturnValue::default()),
        Box::new(ignored_failure_return::IgnoredFailureReturn::default()),
        Box::new(exception_not_thrown::ExceptionNotThrown),
        Box::new(incompatible_collection_lookup::IncompatibleCollectionLookup::default()),
        Box::new(week_year_pattern::WeekYearPattern::default()),
        Box::new(invalid_regex_literal::InvalidRegexLiteral::default()),
        Box::new(replace_all_without_regex::ReplaceAllWithoutRegex::default()),
        Box::new(index_of_positive_check::IndexOfPositiveCheck::default()),
        Box::new(string_builder_char_constructor::StringBuilderCharConstructor::default()),
        // equality and hashing
        Box::new(equals_overload::EqualsOverload),
        Box::new(equals_without_type_check::EqualsWithoutTypeCheck),
        Box::new(atomic_equals::AtomicEquals::default()),
        Box::new(class_name_string_comparison::ClassNameStringComparison::default()),
        Box::new(record_array_component::RecordArrayComponent),
        Box::new(compare_to_min_value::CompareToMinValue),
        Box::new(url_in_hash_collection::UrlInHashCollection::default()),
        Box::new(array_hash_code::ArrayHashCode),
        // concurrency
        Box::new(non_atomic_volatile_update::NonAtomicVolatileUpdate::default()),
        Box::new(wait_without_loop::WaitWithoutLoop),
        Box::new(notify_instead_of_notify_all::NotifyInsteadOfNotifyAll),
        // control flow and suspicious code
        Box::new(boxed_boolean_condition::BoxedBooleanCondition::default()),
        Box::new(overwritten_before_read::OverwrittenBeforeRead::default()),
        Box::new(misleading_indentation::MisleadingIndentation),
        // tests
        Box::new(test_method_not_run::TestMethodNotRun::default()),
    ]
}
