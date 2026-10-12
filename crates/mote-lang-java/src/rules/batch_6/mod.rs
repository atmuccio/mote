//! Rule batch 6: constant conditions, numeric literals and escapes, loop counters and log-argument
//! guards. The author of this batch adds rule modules in this directory and lists them in
//! [`rules`]; nothing outside `src/rules/batch_6/` (except fixtures) needs to change. See
//! `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod constant_if_condition;
mod float_loop_counter;
mod multiple_unary_operators;
mod octal_literal;
mod suspicious_octal_escape;
mod unguarded_log_argument;
mod util;

/// The rules of batch 6, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        Box::new(unguarded_log_argument::UnguardedLogArgument::default()),
        Box::new(octal_literal::OctalLiteral::default()),
        Box::new(multiple_unary_operators::MultipleUnaryOperators),
        Box::new(constant_if_condition::ConstantIfCondition),
        Box::new(float_loop_counter::FloatLoopCounter),
        Box::new(suspicious_octal_escape::SuspiciousOctalEscape),
    ]
}
