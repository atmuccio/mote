//! Rule batch 11: maintainability smells (exception handling and logging, redundant code and casts,
//! raw types, streams, tests). The author of this batch adds rule modules in this directory and
//! lists them in [`rules`]; nothing outside `src/rules/batch_11/` (except fixtures and the
//! registration in `rules/mod.rs`) needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod anonymous_class_could_be_lambda;
mod case_insensitive_equals;
mod catch_programming_error;
mod exception_logged_without_trace;
mod for_loop_counter_misuse;
mod hidden_declaration;
mod identical_methods;
mod invisible_character;
mod keyset_lookup_in_loop;
mod log_and_rethrow;
mod logger_for_wrong_class;
mod nondeterministic_test;
mod package_directory_mismatch;
mod raw_type;
mod redundant_cast;
mod regex_compiled_per_call;
mod simplifiable_stream;
mod static_access_through_instance;
mod stream_missing_bulk_method;

/// The rules of batch 11, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Error handling
        Box::new(log_and_rethrow::LogAndRethrow),
        Box::new(exception_logged_without_trace::ExceptionLoggedWithoutTrace),
        Box::new(catch_programming_error::CatchProgrammingError),
        Box::new(logger_for_wrong_class::LoggerForWrongClass),
        // Simpler or cheaper forms
        Box::new(case_insensitive_equals::CaseInsensitiveEquals),
        Box::new(keyset_lookup_in_loop::KeysetLookupInLoop),
        Box::new(redundant_cast::RedundantCast),
        Box::new(simplifiable_stream::SimplifiableStream),
        Box::new(anonymous_class_could_be_lambda::AnonymousClassCouldBeLambda),
        Box::new(regex_compiled_per_call::RegexCompiledPerCall::default()),
        Box::new(raw_type::RawType),
        Box::new(nondeterministic_test::NondeterministicTest::default()),
        // Structure
        Box::new(for_loop_counter_misuse::ForLoopCounterMisuse::default()),
        Box::new(identical_methods::IdenticalMethods::default()),
        Box::new(hidden_declaration::HiddenDeclaration::default()),
        Box::new(invisible_character::InvisibleCharacter),
        Box::new(static_access_through_instance::StaticAccessThroughInstance::default()),
        Box::new(package_directory_mismatch::PackageDirectoryMismatch),
        Box::new(stream_missing_bulk_method::StreamMissingBulkMethod),
    ]
}
