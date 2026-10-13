//! Grammar tables computed once per process: the tree-sitter language, a fast
//! `kind_id → name` table, and a dense classification of the kinds the framework itself
//! cares about (metrics, comments, statements, declarations).

use std::sync::LazyLock;
use tree_sitter::{Language, Node};

/// The Java grammar: tree-sitter-java-orchard, a maintained fork of tree-sitter-java that reads
/// Java 25 (module imports, flexible constructor bodies, unnamed variables).
pub(crate) static LANGUAGE: LazyLock<Language> =
    LazyLock::new(|| Language::new(tree_sitter_java_orchard::LANGUAGE));

/// `kind_id → kind name`, so `kind(node)` avoids the C-string conversion done by `Node::kind`.
static KIND_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let lang: &'static Language = &LANGUAGE;
    (0..lang.node_kind_count())
        .map(|id| lang.node_kind_for_id(id as u16).unwrap_or(""))
        .collect()
});

/// Every kind id (named or anonymous) whose name is `name`. Aliased grammar rules make one name
/// map to several ids (for example, reserved words re-used as `identifier`).
pub(crate) fn ids_for_kind(name: &str, named_only: bool) -> Vec<u16> {
    let lang: &Language = &LANGUAGE;
    KIND_NAMES
        .iter()
        .enumerate()
        .filter(|(id, n)| **n == name && (!named_only || lang.node_kind_is_named(*id as u16)))
        .map(|(id, _)| id as u16)
        .collect()
}

/// Number of kind ids in the grammar.
pub(crate) fn kind_count() -> usize {
    KIND_NAMES.len()
}

/// The node's kind name (`"if_statement"`, `"identifier"`, …). Same value as
/// [`Node::kind`] but a table lookup instead of a C-string conversion; prefer it in rules.
#[inline]
pub fn kind(node: Node<'_>) -> &'static str {
    KIND_NAMES
        .get(node.kind_id() as usize)
        .copied()
        .unwrap_or("")
}

/// Dense classification of the kinds the framework dispatches on internally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum K {
    Other,
    Program,
    LineComment,
    BlockComment,
    // declarations
    ClassDecl,
    InterfaceDecl,
    EnumDecl,
    RecordDecl,
    AnnotationTypeDecl,
    ClassBody,
    InterfaceBody,
    EnumBody,
    EnumBodyDecls,
    AnnotationTypeBody,
    MethodDecl,
    ConstructorDecl,
    CompactConstructorDecl,
    StaticInitializer,
    FieldDecl,
    ConstantDecl,
    LocalVarDecl,
    Modifiers,
    Annotation,
    MarkerAnnotation,
    ObjectCreation,
    EnumConstant,
    Lambda,
    // statements
    Block,
    ExpressionStatement,
    IfStatement,
    ForStatement,
    EnhancedForStatement,
    WhileStatement,
    DoStatement,
    SwitchExpression,
    SwitchBlock,
    SwitchLabel,
    ReturnStatement,
    BreakStatement,
    ContinueStatement,
    ThrowStatement,
    TryStatement,
    TryWithResourcesStatement,
    CatchClause,
    FinallyClause,
    SynchronizedStatement,
    LabeledStatement,
    AssertStatement,
    YieldStatement,
    ExplicitConstructorInvocation,
    ConstructorBody,
    SwitchBlockStatementGroup,
    // expressions
    TernaryExpression,
    BinaryExpression,
    ParenthesizedExpression,
    UnaryExpression,
    Identifier,
    MethodInvocation,
    FieldAccess,
    Resource,
    InstanceofExpression,
    TypePattern,
    RecordPatternComponent,
    SwitchRule,
    VariableDeclarator,
    CatchFormalParameter,
    CatchType,
    FormalParameter,
    SpreadParameter,
    // classified for rules (see `kinds_named_alike_are_all_classified`)
    AssignmentExpression,
    UpdateExpression,
    InferredParameters,
    StringLiteral,
    ArrayType,
}

impl K {
    pub(crate) fn is_type_decl(self) -> bool {
        matches!(
            self,
            K::ClassDecl | K::InterfaceDecl | K::EnumDecl | K::RecordDecl | K::AnnotationTypeDecl
        )
    }
    pub(crate) fn is_type_body(self) -> bool {
        matches!(
            self,
            K::ClassBody
                | K::InterfaceBody
                | K::EnumBody
                | K::EnumBodyDecls
                | K::AnnotationTypeBody
        )
    }
    /// Nodes that own a `FunctionMetric` (when not nested inside another function).
    pub(crate) fn is_function(self) -> bool {
        matches!(
            self,
            K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl | K::StaticInitializer
        )
    }
}

pub(crate) struct Tables {
    /// `kind_id → K`.
    pub(crate) by_id: Vec<K>,
    pub(crate) f_alternative: u16,
    pub(crate) f_consequence: u16,
    pub(crate) f_body: u16,
    pub(crate) f_operator: u16,
    pub(crate) f_left: u16,
    pub(crate) f_right: u16,
}

const CLASSIFIED: &[(&str, K)] = &[
    ("program", K::Program),
    ("line_comment", K::LineComment),
    ("block_comment", K::BlockComment),
    ("class_declaration", K::ClassDecl),
    ("interface_declaration", K::InterfaceDecl),
    ("enum_declaration", K::EnumDecl),
    ("record_declaration", K::RecordDecl),
    ("annotation_type_declaration", K::AnnotationTypeDecl),
    ("class_body", K::ClassBody),
    ("interface_body", K::InterfaceBody),
    ("enum_body", K::EnumBody),
    ("enum_body_declarations", K::EnumBodyDecls),
    ("annotation_type_body", K::AnnotationTypeBody),
    ("method_declaration", K::MethodDecl),
    ("constructor_declaration", K::ConstructorDecl),
    ("compact_constructor_declaration", K::CompactConstructorDecl),
    ("static_initializer", K::StaticInitializer),
    ("field_declaration", K::FieldDecl),
    ("constant_declaration", K::ConstantDecl),
    ("local_variable_declaration", K::LocalVarDecl),
    ("modifiers", K::Modifiers),
    ("annotation", K::Annotation),
    ("marker_annotation", K::MarkerAnnotation),
    ("object_creation_expression", K::ObjectCreation),
    ("enum_constant", K::EnumConstant),
    ("lambda_expression", K::Lambda),
    ("block", K::Block),
    ("expression_statement", K::ExpressionStatement),
    ("if_statement", K::IfStatement),
    ("for_statement", K::ForStatement),
    ("enhanced_for_statement", K::EnhancedForStatement),
    ("while_statement", K::WhileStatement),
    ("do_statement", K::DoStatement),
    ("switch_expression", K::SwitchExpression),
    ("switch_block", K::SwitchBlock),
    ("switch_label", K::SwitchLabel),
    ("return_statement", K::ReturnStatement),
    ("break_statement", K::BreakStatement),
    ("continue_statement", K::ContinueStatement),
    ("throw_statement", K::ThrowStatement),
    ("try_statement", K::TryStatement),
    ("try_with_resources_statement", K::TryWithResourcesStatement),
    ("catch_clause", K::CatchClause),
    ("finally_clause", K::FinallyClause),
    ("synchronized_statement", K::SynchronizedStatement),
    ("labeled_statement", K::LabeledStatement),
    ("assert_statement", K::AssertStatement),
    ("yield_statement", K::YieldStatement),
    (
        "explicit_constructor_invocation",
        K::ExplicitConstructorInvocation,
    ),
    ("constructor_body", K::ConstructorBody),
    ("switch_block_statement_group", K::SwitchBlockStatementGroup),
    ("ternary_expression", K::TernaryExpression),
    ("binary_expression", K::BinaryExpression),
    ("parenthesized_expression", K::ParenthesizedExpression),
    ("unary_expression", K::UnaryExpression),
    ("identifier", K::Identifier),
    ("method_invocation", K::MethodInvocation),
    ("field_access", K::FieldAccess),
    ("resource", K::Resource),
    ("instanceof_expression", K::InstanceofExpression),
    ("type_pattern", K::TypePattern),
    ("record_pattern_component", K::RecordPatternComponent),
    ("switch_rule", K::SwitchRule),
    ("variable_declarator", K::VariableDeclarator),
    ("catch_formal_parameter", K::CatchFormalParameter),
    ("catch_type", K::CatchType),
    ("formal_parameter", K::FormalParameter),
    ("spread_parameter", K::SpreadParameter),
    ("assignment_expression", K::AssignmentExpression),
    ("update_expression", K::UpdateExpression),
    ("inferred_parameters", K::InferredParameters),
    ("string_literal", K::StringLiteral),
    ("array_type", K::ArrayType),
];

pub(crate) static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let pairs = CLASSIFIED;
    let mut by_id = vec![K::Other; kind_count()];
    for (name, k) in pairs {
        for id in ids_for_kind(name, true) {
            if let Some(slot) = by_id.get_mut(id as usize) {
                *slot = *k;
            }
        }
    }
    let lang: &Language = &LANGUAGE;
    let field = |n: &str| {
        lang.field_id_for_name(n)
            .map(|f| f.get())
            .unwrap_or(u16::MAX)
    };
    Tables {
        by_id,
        f_alternative: field("alternative"),
        f_consequence: field("consequence"),
        f_body: field("body"),
        f_operator: field("operator"),
        f_left: field("left"),
        f_right: field("right"),
    }
});

/// Grammar field ids in [`crate::syntax::Field`] order (0 for a name the grammar lacks, which
/// `child_by_field_id` treats as "no such field", like `child_by_field_name` does).
pub(crate) static FIELD_IDS: LazyLock<[u16; crate::syntax::FIELD_COUNT]> = LazyLock::new(|| {
    let lang: &Language = &LANGUAGE;
    crate::syntax::FIELD_NAMES.map(|n| lang.field_id_for_name(n).map_or(0, |f| f.get()))
});

/// Classify a node (anything unknown, including `ERROR`, is [`K::Other`]).
#[inline]
pub(crate) fn k(node: Node<'_>) -> K {
    TABLES
        .by_id
        .get(node.kind_id() as usize)
        .copied()
        .unwrap_or(K::Other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_classified_kind_exists() {
        for (name, _) in CLASSIFIED {
            assert!(!ids_for_kind(name, true).is_empty(), "unknown kind {name}");
        }
        let t = &*TABLES;
        assert_ne!(t.f_alternative, u16::MAX);
        assert_ne!(t.f_consequence, u16::MAX);
        assert_ne!(t.f_body, u16::MAX);
        assert_ne!(t.f_operator, u16::MAX);
        assert_ne!(t.f_left, u16::MAX);
        assert_ne!(t.f_right, u16::MAX);
    }

    /// `k(n) == K::X` may replace `kind(n) == "x"` only if no anonymous kind is named `x`
    /// (`k` classifies named kinds only).
    #[test]
    fn kinds_named_alike_are_all_classified() {
        for (name, _) in CLASSIFIED {
            assert_eq!(
                ids_for_kind(name, false),
                ids_for_kind(name, true),
                "an anonymous kind is named {name}"
            );
        }
    }

    #[test]
    fn every_field_exists() {
        for (name, id) in crate::syntax::FIELD_NAMES.iter().zip(FIELD_IDS.iter()) {
            assert_ne!(*id, 0, "unknown field {name}");
        }
    }

    #[test]
    fn identifier_has_aliases() {
        assert!(!ids_for_kind("identifier", true).is_empty());
        assert!(ids_for_kind("no_such_kind", true).is_empty());
    }
}
