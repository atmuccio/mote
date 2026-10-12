//! Rule batch 6: promises, React state and keys, ignored call results, imports and globals. The
//! author of this batch adds rule modules in this directory and lists them in [`rules`]; nothing
//! outside `src/rules/batch_6/` (except fixtures) needs to change. See `RULE_AUTHORING.md`.

mod block_scoped_var;
mod ignored_call_result;
mod incoming_value_overwritten;
mod invalid_replacement_reference;
mod jsx_no_unstable_key;
mod no_duplicate_imports;
mod no_global_assign;
mod no_loop_func;
mod no_misused_promises;
mod no_set_state_in_render;
mod not_callable;
mod prefer_array_some;
mod prefer_readonly;
mod redundant_nested_condition;
mod swapped_arguments;
mod unawaited_promise_in_try;
pub(crate) mod util;

use crate::JsRule;

/// The rules of batch 6, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(no_loop_func::NoLoopFunc),
        Box::new(no_set_state_in_render::NoSetStateInRender),
        Box::new(unawaited_promise_in_try::UnawaitedPromiseInTry),
        Box::new(no_duplicate_imports::NoDuplicateImports),
        Box::new(prefer_array_some::PreferArraySome),
        Box::new(prefer_readonly::PreferReadonly),
        Box::new(no_misused_promises::NoMisusedPromises),
        Box::new(not_callable::NotCallable),
        Box::new(jsx_no_unstable_key::JsxNoUnstableKey),
        Box::new(invalid_replacement_reference::InvalidReplacementReference),
        Box::new(incoming_value_overwritten::IncomingValueOverwritten),
        Box::new(redundant_nested_condition::RedundantNestedCondition),
        Box::new(swapped_arguments::SwappedArguments),
        Box::new(ignored_call_result::IgnoredCallResult),
        Box::new(no_global_assign::NoGlobalAssign),
        Box::new(block_scoped_var::BlockScopedVar),
    ]
}
