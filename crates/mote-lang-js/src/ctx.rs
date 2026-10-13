//! [`Ctx`]: everything a rule can see and do for one file.

use crate::facts::Facts;
use mote_core::{
    LineIndex, Location, ParamDefault, ParamValue, RawFinding, RuleIndex, RuleMeta, TextRange,
};
use oxc_ast::ast::{BindingIdentifier, Expression, IdentifierReference};
use oxc_ast::{AstKind, AstType, Comment};
use oxc_semantic::{
    AstNode, AstNodes, NodeId, Reference, ReferenceFlags, Scoping, Semantic, SymbolFlags, SymbolId,
};
use oxc_span::{GetSpan, SourceType, Span};
use std::any::Any;

/// Per-file facts that do not depend on the rule being run.
pub(crate) struct FileInfo<'a> {
    pub path: &'a str,
    pub source: &'a str,
    pub is_test: bool,
    pub source_type: SourceType,
    pub lines: LineIndex<'a>,
}

/// Anything that identifies a node: [`NodeId`], [`AstNode`], or [`AstKind`] (by value or reference).
pub trait AsNodeId {
    fn as_node_id(&self) -> NodeId;
}

impl AsNodeId for NodeId {
    fn as_node_id(&self) -> NodeId {
        *self
    }
}

impl AsNodeId for AstNode<'_> {
    fn as_node_id(&self) -> NodeId {
        self.id()
    }
}

impl AsNodeId for AstKind<'_> {
    fn as_node_id(&self) -> NodeId {
        self.node_id()
    }
}

impl<T: AsNodeId + ?Sized> AsNodeId for &T {
    fn as_node_id(&self) -> NodeId {
        (**self).as_node_id()
    }
}

static NO_RULE: RuleMeta = RuleMeta::new("js/<none>", "js", "<none>");

/// The rule's view of one file. All borrowed data lives for `'a` (the file's analysis), so values
/// returned by accessors can be held while calling [`Ctx::emit`].
pub struct Ctx<'a> {
    semantic: &'a Semantic<'a>,
    file: &'a FileInfo<'a>,
    facts: &'a Facts,
    /// The enabled rules and their parameters, by registry index (`rule_index`).
    rules: &'a RuleIndex,
    /// Per rule, by registry index: values computed once per rule set ([`Ctx::rule_memo`]).
    memos: &'a [std::sync::OnceLock<crate::rule::RuleMemo>],
    meta: &'static RuleMeta,
    rule_index: usize,
    pub(crate) findings: Vec<RawFinding>,
    states: Vec<Option<Box<dyn Any>>>,
    /// The spans of the file's ambient declarations, merged and in order (see
    /// [`Ctx::in_ambient_context`]), built on first use.
    ambient: std::cell::OnceCell<Vec<(u32, u32)>>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(
        semantic: &'a Semantic<'a>,
        file: &'a FileInfo<'a>,
        facts: &'a Facts,
        rules: &'a RuleIndex,
        memos: &'a [std::sync::OnceLock<crate::rule::RuleMemo>],
    ) -> Self {
        Ctx {
            semantic,
            file,
            facts,
            rules,
            memos,
            meta: &NO_RULE,
            rule_index: 0,
            findings: Vec::new(),
            states: Vec::new(),
            ambient: std::cell::OnceCell::new(),
        }
    }

    #[inline]
    pub(crate) fn select(&mut self, index: usize, meta: &'static RuleMeta) {
        self.rule_index = index;
        self.meta = meta;
    }

    // ----------------------------------------------------------------------------------------
    // File
    // ----------------------------------------------------------------------------------------

    /// Repo-relative path with `/` separators.
    pub fn path(&self) -> &'a str {
        self.file.path
    }

    /// Whole source text.
    pub fn source(&self) -> &'a str {
        self.file.source
    }

    /// Source text of `span` (empty if the span is out of range).
    pub fn text(&self, span: Span) -> &'a str {
        self.file
            .source
            .get(span.start as usize..span.end as usize)
            .unwrap_or("")
    }

    /// The file matched a test glob (`*.spec.*`, `__tests__/`, …).
    pub fn is_test(&self) -> bool {
        self.file.is_test
    }

    /// `.ts`, `.tsx`, `.mts`, `.cts`.
    pub fn is_typescript(&self) -> bool {
        self.file.source_type.is_typescript()
    }

    /// JSX syntax is enabled (`.jsx`, `.tsx`, and all plain JavaScript extensions).
    pub fn is_jsx(&self) -> bool {
        self.file.source_type.is_jsx()
    }

    /// An ES module (has `import`/`export`, or is `.mjs`/`.mts`). Script files share one global
    /// scope with other scripts.
    pub fn is_module(&self) -> bool {
        self.semantic.source_type().is_module()
    }

    /// Oxc's source type as parsed.
    pub fn source_type(&self) -> SourceType {
        *self.semantic.source_type()
    }

    /// Does the file contain at least one node of this type? Computed before any rule runs.
    pub fn file_contains(&self, ty: AstType) -> bool {
        self.facts.contains(ty)
    }

    // ----------------------------------------------------------------------------------------
    // Semantic model
    // ----------------------------------------------------------------------------------------

    pub fn semantic(&self) -> &'a Semantic<'a> {
        self.semantic
    }

    /// Flat node list (pre-order) with parent links.
    pub fn nodes(&self) -> &'a AstNodes<'a> {
        self.semantic.nodes()
    }

    /// Scopes, symbols, and references.
    pub fn scoping(&self) -> &'a Scoping {
        self.semantic.scoping()
    }

    /// All comments of the file, sorted by position.
    pub fn comments(&self) -> &'a [Comment] {
        self.semantic.comments()
    }

    /// Is there a comment inside `span`?
    pub fn has_comments_in(&self, span: Span) -> bool {
        self.semantic.has_comments_between(span)
    }

    // ----------------------------------------------------------------------------------------
    // Tree navigation
    // ----------------------------------------------------------------------------------------

    pub fn node(&self, id: NodeId) -> &'a AstNode<'a> {
        self.semantic.nodes().get_node(id)
    }

    /// Parent node; `None` for the `Program`.
    pub fn parent(&self, node: impl AsNodeId) -> Option<&'a AstNode<'a>> {
        let id = node.as_node_id();
        if id == NodeId::ROOT {
            return None;
        }
        Some(self.nodes().parent_node(id))
    }

    /// Kind of the parent node; `None` for the `Program`.
    pub fn parent_kind(&self, node: impl AsNodeId) -> Option<AstKind<'a>> {
        self.parent(node).map(AstNode::kind)
    }

    /// First ancestor that is not a transparent expression wrapper (`x as T`, `x satisfies T`,
    /// `x!`, `<T>x`, `a?.b` chain root). Use this to ask "how is this expression used?".
    pub fn outer_parent(&self, node: impl AsNodeId) -> Option<&'a AstNode<'a>> {
        self.ancestors(node).find(|n| {
            !matches!(
                n.kind(),
                AstKind::TSAsExpression(_)
                    | AstKind::TSSatisfiesExpression(_)
                    | AstKind::TSNonNullExpression(_)
                    | AstKind::TSTypeAssertion(_)
                    | AstKind::TSInstantiationExpression(_)
                    | AstKind::ParenthesizedExpression(_)
                    | AstKind::ChainExpression(_)
            )
        })
    }

    /// Ancestors from the parent up to (and including) the `Program`.
    pub fn ancestors(
        &self,
        node: impl AsNodeId,
    ) -> impl Iterator<Item = &'a AstNode<'a>> + Clone + 'a {
        let nodes = self.nodes();
        nodes
            .ancestor_ids(node.as_node_id())
            .map(move |id| nodes.get_node(id))
    }

    /// Nearest enclosing function or arrow function (methods are `Function` nodes).
    pub fn enclosing_function(&self, node: impl AsNodeId) -> Option<&'a AstNode<'a>> {
        self.ancestors(node).find(|n| n.kind().is_function_like())
    }

    /// Is the node inside any function body (as opposed to module/script top level)?
    pub fn is_in_function(&self, node: impl AsNodeId) -> bool {
        self.enclosing_function(node).is_some()
    }

    /// Does the node execute repeatedly as part of a loop of the *same* function (loop body, or a
    /// `for`/`while` test/update)?
    pub fn is_in_loop(&self, node: impl AsNodeId) -> bool {
        let mut child_span = self.node(node.as_node_id()).span();
        for ancestor in self.ancestors(node) {
            let kind = ancestor.kind();
            let repeats = match kind {
                AstKind::ForStatement(s) => {
                    s.body.span() == child_span
                        || s.test.as_ref().is_some_and(|t| t.span() == child_span)
                        || s.update.as_ref().is_some_and(|u| u.span() == child_span)
                }
                AstKind::ForInStatement(s) => s.body.span() == child_span,
                AstKind::ForOfStatement(s) => s.body.span() == child_span,
                AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) => true,
                _ => false,
            };
            if repeats {
                return true;
            }
            if is_function_boundary(kind) {
                return false;
            }
            child_span = kind.span();
        }
        false
    }

    /// Is the node inside a `finally` block of the same function?
    pub fn is_in_finally(&self, node: impl AsNodeId) -> bool {
        let mut child_span = self.node(node.as_node_id()).span();
        for ancestor in self.ancestors(node) {
            let kind = ancestor.kind();
            if let AstKind::TryStatement(t) = kind {
                if t.finalizer.as_ref().is_some_and(|f| f.span == child_span) {
                    return true;
                }
            }
            if is_function_boundary(kind) {
                return false;
            }
            child_span = kind.span();
        }
        false
    }

    /// Is the node inside a TypeScript ambient context (`declare module …`, `declare namespace …`,
    /// `declare global`)? Code there only describes types; most rules should stay silent.
    pub fn in_ambient_context(&self, node: impl AsNodeId) -> bool {
        // Most files declare no ambient module: no walk up per question (rules ask for every
        // node they report, and on deeply nested code each walk costs the whole depth).
        if !self.file_contains(AstType::TSModuleDeclaration)
            && !self.file_contains(AstType::TSGlobalDeclaration)
        {
            return false;
        }
        let is_ambient = |kind: AstKind<'_>| match kind {
            AstKind::TSModuleDeclaration(m) => m.declare,
            AstKind::TSGlobalDeclaration(_) => true,
            _ => false,
        };
        let node = self.node(node.as_node_id());
        if matches!(
            node.kind(),
            AstKind::TSModuleDeclaration(_) | AstKind::TSGlobalDeclaration(_)
        ) {
            // A declaration is not its own context.
            return self.ancestors(node).any(|n| is_ambient(n.kind()));
        }
        // Spans nest like the tree: a node is inside an ambient declaration when its span is.
        // (Walking up per question cost the whole depth, once per node of deeply nested code.)
        let spans = self.ambient.get_or_init(|| {
            let mut spans: Vec<(u32, u32)> = self
                .nodes()
                .iter()
                .filter(|n| is_ambient(n.kind()))
                .map(|n| (n.span().start, n.span().end))
                .collect();
            spans.sort_unstable();
            let mut merged: Vec<(u32, u32)> = Vec::with_capacity(spans.len());
            for (start, end) in spans {
                match merged.last_mut() {
                    Some(last) if start <= last.1 => last.1 = last.1.max(end),
                    _ => merged.push((start, end)),
                }
            }
            merged
        });
        let span = node.span();
        let k = spans.partition_point(|&(start, _)| start <= span.start);
        k > 0 && span.end <= spans[k - 1].1
    }

    /// Name of the enclosing function unit or class (`Class#method`, `outer.inner`, `obj.run`,
    /// `<anonymous>`) — the same `symbol` that [`Ctx::emit`] attaches automatically.
    pub fn symbol_of(&self, node: impl AsNodeId) -> Option<&'a str> {
        self.symbol_at(self.node(node.as_node_id()).span().start)
    }

    /// Like [`Ctx::symbol_of`] for a byte offset.
    pub fn symbol_at(&self, offset: u32) -> Option<&'a str> {
        self.facts.symbol_at(offset)
    }

    // ----------------------------------------------------------------------------------------
    // Symbols and references
    // ----------------------------------------------------------------------------------------

    /// Symbol declared by a binding identifier (never panics, unlike `ident.symbol_id()`).
    pub fn binding_symbol(&self, ident: &BindingIdentifier<'_>) -> Option<SymbolId> {
        ident.symbol_id.get()
    }

    /// Symbol an identifier reference resolves to; `None` for globals / unresolved names.
    pub fn reference_symbol(&self, ident: &IdentifierReference<'_>) -> Option<SymbolId> {
        let reference = ident.reference_id.get()?;
        self.scoping().get_reference(reference).symbol_id()
    }

    /// Does this identifier refer to a global (i.e. no local declaration shadows it)?
    pub fn is_global_reference(&self, ident: &IdentifierReference<'_>) -> bool {
        match ident.reference_id.get() {
            Some(reference) => self
                .scoping()
                .get_reference(reference)
                .symbol_id()
                .is_none(),
            None => false,
        }
    }

    /// Is `expr` the identifier `name` referring to the global of that name (e.g. `console`,
    /// `undefined`, `window`) rather than a local variable?
    pub fn is_global(&self, expr: &Expression<'_>, name: &str) -> bool {
        match expr.get_inner_expression() {
            Expression::Identifier(ident) => ident.name == name && self.is_global_reference(ident),
            _ => false,
        }
    }

    pub fn symbol_name(&self, symbol: SymbolId) -> &'a str {
        self.scoping().symbol_name(symbol)
    }

    pub fn symbol_flags(&self, symbol: SymbolId) -> SymbolFlags {
        self.scoping().symbol_flags(symbol)
    }

    /// Span of the declaring identifier.
    pub fn symbol_span(&self, symbol: SymbolId) -> Span {
        self.scoping().symbol_span(symbol)
    }

    /// The node that declares the symbol (`VariableDeclarator`, `Function`, `Class`,
    /// `FormalParameter`, `ImportSpecifier`, …) — not the identifier itself.
    pub fn symbol_declaration(&self, symbol: SymbolId) -> &'a AstNode<'a> {
        self.semantic.symbol_declaration(symbol)
    }

    /// All resolved references (reads, writes, type references) to a symbol.
    pub fn references_of(&self, symbol: SymbolId) -> impl Iterator<Item = &'a Reference> + 'a {
        self.scoping().get_resolved_references(symbol)
    }

    /// The symbol is never referenced (no reads, writes, or type uses).
    pub fn is_unused_symbol(&self, symbol: SymbolId) -> bool {
        self.scoping().symbol_is_unused(symbol)
    }

    /// The symbol's value or type is used somewhere (any reference that is not a pure write).
    pub fn is_symbol_read(&self, symbol: SymbolId) -> bool {
        self.references_of(symbol).any(|r| {
            let flags = r.flags();
            flags.is_read() || flags.is_type() || !flags.contains(ReferenceFlags::Write)
        })
    }

    // ----------------------------------------------------------------------------------------
    // Rule parameters (fall back to the rule's declared default, then to zero/false/"")
    // ----------------------------------------------------------------------------------------

    /// Id of the rule currently running.
    pub fn rule_id(&self) -> &'static str {
        self.meta.id
    }

    fn param_value(&self, name: &str) -> Option<&'a ParamValue> {
        self.rules.param(self.rule_index, name)
    }

    fn param_default(&self, name: &str) -> Option<ParamDefault> {
        self.meta
            .params
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.default)
    }

    pub fn param_i64(&self, name: &str) -> i64 {
        self.param_value(name)
            .and_then(ParamValue::as_i64)
            .unwrap_or_else(|| match self.param_default(name) {
                Some(ParamDefault::Int(v)) => v,
                Some(ParamDefault::Float(v)) => v as i64,
                _ => 0,
            })
    }

    pub fn param_f64(&self, name: &str) -> f64 {
        self.param_value(name)
            .and_then(ParamValue::as_f64)
            .unwrap_or_else(|| match self.param_default(name) {
                Some(ParamDefault::Int(v)) => v as f64,
                Some(ParamDefault::Float(v)) => v,
                _ => 0.0,
            })
    }

    pub fn param_bool(&self, name: &str) -> bool {
        self.param_value(name)
            .and_then(ParamValue::as_bool)
            .unwrap_or_else(|| matches!(self.param_default(name), Some(ParamDefault::Bool(true))))
    }

    pub fn param_str(&self, name: &str) -> &'a str {
        match self.param_value(name) {
            Some(ParamValue::Str(s)) => s,
            _ => match self.param_default(name) {
                Some(ParamDefault::Str(s)) => s,
                _ => "",
            },
        }
    }

    /// A value the current rule derives from its parameters alone (`init` must not look at the
    /// file), computed once for every file analysed with the same rule set: settings parsed
    /// from string parameters, say, instead of parsing them again for every file. Each rule
    /// must always ask for the same type `T`.
    pub fn rule_memo<T>(&self, init: impl FnOnce(&Self) -> T) -> &'a T
    where
        T: Any + Send + Sync + std::panic::UnwindSafe + std::panic::RefUnwindSafe,
    {
        // One slot per rule of the registry (the set was resolved for it), so the index exists.
        let memos: &'a [std::sync::OnceLock<crate::rule::RuleMemo>] = self.memos;
        let memo = memos[self.rule_index].get_or_init(|| Box::new(init(self)));
        let any: &'a (dyn Any + Send + Sync) = memo.as_ref();
        any.downcast_ref::<T>()
            .expect("a rule asks for one type of rule memo")
    }

    /// A comma-separated string parameter as trimmed, non-empty items.
    pub fn param_list(&self, name: &str) -> impl Iterator<Item = &'a str> + 'a {
        self.param_str(name)
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    // ----------------------------------------------------------------------------------------
    // Reporting
    // ----------------------------------------------------------------------------------------

    /// Report a finding for the current rule at `span`. The enclosing symbol is attached
    /// automatically; chain [`Emitted::value`] / [`Emitted::secondary`] for more.
    pub fn emit(&mut self, span: Span, message: impl Into<String>) -> Emitted<'_> {
        let file = self.file;
        let finding = RawFinding {
            rule_id: self.meta.id.to_string(),
            message: message.into(),
            range: file.lines.range(span.start, span.end),
            symbol: self.facts.symbol_at(span.start).map(str::to_string),
            value: None,
            secondary: Vec::new(),
            fix: None,
        };
        self.findings.push(finding);
        Emitted {
            finding: self.findings.last_mut(),
            file,
        }
    }

    /// 1-based line of a byte offset.
    pub fn line_of(&self, offset: u32) -> u32 {
        self.file.lines.line_of(offset)
    }

    /// Line/column range of a span.
    pub fn range(&self, span: Span) -> TextRange {
        self.file.lines.range(span.start, span.end)
    }

    /// Text of a 1-based line without its terminator (empty if out of range).
    pub fn line_text(&self, line: u32) -> &'a str {
        self.file.lines.line_text(line)
    }

    // ----------------------------------------------------------------------------------------
    // Per-rule, per-file scratch state
    // ----------------------------------------------------------------------------------------

    /// Scratch state for the current rule in the current file, created with `T::default()` on
    /// first use and dropped after the file. Store `NodeId`s / `Span`s / `SymbolId`s (not
    /// references) and read them back in [`JsRule::run_once`](crate::JsRule::run_once).
    pub fn state<T: Default + 'static>(&mut self) -> &mut T {
        if self.states.len() <= self.rule_index {
            self.states.resize_with(self.rule_index + 1, || None);
        }
        let slot = &mut self.states[self.rule_index];
        if !slot.as_ref().is_some_and(|b| b.is::<T>()) {
            *slot = Some(Box::<T>::default());
        }
        slot.as_mut()
            .and_then(|b| b.downcast_mut::<T>())
            .expect("rule state slot was just initialised with this type")
    }
}

/// Handle to the finding just emitted, for optional extras.
pub struct Emitted<'c> {
    finding: Option<&'c mut RawFinding>,
    file: &'c FileInfo<'c>,
}

impl Emitted<'_> {
    /// Measured value (e.g. a complexity score); also put the number in the message.
    pub fn value(mut self, value: f64) -> Self {
        if let Some(f) = self.finding.as_deref_mut() {
            f.value = Some(value);
        }
        self
    }

    /// A related location in the same file (e.g. "first declared here").
    pub fn secondary(mut self, span: Span, message: impl Into<String>) -> Self {
        if let Some(f) = self.finding.as_deref_mut() {
            f.secondary.push(Location {
                path: self.file.path.to_string(),
                range: self.file.lines.range(span.start, span.end),
                message: Some(message.into()),
            });
        }
        self
    }

    /// Override the automatically computed enclosing symbol.
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        if let Some(f) = self.finding.as_deref_mut() {
            f.symbol = Some(symbol.into());
        }
        self
    }

    /// Attach a fix: byte-range edits of the source that remove the finding and cannot change
    /// what the program does (see `RULE_AUTHORING.md`, "Fixes"). Only for rules whose
    /// `RuleMeta` says `fixable: true`.
    pub fn fix(mut self, description: impl Into<String>, edits: Vec<mote_core::Edit>) -> Self {
        if let Some(f) = self.finding.as_deref_mut() {
            f.fix = Some(mote_core::Fix::new(description, edits));
        }
        self
    }
}

/// Nodes whose contents run in a separate invocation (functions, static blocks, class field
/// initializers).
fn is_function_boundary(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_)
            | AstKind::StaticBlock(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::AccessorProperty(_)
    )
}
