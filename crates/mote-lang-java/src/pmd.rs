//! PMD rule names → the Mote Java rule that targets the same kind of issue.
//!
//! Used by the compatibility suppressions (`@SuppressWarnings("PMD.RuleName")` with
//! `suppressions.compat`) and by `mote migrate --from pmd`. Built from the `equiv` links of the
//! maintainers' category map, which the coverage benchmark uses (a CLI test keeps the two in sync
//! where the map is present): [`PMD_EQUIVALENTS`] for the rules of PMD's quickstart ruleset, which
//! the benchmark runs, and [`PMD_MIGRATION_EQUIVALENTS`] for other PMD rules with a clear
//! equivalent. PMD is BSD-licensed; only its rule names are used here.
//! [`PMD_RENAMED`] maps names of older PMD releases to the current names.

/// Current PMD rule name → Mote rule id, sorted by name.
pub const PMD_EQUIVALENTS: &[(&str, &str)] = &[
    (
        "AbstractClassWithoutAnyMethod",
        "java/abstract-class-without-methods",
    ),
    ("AssignmentInOperand", "java/assignment-in-operand"),
    (
        "AssignmentToNonFinalStatic",
        "java/static-field-set-in-constructor",
    ),
    (
        "AvoidBranchingStatementAsLastInLoop",
        "java/loop-exits-first-pass",
    ),
    (
        "AvoidCatchingGenericException",
        "java/catch-generic-exception",
    ),
    (
        "AvoidDecimalLiteralsInBigDecimalConstructor",
        "java/bigdecimal-from-double",
    ),
    (
        "AvoidInstanceofChecksInCatchClause",
        "java/instanceof-in-catch",
    ),
    (
        "AvoidMultipleUnaryOperators",
        "java/multiple-unary-operators",
    ),
    ("AvoidStringBufferField", "java/string-builder-field"),
    ("AvoidUsingOctalValues", "java/octal-literal"),
    (
        "BigIntegerInstantiation",
        "java/boxed-primitive-constructor",
    ),
    ("ClassNamingConventions", "java/class-naming"),
    (
        "ClassWithOnlyPrivateConstructorsShouldBeFinal",
        "java/private-constructor-class-not-final",
    ),
    ("CloseResource", "java/resource-not-closed-with-try"),
    ("CompareObjectsWithEquals", "java/value-reference-equality"),
    ("ComparisonWithNaN", "java/float-equality"),
    ("ConstantsInInterface", "java/constants-only-interface"),
    ("ControlStatementBraces", "java/missing-braces"),
    ("DanglingJavadoc", "java/dangling-doc-comment"),
    ("DoNotCallGarbageCollectionExplicitly", "java/explicit-gc"),
    (
        "DoNotExtendJavaLangError",
        "java/application-error-subclass",
    ),
    ("DontCallThreadRun", "java/thread-run-direct"),
    ("DontUseFloatTypeForLoopIndices", "java/float-loop-counter"),
    (
        "DoubleBraceInitialization",
        "java/double-brace-initialization",
    ),
    ("DoubleCheckedLocking", "java/double-checked-locking"),
    ("EmptyCatchBlock", "java/empty-catch-block"),
    ("EmptyControlStatement", "java/empty-block"),
    ("EmptyFinalizer", "java/finalize-override"),
    ("EqualsNull", "java/equals-with-null"),
    ("ExtendsObject", "java/redundant-inheritance-declaration"),
    (
        "FinalFieldCouldBeStatic",
        "java/final-field-could-be-static",
    ),
    ("ForLoopCanBeForeach", "java/index-loop-could-be-foreach"),
    (
        "FormalParameterNamingConventions",
        "java/local-variable-naming",
    ),
    ("GuardLogStatement", "java/unguarded-log-argument"),
    ("IdempotentOperations", "java/self-assignment"),
    ("IdenticalCatchBranches", "java/identical-catch-branches"),
    ("ImplicitSwitchFallThrough", "java/switch-fallthrough"),
    ("InstantiableUtilityClass", "java/utility-class-constructor"),
    ("JumbledIncrementer", "java/for-loop-counter-misuse"),
    (
        "LambdaCanBeMethodReference",
        "java/lambda-could-be-method-reference",
    ),
    ("LiteralsFirstInComparisons", "java/literal-first-in-equals"),
    (
        "LocalVariableNamingConventions",
        "java/local-variable-naming",
    ),
    ("LogicInversion", "java/negated-comparison"),
    ("LooseCoupling", "java/concrete-collection-type"),
    ("MethodNamingConventions", "java/method-naming"),
    ("MissingOverride", "java/missing-override"),
    ("NoPackage", "java/default-package"),
    ("NonExhaustiveSwitch", "java/missing-switch-default"),
    ("OneDeclarationPerLine", "java/one-variable-per-declaration"),
    ("OptimizableToArrayCall", "java/presized-to-array"),
    (
        "OverrideBothEqualsAndHashCodeOnComparable",
        "java/compare-to-without-equals",
    ),
    (
        "OverrideBothEqualsAndHashcode",
        "java/equals-without-hashcode",
    ),
    ("PackageCase", "java/package-naming"),
    ("PreserveStackTrace", "java/lost-exception-cause"),
    (
        "PrimitiveWrapperInstantiation",
        "java/boxed-primitive-constructor",
    ),
    (
        "ProtectedMemberInFinalClass",
        "java/protected-in-final-class",
    ),
    (
        "ReturnEmptyCollectionRatherThanNull",
        "java/collection-return-null",
    ),
    ("ReturnFromFinallyBlock", "java/return-in-finally"),
    ("SimplifiableTestAssertion", "java/assert-boolean-literal"),
    ("SimplifiedTernary", "java/simplifiable-ternary"),
    ("SimplifyBooleanReturns", "java/simplifiable-boolean-return"),
    (
        "SimplifyConditional",
        "java/redundant-null-check-before-instanceof",
    ),
    ("SingularField", "java/field-could-be-local"),
    ("SuspiciousOctalEscape", "java/suspicious-octal-escape"),
    (
        "TypeParameterNamingConventions",
        "java/type-parameter-naming",
    ),
    ("UncommentedEmptyConstructor", "java/empty-constructor"),
    ("UncommentedEmptyMethodBody", "java/empty-method"),
    ("UnconditionalIfStatement", "java/constant-if-condition"),
    (
        "UnnecessaryAnnotationValueElement",
        "java/redundant-annotation-value",
    ),
    (
        "UnnecessaryConstructor",
        "java/redundant-default-constructor",
    ),
    (
        "UnnecessaryFullyQualifiedName",
        "java/unnecessary-fully-qualified-name",
    ),
    ("UnnecessaryImport", "java/unused-import"),
    ("UnnecessaryReturn", "java/unnecessary-return"),
    ("UnnecessarySemicolon", "java/empty-statement"),
    ("UnsynchronizedStaticFormatter", "java/static-date-format"),
    (
        "UnusedFormalParameter",
        "java/unused-private-method-parameter",
    ),
    ("UnusedLabel", "java/labeled-statement"),
    ("UnusedLocalVariable", "java/unused-local-variable"),
    ("UnusedPrivateField", "java/unused-private-field"),
    ("UnusedPrivateMethod", "java/unused-private-method"),
    ("UnusedReturnValue", "java/ignored-return-value"),
    ("UseCollectionIsEmpty", "java/collection-size-comparison"),
    (
        "UseEqualsToCompareStrings",
        "java/string-reference-equality",
    ),
    (
        "UseLocaleWithCaseConversions",
        "java/case-conversion-without-locale",
    ),
    ("UseStandardCharsets", "java/standard-charset-lookup"),
    ("UselessOverridingMethod", "java/useless-override"),
    ("UselessParentheses", "java/redundant-parentheses"),
    ("UselessQualifiedThis", "java/redundant-qualified-this"),
    ("VariableCanBeInlined", "java/immediately-returned-variable"),
];

/// PMD rules outside the quickstart ruleset → the Mote Java rule that targets the same kind of
/// issue, sorted by name (the `pmd-migration` section of the category map); unlike
/// [`PMD_EQUIVALENTS`], the coverage benchmark does not run these rules. Used the same way: by
/// `mote migrate --from pmd` and by the `pmd` compat suppressions.
pub const PMD_MIGRATION_EQUIVALENTS: &[(&str, &str)] = &[
    ("AssertEqualsArgumentOrder", "java/assert-argument-order"),
    ("AvoidCallingFinalize", "java/explicit-finalize-call"),
    ("AvoidCatchingNPE", "java/catch-programming-error"),
    ("AvoidCatchingThrowable", "java/catch-throwable-or-error"),
    ("AvoidDuplicateLiterals", "java/duplicate-string-literal"),
    ("AvoidPrintStackTrace", "java/print-stack-trace"),
    ("AvoidReassigningParameters", "java/parameter-reassignment"),
    ("AvoidRethrowingException", "java/useless-catch"),
    (
        "AvoidThrowingRawExceptionTypes",
        "java/generic-exception-thrown",
    ),
    ("CStyleArrayDeclaration", "java/array-brackets-on-name"),
    ("CheckSkipResult", "java/ignored-failure-return"),
    ("CognitiveComplexity", "java/cognitive-complexity"),
    ("CollapsibleIfStatements", "java/collapsible-if"),
    (
        "ConstructorCallsOverridableMethod",
        "java/constructor-calls-overridable",
    ),
    ("CyclomaticComplexity", "java/cyclomatic-complexity"),
    ("DoNotTerminateVM", "java/system-exit"),
    ("DoNotThrowExceptionInFinally", "java/return-in-finally"),
    ("ExcessiveParameterList", "java/too-many-parameters"),
    ("IdenticalConditionalBranches", "java/identical-branches"),
    (
        "LongLiteralEndingWithLowercaseL",
        "java/long-literal-lowercase",
    ),
    ("OnDemandImport", "java/wildcard-import"),
    (
        "SignatureDeclareThrowsException",
        "java/throws-generic-exception",
    ),
    (
        "SimplifyBooleanExpressions",
        "java/boolean-literal-comparison",
    ),
    (
        "StringBufferInstantiationWithChar",
        "java/string-builder-char-constructor",
    ),
    ("StringInstantiation", "java/boxed-primitive-constructor"),
    ("StringToString", "java/redundant-tostring"),
    ("SystemPrintln", "java/system-out-print"),
    ("TooManyFields", "java/too-many-fields"),
    ("TooManyMethods", "java/too-many-methods"),
    ("UnitTestContainsTooManyAsserts", "java/too-many-assertions"),
    ("UnitTestShouldIncludeAssert", "java/test-without-assertion"),
    (
        "UnnecessaryInterfaceDeclaration",
        "java/redundant-inheritance-declaration",
    ),
    (
        "UnnecessaryVarargsArrayCreation",
        "java/explicit-varargs-array",
    ),
    ("UnsupportedJdkApiUsage", "java/internal-jdk-api"),
    (
        "UseStringBufferForStringAppends",
        "java/string-concat-in-loop",
    ),
];

/// Names of older PMD releases (and rules PMD deprecated in favor of another) → the current name
/// they were merged into or renamed to, sorted.
pub const PMD_RENAMED: &[(&str, &str)] = &[
    (
        "AvoidProtectedFieldInFinalClass",
        "ProtectedMemberInFinalClass",
    ),
    (
        "AvoidProtectedMethodInFinalClassNotExtending",
        "ProtectedMemberInFinalClass",
    ),
    ("BooleanInstantiation", "PrimitiveWrapperInstantiation"),
    ("ByteInstantiation", "PrimitiveWrapperInstantiation"),
    ("DontImportJavaLang", "UnnecessaryImport"),
    ("DontImportSun", "UnsupportedJdkApiUsage"),
    ("DuplicateImports", "UnnecessaryImport"),
    ("EmptyFinallyBlock", "EmptyControlStatement"),
    ("EmptyIfStmt", "EmptyControlStatement"),
    ("EmptyInitializer", "EmptyControlStatement"),
    ("EmptyStatementBlock", "EmptyControlStatement"),
    ("EmptyStatementNotInLoop", "UnnecessarySemicolon"),
    ("EmptySwitchStatements", "EmptyControlStatement"),
    ("EmptySynchronizedBlock", "EmptyControlStatement"),
    ("EmptyTryBlock", "EmptyControlStatement"),
    ("EmptyWhileStmt", "EmptyControlStatement"),
    ("GenericsNaming", "TypeParameterNamingConventions"),
    ("GuardDebugLogging", "GuardLogStatement"),
    ("ImportFromSamePackage", "UnnecessaryImport"),
    ("IntegerInstantiation", "PrimitiveWrapperInstantiation"),
    (
        "JUnitTestContainsTooManyAsserts",
        "UnitTestContainsTooManyAsserts",
    ),
    (
        "JUnitTestsShouldIncludeAssert",
        "UnitTestShouldIncludeAssert",
    ),
    ("LongInstantiation", "PrimitiveWrapperInstantiation"),
    ("MissingBreakInSwitch", "ImplicitSwitchFallThrough"),
    (
        "PositionLiteralsFirstInCaseInsensitiveComparisons",
        "LiteralsFirstInComparisons",
    ),
    (
        "PositionLiteralsFirstInComparisons",
        "LiteralsFirstInComparisons",
    ),
    ("ShortInstantiation", "PrimitiveWrapperInstantiation"),
    ("SwitchStmtsShouldHaveDefault", "NonExhaustiveSwitch"),
    ("UnnecessaryLocalBeforeReturn", "VariableCanBeInlined"),
    ("UnusedImports", "UnnecessaryImport"),
    ("UseUtilityClass", "InstantiableUtilityClass"),
    ("UselessOperationOnImmutable", "UnusedReturnValue"),
    ("UselessPureMethodCall", "UnusedReturnValue"),
];

fn lookup(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .binary_search_by(|(k, _)| k.cmp(&key))
        .ok()
        .map(|i| table[i].1)
}

/// The Mote rule for a current PMD rule name, from either table.
fn equivalent(name: &str) -> Option<&'static str> {
    lookup(PMD_EQUIVALENTS, name).or_else(|| lookup(PMD_MIGRATION_EQUIVALENTS, name))
}

/// The Mote rule for a PMD rule name (current or older), if there is one.
pub fn mote_rule_for_pmd(name: &str) -> Option<&'static str> {
    let name = name.trim();
    equivalent(name).or_else(|| lookup(PMD_RENAMED, name).and_then(equivalent))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::LanguagePlugin;

    #[test]
    fn tables_are_sorted_and_point_at_java_rules() {
        for t in [PMD_EQUIVALENTS, PMD_MIGRATION_EQUIVALENTS, PMD_RENAMED] {
            assert!(t.windows(2).all(|w| w[0].0 < w[1].0), "sorted, unique");
        }
        for (pmd, _) in PMD_MIGRATION_EQUIVALENTS {
            assert!(
                lookup(PMD_EQUIVALENTS, pmd).is_none() && lookup(PMD_RENAMED, pmd).is_none(),
                "{pmd}: in two tables"
            );
        }
        let ids: Vec<&str> = crate::JavaPlugin::new()
            .rules()
            .iter()
            .map(|m| m.id)
            .collect();
        // The engine's generic rules (complexity and size limits) exist for every language; the
        // CLI checks the tables against the whole registry.
        let generic = [
            "java/cognitive-complexity",
            "java/cyclomatic-complexity",
            "java/too-many-parameters",
        ];
        for (pmd, id) in PMD_EQUIVALENTS.iter().chain(PMD_MIGRATION_EQUIVALENTS) {
            assert!(
                ids.contains(id) || generic.contains(id),
                "{pmd} → {id}: no such Java rule"
            );
        }
        for (old, new) in PMD_RENAMED {
            assert!(
                equivalent(new).is_some(),
                "{old} → {new}: in neither table of equivalents"
            );
        }
        assert_eq!(
            mote_rule_for_pmd("EmptyCatchBlock"),
            Some("java/empty-catch-block")
        );
        assert_eq!(mote_rule_for_pmd("EmptyIfStmt"), Some("java/empty-block"));
        assert_eq!(
            mote_rule_for_pmd("SystemPrintln"),
            Some("java/system-out-print")
        );
        assert_eq!(
            mote_rule_for_pmd("JUnitTestContainsTooManyAsserts"),
            Some("java/too-many-assertions")
        );
        assert_eq!(mote_rule_for_pmd("NoSuchRule"), None);
    }
}
