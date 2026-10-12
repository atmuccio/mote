//! Per-function metrics computed during the single tree walk.
//!
//! * One [`FunctionMetric`] per method, constructor, compact constructor, static initializer,
//!   and instance initializer block that is **not nested inside another function**. Lambdas and
//!   the methods of local/anonymous classes are folded into the enclosing function (they raise
//!   the nesting level and their decisions count toward it).
//! * **Cyclomatic**: 1 + `if` + `for` + enhanced `for` + `while` + `do` + non-default case
//!   labels + `catch` + `?:` + each `&&`/`||`.
//! * **Cognitive** (after G. Ann Campbell's published method): +1 plus the current nesting
//!   level for `if`, `?:`, `switch`, loops and `catch`; +1 (no nesting) for `else if`, `else`,
//!   labelled `break`/`continue`, and each run of identical boolean operators in a condition
//!   (parentheses and `!` do not break a run: `a && !(b && c) || d` counts 2). Nesting grows
//!   inside the bodies of `if`/`else`, loops, `switch`, `?:`, `catch`, lambdas, and nested
//!   methods; `try`/`finally` do not nest.
//! * **Max nesting**: deepest control-flow depth of an `if`, loop, `switch`, or `try`, plus one.
//!   The depth follows the cognitive nesting, except that everything inside a `try` (its body,
//!   `catch` clauses and `finally`) is one level deeper, as in the JS plugin, while a `catch`
//!   body adds no further level: five nested `try` blocks are five levels deep.
//! * **Statements**: executable statements (blocks, labels, and empty `;` excluded).

use crate::ctx::Ctx;
use crate::kinds::{k, kind, K, TABLES};
use crate::rule::Span;
use crate::syntax;
use crate::syntax::{Field, NodeExt};
use mote_core::FunctionMetric;
use tree_sitter::Node;

#[derive(Default)]
pub(crate) struct Metrics {
    frame: Option<(usize, FunctionMetric)>,
    /// Nesting level of every entered node (parallel to the walk stack).
    levels: Vec<u32>,
    /// Control-flow depth of every entered node (parallel to `levels`), for max nesting.
    depths: Vec<u32>,
    pub(crate) functions: Vec<FunctionMetric>,
    pub(crate) statements: u32,
    pub(crate) classes: u32,
}

fn is_logical(node: Node<'_>) -> bool {
    k(node) == K::BinaryExpression
        && node
            .child_by_field_id(TABLES.f_operator)
            .is_some_and(|op| matches!(kind(op), "&&" | "||"))
}

fn is_statement(kk: K, parent: K) -> bool {
    match kk {
        K::ExpressionStatement
        | K::IfStatement
        | K::ForStatement
        | K::EnhancedForStatement
        | K::WhileStatement
        | K::DoStatement
        | K::ReturnStatement
        | K::BreakStatement
        | K::ContinueStatement
        | K::ThrowStatement
        | K::TryStatement
        | K::TryWithResourcesStatement
        | K::SynchronizedStatement
        | K::AssertStatement
        | K::YieldStatement
        | K::ExplicitConstructorInvocation => true,
        K::LocalVarDecl => parent != K::ForStatement,
        K::SwitchExpression => matches!(
            parent,
            K::Block
                | K::ConstructorBody
                | K::SwitchBlockStatementGroup
                | K::LabeledStatement
                | K::IfStatement
                | K::ForStatement
                | K::EnhancedForStatement
                | K::WhileStatement
                | K::DoStatement
                | K::Program
        ),
        _ => false,
    }
}

/// Number of runs of identical operators in the boolean cluster rooted at `root`.
fn logical_runs(root: Node<'_>) -> u32 {
    let mut ops: Vec<(usize, bool)> = Vec::new();
    let mut stack = vec![root];
    let mut guard = 0;
    while let Some(n) = stack.pop() {
        guard += 1;
        if guard > 100_000 {
            break;
        }
        match k(n) {
            K::BinaryExpression if is_logical(n) => {
                if let Some(op) = n.child_by_field_id(TABLES.f_operator) {
                    ops.push((op.start_byte(), kind(op) == "&&"));
                }
                if let Some(l) = n.child_by_field_id(TABLES.f_left) {
                    stack.push(l);
                }
                if let Some(r) = n.child_by_field_id(TABLES.f_right) {
                    stack.push(r);
                }
            }
            K::ParenthesizedExpression | K::UnaryExpression => {
                stack.extend(syntax::named_children(n))
            }
            _ => {}
        }
    }
    ops.sort_unstable();
    let mut runs = 0;
    let mut last: Option<bool> = None;
    for (_, and) in ops {
        if last != Some(and) {
            runs += 1;
            last = Some(and);
        }
    }
    runs
}

fn has_label(jump: Node<'_>) -> bool {
    syntax::named_children(jump).any(|c| kind(c) == "identifier")
}

fn is_default_label(label: Node<'_>) -> bool {
    syntax::children(label)
        .next()
        .is_some_and(|c| kind(c) == "default")
}

impl Metrics {
    /// Whether a function frame is open (field ids are only needed then).
    pub(crate) fn in_function(&self) -> bool {
        self.frame.is_some()
    }

    fn start_frame<'t>(&mut self, ctx: &Ctx<'t>, node: Node<'t>, kk: K) {
        let params = match kk {
            K::MethodDecl | K::ConstructorDecl => syntax::parameters(node).count() as u32,
            K::CompactConstructorDecl => {
                // The record's components: compact_constructor → class_body → record_declaration.
                let n = ctx.stack.len();
                ctx.stack
                    .get(n.wrapping_sub(2))
                    .map(|rec| syntax::parameters(*rec).count() as u32)
                    .unwrap_or(0)
            }
            _ => 0,
        };
        let anchor = match kk {
            K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl => {
                node.field(Field::Name)
            }
            _ => syntax::children(node).next(),
        }
        .unwrap_or(node);
        let metric = FunctionMetric {
            name: ctx
                .symbol_of(node)
                .unwrap_or_else(|| "<anonymous>".to_string()),
            range: ctx.range_of(Span::from(anchor)),
            start_line: node.start_position().row as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
            cyclomatic: 1,
            cognitive: 0,
            params,
            max_nesting: 0,
            statements: 0,
        };
        self.frame = Some((node.id(), metric));
    }

    /// Called for every node on entry; `ctx.stack` holds its ancestors.
    pub(crate) fn enter<'t>(&mut self, ctx: &Ctx<'t>, node: Node<'t>, kk: K, field_id: u16) {
        let parent = ctx.stack.last().copied();
        let pk = parent.map(k).unwrap_or(K::Other);
        let parent_level = self.levels.last().copied().unwrap_or(0);
        let parent_depth = self.depths.last().copied().unwrap_or(0);
        if kk.is_type_decl() {
            self.classes += 1;
        }
        let stmt = is_statement(kk, pk);
        if stmt {
            self.statements += 1;
        }
        let Some((root_id, m)) = self.frame.as_mut() else {
            if kk.is_function() || (kk == K::Block && matches!(pk, K::ClassBody | K::EnumBodyDecls))
            {
                self.start_frame(ctx, node, kk);
            }
            self.levels.push(0);
            self.depths.push(0);
            return;
        };
        if stmt {
            m.statements += 1;
        }
        let t = &*TABLES;
        let mut level = parent_level;
        match pk {
            K::IfStatement => {
                if field_id == t.f_consequence
                    || (field_id == t.f_alternative && kk != K::IfStatement)
                {
                    level += 1;
                }
            }
            K::ForStatement
            | K::EnhancedForStatement
            | K::WhileStatement
            | K::DoStatement
            | K::SwitchExpression
            | K::CatchClause
            | K::Lambda => {
                if field_id == t.f_body {
                    level += 1;
                }
            }
            K::TernaryExpression => {
                if field_id == t.f_consequence || field_id == t.f_alternative {
                    level += 1;
                }
            }
            K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl => {
                if field_id == t.f_body && parent.is_some_and(|p| p.id() != *root_id) {
                    level += 1;
                }
            }
            K::ClassBody | K::EnumBodyDecls
                // Instance initializer of a nested (anonymous/local) class.
                if kk == K::Block => {
                    level += 1;
                }
            _ => {}
        }
        // The control-flow depth rises like the nesting level, except inside a `try` (every
        // child: body, resources, `catch`, `finally`), and not again for a `catch` body.
        let mut depth = parent_depth + (level - parent_level);
        match pk {
            K::TryStatement | K::TryWithResourcesStatement => depth += 1,
            K::CatchClause if field_id == t.f_body => depth = parent_depth,
            _ => {}
        }
        let else_branch = pk == K::IfStatement && field_id == t.f_alternative;
        if else_branch && kk != K::IfStatement {
            m.cognitive += 1;
        }
        let mut structural = false;
        match kk {
            K::IfStatement => {
                m.cyclomatic += 1;
                m.cognitive += if else_branch { 1 } else { 1 + level };
                structural = true;
            }
            K::ForStatement | K::EnhancedForStatement | K::WhileStatement | K::DoStatement => {
                m.cyclomatic += 1;
                m.cognitive += 1 + level;
                structural = true;
            }
            // Its `try` already counts the depth.
            K::CatchClause => {
                m.cyclomatic += 1;
                m.cognitive += 1 + level;
            }
            K::SwitchExpression => {
                m.cognitive += 1 + level;
                structural = true;
            }
            K::TryStatement | K::TryWithResourcesStatement => structural = true,
            K::TernaryExpression => {
                m.cyclomatic += 1;
                m.cognitive += 1 + level;
            }
            K::SwitchLabel => {
                if !is_default_label(node) {
                    m.cyclomatic += 1;
                }
            }
            K::BreakStatement | K::ContinueStatement => {
                if has_label(node) {
                    m.cognitive += 1;
                }
            }
            K::BinaryExpression if is_logical(node) => {
                m.cyclomatic += 1;
                let cluster_root = !ctx
                    .stack
                    .iter()
                    .rev()
                    .find(|a| !matches!(k(**a), K::ParenthesizedExpression | K::UnaryExpression))
                    .is_some_and(|a| is_logical(*a));
                if cluster_root {
                    m.cognitive += logical_runs(node);
                }
            }
            _ => {}
        }
        if structural {
            m.max_nesting = m.max_nesting.max(depth + 1);
        }
        self.levels.push(level);
        self.depths.push(depth);
    }

    /// Called for every node on exit.
    pub(crate) fn leave(&mut self, node: Node<'_>) {
        self.levels.pop();
        self.depths.pop();
        if self.frame.as_ref().is_some_and(|(id, _)| *id == node.id()) {
            if let Some((_, m)) = self.frame.take() {
                self.functions.push(m);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::JavaPlugin;
    use mote_core::{FunctionMetric, LanguagePlugin, RuleSet, SourceFile};

    fn metrics(src: &str) -> Vec<FunctionMetric> {
        let plugin = JavaPlugin::new();
        let file = SourceFile {
            path: "src/T.java",
            text: src,
            is_test: false,
        };
        plugin.analyze(&file, &RuleSet::default()).functions
    }

    fn one(src: &str) -> FunctionMetric {
        let all = metrics(src);
        assert_eq!(all.len(), 1, "{all:#?}");
        all.into_iter().next().unwrap()
    }

    #[test]
    fn straight_line_code() {
        let m = one("class A { int f(int a, int b) { int c = a + b; return c; } }");
        assert_eq!(m.name, "A#f");
        assert_eq!(
            (
                m.cyclomatic,
                m.cognitive,
                m.max_nesting,
                m.params,
                m.statements
            ),
            (1, 0, 0, 2, 2)
        );
        assert_eq!(m.range.start_line, 1);
    }

    #[test]
    fn if_else_if_else_chain() {
        // if +1, else if +1, else +1 → 3; cyclomatic 1 + 2 ifs = 3
        let m = one(
            "class A { void f(int x) {\n if (x > 0) { a(); }\n else if (x < 0) { b(); }\n else { c(); }\n} }",
        );
        assert_eq!(m.cognitive, 3);
        assert_eq!(m.cyclomatic, 3);
        assert_eq!(m.max_nesting, 1);
    }

    #[test]
    fn nested_structures_add_nesting() {
        // for +1; if (nest 1) +2; while (nest 2) +3; && run +1 → 7
        let m = one(
            "class A { void f(int[] xs) {\n for (int x : xs) {\n  if (x > 0 && x < 9) {\n   while (x-- > 0) { g(); }\n  }\n }\n} }",
        );
        assert_eq!(m.cognitive, 7);
        assert_eq!(m.cyclomatic, 5); // 1 + for + if + && + while
        assert_eq!(m.max_nesting, 3);
    }

    #[test]
    fn boolean_operator_runs() {
        // if +1; `a && b && c || d || e && f` → runs &&, ||, && → +3
        let m = one("class A { void f(boolean a, boolean b, boolean c, boolean d, boolean e, boolean g) {\n if (a && b && c || d || e && g) { h(); }\n} }");
        assert_eq!(m.cognitive, 4);
        // Parentheses and negation don't break a run: `a && !(b && c)` is one run.
        let m = one(
            "class A { boolean f(boolean a, boolean b, boolean c) { return a && !(b && c); } }",
        );
        assert_eq!(m.cognitive, 1);
        assert_eq!(m.cyclomatic, 3);
    }

    #[test]
    fn switch_catch_ternary_and_labels() {
        // switch +1; try: no increment; catch +1; ternary inside catch (nest 1) +2;
        // labelled continue +1 inside for (+1) → total 6
        let m = one(
            "class A { int f(int k) {\n switch (k) { case 1: return 1; case 2: return 2; default: break; }\n try { g(); } catch (RuntimeException e) { return k > 0 ? 1 : 2; }\n outer: for (;;) { continue outer; }\n} }",
        );
        assert_eq!(m.cognitive, 6);
        // 1 + 2 case labels + catch + ternary + for
        assert_eq!(m.cyclomatic, 6);
    }

    #[test]
    fn try_blocks_nest_for_max_nesting_only() {
        // Five nested try blocks: depth 5 (cognitive complexity is unchanged: try adds nothing).
        let m = one(
            "class A { void f() {\n try { try { try { try { try { g(); } finally { } } finally { } } finally { } } finally { } } finally { }\n} }",
        );
        assert_eq!(m.max_nesting, 5);
        assert_eq!(m.cognitive, 0);
        // An if in a catch body sits one level below the try, not two.
        let m = one(
            "class A { void f(int x) {\n try { g(); } catch (RuntimeException e) { if (x > 0) { g(); } }\n} }",
        );
        assert_eq!(m.max_nesting, 2);
        assert_eq!(m.cognitive, 3); // catch +1, if in catch (nest 1) +2
                                    // A loop in a try body.
        let m = one(
            "class A { void f() {\n try { while (ok()) { g(); } } catch (Exception e) { }\n} }",
        );
        assert_eq!(m.max_nesting, 2);
    }

    #[test]
    fn lambdas_and_anonymous_classes_fold_into_enclosing_method() {
        // lambda body nests: if inside lambda (nest 1) +2; anonymous method body nests:
        // if inside run() (nest 1) +2 → 4
        let m = one(
            "class A { void f(java.util.List<Integer> xs) {\n xs.forEach(x -> { if (x > 0) { g(); } });\n new Thread(new Runnable() { public void run() { if (ok()) { g(); } } }).start();\n} }",
        );
        assert_eq!(m.name, "A#f");
        assert_eq!(m.cognitive, 4);
        assert_eq!(m.max_nesting, 2);
    }

    #[test]
    fn constructors_initializers_and_records() {
        let all = metrics(
            "class A {\n static { if (x) { y(); } }\n { z(); }\n A(int a) { this.a = a; }\n}\nrecord P(int x, int y) { P { if (x < 0) throw new IllegalArgumentException(); } }",
        );
        let names: Vec<_> = all
            .iter()
            .map(|m| (m.name.as_str(), m.params, m.cognitive))
            .collect();
        assert_eq!(
            names,
            vec![
                ("A#<static-init>", 0, 1),
                ("A#<instance-init>", 0, 0),
                ("A#A", 1, 0),
                ("P#P", 2, 1)
            ]
        );
    }

    #[test]
    fn else_if_bodies_nest_like_if_bodies() {
        // if +1; else-if +1; nested if inside else-if body (nest 1) +2; else +1 → 5
        let m = one(
            "class A { void f(int a) {\n if (a == 1) { g(); }\n else if (a == 2) { if (b()) { g(); } }\n else { g(); }\n} }",
        );
        assert_eq!(m.cognitive, 5);
    }
}
