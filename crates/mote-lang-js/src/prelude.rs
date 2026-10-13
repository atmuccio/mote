//! Everything a rule module needs: `use crate::prelude::*;`
//!
//! Brings in the rule contract ([`JsRule`], [`Ctx`]), rule metadata types, the whole Oxc AST
//! (`oxc_ast::ast::*`, which includes the operator enums, `Span`, and `SourceType`), `AstKind` /
//! `AstType`, semantic ids and flags, `GetSpan`, `Cow`, `FxHashMap`/`FxHashSet`, and the [`helpers`]
//! module.

pub use crate::ctx::{AsNodeId, Ctx, Emitted};
pub use crate::helpers;
pub use crate::rule::JsRule;
pub use mote_core::{
    fix, Edit, ParamDefault, ParamSpec, Quality, RuleKind, RuleMeta, RuleStatus, Severity,
};
pub use oxc_ast::ast::*;
pub use oxc_ast::{AstKind, AstType};
pub use oxc_semantic::{
    AstNode, NodeId, Reference, ReferenceFlags, ScopeFlags, ScopeId, SymbolFlags, SymbolId,
};
pub use oxc_span::GetSpan;
pub use rustc_hash::{FxHashMap, FxHashSet};
pub use std::borrow::Cow;
