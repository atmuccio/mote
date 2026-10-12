//! `java/empty-catch-block` — a syntactic rule.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["error-handling"],
    params: &[ParamSpec {
        name: "allow_comment",
        default: ParamDefault::Bool(true),
        description: "Accept an empty catch block that contains a comment explaining why the exception is ignored.",
    }],
    remediation_minutes: 5,
    description: r#"Detects `catch` blocks that contain no statements.

**Why it matters.** Swallowing an exception silently hides failures: the program continues in
an unknown state and the original cause is lost, which makes defects slow to diagnose.

**How to fix.** Handle the exception (recover, retry, translate and rethrow, or at least log it).
If ignoring it really is correct, say why in a comment inside the block, or name the variable
`ignored` / `expected` (or `_`) to make the intent explicit.

```java
// Noncompliant
try { cache.load(); } catch (IOException e) { }

// Compliant
try { cache.load(); } catch (IOException e) { log.warn("cache unavailable, starting cold", e); }
try { lock.close(); } catch (IOException ignored) { }
try { lock.close(); } catch (IOException e) { /* best effort: the lock file is recreated on start */ }
```"#,
    ..RuleMeta::new("java/empty-catch-block", "java", "Empty catch block")
};

/// Variable names that document an intentionally ignored exception.
const IGNORED_NAMES: &[&str] = &["ignored", "ignore", "expected", "_"];

pub struct EmptyCatchBlock;

impl<'t> JavaRule<'t> for EmptyCatchBlock {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !syntax::is_empty_block(body) {
            return;
        }
        if ctx.param_bool("allow_comment") && syntax::block_has_comment(body) {
            return;
        }
        let param_name = named_children(node)
            .find(|c| kind(*c) == "catch_formal_parameter")
            .and_then(|p| p.field(Field::Name));
        if let Some(name) = param_name {
            if kind(name) == "underscore_pattern" || IGNORED_NAMES.contains(&ctx.text(name)) {
                return;
            }
        }
        // Report on the `catch` keyword.
        let at = syntax::children(node).next().unwrap_or(node);
        ctx.emit(
            at,
            "Handle this exception or explain in a comment why it can be ignored",
        );
    }
}
