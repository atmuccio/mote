//! The analysis driver: parse, one iterative walk dispatching to rules and feeding metrics,
//! comments and suppressions, then `finish` for every enabled rule.

use crate::ctx::Ctx;
use crate::declared::DeclaredTypes;
use crate::kinds::{self, k, K};
use crate::metrics::Metrics;
use crate::parse::{self, ParseBudget};
use crate::rule::BoxedRule;
use crate::timing::{self, FileTimes, Phase};
use crate::{comments, rules, suppress};
use mote_core::{
    FileAnalysis, MemoKey, RuleIndex, RuleMeta, RuleSet, ScopedSuppression, SourceFile,
};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tree_sitter::{Node, TreeCursor};

/// Rule metadata and the kind → rules dispatch table, compiled once.
pub(crate) struct Registry {
    pub(crate) metas: Vec<&'static RuleMeta>,
    /// This registry's slot in a rule set's memo: its [`Resolved`] form of the set.
    key: MemoKey,
    /// `kind_id → indices of rules subscribed to that kind`.
    pub(crate) dispatch: Vec<Vec<u16>>,
    /// `(rule id, kind)` pairs naming kinds that are not named kinds of the grammar.
    pub(crate) unknown_kinds: Vec<(&'static str, &'static str)>,
    /// The rules to call for a named `method_invocation`, by name: see [`crate::JavaRule::call_names`].
    pub(crate) calls: NameDispatch,
    /// The rules to call for a named `method_declaration`, by name: see
    /// [`crate::JavaRule::method_names`].
    pub(crate) decls: NameDispatch,
}

/// Dispatch lists of one kind whose nodes have a name some rules filter on.
pub(crate) struct NameDispatch {
    /// Name → the rules to call for a node of that name (ascending): the subscribers without
    /// a name filter, and those whose filter accepts the name.
    by_name: FxHashMap<&'static str, Vec<u16>>,
    /// The rules to call for a name no filter lists: the subscribers without a filter.
    default: Vec<u16>,
}

impl NameDispatch {
    /// `subscribers`: the rules subscribed to the kind (ascending); `filtered[i]`: whether rule
    /// `i` declares a name filter; `accepting`: name → the rules whose filter lists it.
    fn new(
        subscribers: &[u16],
        filtered: &[bool],
        accepting: &FxHashMap<&'static str, Vec<u16>>,
    ) -> Self {
        let unfiltered = |ri: &u16| !filtered.get(*ri as usize).copied().unwrap_or(false);
        let default = subscribers.iter().copied().filter(unfiltered).collect();
        let by_name = accepting
            .iter()
            .map(|(name, rules)| {
                let list = subscribers
                    .iter()
                    .copied()
                    .filter(|ri| unfiltered(ri) || rules.contains(ri))
                    .collect();
                (*name, list)
            })
            .collect();
        NameDispatch { by_name, default }
    }

    fn get(&self, name: &str) -> &[u16] {
        self.by_name.get(name).map_or(&self.default, Vec::as_slice)
    }

    /// The same dispatch with only the rules `keep` accepts. Names whose list is then the
    /// default list are dropped (the lookup falls back to it anyway).
    fn restrict(&self, keep: impl Fn(u16) -> bool) -> NameDispatch {
        let filter =
            |list: &[u16]| -> Vec<u16> { list.iter().copied().filter(|ri| keep(*ri)).collect() };
        let default = filter(&self.default);
        let by_name = self
            .by_name
            .iter()
            .map(|(name, list)| (*name, filter(list)))
            .filter(|(_, list)| *list != default)
            .collect();
        NameDispatch { by_name, default }
    }

    /// Whether every name gets the default list — then no subscriber filters on names, so the
    /// default list is also the one for nodes without a name, and the name need not be read.
    fn is_uniform(&self) -> bool {
        self.by_name.is_empty()
    }
}

impl Registry {
    pub(crate) fn new() -> Self {
        let protos = rules::all::<'static>();
        let mut dispatch: Vec<Vec<u16>> = vec![Vec::new(); kinds::kind_count()];
        let mut unknown_kinds = Vec::new();
        let mut metas = Vec::with_capacity(protos.len());
        let mut call_filtered = Vec::with_capacity(protos.len());
        let mut call_rules: FxHashMap<&'static str, Vec<u16>> = FxHashMap::default();
        let mut decl_filtered = Vec::with_capacity(protos.len());
        let mut decl_rules: FxHashMap<&'static str, Vec<u16>> = FxHashMap::default();
        for (i, r) in protos.iter().enumerate() {
            metas.push(r.meta());
            call_filtered.push(!r.call_names().is_empty());
            for name in r.call_names() {
                let slot = call_rules.entry(name).or_default();
                if !slot.contains(&(i as u16)) {
                    slot.push(i as u16);
                }
            }
            decl_filtered.push(!r.method_names().is_empty());
            for name in r.method_names() {
                let slot = decl_rules.entry(name).or_default();
                if !slot.contains(&(i as u16)) {
                    slot.push(i as u16);
                }
            }
            for kind in r.kinds() {
                // Only named kinds are dispatched; anonymous tokens (`==`, `{`) never are.
                let ids = kinds::ids_for_kind(kind, true);
                if ids.is_empty() {
                    unknown_kinds.push((r.meta().id, *kind));
                }
                for id in ids {
                    if let Some(slot) = dispatch.get_mut(id as usize) {
                        if !slot.contains(&(i as u16)) {
                            slot.push(i as u16);
                        }
                    }
                }
            }
        }
        // A named call (method declaration) goes to the subscribers without a name filter and
        // to those whose filter accepts its name; the lists are merged once here, so the walk
        // iterates only the rules it calls. (Every id of a kind has the same subscribers.)
        let subscribers = |kind: &str| -> Vec<u16> {
            kinds::ids_for_kind(kind, true)
                .first()
                .and_then(|id| dispatch.get(*id as usize))
                .cloned()
                .unwrap_or_default()
        };
        let calls = NameDispatch::new(
            &subscribers("method_invocation"),
            &call_filtered,
            &call_rules,
        );
        let decls = NameDispatch::new(
            &subscribers("method_declaration"),
            &decl_filtered,
            &decl_rules,
        );
        Registry {
            metas,
            key: MemoKey::unique(),
            dispatch,
            unknown_kinds,
            calls,
            decls,
        }
    }
}

/// A rule set as the walk uses it, resolved once per set (`RuleSet::memo`) instead of once per
/// file: the enabled rules by registry index and the dispatch tables restricted to them, so
/// neither the setup nor the walk looks a rule id up or skips a disabled rule.
pub(crate) struct Resolved {
    /// The set in registry order: enabled rules and their parameters.
    pub(crate) index: RuleIndex,
    /// Indices of the enabled rules, ascending (the `finish` order).
    enabled: Vec<u16>,
    /// The enabled subscribers of kind id `k` are `flat[starts[k]..starts[k + 1]]`, ascending.
    starts: Vec<u32>,
    flat: Vec<u16>,
    calls: NameDispatch,
    decls: NameDispatch,
}

impl Resolved {
    fn new(reg: &Registry, rules: &RuleSet) -> Self {
        let index = rules.index(reg.metas.iter().map(|m| m.id));
        let on = |ri: u16| index.is_enabled(usize::from(ri));
        let enabled = index
            .enabled()
            .filter_map(|i| u16::try_from(i).ok())
            .collect();
        let mut starts = Vec::with_capacity(reg.dispatch.len() + 1);
        let mut flat = Vec::new();
        for list in &reg.dispatch {
            starts.push(flat.len() as u32);
            flat.extend(list.iter().copied().filter(|ri| on(*ri)));
        }
        starts.push(flat.len() as u32);
        Resolved {
            calls: reg.calls.restrict(on),
            decls: reg.decls.restrict(on),
            index,
            enabled,
            starts,
            flat,
        }
    }

    /// The registry's resolved form of `rules`, built by the first file that uses the set.
    pub(crate) fn of(reg: &Registry, rules: &RuleSet) -> Arc<Resolved> {
        rules.memo(reg.key, |rules| Resolved::new(reg, rules))
    }

    /// The enabled subscribers of a kind id.
    #[inline]
    fn subscribers(&self, kind_id: u16) -> &[u16] {
        let k = usize::from(kind_id);
        match (self.starts.get(k), self.starts.get(k + 1)) {
            (Some(&a), Some(&b)) => self.flat.get(a as usize..b as usize).unwrap_or(&[]),
            _ => &[],
        }
    }
}

struct Walker<'a, 't> {
    ctx: Ctx<'t>,
    rules: Vec<BoxedRule<'t>>,
    resolved: &'a Resolved,
    metrics: Metrics,
    suppressions: Vec<ScopedSuppression>,
    syntax_errors: u32,
    /// The tree contains ERROR/MISSING nodes (otherwise skip the per-node checks).
    has_errors: bool,
    /// Cursor depths of the named nodes on `ctx.stack` (so leaving a node needs no
    /// `cursor.node()` to know whether it was named).
    named_depths: Vec<u32>,
    /// Per-rule timing (`MOTE_RULE_TIMINGS`), off by default.
    timing: Option<FileTimes>,
}

impl<'t> Walker<'_, 't> {
    /// Entry event for the cursor's node at `depth`. Anonymous tokens (punctuation, keywords,
    /// operators) only feed the syntax-error count: nothing else needs them, and skipping them
    /// halves the per-node work.
    fn visit(&mut self, cursor: &TreeCursor<'t>, depth: u32) {
        let node = cursor.node();
        if !node.is_named() {
            if self.has_errors && node.is_missing() {
                self.syntax_errors += 1;
            }
            return;
        }
        let field_id = if self.metrics.in_function() {
            cursor.field_id().map(|f| f.get()).unwrap_or(0)
        } else {
            0
        };
        self.named_depths.push(depth);
        self.enter(node, field_id);
    }

    /// Exit event for the cursor's node at `depth`.
    fn unvisit(&mut self, depth: u32) {
        if self.named_depths.last() == Some(&depth) {
            self.named_depths.pop();
            if let Some(node) = self.ctx.stack.pop() {
                self.ctx.sym_stack.pop();
                self.ctx.struct_stack.pop();
                self.metrics.leave(node);
            }
        }
    }

    fn enter(&mut self, node: Node<'t>, field_id: u16) {
        if self.has_errors && (node.is_error() || node.is_missing()) {
            self.syntax_errors += 1;
        }
        let kk = k(node);
        self.ctx.current = Some(node);
        // The symbol state entering this node: its parent's, plus the parent's step towards it.
        let sym = match (self.ctx.stack.last(), self.ctx.sym_stack.last()) {
            (Some(&parent), Some(entering_parent)) => {
                let mut s = entering_parent.clone();
                self.ctx.sym_step(&mut s, parent, Some(node));
                s
            }
            _ => Default::default(),
        };
        self.ctx.current_sym = Some((node.id(), sym));
        #[cfg(test)]
        check_symbols(&self.ctx, node);
        match kk {
            K::LineComment => {
                comments::collect(&mut self.ctx, node);
                suppress::on_comment(&self.ctx, node, &mut self.suppressions);
            }
            K::BlockComment => comments::collect(&mut self.ctx, node),
            K::Annotation => suppress::on_annotation(&self.ctx, node, &mut self.suppressions),
            _ if DeclaredTypes::is_declaration(kk) => {
                let src = self.ctx.src;
                self.ctx.declared.collect(node, src);
            }
            _ => {}
        }
        self.metrics.enter(&self.ctx, node, kk, field_id);
        let resolved = self.resolved;
        let all = resolved.subscribers(node.kind_id());
        if !all.is_empty() && !self.rules.is_empty() {
            // A named call only goes to the rules whose `call_names` accept its name (and to
            // those without `call_names`); the name is read once for all of them.
            let list: &[u16] = match kk {
                K::MethodInvocation if !resolved.calls.is_uniform() => {
                    match self.ctx.call_name(node) {
                        Some(name) => resolved.calls.get(name),
                        None => all,
                    }
                }
                K::MethodDecl if !resolved.decls.is_uniform() => match self.ctx.name_node(node) {
                    Some(name) => resolved.decls.get(self.ctx.text(name)),
                    None => all,
                },
                _ => all,
            };
            for &ri in list {
                let ri = ri as usize;
                self.ctx.current_rule.set(ri);
                if let Some(rule) = self.rules.get_mut(ri) {
                    match self.timing.as_mut() {
                        None => rule.check(node, &self.ctx),
                        Some(t) => {
                            let start = Instant::now();
                            rule.check(node, &self.ctx);
                            let ns = start.elapsed().as_nanos() as u64;
                            if let (Some(c), Some(n)) =
                                (t.check_ns.get_mut(ri), t.calls.get_mut(ri))
                            {
                                *c += ns;
                                *n += 1;
                            }
                        }
                    }
                }
            }
        }
        let at = self.ctx.stack.len() as u32;
        let nearest = if crate::ctx::is_structural(kk) {
            at
        } else {
            self.ctx
                .struct_stack
                .last()
                .copied()
                .unwrap_or(crate::ctx::NO_STRUCTURAL)
        };
        self.ctx.struct_stack.push(nearest);
        self.ctx.stack.push(node);
        let sym = self
            .ctx
            .current_sym
            .as_ref()
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        self.ctx.sym_stack.push(sym);
    }
}

#[cfg(test)]
thread_local! {
    /// Compare the walker's symbols with the walk up for every node (see `check_symbols`).
    pub(crate) static CHECK_SYMBOLS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Nodes compared so far.
    pub(crate) static SYMBOLS_CHECKED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Test builds: when [`CHECK_SYMBOLS`] is set, the symbol of the node being checked, of its
/// named children and of its parent, read from the walker's state, must equal the walk up.
#[cfg(test)]
fn check_symbols<'t>(ctx: &Ctx<'t>, node: Node<'t>) {
    if !CHECK_SYMBOLS.with(|c| c.get()) {
        return;
    }
    let mut nodes = vec![node];
    nodes.extend(crate::syntax::named_children(node));
    nodes.extend(ctx.stack.last().copied());
    for n in nodes {
        assert_eq!(
            ctx.symbol_from_state(n)
                .unwrap_or_else(|| panic!("not covered: {n:?} under {node:?}")),
            ctx.symbol_of_walk(n),
            "{n:?} at {}",
            n.start_byte()
        );
        // The structural ancestors (jumping on the walk stack) are the ancestors of those kinds.
        let mut child = n;
        let mut expected = Vec::new();
        for a in ctx.ancestors(n) {
            if crate::ctx::is_structural(crate::kinds::k(a)) {
                expected.push((a.id(), child.id()));
            }
            child = a;
        }
        let got: Vec<(usize, usize)> = ctx
            .structural_ancestors(n)
            .map(|(a, c)| (a.id(), c.id()))
            .collect();
        assert_eq!(got, expected, "structural ancestors of {n:?}");
        // The sibling helpers (an index per parent) agree with tree-sitter's own.
        assert_eq!(
            ctx.next_sibling(n),
            n.next_sibling(),
            "next sibling of {n:?}"
        );
        assert_eq!(
            ctx.next_named_sibling(n),
            n.next_named_sibling(),
            "next named sibling of {n:?}"
        );
        assert_eq!(
            ctx.prev_named_sibling(n),
            n.prev_named_sibling(),
            "previous named sibling of {n:?}"
        );
        SYMBOLS_CHECKED.with(|c| c.set(c.get() + 1));
    }
}

/// Syntax trees deeper than this are not analysed; the file is reported with one syntax error,
/// like a parse over budget ([`unanalysable`]). Real code stays below ~100 levels (SymmetricDS:
/// 92); only generated or adversarial input gets near. The walk itself is iterative, but rules
/// and symbols look at ancestors, so the cost per finding grows with depth: the cap keeps a file
/// of nothing but nesting (e.g. 1 MB of `if (b) {`) linear instead of quadratic.
const MAX_TREE_DEPTH: u32 = 10_000;

/// Result for a file that could not be analysed (parse over budget, tree too deep): one syntax
/// error and nothing else.
fn unanalysable() -> FileAnalysis {
    FileAnalysis {
        syntax_errors: 1,
        ..FileAnalysis::default()
    }
}

pub(crate) fn analyze(
    reg: &Registry,
    file: &SourceFile<'_>,
    ruleset: &RuleSet,
    budget: ParseBudget,
    timeout: Option<Duration>,
) -> FileAnalysis {
    let mut timing = timing::enabled().then(|| FileTimes::new(reg.metas.len()));
    let mut clock = timing.as_ref().map(|_| Instant::now());
    // Adds the time since the previous lap to phase `p` (only when timing).
    let mut lap = |timing: &mut Option<FileTimes>, p: Phase| {
        if let (Some(t), Some(c)) = (timing.as_mut(), clock.as_mut()) {
            let now = Instant::now();
            t.add_phase(p, now - *c);
            *c = now;
        }
    };
    let Some(tree) = parse::parse(file.text, budget, timeout) else {
        return unanalysable();
    };
    lap(&mut timing, Phase::Parse);
    let root = tree.root_node();
    let resolved = Resolved::of(reg, ruleset);
    let mut instances: Vec<BoxedRule<'_>> = if resolved.enabled.is_empty() {
        Vec::new()
    } else {
        rules::all()
    };
    debug_assert!(instances.is_empty() || instances.len() == reg.metas.len());
    if instances.len() != reg.metas.len() {
        instances.clear();
    }
    let mut w = Walker {
        ctx: Ctx::new(file, root, &resolved.index, &reg.metas),
        rules: instances,
        resolved: &resolved,
        metrics: Metrics::default(),
        suppressions: Vec::new(),
        syntax_errors: 0,
        has_errors: root.has_error(),
        named_depths: Vec::with_capacity(64),
        timing,
    };
    if w.timing.is_some() {
        w.ctx.prof = Some(Box::default());
    }
    lap(&mut w.timing, Phase::Setup);

    // One iterative pre-order walk with enter/leave events.
    let mut cursor = root.walk();
    let mut depth = 0u32;
    w.named_depths.push(0);
    w.enter(root, 0);
    'walk: loop {
        if cursor.goto_first_child() {
            depth += 1;
            if depth > MAX_TREE_DEPTH {
                return unanalysable();
            }
            w.visit(&cursor, depth);
            continue;
        }
        loop {
            w.unvisit(depth);
            if cursor.goto_next_sibling() {
                w.visit(&cursor, depth);
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
            depth -= 1;
        }
    }

    lap(&mut w.timing, Phase::Walk);
    if let Some(t) = w.timing.as_mut() {
        // The walk lap includes the `check` calls: move them to their own phase.
        t.move_checks_out_of_walk();
    }

    // File-level hooks.
    w.ctx.current = None;
    w.ctx.stack.clear();
    w.ctx.sym_stack.clear();
    w.ctx.struct_stack.clear();
    w.ctx.current_sym = None;
    for &i in &resolved.enabled {
        let i = usize::from(i);
        let Some(rule) = w.rules.get_mut(i) else {
            continue;
        };
        w.ctx.current_rule.set(i);
        match w.timing.as_mut() {
            None => rule.finish(&w.ctx),
            Some(t) => {
                let start = Instant::now();
                rule.finish(&w.ctx);
                if let Some(f) = t.finish_ns.get_mut(i) {
                    *f += start.elapsed().as_nanos() as u64;
                }
            }
        }
    }
    lap(&mut w.timing, Phase::Finish);

    let Walker {
        ctx,
        metrics,
        suppressions,
        syntax_errors,
        mut timing,
        ..
    } = w;
    let mut findings = ctx.findings.into_inner();
    findings.sort_by(|a, b| {
        (
            a.range.start_line,
            a.range.start_col,
            &a.rule_id,
            &a.message,
        )
            .cmp(&(
                b.range.start_line,
                b.range.start_col,
                &b.rule_id,
                &b.message,
            ))
    });
    let analysis = FileAnalysis {
        findings,
        functions: metrics.functions,
        comments: ctx.comments,
        suppressions,
        classes: metrics.classes,
        statements: metrics.statements,
        syntax_errors,
        facts: None,
    };
    lap(&mut timing, Phase::Collect);
    if let Some(t) = timing {
        t.merge(&reg.metas, ctx.prof.as_deref());
    }
    analysis
}

#[cfg(test)]
mod tests {
    use crate::{JavaPlugin, JAVA_RELEASE_PARAM};
    use mote_core::testing::ruleset_for;
    use mote_core::{LanguagePlugin, ParamValue, RuleSet, SourceFile};
    use std::collections::BTreeMap;

    const SRC: &str = "class A {
  int a, b, c;
  void f(Object o) {
    try { g(); } catch (Exception e) { }
    if (o instanceof String) { String s = (String) o; s.length(); }
  }
}
";

    /// The symbols the walker keeps per node equal the walk up from the node, for every named
    /// node of every fixture (the node, its children and its parent: each way they are read).
    #[test]
    fn walker_symbols_equal_the_walk_up() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .expect("fixtures")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "java"))
            .collect();
        paths.sort();
        let extra = [
            "class A { class B { void m() { new Object() { void n() { int x = 1; } }; } } \
             enum E { X { void f() {} }, Y(1); int k; E() {} E(int a) {} } \
             static { int s; } { int i; } interface I { int C = 1; void d(); } \
             record R(int a) { R { } } @interface Q { int v(); } }"
                .to_string(),
            "enum E { A { class In { void g() { new Runnable() { public void run() {} }; } } } }"
                .to_string(),
        ];
        let plugin = JavaPlugin::new();
        let rules = mote_core::testing::ruleset_all(&plugin);
        super::CHECK_SYMBOLS.with(|c| c.set(true));
        let texts = paths
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap_or_default())
            .chain(extra);
        for text in texts {
            let file = SourceFile {
                path: "src/p/A.java",
                text: &text,
                is_test: false,
            };
            plugin.analyze(&file, &rules);
        }
        super::CHECK_SYMBOLS.with(|c| c.set(false));
        let checked = super::SYMBOLS_CHECKED.with(|c| c.get());
        assert!(checked > 100_000, "only {checked} nodes checked");
    }

    fn set(plugin: &JavaPlugin, ids: &[&str], max: i64, release: i64) -> RuleSet {
        let params = BTreeMap::from([
            ("max".to_string(), ParamValue::Int(max)),
            (JAVA_RELEASE_PARAM.to_string(), ParamValue::Int(release)),
        ]);
        ruleset_for(&plugin.rules(), ids, &params)
    }

    fn found(plugin: &JavaPlugin, rules: &RuleSet) -> Vec<String> {
        let file = SourceFile {
            path: "src/A.java",
            text: SRC,
            is_test: false,
        };
        let mut ids: Vec<String> = plugin
            .analyze(&file, rules)
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn each_rule_set_is_resolved_on_its_own() {
        // One plugin, two rule sets used alternately (each resolved once, memoized in the set):
        // enabled rules, parameters and the Java release must all follow the set of the call.
        // A second plugin instance sharing the sets gets the same answers.
        let plugin = JavaPlugin::new();
        let other = JavaPlugin::new();
        let pattern = "java/instanceof-pattern-candidate";
        let a = set(&plugin, &["java/too-many-fields", pattern], 2, 11);
        let b = set(
            &plugin,
            &["java/empty-catch-block", "java/too-many-fields", pattern],
            5,
            17,
        );
        let expect_a = vec!["java/too-many-fields".to_string()];
        let expect_b = vec!["java/empty-catch-block".to_string(), pattern.to_string()];
        for _ in 0..2 {
            assert_eq!(found(&plugin, &a), expect_a);
            assert_eq!(found(&plugin, &b), expect_b);
            assert_eq!(found(&other, &b), expect_b);
            assert_eq!(found(&other, &a), expect_a);
        }
        assert!(found(&plugin, &RuleSet::default()).is_empty());
    }
}
