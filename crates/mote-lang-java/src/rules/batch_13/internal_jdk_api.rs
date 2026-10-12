//! `java/internal-jdk-api` — imports and uses of JDK-internal packages (`sun.*`,
//! `jdk.internal.*`, …).

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["portability", "jdk-internal"],
    remediation_minutes: 30,
    description: r#"Detects imports and fully qualified type references of JDK-internal packages:
`sun.*`, `jdk.internal.*`, and the JDK's private copies of other libraries
(`com.sun.org.apache.*`, `com.sun.xml.internal.*`) and implementation packages
(`com.sun.crypto.provider.*`, `com.sun.imageio.*`, `com.sun.media.sound.*`).

**Why it matters.** These classes are implementation details of one JDK build, not an API:
they change or disappear between releases without notice (`sun.misc.BASE64Encoder` was
removed in Java 9), are missing on other vendors' runtimes, and since Java 16 strong
encapsulation makes most of them inaccessible without `--add-opens`/`--add-exports` flags.
The few kept reachable for compatibility (`sun.misc.Unsafe`, `sun.misc.Signal`) are
officially unsupported and are being replaced.

**How to fix.** Use the supported equivalent: `java.util.Base64` instead of the `sun.misc`
encoders, `VarHandle` and `MethodHandles` instead of `Unsafe`, the public `javax.xml` factories
instead of the internal Xerces and Xalan copies, `java.lang.ref.Cleaner` instead of
`sun.misc.Cleaner`.

```java
// Noncompliant
import sun.misc.BASE64Encoder;
String s = new BASE64Encoder().encode(bytes);

// Compliant
import java.util.Base64;
String s = Base64.getEncoder().encodeToString(bytes);
```

Not reported: the supported `com.sun` APIs (`com.sun.net.httpserver`, `com.sun.management`,
`com.sun.source`, `com.sun.jdi`, …), packages that also exist outside the JDK
(`com.sun.jndi.ldap.ctl` came with a separate LDAP library, `com.sun.jmx.remote` with the
optional JMX remote package), third-party code published under `com.sun` names,
class names in strings (loading an optional internal class reflectively, with a fallback, is
a deliberate choice), and files that belong to such a package themselves."#,
    ..RuleMeta::new("java/internal-jdk-api", "java", "JDK-internal API used")
};

#[derive(Default)]
pub struct InternalJdkApi {
    /// Whether the file mentions an internal package and is not one itself.
    relevant: Option<bool>,
}

/// Package prefixes that are internal to the JDK.
const INTERNAL: &[&str] = &[
    "sun.",
    "jdk.internal.",
    "com.sun.org.apache.",
    "com.sun.xml.internal.",
    "com.sun.crypto.provider.",
    "com.sun.imageio.",
    "com.sun.media.sound.",
];

fn internal_prefix(name: &str) -> Option<&'static str> {
    INTERNAL.iter().copied().find(|p| name.starts_with(p))
}

impl<'t> JavaRule<'t> for InternalJdkApi {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["import_declaration", "scoped_type_identifier"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let relevant = *self.relevant.get_or_insert_with(|| {
            let src = ctx.source();
            (src.contains("sun.") || src.contains("jdk.internal."))
                && !ctx.package_name().is_some_and(|p| {
                    let p = format!("{p}.");
                    internal_prefix(&p).is_some()
                })
        });
        if !relevant {
            return;
        }
        let (target, name): (Node<'t>, String) = if kind(node) == "import_declaration" {
            let Some(path) = named_children(node)
                .find(|c| matches!(kind(*c), "scoped_identifier" | "identifier"))
            else {
                return;
            };
            (path, compact(ctx.text(path)))
        } else {
            // Only the outermost part of a qualified type (`sun.misc.Unsafe`, not `sun.misc`).
            if ctx
                .parent(node)
                .is_some_and(|p| kind(p) == "scoped_type_identifier")
            {
                return;
            }
            (node, compact(ctx.text(node)))
        };
        let Some(prefix) = internal_prefix(&name) else {
            return;
        };
        // `sun` alone is too short to be sure of: require a package segment after it.
        if name.len() <= prefix.len() {
            return;
        }
        let shown = if kind(node) == "import_declaration"
            && syntax::child_of_kind(node, "asterisk").is_some()
        {
            format!("{name}.*")
        } else {
            name
        };
        ctx.emit(
            target,
            format!("Replace `{shown}` with a supported API: JDK-internal classes change without notice and are closed to applications on current releases"),
        );
    }
}

/// The text of a qualified name without whitespace or comments between its parts.
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
