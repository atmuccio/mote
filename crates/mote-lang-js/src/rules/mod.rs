//! Built-in rules.
//!
//! The exemplary rules in this directory show the three rule styles:
//!
//! * syntactic, one node at a time: [`no_debugger`], [`no_var`], [`eqeqeq`], [`no_empty`],
//!   [`no_console`], [`no_explicit_any`], [`no_dupe_keys`];
//! * semantic, over the symbol table after the traversal: [`no_unused_vars`].
//!
//! `batch_1` … `batch_3` are owned by parallel rule authors: each adds its rules to its own
//! module only.

pub mod batch_1;
pub mod batch_10;
pub mod batch_11;
pub mod batch_12;
pub mod batch_2;
pub mod batch_3;
pub mod batch_4;
pub mod batch_5;
pub mod batch_6;
pub mod batch_7;
pub mod batch_8;
pub mod batch_9;
pub mod eqeqeq;
pub mod no_console;
pub mod no_debugger;
pub mod no_dupe_keys;
pub mod no_empty;
pub mod no_explicit_any;
pub mod no_unused_vars;
pub mod no_var;

use crate::JsRule;

/// Every built-in rule, in a stable order.
pub fn all() -> Vec<Box<dyn JsRule>> {
    let mut rules: Vec<Box<dyn JsRule>> = vec![
        Box::new(no_unused_vars::NoUnusedVars),
        Box::new(eqeqeq::Eqeqeq),
        Box::new(no_debugger::NoDebugger),
        Box::new(no_empty::NoEmpty),
        Box::new(no_var::NoVar),
        Box::new(no_explicit_any::NoExplicitAny),
        Box::new(no_console::NoConsole),
        Box::new(no_dupe_keys::NoDupeKeys),
    ];
    rules.extend(batch_1::rules());
    rules.extend(batch_2::rules());
    rules.extend(batch_3::rules());
    rules.extend(batch_4::rules());
    rules.extend(batch_5::rules());
    rules.extend(batch_6::rules());
    rules.extend(batch_7::rules());
    rules.extend(batch_8::rules());
    rules.extend(batch_9::rules());
    rules.extend(batch_10::rules());
    rules.extend(batch_11::rules());
    rules.extend(batch_12::rules());
    rules
}
