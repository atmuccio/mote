//! Rule batch 5: redundant code, language idioms, class design, naming and test assertions. The
//! author of this batch adds rule modules in this directory and lists them in [`rules`]; nothing
//! outside `src/rules/batch_5/` (except fixtures) needs to change. See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod abstract_class_without_methods;
mod assert_argument_order;
mod assert_throws_multiple_calls;
mod assignment_in_operand;
mod case_conversion_without_locale;
mod collection_return_null;
mod concrete_collection_type;
mod constants_only_interface;
mod double_brace_initialization;
mod field_could_be_local;
mod final_field_could_be_static;
mod identical_catch_branches;
mod immediately_returned_variable;
mod index_loop_could_be_foreach;
mod instanceof_in_catch;
mod junit5_test_visibility;
mod lambda_could_be_method_reference;
mod literal_first_in_equals;
mod log_message_concatenation;
mod presized_to_array;
mod private_constructor_class_not_final;
mod protected_in_final_class;
mod redundant_annotation_value;
mod redundant_default_constructor;
mod redundant_qualified_this;
mod simplifiable_ternary;
mod standard_charset_lookup;
mod string_builder_field;
mod type_parameter_naming;
mod unnecessary_fully_qualified_name;
mod unnecessary_return;
mod unused_private_method_parameter;

/// The rules of batch 5, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Tests
        Box::new(assert_argument_order::AssertArgumentOrder::default()),
        Box::new(junit5_test_visibility::Junit5TestVisibility::default()),
        Box::new(assert_throws_multiple_calls::AssertThrowsMultipleCalls),
        // Logging & API
        Box::new(log_message_concatenation::LogMessageConcatenation),
        Box::new(case_conversion_without_locale::CaseConversionWithoutLocale),
        Box::new(standard_charset_lookup::StandardCharsetLookup),
        Box::new(presized_to_array::PresizedToArray),
        Box::new(collection_return_null::CollectionReturnNull),
        // Design
        Box::new(concrete_collection_type::ConcreteCollectionType::default()),
        Box::new(private_constructor_class_not_final::PrivateConstructorClassNotFinal::default()),
        Box::new(final_field_could_be_static::FinalFieldCouldBeStatic),
        Box::new(constants_only_interface::ConstantsOnlyInterface),
        Box::new(abstract_class_without_methods::AbstractClassWithoutMethods),
        Box::new(protected_in_final_class::ProtectedInFinalClass),
        Box::new(string_builder_field::StringBuilderField),
        Box::new(double_brace_initialization::DoubleBraceInitialization),
        Box::new(field_could_be_local::FieldCouldBeLocal::default()),
        Box::new(type_parameter_naming::TypeParameterNaming),
        Box::new(unused_private_method_parameter::UnusedPrivateMethodParameter),
        // Redundancy & idioms
        Box::new(unnecessary_fully_qualified_name::UnnecessaryFullyQualifiedName::default()),
        Box::new(unnecessary_return::UnnecessaryReturn),
        Box::new(literal_first_in_equals::LiteralFirstInEquals),
        Box::new(redundant_annotation_value::RedundantAnnotationValue),
        Box::new(redundant_default_constructor::RedundantDefaultConstructor),
        Box::new(index_loop_could_be_foreach::IndexLoopCouldBeForeach),
        Box::new(identical_catch_branches::IdenticalCatchBranches),
        Box::new(instanceof_in_catch::InstanceofInCatch),
        Box::new(redundant_qualified_this::RedundantQualifiedThis),
        Box::new(simplifiable_ternary::SimplifiableTernary),
        Box::new(immediately_returned_variable::ImmediatelyReturnedVariable),
        Box::new(lambda_could_be_method_reference::LambdaCouldBeMethodReference),
        Box::new(assignment_in_operand::AssignmentInOperand),
    ]
}
