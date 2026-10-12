//! SpotBugs bug pattern names → the Mote Java rules that target the same issue.
//!
//! Used by the compatibility suppressions of the `spotbugs` source (`@SuppressFBWarnings` with
//! `suppressions.compat`). Only patterns Mote covers are listed; a name without an
//! equivalent suppresses nothing. Only the pattern names (identifiers) are used here.

/// Bug pattern name → Mote rule ids, sorted by name.
pub const SPOTBUGS_EQUIVALENTS: &[(&str, &[&str])] = &[
    (
        "BC_EQUALS_METHOD_SHOULD_WORK_FOR_ALL_OBJECTS",
        &["java/equals-without-type-check"],
    ),
    ("BIT_ADD_OF_SIGNED_BYTE", &["java/byte-sign-extension"]),
    ("BIT_IOR_OF_SIGNED_BYTE", &["java/byte-sign-extension"]),
    (
        "BX_UNBOXED_AND_COERCED_FOR_TERNARY_OPERATOR",
        &["java/conditional-mixed-boxed-numbers"],
    ),
    (
        "BX_UNBOXING_IMMEDIATELY_REBOXED",
        &["java/boxing-roundtrip"],
    ),
    ("DB_DUPLICATE_BRANCHES", &["java/duplicate-branch-body"]),
    (
        "DB_DUPLICATE_SWITCH_CLAUSES",
        &["java/duplicate-branch-body"],
    ),
    (
        "DCN_NULLPOINTER_EXCEPTION",
        &["java/catch-programming-error"],
    ),
    ("DC_DOUBLECHECK", &["java/double-checked-locking"]),
    (
        "DC_PARTIALLY_CONSTRUCTED",
        &["java/double-checked-publish-early"],
    ),
    ("DE_MIGHT_IGNORE", &["java/empty-catch-block"]),
    (
        "DLS_DEAD_LOCAL_STORE",
        &["java/overwritten-before-read", "java/unused-local-variable"],
    ),
    (
        "DLS_OVERWRITTEN_INCREMENT",
        &["java/post-increment-self-assignment"],
    ),
    (
        "DL_SYNCHRONIZATION_ON_BOOLEAN",
        &["java/synchronized-on-value"],
    ),
    (
        "DL_SYNCHRONIZATION_ON_BOXED_PRIMITIVE",
        &["java/synchronized-on-value"],
    ),
    (
        "DL_SYNCHRONIZATION_ON_SHARED_CONSTANT",
        &["java/synchronized-on-value"],
    ),
    (
        "DL_SYNCHRONIZATION_ON_UNSHARED_BOXED_PRIMITIVE",
        &["java/synchronized-on-value"],
    ),
    (
        "DMI_ANNOTATION_IS_NOT_VISIBLE_TO_REFLECTION",
        &["java/unretained-annotation-lookup"],
    ),
    ("DMI_BAD_MONTH", &["java/date-field-out-of-range"]),
    (
        "DMI_BIGDECIMAL_CONSTRUCTED_FROM_DOUBLE",
        &["java/bigdecimal-from-double"],
    ),
    (
        "DMI_BLOCKING_METHODS_ON_URL",
        &["java/url-in-hash-collection"],
    ),
    (
        "DMI_CALLING_NEXT_FROM_HASNEXT",
        &["java/has-next-calls-next"],
    ),
    ("DMI_COLLECTION_OF_URLS", &["java/url-in-hash-collection"]),
    (
        "DMI_HARDCODED_ABSOLUTE_FILENAME",
        &["java/hard-coded-absolute-path"],
    ),
    ("DMI_INVOKING_HASHCODE_ON_ARRAY", &["java/array-hash-code"]),
    (
        "DMI_INVOKING_TOSTRING_ON_ANONYMOUS_ARRAY",
        &["java/array-to-string"],
    ),
    ("DMI_INVOKING_TOSTRING_ON_ARRAY", &["java/array-to-string"]),
    (
        "DMI_LONG_BITS_TO_DOUBLE_INVOKED_ON_INT",
        &["java/int-to-long-bits-to-double"],
    ),
    (
        "DMI_SCHEDULED_THREAD_POOL_EXECUTOR_WITH_ZERO_CORE_THREADS",
        &["java/scheduled-pool-without-core-threads"],
    ),
    (
        "DMI_THREAD_PASSED_WHERE_RUNNABLE_EXPECTED",
        &["java/thread-passed-as-runnable"],
    ),
    (
        "DMI_USELESS_SUBSTRING",
        &["java/string-call-with-known-result"],
    ),
    (
        "DMI_USING_REMOVEALL_TO_CLEAR_COLLECTION",
        &["java/collection-passed-to-itself"],
    ),
    (
        "DMI_VACUOUS_SELF_COLLECTION_CALL",
        &["java/collection-passed-to-itself"],
    ),
    ("DM_BOOLEAN_CTOR", &["java/boxed-primitive-constructor"]),
    ("DM_CONVERT_CASE", &["java/case-conversion-without-locale"]),
    ("DM_DEFAULT_ENCODING", &["java/default-charset"]),
    ("DM_EXIT", &["java/system-exit"]),
    ("DM_FP_NUMBER_CTOR", &["java/boxed-primitive-constructor"]),
    ("DM_GC", &["java/explicit-gc"]),
    ("DM_NUMBER_CTOR", &["java/boxed-primitive-constructor"]),
    ("DM_STRING_TOSTRING", &["java/redundant-tostring"]),
    ("EC_NULL_ARG", &["java/equals-with-null"]),
    (
        "EC_UNRELATED_CLASS_AND_INTERFACE",
        &["java/equals-unrelated-types"],
    ),
    ("EC_UNRELATED_INTERFACES", &["java/equals-unrelated-types"]),
    ("EC_UNRELATED_TYPES", &["java/equals-unrelated-types"]),
    ("EQ_ABSTRACT_SELF", &["java/equals-overload"]),
    (
        "EQ_COMPARETO_USE_OBJECT_EQUALS",
        &["java/compare-to-without-equals"],
    ),
    (
        "EQ_OVERRIDING_EQUALS_NOT_SYMMETRIC",
        &["java/equals-asymmetric-subclass"],
    ),
    ("EQ_SELF_NO_OBJECT", &["java/equals-overload"]),
    ("EQ_UNUSUAL", &["java/equals-without-type-check"]),
    (
        "ES_COMPARING_PARAMETER_STRING_WITH_EQ",
        &["java/string-reference-equality"],
    ),
    (
        "ES_COMPARING_STRINGS_WITH_EQ",
        &["java/string-reference-equality"],
    ),
    ("ESync_EMPTY_SYNC", &["java/empty-block"]),
    ("FE_FLOATING_POINT_EQUALITY", &["java/float-equality"]),
    ("FE_TEST_IF_EQUAL_TO_NOT_A_NUMBER", &["java/float-equality"]),
    ("FI_EMPTY", &["java/finalize-override"]),
    ("FI_EXPLICIT_INVOCATION", &["java/explicit-finalize-call"]),
    ("FI_USELESS", &["java/finalize-override"]),
    ("FL_FLOATS_AS_LOOP_COUNTERS", &["java/float-loop-counter"]),
    (
        "GC_UNRELATED_TYPES",
        &["java/incompatible-collection-lookup"],
    ),
    ("HE_EQUALS_NO_HASHCODE", &["java/equals-without-hashcode"]),
    ("HE_EQUALS_USE_HASHCODE", &["java/equals-without-hashcode"]),
    ("HE_HASHCODE_NO_EQUALS", &["java/equals-without-hashcode"]),
    (
        "HE_HASHCODE_USE_OBJECT_EQUALS",
        &["java/equals-without-hashcode"],
    ),
    ("ICAST_BAD_SHIFT_AMOUNT", &["java/shift-out-of-range"]),
    (
        "ICAST_IDIV_CAST_TO_DOUBLE",
        &["java/integer-division-in-floating-context"],
    ),
    (
        "ICAST_INTEGER_MULTIPLY_CAST_TO_LONG",
        &["java/int-multiplication-overflow"],
    ),
    (
        "IC_SUPERCLASS_USES_SUBCLASS_DURING_INITIALIZATION",
        &["java/static-init-uses-subclass"],
    ),
    (
        "IJU_ASSERT_METHOD_INVOKED_FROM_RUN_METHOD",
        &["java/assertion-in-thread"],
    ),
    ("IMSE_DONT_CATCH_IMSE", &["java/catch-programming-error"]),
    (
        "INT_BAD_COMPARISON_WITH_NONNEGATIVE_VALUE",
        &["java/impossible-size-check"],
    ),
    (
        "IP_PARAMETER_IS_DEAD_BUT_OVERWRITTEN",
        &["java/parameter-overwritten-before-read"],
    ),
    ("IS2_INCONSISTENT_SYNC", &["java/unsynchronized-accessor"]),
    ("IT_NO_SUCH_ELEMENT", &["java/iterator-contract"]),
    (
        "JLM_JSR166_LOCK_MONITORENTER",
        &["java/intrinsic-lock-on-lock-object"],
    ),
    (
        "JML_JSR166_CALLING_WAIT_RATHER_THAN_AWAIT",
        &["java/intrinsic-lock-on-lock-object"],
    ),
    (
        "MC_OVERRIDABLE_METHOD_CALL_IN_CONSTRUCTOR",
        &["java/constructor-calls-overridable"],
    ),
    ("MF_CLASS_MASKS_FIELD", &["java/hidden-declaration"]),
    ("MF_METHOD_MASKS_FIELD", &["java/hidden-declaration"]),
    (
        "ML_SYNC_ON_FIELD_TO_GUARD_CHANGING_THAT_FIELD",
        &["java/synchronized-on-reassigned-field"],
    ),
    (
        "ML_SYNC_ON_UPDATED_FIELD",
        &["java/synchronized-on-reassigned-field"],
    ),
    ("MS_MUTABLE_ARRAY", &["java/public-static-mutable-field"]),
    (
        "MS_MUTABLE_COLLECTION",
        &["java/public-static-mutable-field"],
    ),
    ("MS_SHOULD_BE_FINAL", &["java/public-static-mutable-field"]),
    (
        "MWN_MISMATCHED_NOTIFY",
        &[
            "java/wait-notify-outside-synchronized",
            "java/wait-notify-wrong-monitor",
        ],
    ),
    (
        "MWN_MISMATCHED_WAIT",
        &[
            "java/wait-notify-outside-synchronized",
            "java/wait-notify-wrong-monitor",
        ],
    ),
    ("NM_BAD_EQUAL", &["java/object-method-name-typo"]),
    ("NM_CLASS_NAMING_CONVENTION", &["java/class-naming"]),
    ("NM_CONFUSING", &["java/confusing-member-name"]),
    ("NM_FIELD_NAMING_CONVENTION", &["java/field-naming"]),
    (
        "NM_FUTURE_KEYWORD_USED_AS_IDENTIFIER",
        &["java/restricted-identifier"],
    ),
    (
        "NM_FUTURE_KEYWORD_USED_AS_MEMBER_IDENTIFIER",
        &["java/restricted-identifier"],
    ),
    ("NM_LCASE_HASHCODE", &["java/object-method-name-typo"]),
    ("NM_LCASE_TOSTRING", &["java/object-method-name-typo"]),
    (
        "NM_METHOD_CONSTRUCTOR_CONFUSION",
        &["java/confusing-member-name"],
    ),
    ("NM_METHOD_NAMING_CONVENTION", &["java/method-naming"]),
    ("NM_VERY_CONFUSING", &["java/confusing-member-name"]),
    ("NM_WRONG_PACKAGE", &["java/overload-instead-of-override"]),
    (
        "NM_WRONG_PACKAGE_INTENTIONAL",
        &["java/overload-instead-of-override"],
    ),
    (
        "NO_NOTIFY_NOT_NOTIFYALL",
        &["java/notify-instead-of-notify-all"],
    ),
    (
        "NP_CLONE_COULD_RETURN_NULL",
        &["java/tostring-or-clone-returns-null"],
    ),
    ("NP_OPTIONAL_RETURN_NULL", &["java/optional-return-null"]),
    (
        "NP_TOSTRING_COULD_RETURN_NULL",
        &["java/tostring-or-clone-returns-null"],
    ),
    (
        "OBL_UNSATISFIED_OBLIGATION",
        &["java/resource-not-closed-with-try"],
    ),
    (
        "OBL_UNSATISFIED_OBLIGATION_EXCEPTION_EDGE",
        &["java/resource-not-closed-with-try"],
    ),
    (
        "ODR_OPEN_DATABASE_RESOURCE",
        &["java/resource-not-closed-with-try"],
    ),
    (
        "ODR_OPEN_DATABASE_RESOURCE_EXCEPTION_PATH",
        &["java/resource-not-closed-with-try"],
    ),
    ("OS_OPEN_STREAM", &["java/resource-not-closed-with-try"]),
    (
        "OS_OPEN_STREAM_EXCEPTION_PATH",
        &["java/resource-not-closed-with-try"],
    ),
    ("PA_PUBLIC_ARRAY_ATTRIBUTE", &["java/public-instance-field"]),
    (
        "PA_PUBLIC_MUTABLE_OBJECT_ATTRIBUTE",
        &["java/public-instance-field"],
    ),
    (
        "PA_PUBLIC_PRIMITIVE_ATTRIBUTE",
        &["java/public-instance-field"],
    ),
    (
        "PZLA_PREFER_ZERO_LENGTH_ARRAYS",
        &["java/collection-return-null"],
    ),
    (
        "QF_QUESTIONABLE_FOR_LOOP",
        &["java/for-loop-counter-misuse"],
    ),
    (
        "RCN_REDUNDANT_NULLCHECK_WOULD_HAVE_BEEN_A_NPE",
        &["java/null-check-after-dereference"],
    ),
    ("RC_REF_COMPARISON", &["java/value-reference-equality"]),
    (
        "RC_REF_COMPARISON_BAD_PRACTICE",
        &["java/value-reference-equality"],
    ),
    (
        "RC_REF_COMPARISON_BAD_PRACTICE_BOOLEAN",
        &["java/value-reference-equality"],
    ),
    ("REC_CATCH_EXCEPTION", &["java/catch-generic-exception"]),
    (
        "RE_BAD_SYNTAX_FOR_REGULAR_EXPRESSION",
        &["java/invalid-regex-literal"],
    ),
    (
        "RE_CANT_USE_FILE_SEPARATOR_AS_REGULAR_EXPRESSION",
        &["java/separator-used-as-regex"],
    ),
    (
        "RE_POSSIBLE_UNINTENDED_PATTERN",
        &["java/separator-used-as-regex"],
    ),
    ("RU_INVOKE_RUN", &["java/thread-run-direct"]),
    ("RV_01_TO_INT", &["java/random-cast-to-int"]),
    (
        "RV_ABSOLUTE_VALUE_OF_HASHCODE",
        &["java/abs-of-hash-or-random"],
    ),
    (
        "RV_ABSOLUTE_VALUE_OF_RANDOM_INT",
        &["java/abs-of-hash-or-random"],
    ),
    (
        "RV_CHECK_FOR_POSITIVE_INDEXOF",
        &["java/index-of-positive-check"],
    ),
    ("RV_EXCEPTION_NOT_THROWN", &["java/exception-not-thrown"]),
    ("RV_RETURN_VALUE_IGNORED", &["java/ignored-return-value"]),
    (
        "RV_RETURN_VALUE_IGNORED_BAD_PRACTICE",
        &["java/ignored-failure-return"],
    ),
    (
        "RV_RETURN_VALUE_IGNORED_INFERRED",
        &["java/ignored-return-value"],
    ),
    (
        "RV_RETURN_VALUE_IGNORED_NO_SIDE_EFFECT",
        &["java/ignored-return-value"],
    ),
    ("SA_FIELD_DOUBLE_ASSIGNMENT", &["java/chained-assignment"]),
    ("SA_FIELD_SELF_ASSIGNMENT", &["java/self-assignment"]),
    ("SA_FIELD_SELF_COMPARISON", &["java/identical-operands"]),
    ("SA_FIELD_SELF_COMPUTATION", &["java/identical-operands"]),
    ("SA_LOCAL_DOUBLE_ASSIGNMENT", &["java/chained-assignment"]),
    ("SA_LOCAL_SELF_ASSIGNMENT", &["java/self-assignment"]),
    (
        "SA_LOCAL_SELF_ASSIGNMENT_INSTEAD_OF_FIELD",
        &["java/self-assignment"],
    ),
    ("SA_LOCAL_SELF_COMPARISON", &["java/identical-operands"]),
    ("SA_LOCAL_SELF_COMPUTATION", &["java/identical-operands"]),
    (
        "SBSC_USE_STRINGBUFFER_CONCATENATION",
        &["java/string-concat-in-loop"],
    ),
    ("SC_START_IN_CTOR", &["java/this-escape-in-constructor"]),
    (
        "SE_METHOD_MUST_BE_PRIVATE",
        &["java/serialization-hook-signature"],
    ),
    (
        "SE_NO_SUITABLE_CONSTRUCTOR",
        &["java/serialization-constructor-missing"],
    ),
    (
        "SE_NO_SUITABLE_CONSTRUCTOR_FOR_EXTERNALIZATION",
        &["java/serialization-constructor-missing"],
    ),
    (
        "SE_READ_RESOLVE_IS_STATIC",
        &["java/serialization-hook-signature"],
    ),
    (
        "SE_READ_RESOLVE_MUST_RETURN_OBJECT",
        &["java/serialization-hook-signature"],
    ),
    (
        "SF_DEAD_STORE_DUE_TO_SWITCH_FALLTHROUGH",
        &["java/switch-fallthrough"],
    ),
    ("SF_SWITCH_FALLTHROUGH", &["java/switch-fallthrough"]),
    ("SF_SWITCH_NO_DEFAULT", &["java/missing-switch-default"]),
    (
        "SIC_INNER_SHOULD_BE_STATIC",
        &["java/inner-class-could-be-static"],
    ),
    (
        "SQL_BAD_PREPARED_STATEMENT_ACCESS",
        &["java/jdbc-zero-index"],
    ),
    ("SQL_BAD_RESULTSET_ACCESS", &["java/jdbc-zero-index"]),
    (
        "SQL_PREPARED_STATEMENT_GENERATED_IN_LOOP",
        &["java/jdbc-statement-in-loop"],
    ),
    ("SS_SHOULD_BE_STATIC", &["java/final-field-could-be-static"]),
    (
        "STCAL_INVOKE_ON_STATIC_CALENDAR_INSTANCE",
        &["java/static-date-format"],
    ),
    (
        "STCAL_INVOKE_ON_STATIC_DATE_FORMAT_INSTANCE",
        &["java/static-date-format"],
    ),
    (
        "STCAL_STATIC_CALENDAR_INSTANCE",
        &["java/static-date-format"],
    ),
    (
        "STCAL_STATIC_SIMPLE_DATE_FORMAT_INSTANCE",
        &["java/static-date-format"],
    ),
    ("SWL_SLEEP_WITH_LOCK_HELD", &["java/sleep-in-synchronized"]),
    (
        "THROWS_METHOD_THROWS_CLAUSE_BASIC_EXCEPTION",
        &["java/throws-generic-exception"],
    ),
    (
        "THROWS_METHOD_THROWS_CLAUSE_THROWABLE",
        &["java/throws-generic-exception"],
    ),
    (
        "THROWS_METHOD_THROWS_RUNTIMEEXCEPTION",
        &["java/generic-exception-thrown"],
    ),
    ("TLW_TWO_LOCK_WAIT", &["java/wait-with-nested-lock"]),
    ("UCF_USELESS_CONTROL_FLOW", &["java/empty-block"]),
    (
        "UCF_USELESS_CONTROL_FLOW_NEXT_LINE",
        &["java/empty-statement"],
    ),
    (
        "UPM_UNCALLED_PRIVATE_METHOD",
        &["java/unused-private-method"],
    ),
    ("URF_UNREAD_FIELD", &["java/unused-private-field"]),
    ("UUF_UNUSED_FIELD", &["java/unused-private-field"]),
    (
        "VA_FORMAT_STRING_ARG_MISMATCH",
        &["java/format-argument-mismatch"],
    ),
    (
        "VA_FORMAT_STRING_BAD_ARGUMENT",
        &["java/format-argument-mismatch"],
    ),
    (
        "VA_FORMAT_STRING_BAD_CONVERSION",
        &["java/format-argument-mismatch"],
    ),
    (
        "VA_FORMAT_STRING_EXTRA_ARGUMENTS_PASSED",
        &["java/format-argument-mismatch"],
    ),
    (
        "VA_FORMAT_STRING_ILLEGAL",
        &["java/format-argument-mismatch"],
    ),
    (
        "VA_FORMAT_STRING_MISSING_ARGUMENT",
        &["java/format-argument-mismatch"],
    ),
    (
        "VA_FORMAT_STRING_NO_PREVIOUS_ARGUMENT",
        &["java/format-argument-mismatch"],
    ),
    (
        "VO_VOLATILE_INCREMENT",
        &["java/non-atomic-volatile-update"],
    ),
    (
        "VO_VOLATILE_REFERENCE_TO_ARRAY",
        &["java/volatile-mutable-contents"],
    ),
    ("WA_AWAIT_NOT_IN_LOOP", &["java/wait-without-loop"]),
    ("WA_NOT_IN_LOOP", &["java/wait-without-loop"]),
    (
        "WL_USING_GETCLASS_RATHER_THAN_CLASS_LITERAL",
        &["java/synchronized-on-get-class"],
    ),
    ("WMI_WRONG_MAP_ITERATOR", &["java/keyset-lookup-in-loop"]),
];

/// Every Mote rule with a SpotBugs equivalent, sorted and deduplicated: what a
/// `@SuppressFBWarnings` without a value (all of SpotBugs' warnings) stands for.
pub fn all_spotbugs_rules() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = SPOTBUGS_EQUIVALENTS
        .iter()
        .flat_map(|(_, ids)| ids.iter().copied())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The Mote rules a `@SuppressFBWarnings` value stands for, sorted and deduplicated. Like
/// SpotBugs, a value matches every pattern whose name starts with it (`DM_GC`, or `DM` for all
/// `DM…` patterns) and every pattern of the bug code it names in any case (`dm` for `DM_…`).
/// Category names (`CORRECTNESS`, `STYLE`, …) and patterns without an equivalent give nothing.
pub fn mote_rules_for_spotbugs(value: &str) -> Vec<&'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<&'static str> = SPOTBUGS_EQUIVALENTS
        .iter()
        .filter(|(name, _)| {
            name.starts_with(value)
                || name
                    .split('_')
                    .next()
                    .is_some_and(|code| code.eq_ignore_ascii_case(value))
        })
        .flat_map(|(_, ids)| ids.iter().copied())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_prefixes_and_bug_codes() {
        assert_eq!(mote_rules_for_spotbugs("DM_GC"), vec!["java/explicit-gc"]);
        assert_eq!(
            mote_rules_for_spotbugs(" ES_COMPARING_STRINGS_WITH_EQ "),
            vec!["java/string-reference-equality"]
        );
        // A prefix covers every pattern it starts.
        assert_eq!(
            mote_rules_for_spotbugs("SA_LOCAL_SELF"),
            vec!["java/identical-operands", "java/self-assignment"]
        );
        // A bug code, in any case.
        assert_eq!(
            mote_rules_for_spotbugs("np"),
            vec![
                "java/optional-return-null",
                "java/tostring-or-clone-returns-null"
            ]
        );
        for nothing in ["", "CORRECTNESS", "STYLE", "XYZ_NOT_A_PATTERN", "dm_gc"] {
            assert!(mote_rules_for_spotbugs(nothing).is_empty(), "{nothing}");
        }
    }
}

#[cfg(test)]
mod equivalence_tables {
    use mote_core::LanguagePlugin;

    /// Rules the engine provides for every language (not in the plugin's own list).
    const GENERIC: &[&str] = &[
        "java/cognitive-complexity",
        "java/commented-out-code",
        "java/cyclomatic-complexity",
        "java/deep-nesting",
        "java/duplicated-block",
        "java/file-too-long",
        "java/fixme-comment",
        "java/function-too-long",
        "java/todo-comment",
        "java/too-many-parameters",
    ];

    #[test]
    fn spotbugs_and_intellij_tables_are_sorted_and_point_at_java_rules() {
        let ids: Vec<&str> = crate::JavaPlugin::new()
            .rules()
            .iter()
            .map(|m| m.id)
            .chain(GENERIC.iter().copied())
            .collect();
        for (tool, table) in [
            ("spotbugs", crate::spotbugs::SPOTBUGS_EQUIVALENTS),
            ("intellij", crate::intellij::INTELLIJ_EQUIVALENTS),
        ] {
            for w in table.windows(2) {
                assert!(w[0].0 < w[1].0, "{tool}: {} before {}", w[0].0, w[1].0);
            }
            for (name, rules) in table {
                assert!(!rules.is_empty(), "{tool}: {name}");
                assert!(rules.windows(2).all(|w| w[0] < w[1]), "{tool}: {name}");
                for id in *rules {
                    assert!(ids.contains(id), "{tool}: {name} → {id}: no such Java rule");
                }
            }
        }
    }
}
