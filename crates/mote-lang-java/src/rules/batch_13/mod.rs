//! Rule batch 13: maintainability smells (no-op code, JDK API misuse, regular expressions,
//! redundant declarations). The author of this batch adds rule modules in this directory and lists
//! them in [`rules`]; nothing outside `src/rules/batch_13/` (except fixtures and the registration
//! in `rules/mod.rs`) needs to change. See `RULE_AUTHORING.md`.

// The Java regular-expression parser of batch 9 (`rules/shared`).
use super::shared::regex_syntax;
use crate::rule::BoxedRule;

mod util;

mod arithmetic_no_op;
mod confusing_member_name;
mod default_package;
mod explicit_gc;
mod explicit_varargs_array;
mod internal_jdk_api;
mod jdbc_statement_in_loop;
mod lone_block;
mod manual_array_copy;
mod mutable_exception_field;
mod nested_try;
mod nullability_annotation_misuse;
mod prefer_enum_map;
mod redundant_close_in_try_with_resources;
mod redundant_record_member;
mod regex_too_complex;
mod thread_passed_as_runnable;
mod unused_type_parameter;
mod useless_override;

/// The rules of batch 13, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        Box::new(nullability_annotation_misuse::NullabilityAnnotationMisuse::default()),
        Box::new(mutable_exception_field::MutableExceptionField),
        Box::new(thread_passed_as_runnable::ThreadPassedAsRunnable::default()),
        Box::new(unused_type_parameter::UnusedTypeParameter),
        Box::new(arithmetic_no_op::ArithmeticNoOp),
        Box::new(explicit_varargs_array::ExplicitVarargsArray::default()),
        Box::new(redundant_close_in_try_with_resources::RedundantCloseInTryWithResources),
        Box::new(lone_block::LoneBlock),
        Box::new(useless_override::UselessOverride),
        Box::new(redundant_record_member::RedundantRecordMember),
        Box::new(regex_too_complex::RegexTooComplex),
        Box::new(nested_try::NestedTry),
        Box::new(default_package::DefaultPackage::default()),
        Box::new(confusing_member_name::ConfusingMemberName),
        Box::new(prefer_enum_map::PreferEnumMap::default()),
        Box::new(explicit_gc::ExplicitGc),
        Box::new(manual_array_copy::ManualArrayCopy),
        Box::new(jdbc_statement_in_loop::JdbcStatementInLoop::default()),
        Box::new(internal_jdk_api::InternalJdkApi::default()),
    ]
}
