# Writing JavaScript/TypeScript rules for Mote

This guide describes how rules are written in `mote-lang-js`, for anyone reading the code or
building Mote from source. Read `docs/architecture.md` first; its non-negotiables apply to every
rule. The most important ones:

* **Provenance.** ESLint / typescript-eslint / Oxlint (MIT) and PMD (BSD) are fine as sources of
  *ideas and rule names*; the code and the wording are Mote's own. Nothing comes from proprietary
  analyzers (code, rule descriptions, rule keys, output).
* **Precision over recall.** When a rule cannot be sure, it stays silent.
* **Never panic** on any input. **Deterministic** output.
* **Every rule ships with fixtures**, positives and tricky negatives.

The rule-author API described here (`JsRule`, `Ctx`, `helpers`, `prelude`) is internal to Mote: it is
not a stable plugin API and may change in any 0.x release. A helper only one rule needs stays
private to that rule's module.

---

## 1. How rules run

For each file the plugin:

1. parses with Oxc (`.js`/`.mjs`/`.cjs` with JSX enabled; `.ts`, `.tsx`, `.mts`, `.cts`; `.d.ts`
   files are skipped entirely) and builds the semantic model (scopes, symbols, references);
2. computes file facts in one linear pass (function metrics, symbol names, node-type presence);
3. asks each enabled rule's `applies_to(ctx)` once (which rules are enabled, their parameters and
   the node-type dispatch table restricted to them are resolved once per configuration — the
   engine shares one rule set between all files with the same effective configuration — so per
   file only these gates run);
4. walks the flat node list **once**, in source order, calling `run(node, ctx)` on every rule
   subscribed to that node's `AstType`;
5. calls `run_once(ctx)` on every enabled rule;
6. sorts findings and attaches the enclosing symbol (`Class#method`, `outer.inner`, …) to each.

Parse options that shape the tree:

* `preserve_parens: false` — there are **no `ParenthesizedExpression` nodes**. `(a)` is just `a`.
* Recovered syntax errors still produce a tree; files with unrecoverable errors run no rules.
  Files nested too deeply to parse safely on the analysis stack the host declares
  (`JsPlugin::with_analysis_stack`; with the default 2 MiB: hundreds of nested brackets or
  thousands of chained calls; see
  `src/depth.rs`) are reported as one syntax error and are not parsed at all.

Complexity, function length, and parameter-count thresholds are **generic engine rules** computed
from the plugin's `FunctionMetric`s. Do not reimplement them as JS rules.

---

## 2. Adding a rule

Rules are grouped in batch directories, `src/rules/batch_N/`; a rule lives in one of them.

1. Create `src/rules/batch_N/<rule_name>.rs` (snake_case of the rule name), e.g.
   `no_self_compare.rs` for `js/no-self-compare`.
2. Declare it in `src/rules/batch_N/mod.rs` and add it to `rules()`:

   ```rust
   mod no_self_compare;

   pub fn rules() -> Vec<Box<dyn JsRule>> {
       vec![
           Box::new(no_self_compare::NoSelfCompare),
       ]
   }
   ```
3. Add fixtures in `tests/fixtures/<rule-name>[.variant].<ext>` (section 7).
4. Run `cargo test -p mote-lang-js` and
   `cargo clippy -p mote-lang-js --all-targets -- -D warnings`; format with
   `cargo fmt -p mote-lang-js`.

Naming: id `js/<kebab-name>` (use the ESLint/typescript-eslint name when one exists — the ESLint
suppression comments `// eslint-disable-next-line <name>` then work automatically); struct in
`UpperCamelCase`; metadata static in `SCREAMING_SNAKE_CASE`, next to the struct.

### Copy-paste template

````rust
//! `js/no-self-compare`

use crate::prelude::*;

pub struct NoSelfCompare;

pub static NO_SELF_COMPARE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,           // Bug | CodeSmell (default CodeSmell)
    quality: Quality::Reliability, // Reliability | Maintainability (default)
    severity: Severity::High,      // see section 8
    tags: &["suspicious"],
    remediation_minutes: 5,
    params: &[], // see "Parameters" below
    description: r#"Flags comparisons of an expression with itself (`x === x`).

**Why:** the result is always the same (except for `NaN`), so the comparison is almost certainly a
typo for another variable.

**Fix:** compare against the intended value; use `Number.isNaN(x)` to test for `NaN`.

```js
if (a === a) {}          // flagged
if (a === b) {}          // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-self-compare",
        "js",
        "Values should not be compared with themselves",
    )
};

impl JsRule for NoSelfCompare {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SELF_COMPARE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if !helpers::is_comparison(expr.operator) {
            return;
        }
        let (left, right) = (
            helpers::unwrap_expr(&expr.left),
            helpers::unwrap_expr(&expr.right),
        );
        if let (Expression::Identifier(a), Expression::Identifier(b)) = (left, right) {
            if a.name == b.name {
                ctx.emit(
                    expr.span,
                    format!("Compare '{}' with the intended value, not itself", a.name),
                );
            }
        }
    }
}
````

(`src/template_check.rs` is a verbatim copy of this template and a test compares the two: edit both
together.)

The description is Markdown in **our own words**: what it detects, why it matters, how to fix, and a
short flagged / compliant example. The `metadata_is_consistent` test checks ids, language (`"js"`),
and that the description is not trivially short.

---

## 3. The `JsRule` trait

```rust
pub trait JsRule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn types(&self) -> &'static [AstType] { &[] }          // node subscriptions
    fn applies_to(&self, ctx: &Ctx<'_>) -> bool { true }   // per-file gate
    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {}
    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {}
}
```

* Rules are unit structs: stateless and shared between threads. **Never** keep per-file state in
  the struct (no `Cell`, `Mutex`, statics). Use `ctx.state::<T>()`.
* `types()` — the node types for `run`. Rules with only `run_once` return `&[]`.
* `applies_to` — cheap, file-level: `ctx.is_typescript()`, `!ctx.is_test()`, `ctx.is_jsx()`,
  `ctx.file_contains(AstType::JSXElement)`. Returning `false` skips the rule for the file.
* `run` is called in source (pre-)order: a parent before its children.
* `run_once` runs after the whole traversal — for symbol-table rules (see `no_unused_vars.rs`) and
  for rules that aggregate across the file.

Three exemplary styles are in `src/rules/`: purely syntactic (`no_debugger.rs`, `no_var.rs`,
`eqeqeq.rs`, `no_empty.rs`), scope-aware (`no_console.rs`: `console` must be the global),
parameterised / TS-only (`no_explicit_any.rs`), secondary locations (`no_dupe_keys.rs`), and a
semantic `run_once` rule (`no_unused_vars.rs`).

---

## 4. `Ctx` reference

Everything returned by `Ctx` lives for the file (`'a`), so you can keep AST references while
calling `ctx.emit(...)`.

### File

| Method | Meaning |
|---|---|
| `path()` | repo-relative path, `/` separators |
| `source()` / `text(span)` | whole source / text of a span (`""` if out of range — never slice `source()` yourself) |
| `is_test()` | file matched a test glob — relax rules that make no sense in tests |
| `is_typescript()` | `.ts .tsx .mts .cts` |
| `is_jsx()` | JSX syntax enabled (`.jsx`, `.tsx`, and all plain JS) |
| `is_module()` | ES module (has import/export or `.mjs/.mts`); scripts share one global scope |
| `source_type()` | Oxc `SourceType` (e.g. `.is_commonjs()`) |
| `file_contains(AstType)` | the file has at least one node of this type (computed before rules run) |
| `line_of(offset)` / `range(span)` / `line_text(line)` | 1-based line of a byte offset / line-column range / text of a line |

### Tree navigation

`node` arguments accept `&AstNode`, `AstNode`, `NodeId`, or `AstKind` (trait `AsNodeId`).

| Method | Meaning |
|---|---|
| `node(id)` | node by `NodeId` |
| `parent(node)` / `parent_kind(node)` | direct parent (`None` for `Program`) |
| `outer_parent(node)` | first ancestor that is not `as` / `satisfies` / `!` / `<T>x` / `ChainExpression` — "how is this expression used?" |
| `ancestors(node)` | parent … `Program` |
| `enclosing_function(node)` | nearest `Function` / `ArrowFunctionExpression` (methods are `Function` nodes) |
| `is_in_function(node)` | inside any function |
| `is_in_loop(node)` | runs repeatedly in a loop of the *same* function (body, `for` test/update, `while` test) |
| `is_in_finally(node)` | inside a `finally` block of the same function |
| `in_ambient_context(node)` | inside `declare module/namespace` or `declare global` |
| `symbol_of(node)` / `symbol_at(offset)` | enclosing unit name, e.g. `Shop#total`, `outer.inner`, `obj.run` |

```rust
// Is the value of this call discarded?
let discarded = matches!(ctx.outer_parent(node).map(AstNode::kind), Some(AstKind::ExpressionStatement(_)));
```

### Symbols and references

| Method | Meaning |
|---|---|
| `scoping()` | Oxc `Scoping`: scopes, symbols, references |
| `semantic()` / `nodes()` | full Oxc `Semantic` / flat node list |
| `binding_symbol(&BindingIdentifier)` | `Option<SymbolId>` |
| `reference_symbol(&IdentifierReference)` | `Option<SymbolId>`; `None` = global/unresolved |
| `is_global_reference(&IdentifierReference)` | not shadowed by any local declaration |
| `is_global(&Expression, "name")` | `expr` is the global `name` (e.g. `console`, `undefined`, `window`) |
| `symbol_name / symbol_flags / symbol_span(symbol)` | name, `SymbolFlags`, declaring identifier span |
| `symbol_declaration(symbol)` | declaring node (see table) |
| `references_of(symbol)` | all resolved `Reference`s (`.node_id()`, `.flags()`, `.is_read()`, `.is_write()`, `.is_type()`) |
| `is_unused_symbol(symbol)` | no references at all |
| `is_symbol_read(symbol)` | some reference is not a pure write |

`symbol_declaration` returns the *declaring* node, not the identifier:

| Declaration | Node kind |
|---|---|
| `var/let/const/using` (also destructuring, `for…of` heads) | `VariableDeclarator` |
| function declaration or named function expression | `Function` |
| class | `Class` |
| parameter / rest parameter | `FormalParameter` / `FormalParameterRest` |
| catch parameter | `CatchParameter` |
| imports | `ImportSpecifier`, `ImportDefaultSpecifier`, `ImportNamespaceSpecifier`, `TSImportEqualsDeclaration` |
| TypeScript | `TSTypeAliasDeclaration`, `TSInterfaceDeclaration`, `TSEnumDeclaration`, `TSEnumMember`, `TSModuleDeclaration`, `TSTypeParameter` |

Reference flags: plain read `Read`; assignment `Write`; `x++` / `x += 1` → `Read | Write`;
`x.y = 1` → `x` is `Read` (+ `MemberWriteTarget`); type positions → `Type`. Declarations merged or
redeclared (`var x; var x;`, overloads, `interface A` + `class A`) have
`scoping().symbol_redeclarations(symbol)` non-empty.

**Never call the panicking getters** `ident.symbol_id()`, `ident.reference_id()`,
`node.scope_id.get().unwrap()` etc. Use `ctx.binding_symbol`, `ctx.reference_symbol`, or
`.symbol_id.get()` / `.reference_id.get()` (which return `Option`). `AstNode::scope_id()` is safe.

### Parameters

Declare parameters in `RuleMeta.params`; read them with `ctx.param_i64 / param_f64 / param_bool /
param_str / param_list(name)`. A missing or mistyped configured value falls back to the declared
default. `param_list` splits a comma-separated string (`"warn, error"`) into trimmed items.

```rust
params: &[ParamSpec { name: "max", default: ParamDefault::Int(3), description: "Maximum allowed." }],
// …
if count > ctx.param_i64("max") as usize { … }
```

Read parameters *after* the cheap structural checks (each lookup finds the running rule's settings
by its index, then searches its few configured parameters by name).

### Reporting

```rust
ctx.emit(span, "Message");                                   // returns Emitted
ctx.emit(key.span(), format!("Remove or rename the duplicate key '{name}'"))
    .secondary(first_span, "First definition")               // related location, same file
    .value(complexity as f64);                               // measured value for threshold rules
```

* Report the **smallest meaningful span**: the operator, keyword, or name — not whole blocks. Spans
  are byte offsets `[start, end)`; `Span::new(start, end)` builds sub-spans (e.g. the `var` keyword
  is `Span::new(decl.span.start, decl.span.start + 3)`); check with `ctx.text(span)` before relying
  on offsets.
* Messages: one imperative sentence, specific, with the offending name in quotes where useful:
  `Use '===' instead of '=='`, `Remove the unused import 'fs'`. No trailing period.
* The enclosing symbol is attached automatically (`Emitted::symbol` overrides it — rarely needed).

### Per-file state

```rust
#[derive(Default)]
struct Seen { strings: FxHashMap<String, Vec<Span>> }

fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
    if let AstKind::StringLiteral(s) = node.kind() {
        ctx.state::<Seen>().strings.entry(s.value.to_string()).or_default().push(s.span);
    }
}
fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
    let seen = std::mem::take(ctx.state::<Seen>());   // take it, then emit freely
    for (value, spans) in seen.strings { /* … */ }
}
```

State is created with `Default` on first use, is private to the rule, and is dropped after the
file. It must be `'static`: store `Span`, `NodeId`, `SymbolId`, or owned values — not AST
references (get the node back with `ctx.node(id)`).

### Helpers (`crate::helpers`, in the prelude as `helpers::…`)

`unwrap_expr` (strip `as`/`satisfies`/`!`/`<T>`), `string_value` (string or plain template
literal), `number_value` (with unary `-`/`+`), `is_literal`, `is_typeof`, `property_key_name`
(runtime key: `a`/`'a'`/`['a']` → `a`, `1`/`1.0`/`0x1` → `1`), `static_member_names` (`console.log`
→ `("console", "log")`), `call_member_names`, `argument_expr`, `is_loose_equality`, `is_equality`,
`is_comparison`, `mirror_comparison` (`<` ↔ `>`), `negate_equality` (`===` ↔ `!==`), `is_loop`,
`is_empty_block`, `for_each_binding` (every identifier bound by a pattern), `callee_name` (the name
a callee ends in: `f()`, `a.b.m()`, `a?.m()`, `a['m']()` → `f` / `m`; what `callee_names` keys on),
`NameSet` (names to scan a whole file against, with a cheap pre-filter and a dense index per name).

Oxc's own AST methods cover a lot too: `Expression::{is_null, is_undefined, is_void_0,
is_specific_id, is_specific_member_access, get_inner_expression, is_function, is_literal}`,
`CallExpression::callee_name`, `PropertyKey::{static_name, is_specific_static_name, name}`,
`Function::{is_declaration, is_expression}`, `Class::is_declaration`,
`AstKind::{is_function_like, is_iteration_statement, is_statement, is_declaration}`, operator
helpers (`BinaryOperator::{is_equality, is_compare, is_arithmetic}`,
`AssignmentOperator::is_logical`, `UnaryOperator::is_typeof`, `LogicalOperator::is_coalesce`, …).

---

## 5. Matching the Oxc AST

`use crate::prelude::*;` brings the whole AST (`oxc_ast::ast::*`), `AstKind`, `AstType`, the
operator enums, `Span`, `GetSpan`, `SymbolId`/`SymbolFlags`/`ReferenceFlags`/`NodeId`,
`FxHashMap`/`FxHashSet`, and `Cow`.

Subscribe with `AstType::X` and destructure `AstKind::X(node)`; the payload is `&'a X<'a>`:

```rust
fn types(&self) -> &'static [AstType] { &[AstType::CallExpression, AstType::NewExpression] }

fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
    match node.kind() {
        AstKind::CallExpression(call) => { /* call.callee, call.arguments, call.optional */ }
        AstKind::NewExpression(new) => { /* new.callee, new.arguments */ }
        _ => {}
    }
}
```

Child fields are enums (`Expression`, `Statement`, `Argument`, `PropertyKey`, `BindingPattern`,
`TSType`, …) whose variants hold boxed nodes: `Expression::Identifier(id)` (an
`IdentifierReference`), `Expression::StringLiteral(s)` (`s.value.as_str()`),
`Expression::StaticMemberExpression(m)` (`m.object`, `m.property.name`), `Expression::CallExpression`,
`Statement::BlockStatement(b)` (`b.body`), `Argument::SpreadElement(_)` / `arg.as_expression()`.

Shapes worth knowing (verified against the parser):

| Source | Nodes |
|---|---|
| `class A { m(a) {} }` | `Class` → `ClassBody` → `MethodDefinition` (`key`, `kind`, `r#static`, `r#override`) → `Function` → `FormalParameters` + `FunctionBody` |
| `{ k: () => 1, m() {} }` | `ObjectExpression` → `ObjectProperty` (`key`, `value`, `kind` Init/Get/Set, `method`, `shorthand`, `computed`) |
| `x = () => 1` in a class | `PropertyDefinition` (`value: Option<Expression>`) |
| function / arrow bodies | `FunctionBody` (**not** `BlockStatement`); `if`/loop/`try` bodies are `BlockStatement` |
| `a?.b()` | `ChainExpression` → `CallExpression` (`optional`) → `StaticMemberExpression` (`optional`) |
| `x as T`, `x!`, `x satisfies T`, `<T>x` | `TSAsExpression`, `TSNonNullExpression`, `TSSatisfiesExpression`, `TSTypeAssertion` wrapping the expression |
| `let a, b` | `VariableDeclaration` (`kind`, `declare`) → `VariableDeclarator` (`id: BindingPattern`, `init`, `type_annotation`) per name |
| `x: any` | `TSTypeAnnotation` → `TSAnyKeyword` |
| `if … else if` | `IfStatement` whose `alternate` is another `IfStatement` |
| `<Foo bar={x} />` | `JSXElement` → `JSXOpeningElement` (`name`: `JSXElementName::IdentifierReference` for components) |
| a `case` | `SwitchCase` (`test: None` for `default`) |

Other facts:

* `AstNode::span()` / `GetSpan::span()` — every node has a span; `node.id()` is its `NodeId`.
* `node.kind().ty()` gives the `AstType` (handy for `format!("{:?}")` in debugging).
* No parentheses nodes. Use `ctx.outer_parent` / `helpers::unwrap_expr` for TS wrappers.
* A declaration statement like `export const x = 1` is `ExportNamedDeclaration` →
  `VariableDeclaration`; `export default function () {}` is a `Function` with no `id`.
* TypeScript overload signatures, `declare function`, and abstract methods are `Function`s with
  `body: None`.
* Comments are not nodes: `ctx.comments()` (sorted, with `span`, `kind`), `ctx.has_comments_in(span)`.

Oxc sources are in `~/.cargo/registry/src/*/oxc_ast-0.133.0/src/ast/{js,ts,jsx,literal}.rs` for exact
field names.

---

## 6. Precision patterns

* Check that a name refers to the global before assuming what it is
  (`ctx.is_global(&m.object, "console")`); a local `console`, `Promise`, or `undefined` may shadow it.
* Skip `ctx.in_ambient_context(node)` and `declare` nodes for code-shape rules in TypeScript.
* In scripts (`!ctx.is_module()` and no CommonJS), top-level declarations are globals used by other
  files.
* Respect intent markers: `_`-prefixed names, explanatory comments (`ctx.has_comments_in`), `void x`.
* Relax rules for tests with `applies_to` → `!ctx.is_test()` only where the rule makes no sense in
  tests (logging, magic numbers), not for bugs.
* When a rule depends on types you cannot see (TypeScript types of expressions are not resolved),
  require literal evidence in the source, or stay silent.

---

## 7. Fixtures

Format (defined in `crates/mote-core/src/testing.rs`):

* File name: `tests/fixtures/<rule-name>[.variant].<ext>` — the rule is the part before the first
  dot: `no-self-compare.js`, `no-self-compare.typescript.ts`, `no-self-compare.react.tsx`.
* `// expect: <rule-name>` on a line: a finding for that rule must **start** on this line. Repeat
  for several findings on one line: `// expect: no-empty, no-empty`. `// expect-next-line: <rule>`
  targets the following line. Put nothing else after `expect:` — every token is read as a rule id.
  Don't write the text `expect:` anywhere else in a fixture.
* Every other line must produce **no** finding for the rule — that is how false positives are tested.
* Directives (in any comment): `mote-test: params max=3 allow=warn,error`,
  `mote-test: test-file` (analyze as test code), `mote-test: path src/app/x.ts`.
* A comment is itself content: an `expect:` comment *inside* an empty block makes the block
  non-empty. Put the comment after the closing brace.

Pick the extension deliberately — it decides the parser:

| Extension | Parsed as |
|---|---|
| `.js` | JavaScript + JSX; module if it has `import`/`export`, else a **script** (sloppy mode, top-level globals) |
| `.mjs` / `.cjs` | ES module / CommonJS |
| `.jsx` | JavaScript + JSX |
| `.ts` | TypeScript, no JSX (`<T>x` is a type assertion) |
| `.tsx` | TypeScript + JSX |

Cover: the basic positive; each variant of the pattern; nested functions, closures, and callbacks;
hoisting; destructuring; optional chaining; async/generators; classes (static, private `#x`,
getters/setters); TypeScript (types, enums, namespaces, `declare`, decorators, overloads, generics)
where relevant; JSX; and every false-positive trap you can think of as *negative* lines. Each rule
needs at least one `expect:` line; most need a `.ts` variant too.

`assert_all_rules_robust` additionally runs **every** rule on **every** fixture (with a path that has
no extension, so the most permissive grammar, TSX, is used): your rule must not panic on other
rules' fixtures.

### Fixes

A rule may attach a **fix** to a finding: byte-range edits of the source that remove the finding
*without changing what the program does*. `mote fix` applies them; the results JSON, SARIF and
the MCP server carry them.

```rust
let keyword = Span::new(decl.span.start, decl.span.start + 3);
ctx.emit(keyword, "Use 'const' instead of 'let'")
    .fix(
        "Declare with `const`",
        vec![Edit::replace(keyword.start as usize, keyword.end as usize, "const")],
    );
```

* Only for rewrites that are behaviour-preserving by construction: deleting something that has
  no effect, reordering what the language does not order, spelling the same thing another way.
  Where a case is not provably safe, emit the finding without a fix (a TypeScript import whose names are all unused, a backslash in a string that may have been meant for a regular expression). Set
  `fixable: true` in the `RuleMeta` and say in the description what the fix does and why it
  is safe (a "**Fix available.**" paragraph).
* Edits are `Edit::replace(start, end, text)`, `Edit::delete(start, end)` and
  `Edit::insert(at, text)` on byte offsets of `ctx.source()`; the edits of one fix must not
  overlap. `fix::delete_span` deletes a statement or declaration together with its line when
  nothing else is on it; `fix::line_break_at` gives the line break to use in inserted text
  (CRLF files stay CRLF). The engine drops a fix whose edits fall outside the text or inside a
  character, and never offers one on a suppressed finding.
* `mote fix` applies every fix that does not conflict with another, analyzes the file again and
  repeats (at most 10 passes), so a fix need not care about its neighbours. It discards all of a
  file's changes when they would add a syntax error: a fix that breaks code is a bug.
* Fixtures: `<fixture file name>.fixed` next to a fixture (`prefer-const.ts.fixed`) holds the
  text after every fix of the rule is applied; each fixable rule needs at least one. Write them
  with `MOTE_BLESS_FIXED=1 MOTE_FIXTURE=<rule-name> cargo test -p mote-lang-js --test
  fixtures`, then read every changed line. For a case that must get no fix, a `.fixed` file
  identical to the fixture checks that none is offered.

---

## 8. Severity and quality

| Severity | Use for | Examples |
|---|---|---|
| `High` | almost certainly a bug or serious reliability hazard — keep it honest, it gates PRs | duplicate keys, self-assignment, `return` in `finally`, comparing with `NaN` |
| `Medium` | likely problem or significant maintainability cost | empty blocks, unused declarations, `==`, `debugger` |
| `Low` | minor / stylistic | `var`, `console.log`, `any`, naming |

`Quality::Reliability` + `RuleKind::Bug` for bugs; `Quality::Maintainability` +
`RuleKind::CodeSmell` for smells. Set `status: RuleStatus::Incubating` for rules whose precision is
not yet proven on real code (they report but never fail a gate). Set `default_enabled: false` for
opinionated rules.

---

## 9. Performance

The budget: all rules together must keep the plugin within ~2× of raw Oxc parse + semantic. On
VS Code (single thread, release, measured 2026-10-06; Oxc
parse + semantic ~1.65 s) the plugin takes ~1.33× with no rules and ~2.2× with every rule: the
budget is exceeded, and nearly all of the difference is the rules'
own `run` / `run_once` / `applies_to` time (the framework resolves the enabled rules and the
dispatch table once per configuration, so per file it only evaluates the gates and walks).

* **Subscribe narrowly** (`types()`); `run` is only called for those node types.
* **Name the callees** when a `CallExpression` / `NewExpression` rule only acts on calls to certain
  names: `fn callee_names(&self) -> &'static [&'static str]` makes the plugin look the name up once
  per call (`helpers::callee_name`) instead of calling every such rule on every call. List every name
  under which `run` may emit or record state (copy the rule's own early-return check); leave it empty
  when any callee can matter (a local function, any receiver).
* **No full-tree walks** in `run`. `run_once` may iterate the symbol table or `ctx.nodes()` only when
  cheap and linear; prefer collecting what you need during `run` with `ctx.state`.
* Do the cheapest check first (operator, node kind, identifier name) and return early; ancestor
  walks (`ctx.ancestors`, `is_in_loop`) and parameter lookups come after.
* Don't allocate on the hot path: compare `&str`s (`id.name == "foo"`), build `String`s only when
  emitting. Read parameters once per file (keep them in `ctx.state`) when `run` needs them often.
* **Settings derived from parameters** (parsed lists, sets of formats): `ctx.rule_memo(|ctx| …)`
  computes them once per rule set and gives every file the same `&T`, instead of re-reading and
  re-parsing the parameters in every file; the closure may look at parameters only, never at the
  file (see `naming_convention`).
* **A fixed list of names tested on a hot node** (method names on every call statement): put a
  `static FILTER: helpers::NameFilter = helpers::NameFilter::of(&LIST);` in front of the list
  scan or hash lookup; it rejects most other names with one bit test (no false negatives) and is
  built at compile time (see `ignored_call_result`).
* A `run_once` pass over `ctx.nodes()` costs about as much as the plugin's own walk: avoid a second
  one (subscribe instead), and test names against a `helpers::NameSet` before any hashing or
  ancestor walk.
* Anything per-file must be linear in file size (no nested loops over all nodes).
* Measure: `cargo test -p mote-lang-js --release --test corpus -- --ignored --nocapture`
  prints throughput and per-rule finding counts on a real code base (set `MOTE_JS_CORPUS` to a
  checkout of VS Code or another JS/TS repository; the test is skipped when it is absent), including the all-rules / Oxc ratio. Add `MOTE_JS_RULE=js/your-rule
  MOTE_JS_SAMPLES=50` to eyeball samples for false positives, `MOTE_RULE_TIMINGS=1` for the
  time of each rule (`run`, `run_once`, `applies_to`) and each phase, and `MOTE_JS_REPEAT=3` to
  keep the fastest of three timed passes on a busy machine.

---

## 10. Commands and checklist

```text
cargo test -p mote-lang-js                                     # unit tests + all fixtures
MOTE_FIXTURE=no-self-compare cargo test -p mote-lang-js --test fixtures           # one rule
cargo clippy -p mote-lang-js --all-targets -- -D warnings
cargo fmt -p mote-lang-js
cargo test -p mote-lang-js --release --test corpus -- --ignored --nocapture       # real code
```

Checklist for each rule:

- [ ] id `js/<kebab-name>`, struct + `static` meta in `src/rules/batch_N/<name>.rs`, listed in
      `batch_N::rules()`
- [ ] description in our own words with a flagged and a compliant example
- [ ] severity / quality / kind chosen per section 8
- [ ] fixtures with positives and tricky negatives (`.js` and, where relevant, `.ts` / `.tsx`)
- [ ] no `unwrap`/`expect`/indexing that can fail on odd input; no panicking Oxc getters
- [ ] tests, clippy, fmt green; corpus run shows no panics and plausible findings
