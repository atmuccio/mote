//! `java/unused-local-variable` — locals whose value is never read.
//!
//! File-level: locals are registered when their declaration is visited; because Java requires
//! a local to be declared before it is used and the walk is in source order, every later
//! identifier with a registered name is checked against the innermost open declaration of that
//! name (a per-name stack whose scopes nest). Findings are emitted in `finish`.

use super::util::Mods;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["dead-code", "unused"],
    remediation_minutes: 2,
    description: r#"Detects local variables whose value is never read. Assigning to a variable
(`x = …`, `x += …`, `x++` as a statement) does not count as reading it.

**Why it matters.** An unused local is noise that readers have to understand and then discard.
It often hides a real mistake: a result that was computed but forgotten, a variable that was
meant to be passed on, or leftovers of a refactoring.

**How to fix.** Delete the variable. If its initializer has a needed side effect (a call),
keep the call as a plain statement.

```java
// Noncompliant
int total = computeTotal(items);
String unusedLabel = format(total);   // never read
return total;

// Compliant
int total = computeTotal(items);
return total;
```

Not reported: the unnamed variable `_`, names that announce the intent (`ignored…`,
`unused…`, `dummy…`), annotated declarations (for example `@SuppressWarnings("unused")`),
try-with-resources resources, catch parameters, pattern variables, and enhanced-`for`
variables. A read of any same-named variable inside the local's scope (for example a field of
a local class) counts as a use."#,
    ..RuleMeta::new(
        "java/unused-local-variable",
        "java",
        "Unused local variable",
    )
};

struct Local<'t> {
    name_node: Node<'t>,
    name: &'t str,
    /// The symbol of the declaration, taken while it is walked (from `finish`, finding it walks
    /// up from the node: quadratic for locals of deeply nested lambdas), formatted only for
    /// the locals reported.
    symbol: crate::ctx::SymbolKey<'t>,
    /// Byte range in which the variable is visible (after its name).
    start: u32,
    end: u32,
    used: bool,
}

#[derive(Default)]
pub struct UnusedLocalVariable<'t> {
    locals: Vec<Local<'t>>,
    /// Name → indices into `locals` whose scope may still be open (innermost last).
    open: FxHashMap<&'t str, Vec<u32>>,
    /// End of the furthest open scope: past it, `open` is cleared (fast path for identifiers
    /// outside method bodies).
    open_until: u32,
    /// Bit set of [`name_bit`] over the names in `open`: most identifiers are rejected by it
    /// without hashing.
    open_bits: [u64; 4],
}

/// A cheap 8-bit signature of a name (length and first/last bytes).
fn name_bit(name: &str) -> usize {
    let b = name.as_bytes();
    let (first, last) = (
        b.first().copied().unwrap_or(0),
        b.last().copied().unwrap_or(0),
    );
    (first as usize).wrapping_mul(31) ^ (last as usize).wrapping_mul(7) ^ b.len()
}

fn has_bit(bits: &[u64; 4], bit: usize) -> bool {
    bits[(bit >> 6) & 3] & (1 << (bit & 63)) != 0
}

fn is_intentionally_unused(name: &str) -> bool {
    let lower = |p: &str| {
        name.len() >= p.len() && name.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes())
    };
    name == "_" || lower("ignore") || lower("unused") || lower("dummy")
}

impl<'t> UnusedLocalVariable<'t> {
    fn declare(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if Mods::of(node).annotated() {
            return;
        }
        let mut up = ctx.ancestors(node);
        let Some(parent) = up.next() else {
            return;
        };
        let scope = match kind(parent) {
            // Old-style `case X:` groups share one scope: the whole switch block.
            "switch_block_statement_group" => match up.next() {
                Some(sb) => sb,
                None => return,
            },
            "block" | "constructor_body" | "for_statement" => parent,
            _ => return,
        };
        let end = scope.end_byte() as u32;
        if node.start_byte() as u32 >= self.open_until {
            self.open.clear();
            self.open_bits = [0; 4];
        }
        self.open_until = self.open_until.max(end);
        // The names of a declaration statement have its symbol (no member, class or anonymous
        // body lies between them): read it once, for the declaration being checked.
        let mut symbol: Option<crate::ctx::SymbolKey<'t>> = None;
        for d in syntax::declarators(node) {
            let Some(name_node) = d.field(Field::Name) else {
                continue;
            };
            if kind(name_node) != "identifier" {
                continue;
            }
            let name = ctx.text(name_node);
            if is_intentionally_unused(name) {
                continue;
            }
            let symbol = symbol.get_or_insert_with(|| ctx.symbol_key(node)).clone();
            #[cfg(debug_assertions)]
            debug_assert_eq!(symbol.clone().symbol(), ctx.symbol_of(name_node));
            let idx = self.locals.len() as u32;
            self.locals.push(Local {
                name_node,
                name,
                symbol,
                start: name_node.end_byte() as u32,
                end,
                used: false,
            });
            self.open.entry(name).or_default().push(idx);
            let bit = name_bit(name);
            self.open_bits[(bit >> 6) & 3] |= 1 << (bit & 63);
        }
    }

    /// Whether this occurrence of an identifier reads a variable (as opposed to declaring it,
    /// writing it, or naming a method / member).
    fn is_read(node: Node<'t>, ctx: &Ctx<'t>) -> bool {
        let mut up = ctx.ancestors(node);
        let Some(parent) = up.next() else {
            return true;
        };
        let is_field = |f: Field| parent.field(f).is_some_and(|c| c.id() == node.id());
        match kind(parent) {
            "variable_declarator" => !is_field(Field::Name),
            "method_invocation" => !is_field(Field::Name),
            "field_access" => !is_field(Field::Field),
            "assignment_expression" => {
                if !is_field(Field::Left) {
                    return true;
                }
                if syntax::operator_str(parent) == "=" {
                    return false;
                }
                // `x += 1;` as a statement only writes; `f(x += 1)` also reads the result.
                !matches!(
                    up.next().map(kind),
                    Some("expression_statement" | "for_statement")
                )
            }
            "update_expression" => !matches!(
                up.next().map(kind),
                Some("expression_statement" | "for_statement")
            ),
            _ => true,
        }
    }

    fn use_name(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let name = ctx.text(node);
        let Some(stack) = self.open.get_mut(name) else {
            return;
        };
        let at = node.start_byte() as u32;
        // Scopes of same-named locals nest, so everything ended before `at` is on top.
        while let Some(&top) = stack.last() {
            match self.locals.get(top as usize) {
                Some(l) if l.end <= at => {
                    stack.pop();
                }
                _ => break,
            }
        }
        let Some(&top) = stack.last() else {
            return;
        };
        let visible = self
            .locals
            .get(top as usize)
            .is_some_and(|l| l.start <= at && !l.used);
        if visible && Self::is_read(node, ctx) {
            if let Some(l) = self.locals.get_mut(top as usize) {
                l.used = true;
            }
        }
    }
}

impl<'t> JavaRule<'t> for UnusedLocalVariable<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration", "identifier"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "identifier" {
            if self.open.is_empty() {
                return;
            }
            if node.start_byte() as u32 >= self.open_until {
                self.open.clear();
                self.open_bits = [0; 4];
                return;
            }
            if has_bit(&self.open_bits, name_bit(ctx.text(node))) {
                self.use_name(node, ctx);
            }
        } else {
            self.declare(node, ctx);
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for l in self.locals.iter().filter(|l| !l.used) {
            ctx.emit_with_symbol(
                l.name_node,
                l.symbol.clone().symbol(),
                format!("Remove the unused local variable `{}`", l.name),
            );
        }
    }
}
