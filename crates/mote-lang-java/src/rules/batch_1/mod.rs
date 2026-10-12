//! Rule batch 1 — bugs and control flow. The author of this batch adds rule modules in this
//! directory and lists them in [`rules`]; nothing outside `src/rules/batch_1/` (except fixtures)
//! needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod array_to_string;
mod assignment_in_condition;
mod bigdecimal_from_double;
mod boolean_literal_comparison;
mod boxed_primitive_constructor;
mod compare_to_specific_value;
mod deprecated_thread_methods;
mod duplicate_condition;
mod empty_block;
mod empty_statement;
mod equals_with_null;
mod float_equality;
mod identical_branches;
mod identical_operands;
mod integer_division_in_floating_context;
mod missing_switch_default;
mod non_short_circuit_boolean;
mod optional_get_without_check;
mod optional_return_null;
mod redundant_null_check_before_instanceof;
mod redundant_tostring;
mod self_assignment;
mod string_concat_in_loop;
mod switch_fallthrough;
mod thread_run_direct;
mod unreachable_code;
mod value_reference_equality;

/// The rules of batch 1, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        Box::new(self_assignment::SelfAssignment),
        Box::new(identical_operands::IdenticalOperands),
        Box::new(duplicate_condition::DuplicateCondition),
        Box::new(identical_branches::IdenticalBranches),
        Box::new(assignment_in_condition::AssignmentInCondition),
        Box::new(empty_block::EmptyBlock),
        Box::new(empty_statement::EmptyStatement),
        Box::new(missing_switch_default::MissingSwitchDefault),
        Box::new(switch_fallthrough::SwitchFallthrough),
        Box::new(unreachable_code::UnreachableCode::default()),
        Box::new(boxed_primitive_constructor::BoxedPrimitiveConstructor),
        Box::new(bigdecimal_from_double::BigDecimalFromDouble),
        Box::new(float_equality::FloatEquality),
        Box::new(thread_run_direct::ThreadRunDirect),
        Box::new(string_concat_in_loop::StringConcatInLoop::default()),
        Box::new(boolean_literal_comparison::BooleanLiteralComparison),
        Box::new(integer_division_in_floating_context::IntegerDivisionInFloatingContext),
        Box::new(optional_get_without_check::OptionalGetWithoutCheck),
        Box::new(optional_return_null::OptionalReturnNull),
        Box::new(equals_with_null::EqualsWithNull),
        Box::new(compare_to_specific_value::CompareToSpecificValue),
        Box::new(non_short_circuit_boolean::NonShortCircuitBoolean),
        Box::new(deprecated_thread_methods::DeprecatedThreadMethods),
        Box::new(array_to_string::ArrayToString::default()),
        Box::new(redundant_tostring::RedundantToString),
        Box::new(redundant_null_check_before_instanceof::RedundantNullCheckBeforeInstanceof),
        Box::new(value_reference_equality::ValueReferenceEquality),
    ]
}
