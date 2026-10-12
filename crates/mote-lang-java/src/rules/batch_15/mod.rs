//! Rule batch 15: the off-by-default `conventions` style set. Every rule here is incubating, off by
//! default, and carries the tag `conventions`, which adds it to the `conventions` profile
//! (`extends: conventions` in `.mote.yaml`). See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

mod util;

mod array_brackets_on_name;
mod chained_assignment;
mod colon_switch_expression;
mod dangling_doc_comment;
mod declaration_far_from_use;
mod file_name_mismatch;
mod javadoc_empty_tag;
mod member_order;
mod missing_braces;
mod missing_javadoc;
mod modifier_order;
mod multiple_top_level_types;
mod numeric_escape_for_named_char;
mod one_variable_per_declaration;
mod overloads_not_adjacent;
mod redundant_parentheses;
mod split_declaration;
mod test_class_naming;
mod wildcard_import;

/// The rules of batch 15, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Statements and expressions
        Box::new(missing_braces::MissingBraces),
        Box::new(chained_assignment::ChainedAssignment),
        Box::new(redundant_parentheses::RedundantParentheses::default()),
        Box::new(colon_switch_expression::ColonSwitchExpression),
        Box::new(numeric_escape_for_named_char::NumericEscapeForNamedChar),
        // Declarations
        Box::new(one_variable_per_declaration::OneVariablePerDeclaration),
        Box::new(array_brackets_on_name::ArrayBracketsOnName),
        Box::new(modifier_order::ModifierOrder),
        Box::new(split_declaration::SplitDeclaration),
        Box::new(declaration_far_from_use::DeclarationFarFromUse),
        // Files, imports and type layout
        Box::new(wildcard_import::WildcardImport),
        Box::new(multiple_top_level_types::MultipleTopLevelTypes),
        Box::new(file_name_mismatch::FileNameMismatch),
        Box::new(overloads_not_adjacent::OverloadsNotAdjacent),
        Box::new(member_order::MemberOrder),
        // Naming
        Box::new(test_class_naming::TestClassNaming::default()),
        // Documentation
        Box::new(dangling_doc_comment::DanglingDocComment),
        Box::new(javadoc_empty_tag::JavadocEmptyTag),
        Box::new(missing_javadoc::MissingJavadoc::default()),
    ]
}
