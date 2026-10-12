//! The rule-author contract ([`JsRule`]) and the dispatch table built from it.

use crate::ctx::Ctx;
use mote_core::{MemoKey, RuleIndex, RuleMeta, RuleSet};
use oxc_ast::ast_kind::AST_TYPE_MAX;
use oxc_ast::AstType;
use oxc_semantic::AstNode;
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// A JavaScript/TypeScript rule.
///
/// Rules are stateless, shared between threads, and run once per file:
///
/// 1. [`applies_to`](JsRule::applies_to) is asked once per file (cheap gate: test files,
///    TypeScript-only rules, …). Returning `false` skips the rule for the whole file.
/// 2. [`run`](JsRule::run) is called for every node whose [`AstType`] is listed in
///    [`types`](JsRule::types), in source (pre-)order, during the plugin's single pass over the
///    tree. A rule that only cares about calls to certain names narrows its `CallExpression` /
///    `NewExpression` nodes further with [`callee_names`](JsRule::callee_names).
/// 3. [`run_once`](JsRule::run_once) is called after the pass — for file-level rules and for rules
///    that collected state with [`Ctx::state`].
///
/// Per-file state never lives in the rule struct; use [`Ctx::state`].
pub trait JsRule: Send + Sync {
    /// Static metadata (id, severity, description, parameters). Declare it as a `static` beside the
    /// rule.
    fn meta(&self) -> &'static RuleMeta;

    /// Node types this rule wants to see in [`run`](JsRule::run). Return `&[]` for rules that only
    /// implement [`run_once`](JsRule::run_once).
    fn types(&self) -> &'static [AstType] {
        &[]
    }

    /// The callee names [`run`](JsRule::run) can act on for `CallExpression` and `NewExpression`
    /// nodes, as [`helpers::callee_name`](crate::helpers::callee_name) reads them (`f` in `f()`,
    /// `m` in `a.b.m()`, `a?.m()`, `a['m']()`, `new m()`). When non-empty, `run` is only called
    /// for a subscribed call or `new` expression whose callee name is listed: the plugin looks the
    /// name up once per node instead of asking every rule. Other subscribed node types are
    /// unaffected. List every name under which `run` may emit or record anything; leave it empty
    /// (the default) when any callee can matter.
    fn callee_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Per-file gate, evaluated before the traversal. Default: always run.
    fn applies_to(&self, _ctx: &Ctx<'_>) -> bool {
        true
    }

    /// Called for each subscribed node.
    fn run<'a>(&self, _node: &AstNode<'a>, _ctx: &mut Ctx<'a>) {}

    /// Called once per file after the traversal.
    fn run_once<'a>(&self, _ctx: &mut Ctx<'a>) {}
}

/// All rules plus the node-type → rule dispatch table. Built once per plugin instance.
pub(crate) struct Registry {
    pub rules: Vec<Box<dyn JsRule>>,
    /// `rules[i].meta()`, so the dispatch loop needs no virtual call to select a rule.
    pub metas: Vec<&'static RuleMeta>,
    /// `rules[i].types()` is non-empty.
    pub subscribed: Vec<bool>,
    /// Indexed by `AstType as usize`; lists rule indices subscribed to that type (for calls and
    /// `new` expressions, those without [`JsRule::callee_names`]).
    pub table: Vec<Box<[u16]>>,
    /// Rules with [`JsRule::callee_names`] by callee name, for `CallExpression` (`[0]`) and
    /// `NewExpression` (`[1]`) nodes, in registry order.
    pub by_callee: [FxHashMap<&'static str, Box<[u16]>>; 2],
    /// This registry's slot in a rule set's memo: its [`Resolved`] form of the set.
    key: MemoKey,
}

/// Slot of a node type in [`Registry::by_callee`].
fn callee_slot(ty: AstType) -> Option<usize> {
    match ty {
        AstType::CallExpression => Some(0),
        AstType::NewExpression => Some(1),
        _ => None,
    }
}

impl Registry {
    pub fn new(rules: Vec<Box<dyn JsRule>>) -> Self {
        let mut table: Vec<Vec<u16>> = vec![Vec::new(); AST_TYPE_MAX as usize + 1];
        let mut by_callee: [FxHashMap<&'static str, Vec<u16>>; 2] = Default::default();
        for (i, rule) in rules.iter().enumerate() {
            let index = u16::try_from(i).unwrap_or(u16::MAX);
            let names = rule.callee_names();
            for ty in rule.types() {
                if let (Some(slot), false) = (callee_slot(*ty), names.is_empty()) {
                    for name in names {
                        let list = by_callee[slot].entry(name).or_default();
                        if !list.contains(&index) {
                            list.push(index);
                        }
                    }
                    continue;
                }
                let slot = &mut table[*ty as usize];
                if !slot.contains(&index) {
                    slot.push(index);
                }
            }
        }
        let by_callee = by_callee.map(|map| {
            map.into_iter()
                .map(|(name, list)| (name, list.into_boxed_slice()))
                .collect()
        });
        Registry {
            key: MemoKey::unique(),
            by_callee,
            metas: rules.iter().map(|r| r.meta()).collect(),
            subscribed: rules.iter().map(|r| !r.types().is_empty()).collect(),
            rules,
            table: table.into_iter().map(Vec::into_boxed_slice).collect(),
        }
    }

    pub fn metas(&self) -> Vec<&'static RuleMeta> {
        self.metas.clone()
    }

    /// Rule ids in registry order.
    pub fn ids(&self) -> Vec<&'static str> {
        self.metas.iter().map(|m| m.id).collect()
    }
}

/// A rule set as the plugin uses it, resolved once per set (`RuleSet::memo`) instead of once per
/// file: the enabled rules by registry index, and the dispatch tables restricted to them. Per
/// file only the [`JsRule::applies_to`] gates remain to evaluate.
pub(crate) struct Resolved {
    /// The set in registry order: enabled rules and their parameters.
    pub index: RuleIndex,
    /// Indices of the enabled rules, ascending.
    pub enabled: Vec<u16>,
    /// The enabled subscribers of node type `t` (as [`Registry::table`]) are
    /// `flat[starts[t]..starts[t + 1]]`, in registry order.
    pub starts: Vec<u32>,
    pub flat: Vec<u16>,
    /// [`Registry::by_callee`] restricted to the enabled rules (names left without one dropped).
    pub by_callee: [FxHashMap<&'static str, Box<[u16]>>; 2],
    /// The enabled rules listed in `by_callee`, ascending.
    pub keyed: Vec<u16>,
    /// The names of `by_callee`, as filters that reject most other names before hashing.
    pub callee_filter: [crate::helpers::NameFilter; 2],
    /// Per rule (registry order): what it derived once from its parameters for this set (see
    /// [`crate::Ctx::rule_memo`]).
    pub memos: Vec<std::sync::OnceLock<RuleMemo>>,
}

/// A value a rule computes once per rule set ([`crate::Ctx::rule_memo`]).
pub(crate) type RuleMemo =
    Box<dyn std::any::Any + Send + Sync + std::panic::UnwindSafe + std::panic::RefUnwindSafe>;

impl Resolved {
    fn new(registry: &Registry, rules: &RuleSet) -> Self {
        let index = rules.index(registry.metas.iter().map(|m| m.id));
        let on = |i: u16| index.is_enabled(usize::from(i));
        let enabled = index
            .enabled()
            .filter_map(|i| u16::try_from(i).ok())
            .collect();
        let mut starts = Vec::with_capacity(registry.table.len() + 1);
        let mut flat = Vec::new();
        for list in &registry.table {
            starts.push(flat.len() as u32);
            flat.extend(list.iter().copied().filter(|&i| on(i)));
        }
        starts.push(flat.len() as u32);
        let by_callee = registry.by_callee.each_ref().map(|map| {
            map.iter()
                .filter_map(|(name, list)| {
                    let kept: Box<[u16]> = list.iter().copied().filter(|&i| on(i)).collect();
                    (!kept.is_empty()).then_some((*name, kept))
                })
                .collect::<FxHashMap<_, _>>()
        });
        let callee_filter = by_callee
            .each_ref()
            .map(|map| crate::helpers::NameFilter::new(map.keys().copied()));
        let mut keyed: Vec<u16> = by_callee
            .iter()
            .flat_map(|map| map.values().flat_map(|list| list.iter().copied()))
            .collect();
        keyed.sort_unstable();
        keyed.dedup();
        Resolved {
            memos: (0..registry.rules.len())
                .map(|_| std::sync::OnceLock::new())
                .collect(),
            index,
            enabled,
            starts,
            flat,
            by_callee,
            callee_filter,
            keyed,
        }
    }

    /// The registry's resolved form of `rules`, built by the first file that uses the set.
    pub fn of(registry: &Registry, rules: &RuleSet) -> Arc<Resolved> {
        rules.memo(registry.key, |rules| Resolved::new(registry, rules))
    }

    /// The enabled subscribers of node type `ty` (`AstType as usize`).
    #[inline]
    pub fn subscribers(&self, ty: usize) -> &[u16] {
        match (self.starts.get(ty), self.starts.get(ty + 1)) {
            (Some(&a), Some(&b)) => self.flat.get(a as usize..b as usize).unwrap_or(&[]),
            _ => &[],
        }
    }
}
