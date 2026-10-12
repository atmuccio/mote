//! Mote Java plugin, built on tree-sitter-java.
//!
//! * [`JavaPlugin`] implements [`mote_core::LanguagePlugin`].
//! * Rules implement [`JavaRule`] and receive a [`Ctx`] (text, reporting, parameters, tree
//!   navigation, comments, imports, and the file-local [resolver](Ctx::type_of)).
//! * Rules are registered in [`rules::all`]; see `RULE_AUTHORING.md` for the guide.
//!
//! Each file is parsed once (within a deterministic work budget, [`ParseBudget`]) and walked
//! once: the walk dispatches every node to the enabled rules subscribed to its kind and, at the
//! same time, computes function metrics, collects comments, and recognises `@SuppressWarnings` /
//! `@SuppressFBWarnings` suppressions and IntelliJ's `//noinspection` comments; the other
//! compatibility comments (`// NOPMD`, `// CHECKSTYLE:OFF`) are read from the collected comments
//! afterwards (see `suppress.rs`, and [`pmd`], [`spotbugs`] and [`intellij`] for the rule names
//! of those tools).

mod comments;
mod ctx;
mod declared;
pub mod intellij;
mod kinds;
mod lombok;
mod metrics;
mod parse;
pub mod pmd;
pub mod resolve;
mod rule;
pub mod rules;
pub mod spotbugs;
mod suppress;
pub mod syntax;
pub mod timing;
mod varargs;
mod walk;

pub use ctx::{Ancestors, Ctx, Emitted, Import, Members};
pub use declared::DeclaredTypes;
pub use kinds::kind;
pub use resolve::{ClassType, JType, Prim, VarDecl, VarKind};
pub use rule::{BoxedRule, JavaRule, Span};

use mote_core::{FileAnalysis, LanguagePlugin, LexerSpec, RuleMeta, RuleSet, SourceFile};
use std::time::Duration;

/// Everything a rule module needs: `use crate::prelude::*;`.
pub mod prelude {
    pub use crate::ctx::{Ctx, Import};
    pub use crate::declared::DeclaredTypes;
    pub use crate::kinds::kind;
    pub use crate::resolve::{JType, Prim, VarDecl, VarKind};
    pub use crate::rule::{BoxedRule, JavaRule, Span};
    pub use crate::syntax::{self, named_children, Field, NodeExt};
    pub use mote_core::{
        ParamDefault, ParamSpec, Quality, RuleKind, RuleMeta, RuleStatus, Severity,
    };
    pub use rustc_hash::{FxHashMap, FxHashSet};
    pub use tree_sitter::Node;
}

pub use parse::ParseBudget;

/// Rule parameter through which the engine passes the file's Java language release (an
/// integer such as `17`) to every enabled Java rule; absent when unknown. Not a declared
/// parameter of any rule: read it with [`Ctx::java_release`]. Must match the engine's
/// `mote_engine::JAVA_RELEASE_PARAM`.
pub const JAVA_RELEASE_PARAM: &str = "java_release";

/// The Java language plugin.
pub struct JavaPlugin {
    registry: walk::Registry,
    budget: ParseBudget,
    timeout: Option<Duration>,
}

impl JavaPlugin {
    pub fn new() -> Self {
        JavaPlugin {
            registry: walk::Registry::new(),
            budget: ParseBudget::DEFAULT,
            timeout: None,
        }
    }

    /// Override the per-file parse work budget (default [`ParseBudget::DEFAULT`]). A file whose
    /// parse needs more is reported with one syntax error and no other results. The budget counts
    /// parser operations, so the outcome is the same on every machine and at any load.
    pub fn with_parse_budget(mut self, budget: ParseBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Also stop parsing after `timeout` of wall-clock time (off by default). A file that takes
    /// longer is reported with one syntax error and no other results. The outcome then depends on
    /// machine speed and load, so never enable this where results are cached or compared.
    pub fn with_parse_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Kind names used by rules that do not exist in the grammar (should be empty; checked by tests).
    pub fn unknown_rule_kinds(&self) -> &[(&'static str, &'static str)] {
        &self.registry.unknown_kinds
    }
}

impl Default for JavaPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguagePlugin for JavaPlugin {
    fn id(&self) -> &'static str {
        "java"
    }
    fn display_name(&self) -> &'static str {
        "Java"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }
    fn version(&self) -> u32 {
        // 2: deterministic parse budget (no wall-clock deadline); rule fixes.
        // 3: rules read the file's Java release (`Ctx::java_release`); narrowed rules:
        //    instanceof-pattern-candidate (only on Java 16+), constant-if-condition (throw-only
        //    `if (false)`, explained by a comment), resource-not-closed-with-try (lent
        //    connections, close() right after, parent connection closed on failure),
        //    unguarded-log-argument (guarded private helpers, String.valueOf(primitive), enum
        //    toString()), octal-literal (file modes and permissions).
        // 4: compatibility suppressions `// NOPMD`, `@SuppressWarnings("PMD[.Rule]")`,
        //    `// CHECKSTYLE:OFF` … `// CHECKSTYLE:ON`, per source (kinds `compat:pmd`,
        //    `compat:checkstyle`, `compat:suppress-warnings-all`), plus `@SuppressFBWarnings`
        //    (`compat:spotbugs`) and IntelliJ inspection names / `//noinspection`
        //    (`compat:intellij`); 118 more rules (189 -> 307; batches 8-14); rules widened and
        //    narrowed after precision review; incubating rules promoted to stable;
        //    varargs-annotation and case-pattern parse fixes; broader default test globs;
        //    test-class-naming accepts the `Test` prefix and the `TestCase` suffix by default;
        //    exception-not-thrown narrowed. (The engine cache version invalidates caches.)
        4
    }
    fn rules(&self) -> Vec<&'static RuleMeta> {
        self.registry.metas.clone()
    }
    fn lexer(&self) -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: true,
            template_literals: false,
            cpd_skip_line_prefixes: &["import ", "package "],
        }
    }
    fn default_test_globs(&self) -> &'static [&'static str] {
        &[
            "**/src/test/**",
            "**/src/testFixtures/**",
            "**/src/integrationTest/**",
            "**/src/it/**",
            "**/*-integration-tests/**",
            "**/*Test.java",
            "**/*Tests.java",
            "**/*IT.java",
            "**/test/**",
        ]
    }
    fn analyze(&self, file: &SourceFile<'_>, rules: &RuleSet) -> FileAnalysis {
        let mut analysis = walk::analyze(&self.registry, file, rules, self.budget, self.timeout);
        suppress::from_comments(&analysis.comments, &mut analysis.suppressions);
        analysis
    }
}
