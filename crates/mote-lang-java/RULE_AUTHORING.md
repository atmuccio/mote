# Writing Java rules for Mote

This is the guide to how rules are written in `mote-lang-java`, for anyone reading the code or
building Mote from source. Read `docs/architecture.md` first: original rules only (nothing from
proprietary analyzers, no GPL code), precision over recall, one traversal, never panic,
deterministic output. Rule ideas may come from general knowledge, PMD (BSD), Error Prone
(Apache-2.0), ESLint (MIT); the implementation and all wording are Mote's own.

The framework API described here (`JavaRule`, `Ctx`, the resolver, `syntax`) is internal to Mote:
it is not a stable plugin API and may change in any 0.x release. Before writing a private helper,
check the fast accessors in §5, §7 and §10 (`Field` ids, `ctx.call_name`, `call_names`,
`ctx.declared_types`, the walk-stack sibling helpers) — they exist so that rules don't
re-implement them. A helper only one batch needs lives in that batch's directory; the framework
grows when many rules benefit.

---

## 1. Where things go

Rules are grouped in batch directories under `src/rules/`. A rule lives in one of them, next to
the batch's other rules, and its fixtures live in `tests/fixtures/`:

```
crates/mote-lang-java/
  src/rules/batch_N/mod.rs          ← lists the batch's rules
  src/rules/batch_N/<rule_name>.rs  ← one file per rule (snake_case of the rule name)
  src/rules/batch_N/util.rs         ← optional helpers shared by the batch's rules
  tests/fixtures/<rule-name>.java   ← at least one fixture per rule (more: <rule-name>.<variant>.java)
```

Rule ids are `java/<kebab-name>`; they must be unique across all batches. Before picking a
name, check the ids already in `src/rules/` (`grep -r 'RuleMeta::new("java/' src/rules`).

## 2. Adding a rule, step by step

1. Create `src/rules/batch_N/my_rule.rs` from the template below.
2. Register it in `src/rules/batch_N/mod.rs`:
   ```rust
   mod my_rule;

   pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
       vec![
           Box::new(my_rule::MyRule),
           Box::new(other_rule::OtherRule::default()), // stateful rules: Default
       ]
   }
   ```
3. Write `tests/fixtures/my-rule.java` with positives (`// expect: my-rule`) **and** tricky
   negatives (§8).
4. Run `MOTE_FIXTURE=my-rule cargo test -p mote-lang-java --test fixtures`, then the
   full `cargo test -p mote-lang-java` and
   `cargo clippy -p mote-lang-java --all-targets -- -D warnings`. Format with
   `cargo fmt -p mote-lang-java`.

The crate's own tests check every rule automatically: id prefix and kebab-case, unique ids,
non-empty name/description, described parameters, and that every kind in `kinds()` is a real
**named** node kind of tree-sitter-java (a typo would otherwise never fire).

## 3. Template

~~~rust
//! `java/my-rule` — one-line summary.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,            // Bug | CodeSmell
    quality: Quality::Maintainability,    // Reliability for bugs
    severity: Severity::Medium,           // see §9
    tags: &["some-tag"],
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(3),
        description: "Largest number of … that is accepted.",
    }],
    remediation_minutes: 5,
    description: r#"Detects …  (what)

**Why it matters.** …  (why)

**How to fix.** …  (how)

```java
// Noncompliant
…
// Compliant
…
```"#,
    ..RuleMeta::new("java/my-rule", "java", "Short human title")
};

pub struct MyRule;

impl<'t> JavaRule<'t> for MyRule {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    // Only calls named `foo` reach `check` (plus calls without a name, inside syntax errors).
    fn call_names(&self) -> &'static [&'static str] {
        &["foo"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("foo") {
            return;
        }
        let Some(name) = node.field(Field::Name) else { return };
        if syntax::argument_count(node) > ctx.param_i64("max") as usize {
            ctx.emit(name, "Pass fewer arguments to foo()");
        }
    }
}
~~~

A **stateful, file-level** rule keeps per-file data in its own fields (a fresh instance is
created for every file) and reports in `finish`:

```rust
#[derive(Default)]
pub struct MyFileRule<'t> {
    declared: Vec<(Node<'t>, &'t str)>,   // nodes and source slices may be stored
    used: FxHashSet<&'t str>,
}

impl<'t> JavaRule<'t> for MyFileRule<'t> {
    fn meta(&self) -> &'static RuleMeta { &META }
    fn kinds(&self) -> &'static [&'static str] { &["field_declaration", "identifier"] }
    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) { /* collect */ }
    fn finish(&mut self, ctx: &Ctx<'t>) {
        for (node, name) in &self.declared {
            if !self.used.contains(name) {
                ctx.emit(*node, format!("Remove `{name}`"));
            }
        }
    }
}
```

Constructors (`MyRule`, `Default::default()`) must be trivial: no allocation, no table
building. Constant tables go in `static`/`const` items (use `std::sync::LazyLock` if they must be
computed). Iterate `Vec`s, not hash maps, when emitting (the plugin sorts findings anyway, but
keep the logic deterministic).

## 4. The rule lifecycle

* The plugin parses each file once and walks the tree **once**, in pre-order (parents before
  children, source order). For each node whose kind is in your `kinds()`, `check(node, ctx)` is
  called — only if your rule is enabled for this file.
* After the walk, `finish(ctx)` is called once for each enabled rule.
* Which rules are enabled, with which parameters, is resolved once per configuration, not per
  file: the engine shares one rule set between all files with the same effective configuration,
  and the walker builds its dispatch tables (restricted to the enabled rules) from it once. A
  disabled rule costs nothing during the walk, and `ctx.param_*` / `ctx.java_release()` find the
  running rule's settings by its index, without a rule-id lookup.
* A file whose parse exceeds the work budget (`ParseBudget`, proportional to its size) or whose
  tree is deeper than 10 000 levels is reported as one syntax error; no rule sees it. Both
  limits are deterministic and far beyond real code.
* For `method_invocation`, a rule that declares [`call_names`](#10-performance) only sees the
  calls with one of those names (and calls without a name); likewise for `method_declaration`
  and [`method_names`](#10-performance).
* `check` also sees nodes inside `ERROR` subtrees of files with syntax errors: always use
  `Option`-returning accessors (`node.field(Field::Name)`, `let … else { return }`), never index
  or `unwrap()`. A panic costs the whole file its results.
* Don't walk the whole tree yourself. Looking at a node's children, one bounded subtree (a
  class body, a method body), or its ancestors is fine. If you need information from all over
  the file, subscribe to the relevant kinds and collect it.

## 5. `Ctx` reference

All methods take `&self`; references and nodes they return live for the whole file (`'t`), so
you can store them in your rule and freely interleave reads with `emit`.

**Text & file**

| | |
|---|---|
| `ctx.text(node) -> &'t str` | source text of a node |
| `ctx.text_at(span)` | text of a `Span` |
| `ctx.source()`, `ctx.path()` | whole file text; repo-relative path with `/` |
| `ctx.is_test()` | file is test code (skip rules that make no sense in tests) |
| `ctx.line(node)` | 1-based start line |
| `ctx.root()` | the `program` node |

**Reporting** (rule id and `symbol` — `Outer.Inner#method` — are filled in automatically)

```rust
ctx.emit(node, "Message");                                   // smallest meaningful node
ctx.emit_range(Span::cover(first, last), "Message");         // a span across nodes
ctx.emit(name, format!("Method has {n} parameters")).value(n as f64);
ctx.emit(dup, "Remove this duplicate").secondary(original, "first declared here");
```

`Span` is `Copy` (`Span::from(node)`, `Span::cover(a, b)`, `Span::new(start, end)`); store it
when you only need a location later.

**Parameters** of the rule being run (configured value, else the default you declared in
`META.params`): `ctx.param_i64("max")`, `param_f64`, `param_bool`, `param_str`. Reading an
undeclared parameter asserts in debug builds.

**Java release**: `ctx.java_release() -> Option<u32>` is the language release the file is
compiled for (`8`, `11`, `17`, …): `java.release` in `.mote.yaml`, or what the engine read
from the nearest `pom.xml` / `build.gradle(.kts)`. A rule suggesting syntax of a newer release
stays silent when it is `None` or older (`if ctx.java_release().is_none_or(|r| r < 16) { return; }`).
The engine passes it as the reserved parameter `java_release` (`JAVA_RELEASE_PARAM`); don't
declare it in `META.params`. In fixtures: `// mote-test: params java_release=17`.

**Tree navigation** (cheap for the node being checked and its ancestors — the walker keeps the
ancestor stack; for other nodes one descent from the root)

| | |
|---|---|
| `ctx.parent(node)` | parent node |
| `ctx.ancestors(node)` | iterator, innermost first, ends at `program`; `.nth(k)` and `.len()` are O(1) |
| `ctx.depth(node)` | number of ancestors |
| `ctx.next_named_sibling(node)`, `ctx.prev_named_sibling(node)`, `ctx.next_sibling(node)` | same results as the `Node` methods (comments included), parent taken from the walk stack |
| `ctx.enclosing_method(node)` | method / constructor / compact constructor; `None` if a type body is crossed first |
| `ctx.enclosing_function(node)` | also lambdas, static and instance initializers |
| `ctx.enclosing_class(node)` | class/interface/enum/record/annotation declaration, or an anonymous class (`object_creation_expression` / `enum_constant` with a body) |
| `ctx.symbol_of(node)` | `Outer.Inner#member` |
| `ctx.is_inside(node, "finally_clause", stop_at_function)` | ancestor-kind test |

Do **not** call `node.parent()`, `node.next_named_sibling()`, `node.prev_named_sibling()` or
`node.next_sibling()` — tree-sitter recomputes the parent by descending from the root every time
(~1–2 µs on real files). Use the `ctx` versions above.

**Calls and declarations**

| | |
|---|---|
| `ctx.call_name(call) -> Option<&'t str>` | method name of a `method_invocation` (`foo` in `a.<T>foo(x)`), read once per call node and shared by all rules; `None` for other nodes and calls without a name |
| `ctx.declared_types()` | `DeclaredTypes`: every variable, field, parameter, resource and `for` variable name of the file with its declared type node (as written): `type_of_name(name)` (only when all declarations of that name agree), `types_of(name)`. Filled by the walker; complete in `finish`, during `check` it holds the declarations seen so far. A syntactic pre-filter before resolver work |

**Comments, imports, package**

* `ctx.comments() -> &[Comment]` — complete in `finish`; during `check` only the comments that
  precede the current node. To react to comments during the walk, subscribe to
  `line_comment` / `block_comment` (Javadoc is a `block_comment` starting with `/**`).
* `ctx.imports() -> &[Import]` — `path` (`java.util.List`, without `.*`), `is_static`,
  `is_wildcard`, `simple_name()`, `qualifier()`, `node`. Available from the first `check`.
* `ctx.package_name()`, `ctx.header_end()` (byte offset where package/imports end).

**Lombok**: `ctx.has_lombok(class_decl)`, `ctx.field_has_generated_accessors(field_decl)`.
Rules about unused fields, missing accessors, or constructors must skip Lombok classes.

## 6. The resolver

One strategy, file-local: declarations in this file, imports (single-type, on-demand, static),
implicit `java.lang`, and a curated table of ~300 common JDK classes (`src/resolve/jdk.txt`).
Types declared in other files of the repository or in libraries are **unknown**: the API
answers `None` (or `false`), and your rule must stay silent.

| | |
|---|---|
| `ctx.type_of(expr) -> Option<JType>` | static type of an expression |
| `ctx.resolve_type(type_node)` | a type node (`List<String>`, `int[]`) |
| `ctx.declaration_of(identifier) -> Option<VarDecl>` | what a name refers to |
| `ctx.is_subtype_of(&t, "java.util.Collection")` | provably a subtype |
| `ctx.subtype(&t, fqn) -> Option<bool>` | tri-state: `None` when part of the hierarchy is unknown |
| `ctx.as_supertype(&t, "java.lang.Iterable")` | `ArrayList<String>` → `Iterable<String>` |
| `ctx.is_call_to(call, "java.lang.Thread", "sleep")` | instance, static, static-import, or inherited call |
| `ctx.type_name_of(expr)` | FQN when `expr` names a type (`Math` in `Math.max`) — also for imported library types |
| `ctx.resolve_type_name("List", at)` | FQN of a type name visible at a node (imports included) |
| `ctx.class_type(decl)`, `ctx.class_fqn(decl)` | type / FQN of a class declared in this file |
| `resolve::is_known_class(fqn)`, `resolve::is_final_class(fqn)`, `resolve::is_enum_class(fqn)` | JDK table queries (JDK enums are not modeled as subtypes of `java.lang.Enum`; enums of this file are) |

`type_of` handles: locals, parameters, fields (also `this.x`, `Outer.this.x`, inherited from
classes in this file), `var` (from the initializer or the iterable), pattern variables,
literals (text blocks too), `new`, casts, parentheses, string concatenation, arithmetic
promotion, comparisons, array access/creation, `.length`, ternaries, `this`, enum constants
and `values()`/`valueOf`, method calls on known receivers with generic substitution
(`Map<K, V>.entrySet()` → `Set<Map.Entry<K, V>>`), static calls, static imports, and calls to
methods declared in this file (overloads by arity and argument types). Lambda parameters,
method type variables, and ambiguous overloads are unknown.

`JType`: `Prim(Prim)`, `Void`, `Null`, `Class(ClassType { name, args })`, `Array(Box<JType>)`,
`Unknown` (only nested inside type arguments). Helpers: `fqn()`, `simple_name()`,
`is_class(fqn)`, `is_string()`, `is_boxed()`, `unboxed()`, `is_primitive()`,
`is_numeric_primitive()`, `is_numeric()` (incl. wrappers), `is_boolean()`,
`is_boolean_like()`, `is_array()`, `element_type()`, `type_arg(i)`, `is_reference()`,
`is_null()`, `is_void()`, `is_fully_known()`; `Display` gives `Map.Entry<String, ?>`.

`VarDecl`: `kind` (`Local`, `Parameter`, `LambdaParameter`, `CatchParameter`,
`ForEachVariable`, `Resource`, `PatternVariable`, `Field`, `EnumConstant`, `RecordComponent`),
`name`, `decl` (declarator/parameter node), `statement` (the declaration carrying modifiers),
`type_node`, `value` (initializer / iterable), `is_var`.

**Precision rules.** Only report when the resolver *proves* the property you depend on. Use
`is_subtype_of` (true only when proven) to fire; use `subtype(..) == Some(false)` when your rule
fires on "is *not* a subtype". Don't treat `None` as "not a String". Resolve cheaply-rejected
conditions first (method name, operator, argument count) and call `type_of` last.

## 7. Syntax helpers and node kinds

`use crate::prelude::*;` brings `Node`, `kind`, `named_children`, `syntax`, `Field` and
`NodeExt` (`node.field(Field::Name)`), `Span`, `JType`, `VarKind`, `DeclaredTypes`,
`FxHashMap`/`FxHashSet`, and the metadata types.

* `kind(node)` — fast kind name (prefer it over `node.kind()`).
* **Fields by id**: `node.field(Field::Body)` (trait `NodeExt`, in the prelude) or
  `syntax::field(node, Field::Body)` returns the same node as `node.child_by_field_name("body")`
  without looking the name up in the grammar's field table on every call. Prefer it everywhere;
  on hot kinds the by-name lookup is a measurable cost.
* **Comments are extra nodes that can appear anywhere**: `node.named_child(0)` may be a
  comment. Use field access (`node.field(Field::Body)`) or `syntax::named_children(node)`,
  which skips comments. `syntax::children(node)` includes anonymous tokens and comments. Both
  read lists of up to 8 children by index (no allocation) and longer ones with a tree cursor;
  `first_named_child` never allocates.
* `syntax::strip_parens`, `modifiers`, `has_modifier(decl, "static")`, `annotations(decl)`,
  `annotation_name(ann, src)`, `has_annotation(decl, src, "Override")`, `type_body(decl)`,
  `members(body)` (enum constants and `enum_body_declarations` flattened), `declarators`,
  `parameters(method)`, `arguments(call)`, `argument_count`, `operator(expr)`,
  `operator_str(expr)` (`"=="`, `"&&"`, `"+="`), `is_null_literal`, `is_string_literal`,
  `int_literal_value(node, src)`, `string_literal_content(node, src)`, `name_node`,
  `name_of(decl, src)`, `is_empty_block`, `block_has_comment`, `is_type_declaration`,
  `is_method_like`, `is_function_like`, `first_named_child`, `child_of_kind`.

To see the tree for any snippet:
`cargo run -p mote-lang-java --example dump -- Some.java --tree` prints every named node
as `kind [field] line:col` (plus the findings, metrics and comments of the file).

**Common kinds and fields** (tree-sitter-java 0.23.5; `?` optional, `*` repeated)

| Kind | Fields / children |
|---|---|
| `class_declaration` | `name`, `type_parameters?`, `superclass?` (`superclass` → type), `interfaces?` (`super_interfaces`), `permits?`, `body` (`class_body`); child `modifiers` |
| `interface_declaration` | `name`, `type_parameters?`, `body` (`interface_body`); child `extends_interfaces` |
| `enum_declaration` | `name`, `interfaces?`, `body` (`enum_body`: `enum_constant`*, `enum_body_declarations`?) |
| `record_declaration` | `name`, `parameters` (`formal_parameters`), `interfaces?`, `body` |
| `annotation_type_declaration` | `name`, `body` |
| `method_declaration` | `name`, `parameters`, `type`, `body?`, `type_parameters?`; children `modifiers`, `throws` |
| `constructor_declaration` | `name`, `parameters`, `body` (`constructor_body`) |
| `compact_constructor_declaration` | `name`, `body` |
| `field_declaration` / `local_variable_declaration` / `constant_declaration` | `type`, `declarator`* (`variable_declarator`: `name`, `value?`, `dimensions?`) |
| `formal_parameter` / `spread_parameter` | `type`, `name` / (type, `variable_declarator`) |
| `modifiers` | anonymous keywords (`public`, `static`, `final`, …) and `annotation` / `marker_annotation` (`name`, `arguments?`) |
| `static_initializer`; instance initializer | `block`; a `block` directly in a `class_body` |
| `enum_constant` | `name`, `arguments?`, `body?` |
| `lambda_expression` | `parameters` (`identifier` / `inferred_parameters` / `formal_parameters`), `body` (block or expression) |
| `if_statement` | `condition` (`parenthesized_expression`), `consequence`, `alternative?` (an `if_statement` for `else if`) |
| `for_statement` | `init`*, `condition?`, `update`*, `body` |
| `enhanced_for_statement` | `type`, `name`, `value`, `body` |
| `while_statement` / `do_statement` | `condition`, `body` |
| `switch_expression` (also switch *statements*) | `condition`, `body` (`switch_block`: `switch_block_statement_group`* or `switch_rule`*; `switch_label` = `case …` / `default`, may hold `pattern`, `guard`) |
| `try_statement` | `body`; children `catch_clause`*, `finally_clause?` |
| `try_with_resources_statement` | `resources` (`resource_specification` → `resource`: `type`, `name`, `value`), `body`, catches, finally |
| `catch_clause` | `body`; child `catch_formal_parameter` (`name`; child `catch_type` with one or more types) |
| `finally_clause` | child `block` |
| `return_statement`, `throw_statement`, `yield_statement` | expression child |
| `break_statement`, `continue_statement` | optional `identifier` label |
| `labeled_statement`, `synchronized_statement` (`body`), `assert_statement`, `expression_statement` | |
| `explicit_constructor_invocation` | `constructor` (`this`/`super`), `arguments` |
| `method_invocation` | `object?`, `name`, `arguments` (`argument_list`), `type_arguments?` |
| `object_creation_expression` | `type`, `arguments`; child `class_body` for anonymous classes |
| `field_access` | `object`, `field` (`identifier` or `this`) |
| `binary_expression` | `left`, `operator`, `right` |
| `unary_expression` | `operator`, `operand`; `update_expression` (`x++`) has one child |
| `assignment_expression` | `left`, `operator` (`=`, `+=`, …), `right` |
| `ternary_expression` | `condition`, `consequence`, `alternative` |
| `instanceof_expression` | `left`, `right?`, `name?` (pattern variable), `pattern?` |
| `cast_expression` | `type`, `value` |
| `array_access` | `array`, `index`; `array_creation_expression`: `type`, `dimensions`, `value?` |
| `method_reference`, `class_literal`, `this`, `super`, `parenthesized_expression` | |
| literals | `string_literal` (text blocks too), `character_literal`, `decimal_integer_literal`, `hex_integer_literal`, `octal_integer_literal`, `binary_integer_literal`, `decimal_floating_point_literal`, `true`, `false`, `null_literal` |
| names & types | `identifier`, `type_identifier`, `scoped_identifier`, `scoped_type_identifier`, `generic_type` (`type_arguments`), `array_type`, `integral_type`, `floating_point_type`, `boolean_type`, `void_type`, `wildcard` |
| comments | `line_comment`, `block_comment` |

Operators, keywords, and punctuation are anonymous nodes: they can't be subscribed to, but you
can read them (`syntax::operator_str`, `syntax::has_modifier`, `kind(child) == "else"`).

## 8. Fixtures

Format (see `crates/mote-core/src/testing.rs`):

* File name `<rule-name>[.<variant>].java` selects the rule under test; only that rule runs.
* `// expect: <rule-name>` — a finding must **start** on this line (repeat the id for two
  findings on one line); `// expect-next-line: <rule-name>` for the following line. Every other
  line must be clean for that rule.
* Directives in a comment: `mote-test: params max=5 strict=true`,
  `mote-test: test-file`, `mote-test: path src/main/java/Foo.java`. Fixtures run
  without a Java release unless they set one: `mote-test: params java_release=17`.
* Cover positives **and** the false-positive traps: generics, nested/inner/local/anonymous
  classes, lambdas, records, enums, sealed types, text blocks, modern `switch` (arrows,
  patterns, guards), Lombok, unknown (repo/library) types, shadowing, overloads, test code.
* Mind where an `expect:` comment lands: inside an empty block it *is* a comment of that block.
* Every rule is also run on every fixture file (`assert_all_rules_robust`), so fixtures double
  as robustness input for all rules.

Run one rule's fixtures: `MOTE_FIXTURE=my-rule cargo test -p mote-lang-java --test fixtures`.

Check false positives on real code: point `MOTE_JAVA_CORPUS` at a checkout of a Java repository
(the numbers here are from [SymmetricDS](https://github.com/JumpMind/symmetric-ds); the test is
skipped when the corpus is absent), then
`MOTE_SHOW=my-rule cargo test -p mote-lang-java --test corpus -- --nocapture`
prints every finding of the rule over it with its symbol; read a sample of them. The
harness runs the plugin without the engine, so no Java release is known; add
`MOTE_JAVA_RELEASE=21` (SymmetricDS's release) for rules gated on one.

## 9. Severity, quality, messages

* `HIGH` — almost certainly a bug or a serious reliability hazard. The default gate fails PRs on
  new HIGH findings: keep it honest.
* `MEDIUM` — likely problem or significant maintainability cost.
* `LOW` — minor or stylistic.
* `RuleKind::Bug` + `Quality::Reliability` for defects; `RuleKind::CodeSmell` +
  `Quality::Maintainability` for smells (a smell that causes failures, like an empty catch, may
  use `Reliability`).
* Messages: one imperative sentence, specific, no trailing period, name the culprit where
  useful: ``Remove this unused private field `count` ``. Put measured numbers in `.value(..)`
  as well as in the message.
* Location: the smallest meaningful node — a name, an operator, a keyword — not a whole block.
* Description: Markdown, our own words: what, why it matters, how to fix, a short noncompliant
  and compliant example, and what is deliberately not reported.
* New rules default to `status: RuleStatus::Stable`; mark heuristic rules
  `RuleStatus::Incubating` (they report but never fail a gate) and `default_enabled: false` for
  opinionated ones.

## 10. Performance

Budget: the whole analysis must stay within 2× of parse time. On SymmetricDS (330k lines,
single thread, release) parsing takes ~1.55 s; the framework's walk, metrics, and comments bring
it to ~2.3 s (1.45× — a bare tree-sitter cursor walk alone is ~0.4 s of that); all ~330 rules
add ~1.45 s (total ~3.7 s, ~2.4× with `MOTE_JAVA_RELEASE=21`, measured 2026-10-06; merely dispatching to them, with every `check` skipped, already
costs ~0.15 s). Nearly all of that
is the rules' own `check`/`finish` time: the framework resolves the enabled rules and their
dispatch lists once per configuration, so it adds little per file beyond the calls themselves.
The budget is exceeded, so there is **no
headroom**: a new rule should cost well under 5 ms on SymmetricDS, and one on a hot kind
(`identifier`, `method_invocation`, `binary_expression`, `expression_statement`) has to reject
in a few tens of nanoseconds.

**Measure** with
`cargo test --release -p mote-lang-java --test corpus -- --ignored --nocapture corpus_throughput`
(compare the "all rules (1 thread)" line with and without your rule). With
`MOTE_RULE_TIMINGS=1` it adds a profiled pass and prints the time of each phase (parse,
walk, check, finish), the resolver's internal sections (name lookups, scope indexing, type
nodes, override checks), and the most expensive rules with their call counts ("net" subtracts
the profiler's own clock reads). The `dump` example prints the same for one file:
`MOTE_RULE_TIMINGS=1 cargo run --release -p mote-lang-java --example dump -- F.java`.

**Hot paths.** What a `check` costs is mostly how many tree-sitter operations it does:

* **`method_invocation`: declare `call_names`.** The walker reads each call's name once and only
  calls the rules whose `call_names` contain it (plus every rule for calls without a name,
  inside syntax errors). Declare it whenever `check` does nothing for any other name — then
  still test the name in `check` (`ctx.call_name(node)`), for the nameless calls. Most calls
  then cost your rule nothing at all.
* **`method_declaration` about one method (`equals`, `finalize`, `compareTo`): declare
  `method_names`**, the same filter on the declared name.
* **Members of a type body: `ctx.members(body)`**, not `syntax::members(body)`: the same nodes,
  read from the tree once per file and shared by every rule that iterates them.
* **The same analysis of a node in two rules: `ctx.shared_memo(tag, node.id(), || …)`** runs it
  once per file (the value must not borrow from the tree).
* **The operator of the expression being checked: `ctx.operator_str(node)`**, and **the name of
  the declaration being checked: `ctx.name_node(node)` / `ctx.name_of(node)`**: the same nodes
  as `syntax::operator_str` / `syntax::name_node`, read once for all the rules on that node.
* **Fields by id**: `node.field(Field::Name)`, never `child_by_field_name("name")`.
* **No parent or sibling methods on `Node`**: `ctx.parent`, `ctx.ancestors(..).nth(k)`,
  `ctx.next_named_sibling`, … instead (see §5).
* **Children**: `syntax::named_children` / `children` / `first_named_child` (no allocation for
  short lists); don't collect into a `Vec` to count or index.
* **Cheapest test first**: `kind`, operator, name, argument count, literal kinds (`x == 0` is
  never a string comparison), then modifiers/annotations, then `ctx.declared_types()` (a hash
  lookup), and only then `type_of` / `is_call_to` / `subtype`.
* **Resolver cost model**: answers are memoized per file (types per expression, name lookups,
  class facts, per-method scope indexes), so asking again is cheap; the expensive part is the
  first name lookup inside a method, which indexes that method's declarations. A rule that
  resolves one identifier in every method of a file pays for indexing all of them.
* **Subtree scans** cost ~0.1 µs per node visited: bound them (one statement, one method body)
  and prefer a text pre-filter on `ctx.text(node)` (`contains("close")`) before walking.
* **Class members**: the members of a class body are re-read by every rule that iterates them;
  test the cheapest property first (the name) before modifiers or parameters.
* **An index of a subtree, built on first need** (every assignment of a loop body, say), that
  later questions about nested nodes read from: track what is indexed with
  `rules::shared::util::CoveredRanges` (index a body only when it is not inside one indexed
  before; bodies are subtrees, so ranges nest or are disjoint) instead of indexing the whole file
  for the first node that asks or re-indexing every nested body (see `for_loop_counter_misuse`).

Don't:
* Walk the whole tree from `check` or `finish`.
* Allocate per node (e.g. collect children into a `Vec` just to count them — use iterators).
* Call `type_of` on every node "just in case".
* Keep global mutable state; rules run concurrently on different files (each file has its own
  rule instances, so plain fields are fine).
