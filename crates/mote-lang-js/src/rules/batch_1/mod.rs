//! Rule batch 1: likely bugs. See `RULE_AUTHORING.md` at the crate root.
//!
//! Shared private helpers (side-effect-free structural equality, statement exit analysis) live in
//! [`util`].

use crate::JsRule;

mod array_callback_return;
mod for_direction;
mod getter_return;
mod no_async_promise_executor;
mod no_compare_neg_zero;
mod no_cond_assign;
mod no_constant_condition;
mod no_dupe_class_members;
mod no_dupe_else_if;
mod no_duplicate_case;
mod no_empty_pattern;
mod no_ex_assign;
mod no_fallthrough;
mod no_func_assign;
mod no_loss_of_precision;
mod no_self_assign;
mod no_self_compare;
mod no_sparse_arrays;
mod no_template_curly_in_string;
mod no_unreachable;
mod no_unsafe_finally;
mod no_unsafe_negation;
mod no_unused_expressions;
mod use_isnan;
mod util;
mod valid_typeof;

/// Every rule of this batch, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(no_self_assign::NoSelfAssign),
        Box::new(no_self_compare::NoSelfCompare),
        Box::new(no_dupe_else_if::NoDupeElseIf),
        Box::new(no_duplicate_case::NoDuplicateCase),
        Box::new(no_dupe_class_members::NoDupeClassMembers),
        Box::new(no_unreachable::NoUnreachable),
        Box::new(no_unsafe_finally::NoUnsafeFinally),
        Box::new(no_fallthrough::NoFallthrough),
        Box::new(use_isnan::UseIsnan),
        Box::new(valid_typeof::ValidTypeof),
        Box::new(no_cond_assign::NoCondAssign),
        Box::new(no_constant_condition::NoConstantCondition),
        Box::new(no_sparse_arrays::NoSparseArrays),
        Box::new(no_compare_neg_zero::NoCompareNegZero),
        Box::new(no_unsafe_negation::NoUnsafeNegation),
        Box::new(for_direction::ForDirection),
        Box::new(no_async_promise_executor::NoAsyncPromiseExecutor),
        Box::new(no_loss_of_precision::NoLossOfPrecision),
        Box::new(array_callback_return::ArrayCallbackReturn),
        Box::new(no_template_curly_in_string::NoTemplateCurlyInString),
        Box::new(getter_return::GetterReturn),
        Box::new(no_empty_pattern::NoEmptyPattern),
        Box::new(no_func_assign::NoFuncAssign),
        Box::new(no_ex_assign::NoExAssign),
        Box::new(no_unused_expressions::NoUnusedExpressions),
    ]
}
