//! Rule batch 2 — maintainability. Owned by one rule author; add rule modules here and list them
//! in [`rules`]. See `RULE_AUTHORING.md` at the crate root.

#[cfg(test)]
mod bench;
mod collapsible_if;
mod default_case;
mod no_alert;
mod no_array_constructor;
mod no_duplicate_string;
mod no_else_return;
mod no_empty_function;
mod no_eval;
mod no_extra_boolean_cast;
mod no_identical_functions;
mod no_lone_blocks;
mod no_lonely_if;
mod no_multi_assign;
mod no_nested_ternary;
mod no_new_wrappers;
mod no_param_reassign;
mod no_redundant_boolean;
mod no_return_assign;
mod no_throw_literal;
mod no_useless_concat;
mod no_useless_constructor;
mod no_useless_rename;
mod no_useless_return;
mod prefer_const;
mod prefer_template;
mod util;

use crate::JsRule;

/// Every rule of this batch, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(prefer_const::PreferConst),
        Box::new(no_nested_ternary::NoNestedTernary),
        Box::new(no_else_return::NoElseReturn),
        Box::new(no_lonely_if::NoLonelyIf),
        Box::new(collapsible_if::CollapsibleIf),
        Box::new(no_useless_return::NoUselessReturn),
        Box::new(no_param_reassign::NoParamReassign),
        Box::new(no_duplicate_string::NoDuplicateString),
        Box::new(no_identical_functions::NoIdenticalFunctions),
        Box::new(no_redundant_boolean::NoRedundantBoolean),
        Box::new(no_useless_concat::NoUselessConcat),
        Box::new(no_extra_boolean_cast::NoExtraBooleanCast),
        Box::new(no_multi_assign::NoMultiAssign),
        Box::new(no_new_wrappers::NoNewWrappers),
        Box::new(no_array_constructor::NoArrayConstructor),
        Box::new(no_throw_literal::NoThrowLiteral),
        Box::new(no_useless_constructor::NoUselessConstructor),
        Box::new(no_lone_blocks::NoLoneBlocks),
        Box::new(no_alert::NoAlert),
        Box::new(no_eval::NoEval),
        Box::new(no_empty_function::NoEmptyFunction),
        Box::new(prefer_template::PreferTemplate),
        Box::new(default_case::DefaultCase),
        Box::new(no_useless_rename::NoUselessRename),
        Box::new(no_return_assign::NoReturnAssign),
    ]
}
