//! IntelliJ IDEA inspection names → the Mote Java rules that target the same issue.
//!
//! Used by the compatibility suppressions of the `intellij` source: `//noinspection Name`
//! comments and `@SuppressWarnings("Name")` with `suppressions.compat`. The names are the
//! inspections' suppression ids, including the javac- and Eclipse-style ones IntelliJ accepts too
//! (`unused`, `rawtypes`, `fallthrough`, `finally`, `resource`) and the names of the old data-flow
//! inspection and its successors (`ConstantConditions`, `ConstantValue`, `DataFlowIssue`); `ALL`
//! (every inspection) is handled by the caller and stands for every rule listed here. Only
//! inspections Mote covers are listed; a name without an equivalent suppresses nothing.
//! IntelliJ IDEA Community is Apache-2.0 licensed; only inspection names are used here.

/// Inspection name → Mote rule ids, sorted by name (byte order: upper case first).
pub const INTELLIJ_EQUIVALENTS: &[(&str, &[&str])] = &[
    (
        "AccessStaticViaInstance",
        &["java/static-access-through-instance"],
    ),
    ("ArrayHashCode", &["java/array-hash-code"]),
    ("AssertWithSideEffects", &["java/assert-with-side-effect"]),
    (
        "AssignmentToForLoopParameter",
        &["java/for-loop-counter-misuse"],
    ),
    (
        "AssignmentToMethodParameter",
        &["java/parameter-reassignment"],
    ),
    (
        "AssignmentUsedAsCondition",
        &["java/assignment-in-condition"],
    ),
    (
        "AutoCloseableResource",
        &["java/resource-not-closed-with-try"],
    ),
    (
        "BooleanConstructorCall",
        &["java/boxed-primitive-constructor"],
    ),
    ("CStyleArrayDeclaration", &["java/array-brackets-on-name"]),
    (
        "CachedNumberConstructorCall",
        &["java/boxed-primitive-constructor"],
    ),
    ("CallToPrintStackTrace", &["java/print-stack-trace"]),
    ("CallToSystemExit", &["java/system-exit"]),
    ("CallToSystemGC", &["java/explicit-gc"]),
    ("CallToThreadRun", &["java/thread-run-direct"]),
    ("CatchGenericClass", &["java/catch-generic-exception"]),
    ("CatchMayIgnoreException", &["java/empty-catch-block"]),
    ("CharsetObjectCanBeUsed", &["java/standard-charset-lookup"]),
    (
        "ClassExplicitlyExtendsObject",
        &["java/redundant-inheritance-declaration"],
    ),
    ("ClassNamingConvention", &["java/class-naming"]),
    (
        "ClassWithOnlyPrivateConstructors",
        &["java/private-constructor-class-not-final"],
    ),
    (
        "CollectionAddedToSelf",
        &["java/collection-passed-to-itself"],
    ),
    ("CommentedOutCode", &["java/commented-out-code"]),
    (
        "ComparableImplementedButEqualsNotOverridden",
        &["java/compare-to-without-equals"],
    ),
    ("ComparisonToNaN", &["java/float-equality"]),
    (
        "ConditionalExpressionWithIdenticalBranches",
        &["java/identical-branches"],
    ),
    ("ConstantConditions", &["java/null-check-after-dereference"]),
    ("ConstantNamingConvention", &["java/constant-naming"]),
    ("ConstantValue", &["java/null-check-after-dereference"]),
    (
        "ContinueOrBreakFromFinallyBlock",
        &["java/return-in-finally"],
    ),
    (
        "ControlFlowStatementWithoutBraces",
        &["java/missing-braces"],
    ),
    ("Convert2Lambda", &["java/anonymous-class-could-be-lambda"]),
    (
        "Convert2MethodRef",
        &["java/lambda-could-be-method-reference"],
    ),
    ("CovariantEquals", &["java/equals-overload"]),
    ("DanglingJavadoc", &["java/dangling-doc-comment"]),
    ("DataFlowIssue", &["java/null-check-after-dereference"]),
    ("DoubleCheckedLocking", &["java/double-checked-locking"]),
    ("DuplicateBranchesInSwitch", &["java/duplicate-branch-body"]),
    ("DuplicateCondition", &["java/duplicate-condition"]),
    ("DuplicatedCode", &["java/duplicated-block"]),
    ("EmptyCatchBlock", &["java/empty-catch-block"]),
    ("EmptyFinallyBlock", &["java/empty-block"]),
    ("EmptyMethod", &["java/empty-method"]),
    (
        "EmptyStatementBody",
        &["java/empty-block", "java/empty-statement"],
    ),
    ("EmptySynchronizedStatement", &["java/empty-block"]),
    ("EmptyTryBlock", &["java/empty-block"]),
    ("EqualsAndHashcode", &["java/equals-without-hashcode"]),
    (
        "EqualsBetweenInconvertibleTypes",
        &["java/equals-unrelated-types"],
    ),
    (
        "EqualsWhichDoesntCheckParameterClass",
        &["java/equals-without-type-check"],
    ),
    (
        "ExceptionFromCatchWhichDoesntWrap",
        &["java/lost-exception-cause"],
    ),
    (
        "ExceptionNameDoesntEndWithException",
        &["java/exception-naming"],
    ),
    ("FallthruInSwitchStatement", &["java/switch-fallthrough"]),
    ("FieldCanBeLocal", &["java/field-could-be-local"]),
    ("FieldMayBeStatic", &["java/final-field-could-be-static"]),
    ("FinalizeCalledExplicitly", &["java/explicit-finalize-call"]),
    (
        "FinallyBlockCannotCompleteNormally",
        &["java/return-in-finally"],
    ),
    ("FloatingPointEquality", &["java/float-equality"]),
    (
        "ForLoopReplaceableByForEach",
        &["java/index-loop-could-be-foreach"],
    ),
    (
        "IfStatementWithIdenticalBranches",
        &["java/duplicate-branch-body"],
    ),
    ("ImplicitArrayToString", &["java/array-to-string"]),
    ("ImplicitDefaultCharsetUsage", &["java/default-charset"]),
    (
        "InnerClassMayBeStatic",
        &["java/inner-class-could-be-static"],
    ),
    ("InstanceVariableNamingConvention", &["java/field-naming"]),
    ("InstanceofCatchParameter", &["java/instanceof-in-catch"]),
    (
        "IntegerDivisionInFloatingPointContext",
        &["java/integer-division-in-floating-context"],
    ),
    (
        "IntegerMultiplicationImplicitCastToLong",
        &["java/int-multiplication-overflow"],
    ),
    (
        "IteratorHasNextCallsIteratorNext",
        &["java/has-next-calls-next"],
    ),
    (
        "JUnitTestClassNamingConvention",
        &["java/test-class-naming"],
    ),
    (
        "JUnitTestMethodWithNoAssertions",
        &["java/test-without-assertion"],
    ),
    (
        "KeySetIterationMayUseEntrySet",
        &["java/keyset-lookup-in-loop"],
    ),
    ("LabeledStatement", &["java/labeled-statement"]),
    ("ListRemoveInLoop", &["java/list-remove-in-index-loop"]),
    (
        "LocalVariableNamingConvention",
        &["java/local-variable-naming"],
    ),
    (
        "LoggerInitializedWithForeignClass",
        &["java/logger-for-wrong-class"],
    ),
    (
        "LongLiteralsEndingWithLowercaseL",
        &["java/long-literal-lowercase"],
    ),
    (
        "LoopStatementsThatDontLoop",
        &["java/loop-exits-first-pass"],
    ),
    ("MagicNumber", &["java/magic-number"]),
    ("MalformedFormatString", &["java/format-argument-mismatch"]),
    ("MalformedRegex", &["java/invalid-regex-literal"]),
    ("ManualArrayCopy", &["java/manual-array-copy"]),
    ("MathRandomCastToInt", &["java/random-cast-to-int"]),
    ("MethodNameSameAsClassName", &["java/confusing-member-name"]),
    ("MethodNamingConvention", &["java/method-naming"]),
    (
        "MisorderedAssertEqualsArguments",
        &["java/assert-argument-order"],
    ),
    (
        "MissingDeprecatedAnnotation",
        &["java/deprecated-annotation-missing"],
    ),
    ("MissingOverrideAnnotation", &["java/missing-override"]),
    ("MissortedModifiers", &["java/modifier-order"]),
    (
        "MultipleTopLevelClassesInFile",
        &["java/multiple-top-level-types"],
    ),
    (
        "MultipleVariablesInDeclaration",
        &["java/one-variable-per-declaration"],
    ),
    ("NestedAssignment", &["java/assignment-in-operand"]),
    ("NestedConditionalExpression", &["java/nested-ternary"]),
    ("NestedTryStatement", &["java/nested-try"]),
    (
        "NewStringBufferWithCharArgument",
        &["java/string-builder-char-constructor"],
    ),
    (
        "NonAtomicOperationOnVolatileField",
        &["java/non-atomic-volatile-update"],
    ),
    (
        "NonShortCircuitBooleanExpression",
        &["java/non-short-circuit-boolean"],
    ),
    ("NumberEquality", &["java/value-reference-equality"]),
    ("NumericOverflow", &["java/int-constant-overflow"]),
    ("ObjectEquality", &["java/value-reference-equality"]),
    ("ObjectNotify", &["java/notify-instead-of-notify-all"]),
    ("OctalLiteral", &["java/octal-literal"]),
    ("OnDemandImport", &["java/wildcard-import"]),
    ("OptionalAssignedToNull", &["java/optional-null"]),
    (
        "OptionalGetWithoutIsPresent",
        &["java/optional-get-without-check"],
    ),
    ("OverlyComplexMethod", &["java/cyclomatic-complexity"]),
    ("OverlyLongMethod", &["java/function-too-long"]),
    ("OverlyNestedMethod", &["java/deep-nesting"]),
    (
        "OverridableMethodCallDuringObjectConstruction",
        &["java/constructor-calls-overridable"],
    ),
    ("PackageNamingConvention", &["java/package-naming"]),
    ("ParameterNamingConvention", &["java/local-variable-naming"]),
    ("PointlessArithmeticExpression", &["java/arithmetic-no-op"]),
    (
        "PointlessBooleanExpression",
        &["java/boolean-literal-comparison"],
    ),
    (
        "ProhibitedExceptionCaught",
        &[
            "java/catch-generic-exception",
            "java/catch-throwable-or-error",
        ],
    ),
    (
        "ProhibitedExceptionDeclared",
        &["java/throws-generic-exception"],
    ),
    (
        "ProhibitedExceptionThrown",
        &["java/generic-exception-thrown"],
    ),
    (
        "ProtectedMemberInFinalClass",
        &["java/protected-in-final-class"],
    ),
    ("PublicField", &["java/public-instance-field"]),
    (
        "PublicStaticArrayField",
        &["java/public-static-mutable-field"],
    ),
    (
        "PublicStaticCollectionField",
        &["java/public-static-mutable-field"],
    ),
    ("RawUseOfParameterizedType", &["java/raw-type"]),
    ("RedundantArrayCreation", &["java/explicit-varargs-array"]),
    ("RedundantCast", &["java/redundant-cast"]),
    (
        "RedundantIfStatement",
        &["java/simplifiable-boolean-return"],
    ),
    (
        "RedundantNoArgConstructor",
        &["java/redundant-default-constructor"],
    ),
    ("ReplaceAllDot", &["java/separator-used-as-regex"]),
    ("ResultOfMethodCallIgnored", &["java/ignored-return-value"]),
    ("ResultSetIndexZero", &["java/jdbc-zero-index"]),
    (
        "ReturnNull",
        &["java/collection-return-null", "java/optional-return-null"],
    ),
    ("ShiftOutOfRange", &["java/shift-out-of-range"]),
    ("SillyAssignment", &["java/self-assignment"]),
    ("SimplifiableAssertion", &["java/assert-boolean-literal"]),
    (
        "SimplifiableConditionalExpression",
        &["java/simplifiable-ternary"],
    ),
    (
        "SimplifiableIfStatement",
        &[
            "java/simplifiable-boolean-if-else",
            "java/simplifiable-boolean-return",
        ],
    ),
    (
        "SimplifiableJUnitAssertion",
        &["java/assert-boolean-literal"],
    ),
    ("SimplifyStreamApiCallChains", &["java/simplifiable-stream"]),
    (
        "SizeReplaceableByIsEmpty",
        &["java/collection-size-comparison"],
    ),
    ("SleepWhileHoldingLock", &["java/sleep-in-synchronized"]),
    (
        "StaticInitializerReferencesSubClass",
        &["java/static-init-uses-subclass"],
    ),
    ("StaticVariableNamingConvention", &["java/field-naming"]),
    ("StringBufferField", &["java/string-builder-field"]),
    (
        "StringConcatenationArgumentToLogCall",
        &["java/log-message-concatenation"],
    ),
    ("StringConcatenationInLoop", &["java/string-concat-in-loop"]),
    ("StringEquality", &["java/string-reference-equality"]),
    (
        "StringToUpperCaseOrToLowerCaseWithoutLocale",
        &["java/case-conversion-without-locale"],
    ),
    ("SuspiciousDateFormat", &["java/week-year-pattern"]),
    (
        "SuspiciousMethodCalls",
        &["java/incompatible-collection-lookup"],
    ),
    (
        "SwitchStatementWithoutDefaultBranch",
        &["java/missing-switch-default"],
    ),
    (
        "SynchronizationOnGetClass",
        &["java/synchronized-on-get-class"],
    ),
    ("SynchronizeOnLock", &["java/intrinsic-lock-on-lock-object"]),
    (
        "SynchronizeOnNonFinalField",
        &["java/synchronized-on-reassigned-field"],
    ),
    (
        "SynchronizeOnValueBasedClass",
        &["java/synchronized-on-value"],
    ),
    (
        "TestMethodWithoutAssertion",
        &["java/test-without-assertion"],
    ),
    (
        "ThisEscapedInObjectConstruction",
        &["java/this-escape-in-constructor"],
    ),
    ("ThrowFromFinallyBlock", &["java/return-in-finally"]),
    ("ThrowableNotThrown", &["java/exception-not-thrown"]),
    ("TodoComment", &["java/fixme-comment", "java/todo-comment"]),
    (
        "TryFinallyCanBeTryWithResources",
        &["java/prefer-try-with-resources"],
    ),
    (
        "TypeParameterNamingConvention",
        &["java/type-parameter-naming"],
    ),
    ("UNUSED_IMPORT", &["java/unused-import"]),
    (
        "UnnecessaryFullyQualifiedName",
        &["java/unnecessary-fully-qualified-name"],
    ),
    (
        "UnnecessaryLocalVariable",
        &["java/immediately-returned-variable"],
    ),
    ("UnnecessaryParentheses", &["java/redundant-parentheses"]),
    ("UnnecessaryReturnStatement", &["java/unnecessary-return"]),
    ("UnnecessarySemicolon", &["java/empty-statement"]),
    (
        "UnpredictableBigDecimalConstructorCall",
        &["java/bigdecimal-from-double"],
    ),
    (
        "UnusedAssignment",
        &[
            "java/overwritten-before-read",
            "java/parameter-overwritten-before-read",
        ],
    ),
    ("UnusedImport", &["java/unused-import"]),
    ("UnusedLabel", &["java/labeled-statement"]),
    ("UseOfSystemOutOrSystemErr", &["java/system-out-print"]),
    (
        "UtilityClassWithPublicConstructor",
        &["java/utility-class-constructor"],
    ),
    (
        "UtilityClassWithoutPrivateConstructor",
        &["java/utility-class-constructor"],
    ),
    ("VolatileArrayField", &["java/volatile-mutable-contents"]),
    ("WaitNotInLoop", &["java/wait-without-loop"]),
    (
        "WaitNotifyNotInSynchronizedContext",
        &["java/wait-notify-outside-synchronized"],
    ),
    ("WaitWhileHoldingTwoLocks", &["java/wait-with-nested-lock"]),
    ("fallthrough", &["java/switch-fallthrough"]),
    ("finally", &["java/return-in-finally"]),
    ("rawtypes", &["java/raw-type"]),
    ("resource", &["java/resource-not-closed-with-try"]),
    (
        "unused",
        &[
            "java/unused-local-variable",
            "java/unused-private-class",
            "java/unused-private-field",
            "java/unused-private-method",
            "java/unused-private-method-parameter",
        ],
    ),
];

/// IntelliJ's name for every inspection (`//noinspection ALL`, `@SuppressWarnings("ALL")`),
/// compared case-sensitively (`"all"` is javac's and Eclipse's, another source).
pub const ALL_INSPECTIONS: &str = "ALL";

/// Every Mote rule with an IntelliJ equivalent, sorted and deduplicated: what
/// [`ALL_INSPECTIONS`] stands for.
pub fn all_inspection_rules() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = INTELLIJ_EQUIVALENTS
        .iter()
        .flat_map(|(_, ids)| ids.iter().copied())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The Mote rules an IntelliJ inspection name stands for (empty without an equivalent).
pub fn mote_rules_for_inspection(name: &str) -> &'static [&'static str] {
    let name = name.trim();
    INTELLIJ_EQUIVALENTS
        .binary_search_by(|(k, _)| k.cmp(&name))
        .map_or(&[], |i| INTELLIJ_EQUIVALENTS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        assert_eq!(
            mote_rules_for_inspection("OptionalAssignedToNull"),
            ["java/optional-null"]
        );
        assert_eq!(mote_rules_for_inspection(" unused ").len(), 5);
        assert!(mote_rules_for_inspection("SpellCheckingInspection").is_empty());
        assert!(mote_rules_for_inspection("optionalassignedtonull").is_empty());
        assert!(mote_rules_for_inspection(ALL_INSPECTIONS).is_empty());
    }
}
