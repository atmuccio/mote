//! Rule batch 4: likely bugs (promises, regular expressions, redeclarations, useless assignments
//! and escapes, React keys and components, exclusive tests). The author of this batch adds rule
//! modules in this directory and lists them in [`rules`]; nothing outside `src/rules/batch_4/`
//! (except fixtures) needs to change. See `RULE_AUTHORING.md`.

mod exception_not_thrown;
mod extra_arguments;
mod floating_promise;
mod index_of_positive_check;
mod invalid_regexp;
mod jsx_no_array_index_key;
mod no_case_declarations;
mod no_duplicated_branches;
mod no_exclusive_tests;
mod no_nested_component_definition;
mod no_prototype_builtins;
mod no_redeclare;
mod no_shadow_restricted_names;
mod no_unassigned_vars;
mod no_undeclared_assignment;
mod no_unsafe_function_type;
mod no_useless_assignment;
mod no_useless_escape;
mod overwritten_before_read;
mod prefer_rest_params;
mod prefer_spread;
mod preserve_caught_error;
mod sort_without_comparator;
mod util;

use crate::JsRule;

/// The rules of batch 4, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(no_prototype_builtins::NoPrototypeBuiltins),
        Box::new(no_unsafe_function_type::NoUnsafeFunctionType),
        Box::new(no_useless_assignment::NoUselessAssignment),
        Box::new(no_useless_escape::NoUselessEscape),
        Box::new(preserve_caught_error::PreserveCaughtError),
        Box::new(no_case_declarations::NoCaseDeclarations),
        Box::new(prefer_rest_params::PreferRestParams),
        Box::new(prefer_spread::PreferSpread),
        Box::new(no_unassigned_vars::NoUnassignedVars),
        Box::new(exception_not_thrown::ExceptionNotThrown),
        Box::new(no_duplicated_branches::NoDuplicatedBranches),
        Box::new(sort_without_comparator::SortWithoutComparator),
        Box::new(invalid_regexp::InvalidRegexp),
        Box::new(index_of_positive_check::IndexOfPositiveCheck),
        Box::new(floating_promise::FloatingPromise),
        Box::new(no_redeclare::NoRedeclare),
        Box::new(extra_arguments::ExtraArguments),
        Box::new(no_undeclared_assignment::NoUndeclaredAssignment),
        Box::new(no_shadow_restricted_names::NoShadowRestrictedNames),
        Box::new(jsx_no_array_index_key::JsxNoArrayIndexKey),
        Box::new(no_nested_component_definition::NoNestedComponentDefinition),
        Box::new(no_exclusive_tests::NoExclusiveTests),
        Box::new(overwritten_before_read::OverwrittenBeforeRead),
    ]
}
