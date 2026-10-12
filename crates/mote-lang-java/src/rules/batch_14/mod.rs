//! Rule batch 14: maintainability smells (streams, loops, tests, regular expressions,
//! declarations). The author of this batch adds rule modules in this directory and lists them in
//! [`rules`]; nothing outside `src/rules/batch_14/` (except fixtures and the registration in
//! `rules/mod.rs`) needs to change. See `RULE_AUTHORING.md`.

use super::shared::{regex_call, regex_syntax, util};
use crate::rule::BoxedRule;

mod common;

mod application_error_subclass;
mod arrays_as_list_stream;
mod class_literal_is_instance;
mod empty_source_file;
mod foreach_element_cast;
mod hard_coded_absolute_path;
mod linear_access_in_loop;
mod nested_code_too_long;
mod peek_side_effect;
mod redundant_inheritance_declaration;
mod redundant_regex_syntax;
mod restricted_identifier;
mod substring_then_search;
mod test_could_be_parameterized;
mod too_many_assertions;
mod unclear_operator_precedence;
mod wildcard_return_type;

/// The rules of batch 14, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Regular expressions
        Box::new(redundant_regex_syntax::RedundantRegexSyntax),
        // Streams and collections
        Box::new(peek_side_effect::PeekSideEffect),
        Box::new(arrays_as_list_stream::ArraysAsListStream),
        Box::new(linear_access_in_loop::LinearAccessInLoop::default()),
        Box::new(substring_then_search::SubstringThenSearch),
        // Portability
        Box::new(hard_coded_absolute_path::HardCodedAbsolutePath),
        // Tests
        Box::new(test_could_be_parameterized::TestCouldBeParameterized::default()),
        Box::new(too_many_assertions::TooManyAssertions::default()),
        // Readability
        Box::new(unclear_operator_precedence::UnclearOperatorPrecedence),
        Box::new(class_literal_is_instance::ClassLiteralIsInstance),
        Box::new(foreach_element_cast::ForeachElementCast),
        Box::new(nested_code_too_long::NestedCodeTooLong),
        Box::new(wildcard_return_type::WildcardReturnType::default()),
        // Types and files
        Box::new(application_error_subclass::ApplicationErrorSubclass),
        Box::new(empty_source_file::EmptySourceFile),
        Box::new(redundant_inheritance_declaration::RedundantInheritanceDeclaration::default()),
        // Portability across Java releases
        Box::new(restricted_identifier::RestrictedIdentifier),
    ]
}
