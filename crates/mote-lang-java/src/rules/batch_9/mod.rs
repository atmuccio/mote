//! Rule batch 9: regular-expression pitfalls and rare Java bugs (varargs, boxing, threads,
//! synchronization, finalization). The author of this batch adds rule modules in this directory and
//! lists them in [`rules`]; nothing outside `src/rules/batch_9/` (except fixtures) needs to change.
//! See `RULE_AUTHORING.md`.

use crate::rule::BoxedRule;

// The regex parser, the regex call-site helper and this batch's helpers (`util`) live in
// `rules/shared`, which other batches use too.
use super::shared::{regex_call, regex_syntax, util};

mod regex_checks;

mod ambiguous_varargs_argument;
mod boxing_roundtrip;
mod constructor_reference_capacity;
mod deprecated_annotation_missing;
mod explicit_finalize_call;
mod int_to_long_bits_to_double;
mod negated_comparison;
mod override_drops_synchronized;
mod regex_case_insensitive_non_ascii;
mod regex_control_escape;
mod regex_empty_alternative;
mod regex_lazy_quantifier_at_end;
mod regex_never_matches;
mod regex_shadowed_alternative;
mod regex_stack_overflow_risk;
mod regex_suspicious_character_class;
mod static_init_uses_subclass;
mod string_call_with_known_result;
mod thread_subclass_without_run;
mod virtual_thread_platform_setting;
mod volatile_mutable_contents;

/// The rules of batch 9, in a stable order.
pub fn rules<'t>() -> Vec<BoxedRule<'t>> {
    vec![
        // Regular expressions
        Box::new(regex_never_matches::RegexNeverMatches),
        Box::new(regex_empty_alternative::RegexEmptyAlternative),
        Box::new(regex_shadowed_alternative::RegexShadowedAlternative),
        Box::new(regex_stack_overflow_risk::RegexStackOverflowRisk),
        Box::new(regex_case_insensitive_non_ascii::RegexCaseInsensitiveNonAscii),
        Box::new(regex_suspicious_character_class::RegexSuspiciousCharacterClass),
        Box::new(regex_control_escape::RegexControlEscape),
        Box::new(regex_lazy_quantifier_at_end::RegexLazyQuantifierAtEnd),
        // API misuse
        Box::new(string_call_with_known_result::StringCallWithKnownResult),
        Box::new(constructor_reference_capacity::ConstructorReferenceCapacity),
        Box::new(explicit_finalize_call::ExplicitFinalizeCall),
        Box::new(int_to_long_bits_to_double::IntToLongBitsToDouble),
        Box::new(boxing_roundtrip::BoxingRoundtrip),
        Box::new(ambiguous_varargs_argument::AmbiguousVarargsArgument::default()),
        Box::new(static_init_uses_subclass::StaticInitUsesSubclass::default()),
        // Concurrency
        Box::new(thread_subclass_without_run::ThreadSubclassWithoutRun::default()),
        Box::new(override_drops_synchronized::OverrideDropsSynchronized::default()),
        Box::new(virtual_thread_platform_setting::VirtualThreadPlatformSetting::default()),
        Box::new(volatile_mutable_contents::VolatileMutableContents::default()),
        // Documentation and readability
        Box::new(deprecated_annotation_missing::DeprecatedAnnotationMissing::default()),
        Box::new(negated_comparison::NegatedComparison),
    ]
}
