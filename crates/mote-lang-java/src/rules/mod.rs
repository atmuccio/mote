//! Rule registry.
//!
//! Exemplary rules live in this directory; rule batches live in `batch_1` … `batch_3` (each
//! owned by one author, who only edits files inside that directory plus fixtures).
//! [`all`] creates a **fresh instance of every rule** — the plugin calls it once at start-up
//! (metadata and dispatch table) and once per analyzed file, so the order must be stable.

use crate::rule::BoxedRule;
use mote_core::RuleMeta;

pub mod batch_1;
pub mod batch_10;
pub mod batch_11;
pub mod batch_12;
pub mod batch_13;
pub mod batch_14;
pub mod batch_15;
pub mod batch_2;
pub mod batch_3;
pub mod batch_4;
pub mod batch_5;
pub mod batch_6;
pub mod batch_7;
pub mod batch_8;
pub mod batch_9;

mod shared;

mod collection_size_comparison;
mod empty_catch_block;
mod equals_without_hashcode;
mod print_stack_trace;
mod return_in_finally;
mod string_reference_equality;
mod unused_import;
mod unused_private_field;

/// Placeholder metadata used if no rule is current (never reported).
pub(crate) static NO_RULE: RuleMeta = RuleMeta::new("java/none", "java", "none");

/// Every Java rule, in a stable order.
pub fn all<'t>() -> Vec<BoxedRule<'t>> {
    let mut v: Vec<BoxedRule<'t>> = vec![
        Box::new(empty_catch_block::EmptyCatchBlock),
        Box::new(string_reference_equality::StringReferenceEquality),
        Box::new(collection_size_comparison::CollectionSizeComparison),
        Box::new(unused_import::UnusedImport::default()),
        Box::new(return_in_finally::ReturnInFinally),
        Box::new(equals_without_hashcode::EqualsWithoutHashCode),
        Box::new(unused_private_field::UnusedPrivateField::default()),
        Box::new(print_stack_trace::PrintStackTrace),
    ];
    v.extend(batch_1::rules());
    v.extend(batch_2::rules());
    v.extend(batch_3::rules());
    v.extend(batch_4::rules());
    v.extend(batch_5::rules());
    v.extend(batch_6::rules());
    v.extend(batch_7::rules());
    v.extend(batch_8::rules());
    v.extend(batch_9::rules());
    v.extend(batch_10::rules());
    v.extend(batch_11::rules());
    v.extend(batch_12::rules());
    v.extend(batch_13::rules());
    v.extend(batch_14::rules());
    v.extend(batch_15::rules());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JavaPlugin;
    use mote_core::LanguagePlugin;

    #[test]
    fn rule_metadata_is_well_formed() {
        let rules = all();
        let mut ids: Vec<&str> = rules.iter().map(|r| r.meta().id).collect();
        for r in &rules {
            let m = r.meta();
            assert!(
                m.id.starts_with("java/"),
                "{}: id must start with java/",
                m.id
            );
            let name = &m.id["java/".len()..];
            assert!(
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{}: id must be kebab-case",
                m.id
            );
            assert_eq!(m.language, "java", "{}: language must be `java`", m.id);
            assert!(!m.name.is_empty(), "{}: empty name", m.id);
            assert!(m.description.len() > 80, "{}: description too short", m.id);
            for p in m.params {
                assert!(
                    !p.description.is_empty(),
                    "{}: parameter {} lacks a description",
                    m.id,
                    p.name
                );
            }
        }
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate rule ids");
    }

    #[test]
    fn registry_order_is_stable_and_kinds_exist() {
        let a: Vec<&str> = all().iter().map(|r| r.meta().id).collect();
        let b: Vec<&str> = all().iter().map(|r| r.meta().id).collect();
        assert_eq!(a, b);
        let plugin = JavaPlugin::new();
        assert!(
            plugin.unknown_rule_kinds().is_empty(),
            "unknown node kinds: {:?}",
            plugin.unknown_rule_kinds()
        );
        assert_eq!(plugin.rules().len(), a.len());
    }

    #[test]
    fn call_names_only_on_method_invocation_rules() {
        for r in all() {
            let names = r.call_names();
            if names.is_empty() {
                continue;
            }
            assert!(
                r.kinds().contains(&"method_invocation"),
                "{}: call_names without subscribing to method_invocation",
                r.meta().id
            );
            assert!(
                names.iter().all(|n| !n.is_empty()),
                "{}: empty call name",
                r.meta().id
            );
        }
    }

    #[test]
    fn method_names_only_on_method_declaration_rules() {
        for r in all() {
            let names = r.method_names();
            if names.is_empty() {
                continue;
            }
            assert!(
                r.kinds().contains(&"method_declaration"),
                "{}: method_names without subscribing to method_declaration",
                r.meta().id
            );
            assert!(
                names.iter().all(|n| !n.is_empty()),
                "{}: empty method name",
                r.meta().id
            );
        }
    }
}
