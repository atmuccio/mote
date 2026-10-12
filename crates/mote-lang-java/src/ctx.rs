//! [`Ctx`]: everything a rule can ask about the file being analyzed.
//!
//! Rules receive `&Ctx`; every method takes `&self` and returns plain values or references
//! that stay valid for the whole file, so reading (`ctx.imports()`) and reporting
//! (`ctx.emit(..)`) can be freely interleaved.

use crate::kinds::{k, K};
use crate::resolve::ResolveCache;
use crate::rule::Span;
use crate::syntax::{self, node_text};
use mote_core::{
    Comment, Location, ParamDefault, ParamValue, RawFinding, RuleIndex, RuleMeta, SourceFile,
    TextRange,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tree_sitter::Node;

/// One `import` declaration.
#[derive(Clone, Copy, Debug)]
pub struct Import<'t> {
    /// The `import_declaration` node.
    pub node: Node<'t>,
    /// Dotted path without `.*`: `java.util.List`, `java.util` (wildcard),
    /// `org.junit.Assert.assertEquals` (static).
    pub path: &'t str,
    pub is_static: bool,
    pub is_wildcard: bool,
}

impl<'t> Import<'t> {
    /// Last path segment (`List`, `assertEquals`); `""` for wildcards.
    pub fn simple_name(&self) -> &'t str {
        if self.is_wildcard {
            ""
        } else {
            self.path.rsplit('.').next().unwrap_or(self.path)
        }
    }
    /// Everything before the last segment for single imports (`java.util`); the path itself for
    /// wildcards.
    pub fn qualifier(&self) -> &'t str {
        if self.is_wildcard {
            self.path
        } else {
            self.path.rsplit_once('.').map(|(q, _)| q).unwrap_or("")
        }
    }
}

/// Line starts plus column lookups. Columns count Unicode scalar values (like
/// `mote_core::LineIndex`). On an ASCII line a column is a subtraction; on other lines
/// consecutive queries resume from the previous position, and lines longer than
/// [`COLUMN_MARK_BYTES`] get column marks every that many bytes (built on first use), so that
/// locations on a huge line cost the same in any order (a finding with a secondary location
/// back at the start of a megabyte line counted the whole line each time).
pub(crate) struct Lines {
    starts: Vec<u32>,
    /// Lines (0-based) holding a non-ASCII byte.
    non_ascii: FxHashSet<u32>,
    /// (line start, byte offset, column) of the last computed position.
    cache: Cell<(u32, u32, u32)>,
    /// Column marks of long non-ASCII lines: (byte offset, column) at character boundaries about
    /// [`COLUMN_MARK_BYTES`] apart, by 0-based line.
    marks: RefCell<FxHashMap<u32, Vec<(u32, u32)>>>,
}

/// Spacing of the column marks of a long non-ASCII line (see [`Lines`]).
const COLUMN_MARK_BYTES: u32 = 4096;

impl Lines {
    pub(crate) fn new(src: &str) -> Self {
        let mut starts = Vec::with_capacity(src.len() / 32 + 1);
        starts.push(0);
        let mut non_ascii = FxHashSet::default();
        for (i, &b) in src.as_bytes().iter().enumerate() {
            if b == b'\n' {
                starts.push(i as u32 + 1);
            } else if b >= 0x80 {
                non_ascii.insert(starts.len() as u32 - 1);
            }
        }
        Lines {
            starts,
            non_ascii,
            cache: Cell::new((0, 0, 1)),
            marks: RefCell::new(FxHashMap::default()),
        }
    }

    /// The last column mark `(byte offset, column)` at or before `off` on the line `index`
    /// (0-based) starting at byte `start`: the line start itself for a short line.
    fn mark_before(&self, src: &str, index: u32, start: u32, off: u32) -> (u32, u32) {
        let end = self
            .starts
            .get(index as usize + 1)
            .copied()
            .unwrap_or(src.len() as u32);
        if end - start <= COLUMN_MARK_BYTES {
            return (start, 1);
        }
        let Ok(mut all) = self.marks.try_borrow_mut() else {
            return (start, 1);
        };
        let marks = all.entry(index).or_insert_with(|| {
            let mut marks = vec![(start, 1)];
            let (mut at, mut col) = (start, 1u32);
            loop {
                let mut next = at + COLUMN_MARK_BYTES;
                if next >= end {
                    break;
                }
                while !src.is_char_boundary(next as usize) {
                    next += 1;
                }
                col += src[at as usize..next as usize].chars().count() as u32;
                at = next;
                marks.push((at, col));
            }
            marks
        });
        let k = marks.partition_point(|&(b, _)| b <= off);
        marks[k.saturating_sub(1)]
    }

    /// 1-based line containing `offset`.
    pub(crate) fn line_of(&self, offset: u32) -> u32 {
        match self.starts.binary_search(&offset) {
            Ok(i) => i as u32 + 1,
            Err(i) => i as u32,
        }
    }

    /// 1-based (line, column) of a byte offset.
    pub(crate) fn line_col(&self, src: &str, offset: u32) -> (u32, u32) {
        let mut off = (offset as usize).min(src.len());
        while off > 0 && !src.is_char_boundary(off) {
            off -= 1;
        }
        let off = off as u32;
        let line = self.line_of(off).max(1);
        let start = self.starts.get(line as usize - 1).copied().unwrap_or(0);
        if !self.non_ascii.contains(&(line - 1)) {
            return (line, off.saturating_sub(start) + 1);
        }
        let (c_start, c_off, c_col) = self.cache.get();
        let mark = self.mark_before(src, line - 1, start, off);
        let (from, base) = if c_start == start && c_off <= off && c_off >= mark.0 {
            (c_off, c_col)
        } else {
            mark
        };
        let chars = src
            .get(from as usize..off as usize)
            .map(|t| t.chars().count())
            .unwrap_or(0) as u32;
        let col = base + chars;
        self.cache.set((start, off, col));
        (line, col)
    }

    pub(crate) fn range(&self, src: &str, start: u32, end: u32) -> TextRange {
        let (start_line, start_col) = self.line_col(src, start);
        let (end_line, end_col) = self.line_col(src, end.max(start));
        TextRange {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

/// Iterator over the ancestors of a node, innermost first (see [`Ctx::ancestors`]).
pub struct Ancestors<'c, 't> {
    inner: AncInner<'c, 't>,
}

enum AncInner<'c, 't> {
    Stack(std::iter::Rev<std::slice::Iter<'c, Node<'t>>>),
    Owned(std::iter::Rev<std::vec::IntoIter<Node<'t>>>),
    /// A descendant of the node being checked: the path below that node (innermost first),
    /// then the walk stack.
    Below(
        std::iter::Rev<std::vec::IntoIter<Node<'t>>>,
        std::iter::Rev<std::slice::Iter<'c, Node<'t>>>,
    ),
}

impl<'t> Iterator for Ancestors<'_, 't> {
    type Item = Node<'t>;
    #[inline]
    fn next(&mut self) -> Option<Node<'t>> {
        match &mut self.inner {
            AncInner::Stack(it) => it.next().copied(),
            AncInner::Owned(it) => it.next(),
            AncInner::Below(near, stack) => near.next().or_else(|| stack.next().copied()),
        }
    }

    /// O(1) (no intermediate ancestors are visited).
    fn nth(&mut self, n: usize) -> Option<Node<'t>> {
        match &mut self.inner {
            AncInner::Stack(it) => it.nth(n).copied(),
            AncInner::Owned(it) => it.nth(n),
            AncInner::Below(near, stack) => {
                let len = near.len();
                if n < len {
                    near.nth(n)
                } else {
                    near.nth(len);
                    stack.nth(n - len).copied()
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}

impl ExactSizeIterator for Ancestors<'_, '_> {
    /// The number of ancestors left (the depth of the node for a fresh iterator).
    fn len(&self) -> usize {
        match &self.inner {
            AncInner::Stack(it) => it.len(),
            AncInner::Owned(it) => it.len(),
            AncInner::Below(near, stack) => near.len() + stack.len(),
        }
    }
}

/// The members of a type body (see [`Ctx::members`]), by value, from a list shared by every
/// iteration over that body.
#[derive(Clone)]
pub struct Members<'t> {
    list: Rc<[Node<'t>]>,
    next: usize,
}

impl<'t> Iterator for Members<'t> {
    type Item = Node<'t>;
    #[inline]
    fn next(&mut self) -> Option<Node<'t>> {
        let n = self.list.get(self.next).copied();
        self.next += 1;
        n
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.list.len().saturating_sub(self.next);
        (n, Some(n))
    }
}

impl ExactSizeIterator for Members<'_> {}

/// Handle to a finding that was just emitted; add details with the builder methods.
pub struct Emitted<'c, 't> {
    ctx: &'c Ctx<'t>,
    index: usize,
}

impl Emitted<'_, '_> {
    fn update(&self, f: impl FnOnce(&mut RawFinding)) {
        if let Ok(mut all) = self.ctx.findings.try_borrow_mut() {
            if let Some(x) = all.get_mut(self.index) {
                f(x);
            }
        }
    }
    /// Attach the measured value (complexity, count, …).
    pub fn value(self, v: f64) -> Self {
        self.update(|f| f.value = Some(v));
        self
    }
    /// Add a secondary location (e.g. the other half of a pair).
    pub fn secondary(self, at: impl Into<Span>, message: impl Into<String>) -> Self {
        let range = self.ctx.range_of(at.into());
        let loc = Location {
            path: self.ctx.path.to_string(),
            range,
            message: Some(message.into()),
        };
        self.update(|f| f.secondary.push(loc));
        self
    }
    /// Override the automatically computed symbol.
    pub fn symbol(self, symbol: impl Into<String>) -> Self {
        let s = symbol.into();
        self.update(|f| f.symbol = Some(s));
        self
    }
}

/// Per-file analysis context handed to rules.
pub struct Ctx<'t> {
    pub(crate) src: &'t str,
    pub(crate) path: &'t str,
    pub(crate) is_test: bool,
    pub(crate) root: Node<'t>,
    /// The enabled rules and their parameters, by registry index (as `metas`).
    pub(crate) rules: &'t RuleIndex,
    pub(crate) metas: &'t [&'static RuleMeta],
    pub(crate) current_rule: Cell<usize>,
    /// The node being dispatched (None during `finish`).
    pub(crate) current: Option<Node<'t>>,
    /// Ancestors of `current`, root first.
    pub(crate) stack: Vec<Node<'t>>,
    pub(crate) findings: RefCell<Vec<RawFinding>>,
    pub(crate) comments: Vec<Comment>,
    pub(crate) lines: Lines,
    pub(crate) imports: Vec<Import<'t>>,
    /// The top-level type declarations, in order (the resolver looks names up among them).
    pub(crate) top_types: Vec<Node<'t>>,
    pub(crate) package: Option<&'t str>,
    /// Byte offset where the package/import header ends.
    pub(crate) header_end: usize,
    pub(crate) res: ResolveCache<'t>,
    /// Resolver time and framework counters (`MOTE_RULE_TIMINGS` only).
    pub(crate) prof: Option<Box<crate::timing::CtxProf>>,
    /// `(node id, method name)` of the last `method_invocation` asked about (see
    /// [`Ctx::call_name`]); the walker fills it for the node being dispatched.
    call_cache: Cell<(usize, Option<&'t str>)>,
    /// `(node id, operator token)` of the last expression asked about (see [`Ctx::operator`]).
    op_cache: Cell<(usize, Option<Node<'t>>)>,
    /// `(node id, name node)` of the last declaration asked about (see [`Ctx::name_node`]);
    /// the walker fills it for the method declaration being dispatched.
    pub(crate) name_cache: Cell<(usize, Option<Node<'t>>)>,
    /// Declared types by name, filled by the walker (see [`Ctx::declared_types`]).
    pub(crate) declared: crate::declared::DeclaredTypes<'t>,
    /// Per-file results shared by several rules (see [`Ctx::shared_memo`]).
    shared: RefCell<SharedMemo>,
    /// Members of the type bodies asked about (see [`Ctx::members`]), by body id.
    members: RefCell<FxHashMap<usize, Rc<[Node<'t>]>>>,
    /// The root path (named ancestors, root first) of the node last looked up off the walk
    /// stack, and that node: the next lookup descends from their deepest node containing it
    /// instead of from the root (lookups come in clusters: one method, one class).
    last_path: RefCell<(Vec<Node<'t>>, Option<Node<'t>>)>,
    /// Whole-word occurrences of every word of the source (see [`Ctx::word_offsets`]), built on
    /// first use.
    words: std::cell::OnceCell<WordIndex<'t>>,
    /// Whether the source mentions `lombok` (see `Ctx::mentions_lombok`), on first use.
    pub(crate) lombok: std::cell::OnceCell<bool>,
    /// [`SymState`] entering each node of `stack` (kept by the walker, in step with `stack`).
    pub(crate) sym_stack: Vec<SymState<'t>>,
    /// For each position of `stack`, the nearest position at or below it holding a structural
    /// node ([`is_structural`]), or [`NO_STRUCTURAL`] (kept by the walker, in step with `stack`).
    pub(crate) struct_stack: Vec<u32>,
    /// The id of the node being checked and the [`SymState`] entering it.
    pub(crate) current_sym: Option<(usize, SymState<'t>)>,
    /// `Outer.Inner` of the type declarations seen, by node id (see [`Ctx::sym_step`]).
    type_paths: RefCell<FxHashMap<usize, Rc<str>>>,
    /// Children of the parents asked about by the sibling helpers, by parent id.
    children: RefCell<FxHashMap<usize, Rc<[Node<'t>]>>>,
}

/// No structural node at or below a walk stack position (see [`Ctx::struct_stack`]).
pub(crate) const NO_STRUCTURAL: u32 = u32::MAX;

/// Kinds [`Ctx::structural_ancestors`] yields: member and type declarations, type bodies,
/// lambdas, anonymous classes and enum constants — the only ancestors the scope helpers
/// ([`Ctx::enclosing_method`], [`Ctx::enclosing_function`], [`Ctx::enclosing_class`]) and the
/// resolver's variable lookup act on.
pub(crate) fn is_structural(kk: K) -> bool {
    matches!(
        kk,
        K::MethodDecl
            | K::ConstructorDecl
            | K::CompactConstructorDecl
            | K::StaticInitializer
            | K::FieldDecl
            | K::ConstantDecl
            | K::Lambda
            | K::EnumConstant
            | K::ObjectCreation
    ) || kk.is_type_decl()
        || kk.is_type_body()
}

/// Iterator of [`Ctx::structural_ancestors`]: `(ancestor, its child on the path)`.
pub(crate) struct StructuralAncestors<'c, 't> {
    ctx: &'c Ctx<'t>,
    /// Named nodes below the walk stack still to visit (the path from the node being checked
    /// down to the parent of the node asked about; visited from the end).
    below: Vec<Node<'t>>,
    /// The child of the next ancestor visited.
    child: Node<'t>,
    /// The highest walk stack position still to visit.
    top: Option<usize>,
    /// Off the walk stack: every ancestor, filtered.
    plain: Option<Ancestors<'c, 't>>,
}

impl<'t> Iterator for StructuralAncestors<'_, 't> {
    type Item = (Node<'t>, Node<'t>);

    fn next(&mut self) -> Option<(Node<'t>, Node<'t>)> {
        if let Some(plain) = self.plain.as_mut() {
            for a in plain.by_ref() {
                let child = std::mem::replace(&mut self.child, a);
                if is_structural(k(a)) {
                    return Some((a, child));
                }
            }
            return None;
        }
        while let Some(a) = self.below.pop() {
            let child = std::mem::replace(&mut self.child, a);
            if is_structural(k(a)) {
                return Some((a, child));
            }
        }
        let top = self.top?;
        let at = *self.ctx.struct_stack.get(top)?;
        if at == NO_STRUCTURAL {
            self.top = None;
            return None;
        }
        let at = at as usize;
        let a = *self.ctx.stack.get(at)?;
        let child = if at == top {
            self.child
        } else {
            *self.ctx.stack.get(at + 1)?
        };
        self.top = at.checked_sub(1);
        self.child = a;
        Some((a, child))
    }
}

/// What [`Ctx::symbol_of`] knows about a node from its ancestors: the walk up from the node,
/// computed top-down instead, one step per node entered (see [`Ctx::sym_step`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct SymState<'t> {
    /// The enclosing type declarations, `Outer.Inner`.
    types: Option<Rc<str>>,
    /// The member of the innermost type the node belongs to so far: the lowest member
    /// declaration on the path.
    last: Option<&'t str>,
    /// Set by the outermost anonymous class body (or enum constant body) on the path below the
    /// innermost type: the member that code inside it is attributed to.
    frozen: Option<Option<&'t str>>,
}

impl SymState<'_> {
    /// The symbol (see [`Ctx::symbol_of`]) of a node whose state, after its own step, this is.
    fn symbol(&self) -> Option<String> {
        let types = self.types.as_ref()?;
        let Some(member) = self.frozen.unwrap_or(self.last) else {
            return Some(types.to_string());
        };
        Some(format!("{types}#{member}"))
    }
}

/// [`Ctx::symbol_key`]: a symbol, or what it is formatted from.
#[derive(Clone, Debug)]
pub(crate) struct SymbolKey<'t>(Result<Option<String>, SymState<'t>>);

impl SymbolKey<'_> {
    /// The symbol, as [`Ctx::symbol_of`] gives it.
    pub(crate) fn symbol(self) -> Option<String> {
        match self.0 {
            Ok(symbol) => symbol,
            Err(state) => state.symbol(),
        }
    }
}

/// `(tag, id)` → a value of the type the tag's users agree on.
type SharedMemo = FxHashMap<(&'static str, usize), Box<dyn Any>>;

impl<'t> Ctx<'t> {
    pub(crate) fn new(
        file: &SourceFile<'t>,
        root: Node<'t>,
        rules: &'t RuleIndex,
        metas: &'t [&'static RuleMeta],
    ) -> Self {
        let src = file.text;
        let mut imports = Vec::new();
        let mut top_types = Vec::new();
        let mut package = None;
        let mut header_end = 0;
        for c in syntax::named_children(root) {
            match crate::kinds::kind(c) {
                "package_declaration" => {
                    package = syntax::named_children(c)
                        .find(|n| {
                            matches!(crate::kinds::kind(*n), "scoped_identifier" | "identifier")
                        })
                        .map(|n| node_text(n, src));
                    header_end = header_end.max(c.end_byte());
                }
                // Imports containing syntax errors (e.g. Java 25 `import module`, unsupported by the
                // grammar) are unreliable: ignore them.
                "import_declaration" if !c.has_error() => {
                    let mut is_static = false;
                    let mut is_wildcard = false;
                    let mut path = "";
                    for part in syntax::children(c) {
                        match crate::kinds::kind(part) {
                            "static" => is_static = true,
                            "asterisk" => is_wildcard = true,
                            "scoped_identifier" | "identifier" => path = node_text(part, src),
                            _ => {}
                        }
                    }
                    imports.push(Import {
                        node: c,
                        path,
                        is_static,
                        is_wildcard,
                    });
                    header_end = header_end.max(c.end_byte());
                }
                "import_declaration" => header_end = header_end.max(c.end_byte()),
                _ if k(c).is_type_decl() => top_types.push(c),
                _ => {}
            }
        }
        Ctx {
            src,
            path: file.path,
            is_test: file.is_test,
            root,
            rules,
            metas,
            current_rule: Cell::new(0),
            current: None,
            stack: Vec::with_capacity(64),
            findings: RefCell::new(Vec::new()),
            comments: Vec::new(),
            lines: Lines::new(src),
            imports,
            top_types,
            package,
            header_end,
            res: ResolveCache::default(),
            prof: None,
            call_cache: Cell::new((0, None)),
            op_cache: Cell::new((usize::MAX, None)),
            name_cache: Cell::new((usize::MAX, None)),
            declared: Default::default(),
            shared: RefCell::new(FxHashMap::default()),
            members: RefCell::new(FxHashMap::default()),
            last_path: RefCell::new((Vec::new(), None)),
            words: std::cell::OnceCell::new(),
            lombok: std::cell::OnceCell::new(),
            sym_stack: Vec::with_capacity(64),
            struct_stack: Vec::with_capacity(64),
            current_sym: None,
            type_paths: RefCell::new(FxHashMap::default()),
            children: RefCell::new(FxHashMap::default()),
        }
    }

    /// The members of a type body — the same nodes, in the same order, as
    /// [`syntax::members`] — read from the tree once per file and body: many rules iterate the
    /// members of every class, and a slice is much cheaper to walk than the tree.
    pub fn members(&self, body: Node<'t>) -> Members<'t> {
        if let Ok(map) = self.members.try_borrow() {
            if let Some(list) = map.get(&body.id()) {
                return Members {
                    list: list.clone(),
                    next: 0,
                };
            }
        }
        let list: Rc<[Node<'t>]> = syntax::members(body).collect();
        if let Ok(mut map) = self.members.try_borrow_mut() {
            map.insert(body.id(), list.clone());
        }
        Members { list, next: 0 }
    }

    /// A per-file memo shared by all rules: `compute` runs once per `(tag, id)` (`id`
    /// usually a node id) and every later request, from any rule, gets a copy of its value —
    /// for rules built on the same analysis of a node. `compute` must not depend on the rule
    /// asking (no parameters), and `T` must not borrow from the tree (store what is needed to
    /// find nodes again, not nodes).
    pub(crate) fn shared_memo<T: Clone + 'static>(
        &self,
        tag: &'static str,
        id: usize,
        compute: impl FnOnce() -> T,
    ) -> T {
        if let Ok(map) = self.shared.try_borrow() {
            if let Some(v) = map.get(&(tag, id)).and_then(|b| b.downcast_ref::<T>()) {
                return v.clone();
            }
        }
        let v = compute();
        if let Ok(mut map) = self.shared.try_borrow_mut() {
            map.insert((tag, id), Box::new(v.clone()));
        }
        v
    }

    /// Times a framework section until the guard is dropped (profiling only; one branch
    /// otherwise).
    #[inline]
    pub(crate) fn section(&self, s: crate::timing::Section) -> crate::timing::SectionGuard<'_> {
        crate::timing::SectionGuard::new(self.prof.as_deref(), s)
    }

    // ----------------------------------------------------------------- file & text

    /// The whole source text.
    pub fn source(&self) -> &'t str {
        self.src
    }

    /// Whether the word index ([`Ctx::word_offsets`]) has been built for this file.
    pub(crate) fn has_word_index(&self) -> bool {
        self.words.get().is_some()
    }

    /// Byte offsets, in order, of the whole-word occurrences of `name` in the source (code,
    /// comments and strings alike): the maximal runs of identifier bytes (ASCII letters and
    /// digits, `_`, `$`, and every non-ASCII byte) equal to `name`. `None` when `name` is empty or
    /// has other bytes (callers then scan the text). The index is built once per file, on first
    /// use, so that rules looking up the uses of many names by text stay linear in the file
    /// (scanning the whole source per field or per parameter was quadratic).
    pub(crate) fn word_offsets(&self, name: &str) -> Option<&[u32]> {
        if name.is_empty() || !name.bytes().all(is_word_byte) {
            return None;
        }
        Some(
            self.words
                .get_or_init(|| WordIndex::new(self.src))
                .get(name),
        )
    }
    /// Repo-relative path with `/` separators.
    pub fn path(&self) -> &'t str {
        self.path
    }
    /// Whether the file is test code (rules may stay silent in tests where they make no sense).
    pub fn is_test(&self) -> bool {
        self.is_test
    }
    /// The `program` node.
    pub fn root(&self) -> Node<'t> {
        self.root
    }
    /// Source text of a node.
    #[inline]
    pub fn text(&self, node: Node<'_>) -> &'t str {
        node_text(node, self.src)
    }
    /// Source text of a span.
    pub fn text_at(&self, span: impl Into<Span>) -> &'t str {
        let s = span.into();
        self.src.get(s.start as usize..s.end as usize).unwrap_or("")
    }
    /// The method name of a `method_invocation` (the text of its `name` field: `foo` in
    /// `a.<T>foo(x)`); `None` for other nodes and for calls without a name (syntax errors).
    /// Read once per call node and shared by all rules: the cheapest first test for rules on
    /// `method_invocation` (see also [`crate::JavaRule::call_names`]).
    #[inline]
    pub fn call_name(&self, call: Node<'t>) -> Option<&'t str> {
        let (id, name) = self.call_cache.get();
        if id == call.id() {
            return name;
        }
        if k(call) != K::MethodInvocation {
            return None;
        }
        let name = syntax::field(call, syntax::Field::Name).map(|n| self.text(n));
        self.call_cache.set((call.id(), name));
        name
    }

    /// The operator token of a binary, unary, or assignment expression — the same node as
    /// [`syntax::operator`], remembered for the last expression asked about: the many rules on
    /// `binary_expression` all start by reading it.
    #[inline]
    pub fn operator(&self, expr: Node<'t>) -> Option<Node<'t>> {
        let (id, op) = self.op_cache.get();
        if id == expr.id() {
            return op;
        }
        let op = syntax::operator(expr);
        self.op_cache.set((expr.id(), op));
        op
    }

    /// The operator of an expression as a string (`"=="`, `"+="`), or `""` — the same as
    /// [`syntax::operator_str`], through [`Ctx::operator`].
    #[inline]
    pub fn operator_str(&self, expr: Node<'t>) -> &'static str {
        self.operator(expr).map(crate::kinds::kind).unwrap_or("")
    }

    /// The name identifier of a declaration (its `name` field) — the same node as
    /// [`syntax::name_node`], remembered for the last declaration asked about: the many rules
    /// on `method_declaration` all read it.
    #[inline]
    pub fn name_node(&self, decl: Node<'t>) -> Option<Node<'t>> {
        let (id, name) = self.name_cache.get();
        if id == decl.id() {
            return name;
        }
        let name = syntax::name_node(decl);
        self.name_cache.set((decl.id(), name));
        name
    }

    /// The name text of a declaration, or `""` — the same as [`syntax::name_of`], through
    /// [`Ctx::name_node`].
    #[inline]
    pub fn name_of(&self, decl: Node<'t>) -> &'t str {
        self.name_node(decl).map(|n| self.text(n)).unwrap_or("")
    }

    /// 1-based line where the node starts.
    pub fn line(&self, node: Node<'_>) -> u32 {
        node.start_position().row as u32 + 1
    }
    /// Line/column range of a span.
    pub fn range_of(&self, span: impl Into<Span>) -> TextRange {
        let s = span.into();
        self.lines.range(self.src, s.start, s.end)
    }
    /// All comments seen so far, in source order. Complete in [`crate::JavaRule::finish`];
    /// during `check` it holds the comments that precede the current node.
    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }
    /// The `package` name, if declared.
    pub fn package_name(&self) -> Option<&'t str> {
        self.package
    }
    /// All import declarations, in source order.
    pub fn imports(&self) -> &[Import<'t>] {
        &self.imports
    }
    /// Byte offset where the package/import header ends (nodes before it belong to the header).
    pub fn header_end(&self) -> usize {
        self.header_end
    }
    /// The declared type (as written) of every variable, field, and parameter name of the file:
    /// a syntactic pre-filter before any resolver work. Complete in
    /// [`crate::JavaRule::finish`]; during `check` it holds the declarations seen so far.
    pub fn declared_types(&self) -> &crate::declared::DeclaredTypes<'t> {
        &self.declared
    }

    // ----------------------------------------------------------------- rule & params

    /// Metadata of the rule currently running.
    pub fn meta(&self) -> &'static RuleMeta {
        self.metas
            .get(self.current_rule.get())
            .copied()
            .unwrap_or(&crate::rules::NO_RULE)
    }
    /// Id of the rule currently running.
    pub fn rule_id(&self) -> &'static str {
        self.meta().id
    }
    fn param_default(&self, name: &str) -> Option<ParamDefault> {
        let found = self
            .meta()
            .params
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.default);
        debug_assert!(
            found.is_some(),
            "rule {} reads undeclared parameter `{name}`",
            self.rule_id()
        );
        found
    }
    /// The configured value of a parameter of the current rule (by index: no rule-id lookup).
    fn param_value(&self, name: &str) -> Option<&ParamValue> {
        self.rules.param(self.current_rule.get(), name)
    }
    /// Integer parameter of the current rule (configured value, else the declared default).
    pub fn param_i64(&self, name: &str) -> i64 {
        let d = match self.param_default(name) {
            Some(ParamDefault::Int(v)) => v,
            Some(ParamDefault::Float(v)) => v as i64,
            _ => 0,
        };
        self.param_value(name)
            .and_then(ParamValue::as_i64)
            .unwrap_or(d)
    }
    /// Float parameter of the current rule.
    pub fn param_f64(&self, name: &str) -> f64 {
        let d = match self.param_default(name) {
            Some(ParamDefault::Int(v)) => v as f64,
            Some(ParamDefault::Float(v)) => v,
            _ => 0.0,
        };
        self.param_value(name)
            .and_then(ParamValue::as_f64)
            .unwrap_or(d)
    }
    /// Boolean parameter of the current rule.
    pub fn param_bool(&self, name: &str) -> bool {
        let d = matches!(self.param_default(name), Some(ParamDefault::Bool(true)));
        self.param_value(name)
            .and_then(ParamValue::as_bool)
            .unwrap_or(d)
    }
    /// The Java language release the file is compiled for (`8`, `11`, `17`, `21`), when known:
    /// `java.release` in the configuration, or read by the engine from the nearest build file.
    /// Rules about newer language features stay silent when it is unknown or too old. Fixtures
    /// set it with `// mote-test: params java_release=17`.
    pub fn java_release(&self) -> Option<u32> {
        self.param_value(crate::JAVA_RELEASE_PARAM)
            .and_then(ParamValue::as_i64)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0)
    }
    /// String parameter of the current rule.
    pub fn param_str(&self, name: &str) -> &str {
        let d = match self.param_default(name) {
            Some(ParamDefault::Str(s)) => s,
            _ => "",
        };
        self.param_value(name)
            .and_then(ParamValue::as_str)
            .unwrap_or(d)
    }

    // ----------------------------------------------------------------- reporting

    fn push(&self, span: Span, message: String, symbol: Option<String>) -> Emitted<'_, 't> {
        let finding = RawFinding {
            rule_id: self.rule_id().to_string(),
            message,
            range: self.range_of(span),
            symbol,
            value: None,
            secondary: Vec::new(),
        };
        let index = match self.findings.try_borrow_mut() {
            Ok(mut all) => {
                all.push(finding);
                all.len() - 1
            }
            Err(_) => usize::MAX,
        };
        Emitted { ctx: self, index }
    }

    /// Report a finding for the current rule at `node`. The symbol (`Class#method`) is filled
    /// in automatically. Chain `.value(..)` / `.secondary(..)` on the result if needed.
    pub fn emit(&self, node: Node<'t>, message: impl Into<String>) -> Emitted<'_, 't> {
        let symbol = self.symbol_of(node);
        self.push(Span::from(node), message.into(), symbol)
    }

    /// [`Ctx::emit`] with the symbol computed earlier (by [`Ctx::symbol_of`] while the node was
    /// being walked, which costs no walk up the tree): for findings reported from
    /// [`crate::JavaRule::finish`] about nodes deep in the tree.
    pub fn emit_with_symbol(
        &self,
        node: Node<'t>,
        symbol: Option<String>,
        message: impl Into<String>,
    ) -> Emitted<'_, 't> {
        self.push(Span::from(node), message.into(), symbol)
    }

    /// Report a finding at an arbitrary span (`Span::cover(a, b)`, a stored `Span`, a byte range).
    pub fn emit_range(&self, span: impl Into<Span>, message: impl Into<String>) -> Emitted<'_, 't> {
        let span = span.into();
        let (start, end) = (span.start as usize, span.end as usize);
        // Descend from the node being checked when it holds the span (the usual case): from the
        // root, every finding costs the whole depth.
        let from = self
            .current
            .filter(|c| c.start_byte() <= start && end <= c.end_byte() && start < end)
            .unwrap_or(self.root);
        let symbol = from
            .descendant_for_byte_range(start, end)
            .and_then(|n| self.symbol_of(n));
        self.push(span, message.into(), symbol)
    }

    // ----------------------------------------------------------------- tree navigation

    /// Root-to-parent path of an arbitrary node (used when it is not on the walk stack).
    fn path_to(&self, node: Node<'t>) -> Vec<Node<'t>> {
        self.path_between(self.root, node).unwrap_or_default()
    }

    /// [`Ctx::path_to`], descending from the deepest node of the previous lookup's path that
    /// is an ancestor of `node` (or the previous node itself) instead of from the root. The
    /// named ancestors above that node are the same for both, so the path is the same.
    fn path_to_cached(&self, node: Node<'t>) -> Vec<Node<'t>> {
        let (start, end) = (node.start_byte(), node.end_byte());
        let mut found: Option<Vec<Node<'t>>> = None;
        if let Ok(cache) = self.last_path.try_borrow() {
            let (path, last) = &*cache;
            // `path` then the previous node: `chain[..i]` are the named ancestors of `chain[i]`.
            let chain = path.iter().copied().chain(*last);
            let deepest = chain
                .enumerate()
                .filter(|(_, x)| x.start_byte() <= start && end <= x.end_byte())
                .last();
            if let Some((i, x)) = deepest {
                if let Some(rest) = self.path_between(x, node) {
                    let mut p = Vec::with_capacity(i + rest.len());
                    p.extend_from_slice(&path[..i.min(path.len())]);
                    p.extend(rest);
                    found = Some(p);
                }
            }
        }
        let p = found.unwrap_or_else(|| self.path_to(node));
        if let Ok(mut cache) = self.last_path.try_borrow_mut() {
            cache.0.clear();
            cache.0.extend_from_slice(&p);
            cache.1 = Some(node);
        }
        p
    }

    /// Named nodes from `from` (inclusive) down to the parent of `node`; `None` when `node` is
    /// not a descendant of `from`.
    fn path_between(&self, from: Node<'t>, node: Node<'t>) -> Option<Vec<Node<'t>>> {
        let mut path = Vec::new();
        let mut cur = from;
        let target = node.id();
        for _ in 0..1_000_000 {
            if cur.id() == target {
                return Some(path);
            }
            match cur.child_with_descendant(node) {
                Some(c) if c.id() != cur.id() => {
                    // The walk stack holds named nodes only; mirror that here.
                    if cur.is_named() {
                        path.push(cur);
                    }
                    cur = c;
                }
                _ => return None,
            }
        }
        None
    }

    /// Ancestors of `node`, innermost (parent) first, ending with the `program` node.
    /// O(1) setup for the node being checked and its ancestors; a descendant of the node being
    /// checked costs a descent from that node; other nodes cost one descent from the root.
    pub fn ancestors(&self, node: Node<'t>) -> Ancestors<'_, 't> {
        let id = node.id();
        if let Some(cur) = self.current {
            if cur.id() == id {
                return Ancestors {
                    inner: AncInner::Stack(self.stack.iter().rev()),
                };
            }
            // Rules mostly ask about nodes inside the one being checked (`ctx.emit(name, …)`):
            // descend from it rather than from the root, which on deeply nested code would make
            // every such call cost the whole depth.
            if cur.start_byte() <= node.start_byte() && node.end_byte() <= cur.end_byte() {
                let _g = self.section(crate::timing::Section::AncestorsBelow);
                if let Some(near) = self.path_between(cur, node) {
                    return Ancestors {
                        inner: AncInner::Below(near.into_iter().rev(), self.stack.iter().rev()),
                    };
                }
            }
        }
        if let Some(pos) = self.stack.iter().rposition(|n| n.id() == id) {
            return Ancestors {
                inner: AncInner::Stack(self.stack[..pos].iter().rev()),
            };
        }
        let _g = self.section(crate::timing::Section::AncestorsFromRoot);
        Ancestors {
            inner: AncInner::Owned(self.path_to_cached(node).into_iter().rev()),
        }
    }

    /// The parent of `node` (cheap for the node being checked and its ancestors).
    pub fn parent(&self, node: Node<'t>) -> Option<Node<'t>> {
        self.ancestors(node).next()
    }

    /// Number of ancestors of `node` (0 for the `program` node); O(1) for the node being
    /// checked and its ancestors.
    pub fn depth(&self, node: Node<'t>) -> usize {
        self.ancestors(node).len()
    }

    /// Same as `node.next_named_sibling()` (comments included), but the parent comes from the
    /// walk stack instead of a descent from the root: use it for the node being checked, its
    /// ancestors, and their children.
    pub fn next_named_sibling(&self, node: Node<'t>) -> Option<Node<'t>> {
        let (siblings, i) = self.siblings_of(node)?;
        siblings
            .get(i + 1..)?
            .iter()
            .copied()
            .find(|n| n.is_named())
    }

    /// The children of `node`'s parent (anonymous tokens included) and `node`'s index among
    /// them. The children are read once per parent: walking them for every question was
    /// quadratic in a class of thousands of members or a block of thousands of statements.
    fn siblings_of(&self, node: Node<'t>) -> Option<(Rc<[Node<'t>]>, usize)> {
        let parent = self.parent(node)?;
        let cached = self
            .children
            .try_borrow()
            .ok()
            .and_then(|map| map.get(&parent.id()).cloned());
        let siblings = match cached {
            Some(s) => s,
            None => {
                let mut c = parent.walk();
                let s: Rc<[Node<'t>]> = parent.children(&mut c).collect();
                if let Ok(mut map) = self.children.try_borrow_mut() {
                    map.insert(parent.id(), s.clone());
                }
                s
            }
        };
        // Children are in source order; zero-width ones may share a start with a neighbor.
        let start = node.start_byte();
        let mut i = siblings.partition_point(|s| s.start_byte() < start);
        while let Some(s) = siblings.get(i) {
            if s.id() == node.id() {
                return Some((siblings, i));
            }
            if s.start_byte() > start {
                break;
            }
            i += 1;
        }
        None
    }

    /// Same as `node.prev_named_sibling()` (comments included); see
    /// [`Ctx::next_named_sibling`].
    pub fn prev_named_sibling(&self, node: Node<'t>) -> Option<Node<'t>> {
        let (siblings, i) = self.siblings_of(node)?;
        siblings[..i].iter().rev().copied().find(|n| n.is_named())
    }

    /// Same as `node.next_sibling()` (anonymous tokens included); see
    /// [`Ctx::next_named_sibling`].
    pub fn next_sibling(&self, node: Node<'t>) -> Option<Node<'t>> {
        let (siblings, i) = self.siblings_of(node)?;
        siblings.get(i + 1).copied()
    }

    /// Nearest enclosing method, constructor, or compact constructor — `None` when a type body
    /// is crossed first (e.g. for a field initializer). Lambdas are looked through.
    pub fn enclosing_method(&self, node: Node<'t>) -> Option<Node<'t>> {
        for (a, _) in self.structural_ancestors(node) {
            match k(a) {
                K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl => return Some(a),
                kk if kk.is_type_body() => return None,
                _ => {}
            }
        }
        None
    }

    /// Nearest enclosing function-like node: method, constructor, compact constructor, lambda,
    /// static initializer, or instance initializer block. `None` when a type body is crossed.
    pub fn enclosing_function(&self, node: Node<'t>) -> Option<Node<'t>> {
        for (a, child) in self.structural_ancestors(node) {
            match k(a) {
                K::MethodDecl
                | K::ConstructorDecl
                | K::CompactConstructorDecl
                | K::Lambda
                | K::StaticInitializer => return Some(a),
                K::ClassBody | K::EnumBodyDecls => {
                    return (child.id() != node.id() && k(child) == K::Block).then_some(child)
                }
                kk if kk.is_type_body() => return None,
                _ => {}
            }
        }
        None
    }

    /// Nearest enclosing type: a class/interface/enum/record/annotation declaration, or an
    /// anonymous class (`object_creation_expression` with a body, or an `enum_constant` with a
    /// body). Use [`syntax::type_body`] to get its members.
    pub fn enclosing_class(&self, node: Node<'t>) -> Option<Node<'t>> {
        for (a, child) in self.structural_ancestors(node) {
            // `child` is `node` itself for its parent, which the walk up did not compare.
            let below = (child.id() != node.id()).then_some(child);
            let ka = k(a);
            if ka.is_type_decl() && below.is_some_and(|p| k(p).is_type_body()) {
                return Some(a);
            }
            if matches!(ka, K::ObjectCreation | K::EnumConstant)
                && below.is_some_and(|p| k(p) == K::ClassBody)
            {
                return Some(a);
            }
        }
        None
    }

    /// The ancestors of `node` of the kinds the scope helpers act on ([`is_structural`]),
    /// innermost first, each with its child on the path to `node` (`node` itself for its
    /// parent). For the node being checked, the nodes below it and its ancestors, the walk
    /// jumps from one to the next on the walk stack ([`Ctx::struct_stack`]): a walk over every
    /// ancestor per question made the scope helpers and name lookups quadratic in the depth.
    pub(crate) fn structural_ancestors(&self, node: Node<'t>) -> StructuralAncestors<'_, 't> {
        let plain = || StructuralAncestors {
            ctx: self,
            below: Vec::new(),
            child: node,
            top: None,
            plain: Some(self.ancestors(node)),
        };
        if self.struct_stack.len() != self.stack.len() {
            return plain();
        }
        let on_stack = |top: Option<usize>, below: Vec<Node<'t>>| StructuralAncestors {
            ctx: self,
            below,
            child: node,
            top,
            plain: None,
        };
        if let Some(cur) = self.current {
            if cur.id() == node.id() {
                return on_stack(self.stack.len().checked_sub(1), Vec::new());
            }
            if cur.start_byte() <= node.start_byte() && node.end_byte() <= cur.end_byte() {
                if let Some(path) = self.path_between(cur, node) {
                    return on_stack(self.stack.len().checked_sub(1), path);
                }
            }
        }
        match self.stack.iter().rposition(|n| n.id() == node.id()) {
            Some(pos) => on_stack(pos.checked_sub(1), Vec::new()),
            None => plain(),
        }
    }

    /// `Outer.Inner#member` for any node: the chain of named enclosing types plus the member
    /// (method, constructor, field, initializer) the node belongs to. Members of anonymous classes
    /// are attributed to the member that contains the anonymous class.
    ///
    /// For the node being checked, its ancestors and the nodes below it this is read from the
    /// state the walker keeps per node ([`SymState`]), in time independent of the depth: every
    /// `emit` asks, and a walk to the root per finding made rules that report on every level of
    /// deeply nested code quadratic. Other nodes take the walk ([`Ctx::symbol_of_walk`]).
    pub fn symbol_of(&self, node: Node<'t>) -> Option<String> {
        if let Some(fast) = self.symbol_from_state(node) {
            #[cfg(debug_assertions)]
            if self.stack.len() <= 256 {
                debug_assert_eq!(fast, self.symbol_of_walk(node), "symbol of {node:?}");
            }
            return fast;
        }
        self.symbol_of_walk(node)
    }

    /// The effect of the ancestor `a` on the symbol of the nodes below it, `child` being its child
    /// on the path (`None`: `a` is the node itself): the top-down form of one step of
    /// [`Ctx::symbol_of_walk`]. Walking down, a member declaration replaces the member (the
    /// lowest one wins), the first anonymous class body or enum constant body entered freezes it
    /// (code inside belongs to the member around it), and a type declaration starts over.
    pub(crate) fn sym_step(&self, s: &mut SymState<'t>, a: Node<'t>, child: Option<Node<'t>>) {
        let set = |s: &mut SymState<'t>, member: Option<&'t str>| {
            if s.frozen.is_none() && member.is_some() {
                s.last = member;
            }
        };
        let ka = k(a);
        match ka {
            K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl => {
                set(s, syntax::name_node(a).map(|n| self.text(n)))
            }
            K::StaticInitializer => set(s, Some("<static-init>")),
            K::FieldDecl | K::ConstantDecl => set(
                s,
                syntax::declarators(a)
                    .next()
                    .and_then(syntax::name_node)
                    .map(|n| self.text(n)),
            ),
            K::ClassBody | K::EnumBodyDecls => {
                if child.is_some_and(|c| k(c) == K::Block) {
                    set(s, Some("<instance-init>"));
                }
            }
            K::ObjectCreation => {
                if child.is_some_and(|c| k(c) == K::ClassBody) && s.frozen.is_none() {
                    s.frozen = Some(s.last);
                }
            }
            K::EnumConstant => {
                let name = syntax::name_node(a).map(|n| self.text(n));
                if child.is_some_and(|c| k(c) == K::ClassBody) {
                    if s.frozen.is_none() {
                        s.frozen = Some(name.or(s.last));
                    }
                } else {
                    set(s, name);
                }
            }
            _ if ka.is_type_decl() => {
                let types = {
                    let mut memo = self.type_paths.borrow_mut();
                    memo.entry(a.id())
                        .or_insert_with(|| {
                            let name = syntax::name_of(a, self.src);
                            match &s.types {
                                Some(outer) => format!("{outer}.{name}").into(),
                                None => name.into(),
                            }
                        })
                        .clone()
                };
                *s = SymState {
                    types: Some(types),
                    last: None,
                    frozen: None,
                };
            }
            _ => {}
        }
    }

    /// [`Ctx::symbol_of`] from the walker's per-node state: `None` when `node` is neither the
    /// node being checked, below it, nor one of its ancestors (or the state is not the walker's).
    pub(crate) fn symbol_from_state(&self, node: Node<'t>) -> Option<Option<String>> {
        self.state_of(node).map(|s| s.symbol())
    }

    /// What [`Ctx::symbol_of`] answers for `node`, unformatted: for a rule that keeps many
    /// declarations and reports few of them from `finish` (formatting the symbol of each is
    /// most of the cost of keeping it).
    pub(crate) fn symbol_key(&self, node: Node<'t>) -> SymbolKey<'t> {
        match self.state_of(node) {
            Some(s) => {
                #[cfg(debug_assertions)]
                if self.stack.len() <= 256 {
                    debug_assert_eq!(s.symbol(), self.symbol_of_walk(node), "symbol of {node:?}");
                }
                SymbolKey(Err(s))
            }
            None => SymbolKey(Ok(self.symbol_of_walk(node))),
        }
    }

    /// The walker's [`SymState`] of `node` after its own step (see [`Ctx::symbol_from_state`]).
    fn state_of(&self, node: Node<'t>) -> Option<SymState<'t>> {
        let cur = self.current?;
        let (cur_id, cur_state) = self.current_sym.as_ref()?;
        if *cur_id != cur.id() || self.sym_stack.len() != self.stack.len() {
            return None;
        }
        let mut s = if node.id() == cur.id() {
            cur_state.clone()
        } else if let Some(path) = (cur.start_byte() <= node.start_byte()
            && node.end_byte() <= cur.end_byte())
        .then(|| self.path_between(cur, node))
        .flatten()
        {
            // `path`: the named nodes from `cur` down to the parent of `node`.
            let mut s = cur_state.clone();
            for (i, &a) in path.iter().enumerate() {
                self.sym_step(&mut s, a, Some(path.get(i + 1).copied().unwrap_or(node)));
            }
            s
        } else {
            let pos = self.stack.iter().rposition(|n| n.id() == node.id())?;
            self.sym_stack.get(pos)?.clone()
        };
        self.sym_step(&mut s, node, None);
        Some(s)
    }

    /// [`Ctx::symbol_of`] by walking up from `node`.
    pub(crate) fn symbol_of_walk(&self, node: Node<'t>) -> Option<String> {
        let mut candidate: Option<&'t str> = None;
        let mut member: Option<&'t str> = None;
        let mut fixed = false;
        let mut types: Vec<&'t str> = Vec::new();
        let mut prev: Option<Node<'t>> = None;
        for a in std::iter::once(node).chain(self.ancestors(node)) {
            let ka = k(a);
            match ka {
                K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl => {
                    if candidate.is_none() {
                        candidate = syntax::name_node(a).map(|n| self.text(n));
                    }
                }
                K::StaticInitializer => {
                    if candidate.is_none() {
                        candidate = Some("<static-init>");
                    }
                }
                K::FieldDecl | K::ConstantDecl => {
                    if candidate.is_none() {
                        candidate = syntax::declarators(a)
                            .next()
                            .and_then(syntax::name_node)
                            .map(|n| self.text(n));
                    }
                }
                K::ClassBody | K::EnumBodyDecls => {
                    if candidate.is_none() && prev.is_some_and(|p| k(p) == K::Block) {
                        candidate = Some("<instance-init>");
                    }
                }
                K::ObjectCreation => {
                    if prev.is_some_and(|p| k(p) == K::ClassBody) {
                        candidate = None;
                    }
                }
                K::EnumConstant => {
                    if prev.is_some_and(|p| k(p) == K::ClassBody) || candidate.is_none() {
                        candidate = syntax::name_node(a).map(|n| self.text(n));
                    }
                }
                _ if ka.is_type_decl() => {
                    if !fixed {
                        member = candidate;
                        fixed = true;
                    }
                    types.push(syntax::name_of(a, self.src));
                }
                _ => {}
            }
            prev = Some(a);
        }
        if types.is_empty() {
            return None;
        }
        let mut s = String::with_capacity(32);
        for (i, t) in types.iter().rev().enumerate() {
            if i > 0 {
                s.push('.');
            }
            s.push_str(t);
        }
        if let Some(m) = member {
            s.push('#');
            s.push_str(m);
        }
        Some(s)
    }

    /// Whether `node` is inside a subtree of the given kind name, stopping at the first
    /// function or type boundary (`stop_at_function`) or not.
    pub fn is_inside(&self, node: Node<'t>, kind_name: &str, stop_at_function: bool) -> bool {
        for a in self.ancestors(node) {
            if crate::kinds::kind(a) == kind_name {
                return true;
            }
            if stop_at_function && (syntax::is_function_like(a) || k(a).is_type_body()) {
                return false;
            }
        }
        false
    }
}

/// A byte of a word for [`Ctx::word_offsets`].
/// The whole-word occurrences of every word of a source (see [`Ctx::word_offsets`]), stored
/// flat: one slice of `offsets` per distinct word, in source order (instead of a vector per
/// word, which costs an allocation or more per distinct word of every file indexed).
struct WordIndex<'t> {
    /// Word → its number (in order of first occurrence).
    ids: FxHashMap<&'t str, u32>,
    /// The offsets of word `i` are `offsets[starts[i]..starts[i + 1]]`.
    starts: Vec<u32>,
    offsets: Vec<u32>,
}

impl<'t> WordIndex<'t> {
    fn new(src: &'t str) -> Self {
        let mut ids: FxHashMap<&'t str, u32> = FxHashMap::default();
        // (word number, offset) of every occurrence, in source order.
        let mut occurrences: Vec<(u32, u32)> = Vec::with_capacity(src.len() / 8);
        let b = src.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if !is_word_byte(b[i]) {
                i += 1;
                continue;
            }
            let start = i;
            while i < b.len() && is_word_byte(b[i]) {
                i += 1;
            }
            // Runs start and end at character boundaries: non-ASCII bytes are word bytes.
            if let Some(word) = src.get(start..i) {
                let next = ids.len() as u32;
                let id = *ids.entry(word).or_insert(next);
                occurrences.push((id, start as u32));
            }
        }
        // Counting sort by word number; stable, so each word's offsets stay in source order.
        let mut starts = vec![0u32; ids.len() + 1];
        for &(id, _) in &occurrences {
            starts[id as usize + 1] += 1;
        }
        for w in 1..starts.len() {
            starts[w] += starts[w - 1];
        }
        let mut fill = starts.clone();
        let mut offsets = vec![0u32; occurrences.len()];
        for &(id, at) in &occurrences {
            let slot = &mut fill[id as usize];
            offsets[*slot as usize] = at;
            *slot += 1;
        }
        WordIndex {
            ids,
            starts,
            offsets,
        }
    }

    /// The offsets of `word`, in order (empty when it does not occur).
    fn get(&self, word: &str) -> &[u32] {
        let Some(&id) = self.ids.get(word) else {
            return &[];
        };
        let (a, b) = (self.starts[id as usize], self.starts[id as usize + 1]);
        self.offsets.get(a as usize..b as usize).unwrap_or(&[])
    }
}

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, ParseBudget};
    use mote_core::{RuleIndex, SourceFile};

    /// Columns agree with `mote_core::LineIndex` for any query order, on ASCII and
    /// non-ASCII lines, short and long (with column marks).
    #[test]
    fn columns_agree_with_the_core_line_index_in_any_order() {
        let long: String = (0..20_000)
            .map(|i| match i % 7 {
                0 => "é",
                1 => "😀",
                2 => "ab",
                _ => "x",
            })
            .collect();
        let src = format!(
            "plain ascii line\nnön ascii\n{long}\n{}\n\nend é",
            "y".repeat(9000)
        );
        let lines = Lines::new(&src);
        let core = mote_core::LineIndex::new(&src);
        let mut offsets: Vec<u32> = (0..src.len() as u32).step_by(97).collect();
        offsets.push(src.len() as u32);
        // Forward, backward, and interleaved queries.
        let mut order = offsets.clone();
        order.extend(offsets.iter().rev());
        order.extend(
            offsets
                .iter()
                .step_by(2)
                .zip(offsets.iter().rev())
                .flat_map(|(a, b)| [*a, *b]),
        );
        for off in order {
            assert_eq!(
                lines.line_col(&src, off),
                core.line_col(off),
                "offset {off}"
            );
        }
    }

    /// For the node being checked, the ancestors of any descendant (found by descending from
    /// that node) are exactly those found by descending from the root.
    #[test]
    fn ancestors_below_the_current_node_match_a_root_descent() {
        let src = "package p;\nclass A<T> extends B {\n  @SuppressWarnings(\"x\") int f = 1;\n  void m(int a) {\n    if (a > 0) { g(a, (a + 1) * 2); } else { new Object() { void h() { this.f = f; } }; }\n    for (var x : list) try { x.y().z(); } catch (Exception e) { }\n    Runnable r = () -> { int[] q = {1, 2}; };\n  }\n  enum E { X { int k; }, Y; int j; }\n}\n";
        let tree = parse(src, ParseBudget::DEFAULT, None).expect("parse");
        let file = SourceFile {
            path: "src/A.java",
            text: src,
            is_test: false,
        };
        let rules = RuleIndex::default();
        let mut ctx = Ctx::new(&file, tree.root_node(), &rules, &[]);
        let mut checked = 0;
        // Walk like the plugin: `current` is the named node being checked, `stack` its named
        // ancestors.
        let mut todo = vec![(tree.root_node(), Vec::new())];
        while let Some((node, stack)) = todo.pop() {
            ctx.current = Some(node);
            ctx.stack = stack.clone();
            let mut descendants = vec![node];
            let mut i = 0;
            while i < descendants.len() {
                let n = descendants[i];
                let mut c = n.walk();
                descendants.extend(n.children(&mut c));
                i += 1;
            }
            for d in descendants {
                let got: Vec<usize> = ctx.ancestors(d).map(|a| a.id()).collect();
                let want: Vec<usize> = ctx.path_to(d).iter().rev().map(|a| a.id()).collect();
                assert_eq!(got, want, "{} under {}", d.kind(), node.kind());
                checked += 1;
            }
            let mut inner = stack.clone();
            inner.push(node);
            let mut c = node.walk();
            for child in node.named_children(&mut c) {
                todo.push((child, inner.clone()));
            }
        }
        assert!(checked > 1_000, "{checked}");
    }

    /// Off the walk stack (as in `finish`), the ancestors found from the previous lookup's path
    /// are those found by descending from the root, whatever the order of the lookups.
    #[test]
    fn cached_root_paths_match_a_root_descent() {
        let src = "package p;\nclass A<T> extends B {\n  @SuppressWarnings(\"x\") int f = 1;\n  void m(int a) {\n    if (a > 0) { g(a, (a + 1) * 2); } else { new Object() { void h() { this.f = f; } }; }\n    for (var x : list) try { x.y().z(); } catch (Exception e) { }\n    Runnable r = () -> { int[] q = {1, 2}; };\n  }\n  enum E { X { int k; }, Y; int j; }\n}\nclass C { void n() { m(); } }\n";
        let tree = parse(src, ParseBudget::DEFAULT, None).expect("parse");
        let file = SourceFile {
            path: "src/A.java",
            text: src,
            is_test: false,
        };
        let rules = RuleIndex::default();
        let ctx = Ctx::new(&file, tree.root_node(), &rules, &[]);
        let mut nodes = vec![tree.root_node()];
        let mut i = 0;
        while i < nodes.len() {
            let n = nodes[i];
            let mut c = n.walk();
            nodes.extend(n.children(&mut c));
            i += 1;
        }
        // In tree order, in reverse, and interleaved from both ends.
        let mut order: Vec<Node<'_>> = nodes.clone();
        order.extend(nodes.iter().rev().copied());
        for k in 0..nodes.len() {
            order.push(nodes[k]);
            order.push(nodes[nodes.len() - 1 - k]);
        }
        for n in order {
            let got: Vec<usize> = ctx.ancestors(n).map(|a| a.id()).collect();
            let want: Vec<usize> = ctx.path_to(n).iter().rev().map(|a| a.id()).collect();
            assert_eq!(got, want, "{} at {:?}", n.kind(), n.start_position());
        }
    }

    /// The walk-stack sibling helpers and `nth`/`len` agree with tree-sitter / plain iteration
    /// on every node of `src`.
    fn assert_navigation_matches(src: &str, what: &str) -> usize {
        let Some(tree) = parse(src, ParseBudget::DEFAULT, None) else {
            return 0;
        };
        let file = SourceFile {
            path: "src/A.java",
            text: src,
            is_test: false,
        };
        let rules = RuleIndex::default();
        let mut ctx = Ctx::new(&file, tree.root_node(), &rules, &[]);
        let mut checked = 0;
        let mut todo = vec![(tree.root_node(), Vec::new())];
        while let Some((node, stack)) = todo.pop() {
            // As during the walk: `node` is current and its named ancestors are on the stack.
            ctx.current = Some(node);
            ctx.stack = stack.clone();
            let mut c = node.walk();
            let kids: Vec<Node<'_>> = node.children(&mut c).collect();
            for n in std::iter::once(node).chain(kids.iter().copied()) {
                assert_eq!(
                    ctx.next_named_sibling(n),
                    n.next_named_sibling(),
                    "{what}: next_named_sibling of {} at {:?}",
                    n.kind(),
                    n.start_position()
                );
                assert_eq!(ctx.prev_named_sibling(n), n.prev_named_sibling(), "{what}");
                assert_eq!(ctx.next_sibling(n), n.next_sibling(), "{what}");
                let all: Vec<usize> = ctx.ancestors(n).map(|a| a.id()).collect();
                assert_eq!(ctx.depth(n), all.len(), "{what}");
                for k in [0, 1, 2, 5] {
                    assert_eq!(
                        ctx.ancestors(n).nth(k).map(|a| a.id()),
                        all.get(k).copied(),
                        "{what}"
                    );
                }
                checked += 1;
            }
            let mut inner = stack.clone();
            inner.push(node);
            let mut c = node.walk();
            for child in node.named_children(&mut c) {
                todo.push((child, inner.clone()));
            }
        }
        checked
    }

    fn java_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                java_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "java") {
                out.push(p);
            }
        }
    }

    #[test]
    fn sibling_helpers_match_tree_sitter_on_fixtures() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut files = Vec::new();
        java_files(&dir, &mut files);
        let mut checked = 0;
        for f in &files {
            let src = std::fs::read_to_string(f).unwrap_or_default();
            checked += assert_navigation_matches(&src, &f.display().to_string());
        }
        assert!(checked > 10_000, "{checked}");
    }

    /// Same over part of a Java corpus (`MOTE_JAVA_CORPUS`; skipped when unset).
    #[test]
    #[ignore = "corpus check; run in release"]
    fn sibling_helpers_match_tree_sitter_on_corpus() {
        let Ok(root) = std::env::var("MOTE_JAVA_CORPUS") else {
            eprintln!("MOTE_JAVA_CORPUS not set; skipping");
            return;
        };
        let mut files = Vec::new();
        java_files(std::path::Path::new(&root), &mut files);
        files.sort();
        let mut checked = 0;
        for f in files.iter().step_by(7) {
            let bytes = std::fs::read(f).unwrap_or_default();
            let src = String::from_utf8_lossy(&bytes);
            checked += assert_navigation_matches(&src, &f.display().to_string());
        }
        eprintln!("{checked} nodes");
    }
}
