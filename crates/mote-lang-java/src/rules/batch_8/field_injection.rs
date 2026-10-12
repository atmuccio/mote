//! `java/field-injection` — dependencies injected straight into fields (`@Autowired`,
//! `@Inject`) instead of through the constructor.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["spring", "dependency-injection", "design"],
    remediation_minutes: 10,
    description: r#"Detects fields annotated with Spring's `@Autowired` or with `@Inject`
(`javax.inject` / `jakarta.inject`) in production code.

**Why it matters.** With field injection the container writes the dependencies into the
object after it has been built, through reflection. The class's constructor no longer states
what the class needs, the fields cannot be `final`, the object can exist in a half-wired
state (in a unit test, or when created with `new`), and circular or excessive dependencies
stay hidden. Constructor injection makes every dependency explicit, allows immutable
fields, and lets tests create the object with plain `new`; the framework documentation
recommends it for mandatory dependencies.

**How to fix.** Declare the dependencies as `final` fields and take them as constructor
parameters (a single constructor needs no annotation in Spring).

```java
// Noncompliant
@Service
class OrderService {
    @Autowired
    private OrderRepository repository;
}

// Compliant
@Service
class OrderService {
    private final OrderRepository repository;

    OrderService(OrderRepository repository) {
        this.repository = repository;
    }
}
```

Off by default: it is a design preference. Test classes are not reported (injecting fixtures
into test fields is the normal way to write container tests)."#,
    ..RuleMeta::new("java/field-injection", "java", "Field injection")
};

#[derive(Default)]
pub struct FieldInjection {
    applies: Option<bool>,
}

const INJECT_ANNOTATIONS: &[&str] = &[
    "org.springframework.beans.factory.annotation.Autowired",
    "javax.inject.Inject",
    "jakarta.inject.Inject",
];

impl<'t> JavaRule<'t> for FieldInjection {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !*self.applies.get_or_insert_with(|| {
            !ctx.is_test() && {
                let src = ctx.source();
                src.contains("@Autowired") || src.contains("@Inject")
            }
        }) {
            return;
        }
        let src = ctx.source();
        for ann in syntax::annotations(node) {
            let name = syntax::annotation_name(ann, src);
            if name != "Autowired" && name != "Inject" {
                continue;
            }
            let Some(name_node) = ann.field(Field::Name) else {
                continue;
            };
            let written = ctx.text(name_node);
            let fqn = if written.contains('.') {
                Some(written.to_string())
            } else {
                ctx.resolve_type_name(written, ann)
            };
            if !fqn
                .as_deref()
                .is_some_and(|f| INJECT_ANNOTATIONS.contains(&f))
            {
                continue;
            }
            let field = syntax::declarators(node)
                .next()
                .map(|d| syntax::name_of(d, src))
                .unwrap_or("");
            ctx.emit(
                ann,
                format!("Inject `{field}` through the constructor instead of with `@{name}` on the field"),
            );
        }
    }
}
