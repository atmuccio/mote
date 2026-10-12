//! `js/no-useless-constructor`

use crate::prelude::*;

pub struct NoUselessConstructor;

pub static NO_USELESS_CONSTRUCTOR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 2,
    description: r#"Flags class constructors that do exactly what the implicit default constructor
would do: an empty constructor in a class without `extends`, or a constructor in a derived class
that only passes its parameters, unchanged and in order, to `super(…)`.

**Why:** the constructor is dead weight: more code to read that changes nothing.

**Fix:** remove it. Not reported: `private` and `protected` constructors (they restrict who can
create instances), constructors with TypeScript parameter properties (`constructor(private
x: number) {}`) or decorated parameters, constructors containing a comment, overloaded
constructors, and TypeScript constructors whose typed parameters narrow the inherited signature.

```js
class Shape {
  constructor() {}                    // flagged
}
class Circle extends Shape {
  constructor(...args) {
    super(...args);                   // flagged: the default does the same
  }
}

class Square extends Shape {
  constructor(size) {
    super(size, size);                // compliant: adapts the arguments
  }
}
```
"#,
    ..RuleMeta::new(
        "js/no-useless-constructor",
        "js",
        "Useless constructors should be removed",
    )
};

/// Do the `super(…)` arguments forward exactly the parameters, in order?
fn forwards_parameters(params: &FormalParameters<'_>, args: &[Argument<'_>]) -> bool {
    // `super(...arguments)` forwards everything.
    if let [Argument::SpreadElement(spread)] = args {
        if spread.argument.is_specific_id("arguments") {
            return true;
        }
    }
    let expected = params.items.len() + usize::from(params.rest.is_some());
    if args.len() != expected {
        return false;
    }
    for (param, arg) in params.items.iter().zip(args) {
        let BindingPattern::BindingIdentifier(name) = &param.pattern else {
            return false;
        };
        if param.initializer.is_some() {
            return false;
        }
        match arg {
            Argument::Identifier(id) if id.name == name.name => {}
            _ => return false,
        }
    }
    if let Some(rest) = &params.rest {
        let BindingPattern::BindingIdentifier(name) = &rest.rest.argument else {
            return false;
        };
        match args.last() {
            Some(Argument::SpreadElement(spread))
                if spread.argument.is_specific_id(name.name.as_str()) => {}
            _ => return false,
        }
    }
    true
}

impl JsRule for NoUselessConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_CONSTRUCTOR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::MethodDefinition]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::MethodDefinition(method) = node.kind() else {
            return;
        };
        if method.kind != MethodDefinitionKind::Constructor
            || !method.decorators.is_empty()
            || matches!(
                method.accessibility,
                Some(TSAccessibility::Private | TSAccessibility::Protected)
            )
        {
            return;
        }
        let function = &method.value;
        let Some(body) = &function.body else {
            return;
        };
        let params = &function.params;
        if params.items.iter().any(|p| {
            p.accessibility.is_some() || p.readonly || p.r#override || !p.decorators.is_empty()
        }) || params
            .rest
            .as_ref()
            .is_some_and(|r| !r.decorators.is_empty())
        {
            return;
        }
        if !body.directives.is_empty() || ctx.has_comments_in(body.span) {
            return;
        }
        let Some(class_body) = ctx.parent(node) else {
            return;
        };
        let Some(AstKind::Class(class)) = ctx.parent_kind(class_body) else {
            return;
        };
        if class.declare {
            return;
        }
        // Overloaded constructors need their implementation.
        let overloaded = class.body.body.iter().any(|element| {
            matches!(element, ClassElement::MethodDefinition(m)
                if m.kind == MethodDefinitionKind::Constructor && m.value.body.is_none())
        });
        if overloaded {
            return;
        }
        let typed = params.items.iter().any(|p| p.type_annotation.is_some())
            || params
                .rest
                .as_ref()
                .is_some_and(|r| r.type_annotation.is_some());
        let message = if class.super_class.is_none() {
            // An empty constructor; in TypeScript, parameters would declare a signature.
            let has_params = !params.items.is_empty() || params.rest.is_some();
            if !body.statements.is_empty() || (has_params && (ctx.is_typescript() || typed)) {
                return;
            }
            "Remove this empty constructor: the default constructor does the same"
        } else {
            // A public modifier may widen a protected parent constructor.
            if method.accessibility.is_some() || typed {
                return;
            }
            let [Statement::ExpressionStatement(stmt)] = body.statements.as_slice() else {
                return;
            };
            let Expression::CallExpression(call) = &stmt.expression else {
                return;
            };
            if !matches!(call.callee, Expression::Super(_))
                || !forwards_parameters(params, &call.arguments)
            {
                return;
            }
            "Remove this constructor: it only forwards its arguments to 'super'"
        };
        ctx.emit(method.key.span(), message);
    }
}
