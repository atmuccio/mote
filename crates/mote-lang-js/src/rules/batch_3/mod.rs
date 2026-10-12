//! Rule batch 3 — TypeScript & React. Owned by one rule author; add rule modules here and list
//! them in [`rules`]. See `RULE_AUTHORING.md` at the crate root.

use crate::JsRule;

mod adjacent_overload_signatures;
mod ban_ts_comment;
mod jsx_key;
mod jsx_no_duplicate_props;
mod no_array_delete;
mod no_confusing_non_null_assertion;
mod no_duplicate_enum_values;
mod no_empty_interface;
mod no_extra_non_null_assertion;
mod no_inferrable_types;
mod no_misused_new;
mod no_mixed_enums;
mod no_namespace;
mod no_non_null_assertion;
mod no_require_imports;
mod no_this_alias;
mod no_unnecessary_type_constraint;
mod no_useless_empty_export;
mod no_wrapper_object_types;
mod prefer_as_const;
mod rules_of_hooks;
mod util;

/// Every rule of this batch, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(no_non_null_assertion::NoNonNullAssertion),
        Box::new(no_extra_non_null_assertion::NoExtraNonNullAssertion),
        Box::new(no_confusing_non_null_assertion::NoConfusingNonNullAssertion),
        Box::new(no_empty_interface::NoEmptyInterface),
        Box::new(no_inferrable_types::NoInferrableTypes),
        Box::new(ban_ts_comment::BanTsComment),
        Box::new(no_namespace::NoNamespace),
        Box::new(no_duplicate_enum_values::NoDuplicateEnumValues),
        Box::new(no_mixed_enums::NoMixedEnums),
        Box::new(no_misused_new::NoMisusedNew),
        Box::new(no_this_alias::NoThisAlias),
        Box::new(no_array_delete::NoArrayDelete),
        Box::new(no_require_imports::NoRequireImports),
        Box::new(prefer_as_const::PreferAsConst),
        Box::new(no_unnecessary_type_constraint::NoUnnecessaryTypeConstraint),
        Box::new(no_wrapper_object_types::NoWrapperObjectTypes),
        Box::new(adjacent_overload_signatures::AdjacentOverloadSignatures),
        Box::new(no_useless_empty_export::NoUselessEmptyExport),
        Box::new(jsx_key::JsxKey),
        Box::new(jsx_no_duplicate_props::JsxNoDuplicateProps),
        Box::new(rules_of_hooks::RulesOfHooks),
    ]
}
