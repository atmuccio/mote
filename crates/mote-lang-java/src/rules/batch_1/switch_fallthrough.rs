//! `java/switch-fallthrough` — a `case` that runs on into the next one.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["switch", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects a non-empty group of statements under classic `case X:` labels that
can run on into the next label because it doesn't end with `break`, `return`, `throw`,
`continue`, or `yield`.

**Why it matters.** Falling through is occasionally intended but far more often a forgotten
`break`: the next case's code then runs too, producing wrong results that tests covering each
case separately may not catch.

**How to fix.** End the case with `break` (or `return`/`throw`). If falling through is
intended, say so with a comment such as `// falls through` just before the next label.

```java
// Noncompliant
switch (level) {
    case WARN:
        warnings++;
    case ERROR:            // WARN also counts as an error
        errors++;
        break;
}

// Compliant
switch (level) {
    case WARN:
        warnings++;
        break;
    case ERROR:
        errors++;
        break;
}
```

Not reported: several labels sharing one group of statements (`case A: case B: …`), the last
group, arrow-form switches (they never fall through), groups whose ending the rule can't be sure
about (a nested `switch`, a loop, a labeled statement, a call to a helper such as `fail(..)` or
`System.exit(..)`), a case that falls only into an empty trailing group (`default:` with
no statements), and fall-throughs documented by a comment containing "fall" (`// falls
through`, `/* FALLTHROUGH */`)."#,
    ..RuleMeta::new(
        "java/switch-fallthrough",
        "java",
        "switch case falls through",
    )
};

pub struct SwitchFallthrough;

fn contains_fall(text: &str) -> bool {
    text.as_bytes()
        .windows(4)
        .any(|w| w.eq_ignore_ascii_case(b"fall"))
}

impl<'t> JavaRule<'t> for SwitchFallthrough {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["switch_block"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if node.has_error() {
            return;
        }
        let groups: Vec<Node<'t>> = named_children(node)
            .filter(|g| kind(*g) == "switch_block_statement_group")
            .collect();
        // Groups from this index on hold no statements (`case 9: default:` at the end): falling
        // into them runs nothing.
        let empty_tail = groups
            .iter()
            .rposition(|g| {
                named_children(*g).any(|c| kind(c) != "switch_label" && !syntax::is_comment(c))
            })
            .map_or(0, |i| i + 1);
        for (i, pair) in groups.windows(2).enumerate() {
            let (group, next) = (pair[0], pair[1]);
            if i + 1 >= empty_tail {
                continue;
            }
            // The last statement of the group (labels excluded); none = stacked labels.
            let Some(last) = util::last_named_child(group).filter(|c| kind(*c) != "switch_label")
            else {
                continue;
            };
            if util::completes_normally(ctx, last) != Some(true) {
                continue;
            }
            // Comments between the last statement and the next label (they may be attached to
            // either group or to the switch block).
            let gap = ctx
                .source()
                .get(last.end_byte()..next.start_byte())
                .unwrap_or("");
            if contains_fall(gap) {
                continue;
            }
            let at = named_children(group)
                .find(|c| kind(*c) == "switch_label")
                .unwrap_or(group);
            let next_label = named_children(next)
                .find(|c| kind(*c) == "switch_label")
                .unwrap_or(next);
            ctx.emit(
                at,
                "End this case with `break`, `return` or `throw`, or add a `// falls through` comment",
            )
            .secondary(next_label, "execution continues here");
        }
    }
}
