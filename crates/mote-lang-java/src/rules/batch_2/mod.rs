//! Rule batch 2 — design, naming, unused code. The author of this batch adds rule modules in
//! this directory and lists them in [`rules`]; nothing outside `src/rules/batch_2/` (except
//! fixtures) needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod class_naming;
mod collapsible_if;
mod constant_naming;
mod duplicate_string_literal;
mod else_after_return;
mod empty_constructor;
mod empty_method;
mod field_naming;
mod local_variable_naming;
mod long_literal_lowercase;
mod magic_number;
mod method_naming;
mod missing_override;
mod nested_ternary;
mod package_naming;
mod parameter_reassignment;
mod public_instance_field;
mod public_static_mutable_field;
mod simplifiable_boolean_return;
mod system_exit;
mod system_out_print;
mod too_many_fields;
mod too_many_methods;
mod unused_local_variable;
mod unused_private_class;
mod unused_private_method;
mod util;
mod utility_class_constructor;

/// The rules of batch 2, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // naming
        Box::new(class_naming::ClassNaming::default()),
        Box::new(method_naming::MethodNaming::default()),
        Box::new(constant_naming::ConstantNaming::default()),
        Box::new(field_naming::FieldNaming::default()),
        Box::new(local_variable_naming::LocalVariableNaming::default()),
        Box::new(package_naming::PackageNaming::default()),
        // unused code
        Box::new(unused_local_variable::UnusedLocalVariable::default()),
        Box::new(unused_private_method::UnusedPrivateMethod::default()),
        Box::new(unused_private_class::UnusedPrivateClass::default()),
        // design
        Box::new(utility_class_constructor::UtilityClassConstructor),
        Box::new(public_static_mutable_field::PublicStaticMutableField),
        Box::new(public_instance_field::PublicInstanceField),
        Box::new(magic_number::MagicNumber::default()),
        Box::new(duplicate_string_literal::DuplicateStringLiteral::default()),
        Box::new(parameter_reassignment::ParameterReassignment::default()),
        Box::new(collapsible_if::CollapsibleIf),
        Box::new(else_after_return::ElseAfterReturn),
        Box::new(simplifiable_boolean_return::SimplifiableBooleanReturn),
        Box::new(empty_method::EmptyMethod),
        Box::new(empty_constructor::EmptyConstructor),
        Box::new(missing_override::MissingOverride::default()),
        Box::new(too_many_methods::TooManyMethods),
        Box::new(too_many_fields::TooManyFields),
        Box::new(nested_ternary::NestedTernary),
        Box::new(system_out_print::SystemOutPrint::default()),
        Box::new(system_exit::SystemExit),
        Box::new(long_literal_lowercase::LongLiteralLowercase),
    ]
}
