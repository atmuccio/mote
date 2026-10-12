//! Other tools' rule names in `.mote.yaml` rule maps: the Mote rule to write instead.
//!
//! A configuration carried over by hand often keeps ESLint or PMD names (`no-unused-vars`,
//! `@typescript-eslint/no-explicit-any`, `SystemPrintln`,
//! `category/java/errorprone.xml/EmptyCatchBlock`). Validation reports them as unknown rules;
//! [`ForeignNames`] (registered on the CLI's registry) says which Mote rule is equivalent,
//! from the tables `mote migrate` uses ([`mote_lang_java::pmd`] and the ESLint mapping
//! of `migrate`), and points at `mote migrate` for converting a whole configuration.

use crate::commands::migrate::eslint;
use mote_engine::{ForeignRule, ForeignRuleNames, Registry};

const PMD_MIGRATE: &str = "`mote migrate --from pmd <ruleset.xml>` converts a whole PMD ruleset";
const ESLINT_MIGRATE: &str =
    "`mote migrate --from eslint <config>` converts a whole ESLint configuration";
const LIST: &str = "`mote rules --all` lists the rules";

/// The ESLint and PMD names `mote migrate` knows.
pub struct ForeignNames;

impl ForeignRuleNames for ForeignNames {
    fn explain(&self, key: &str, registry: &Registry) -> Option<ForeignRule> {
        pmd(key, registry).or_else(|| eslint(key, registry))
    }
}

/// `SystemPrintln`, `category/java/bestpractices.xml/SystemPrintln`,
/// `rulesets/java/basic.xml/EmptyCatchBlock`, or a whole category.
fn pmd(key: &str, registry: &Registry) -> Option<ForeignRule> {
    let reference = key
        .strip_prefix("category/java/")
        .or_else(|| key.strip_prefix("rulesets/java/"));
    let name = match reference {
        Some(rest) => match rest.rsplit_once('/') {
            Some((_, name)) => name,
            None => {
                return Some(ForeignRule::NoEquivalent(format!(
                    "that is a PMD category or ruleset, which is not expanded: enable the \
                     Mote rules you want, or a glob such as `java/*` ({LIST})"
                )))
            }
        },
        None => key,
    };
    if !is_pmd_name(name) {
        return None;
    }
    let what = if reference.is_some() {
        "a PMD rule reference"
    } else {
        "a PMD rule name"
    };
    match pmd_equivalent(name).filter(|id| registry.rule(id).is_some()) {
        Some(id) => Some(ForeignRule::Equivalent(format!(
            "that is {what}; the Mote rule is `{id}` ({PMD_MIGRATE})"
        ))),
        None if reference.is_some() => Some(ForeignRule::NoEquivalent(format!(
            "that is {what}, and Mote has no equivalent of PMD's `{name}` ({LIST})"
        ))),
        None => Some(ForeignRule::NoEquivalent(format!(
            "that looks like {what}, and Mote has no equivalent ({LIST})"
        ))),
    }
}

/// The Mote rule for a PMD rule name (the Java plugin's table; none in a build without it).
fn pmd_equivalent(name: &str) -> Option<&'static str> {
    #[cfg(feature = "java")]
    return mote_lang_java::pmd::mote_rule_for_pmd(name);
    #[cfg(not(feature = "java"))]
    {
        let _ = name;
        None
    }
}

/// PMD's rule names are UpperCamelCase words: `EmptyCatchBlock`.
fn is_pmd_name(name: &str) -> bool {
    name.len() > 1
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.bytes().all(|b| b.is_ascii_alphanumeric())
        && name.bytes().any(|b| b.is_ascii_lowercase())
}

/// `no-unused-vars`, `complexity`, `@typescript-eslint/no-explicit-any`, `react/jsx-key`, or an
/// ESLint plugin's rule without an equivalent.
fn eslint(key: &str, registry: &Registry) -> Option<ForeignRule> {
    let language_prefix = key
        .split_once('/')
        .is_some_and(|(prefix, _)| prefix == "mote" || registry.plugin_index(prefix).is_some());
    if language_prefix {
        return None;
    }
    if let Some(id) = eslint::mote_rule(key, registry) {
        // A bare name that is also another language's rule (`cognitive-complexity`) is a
        // missing language prefix rather than an ESLint name: the suggestion lists both.
        let bare = !key.contains('/');
        let elsewhere = bare
            && registry
                .rule_ids()
                .iter()
                .any(|other| *other != id && other.rsplit('/').next() == Some(key));
        if elsewhere {
            return None;
        }
        let limit = eslint::THRESHOLDS
            .iter()
            .find(|(name, _, _)| *name == key)
            .map(|(_, _, param)| format!(", with the limit in its parameter `{param}`"))
            .unwrap_or_default();
        return Some(ForeignRule::Equivalent(format!(
            "that is an ESLint rule; the Mote rule is `{id}`{limit} ({ESLINT_MIGRATE})"
        )));
    }
    let plugin = key.starts_with('@') || key.contains('/');
    plugin.then(|| {
        ForeignRule::NoEquivalent(format!(
            "that looks like an ESLint plugin rule, and Mote has no equivalent ({LIST})"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn explain(key: &str) -> Option<ForeignRule> {
        let registry = crate::plugins::registry().unwrap();
        ForeignNames.explain(key, &registry)
    }

    fn equivalent(key: &str) -> String {
        match explain(key) {
            Some(ForeignRule::Equivalent(text)) => text,
            other => panic!("{key}: {other:?}"),
        }
    }

    #[cfg(feature = "js")]
    #[test]
    fn eslint_names() {
        assert_eq!(
            equivalent("no-unused-vars"),
            "that is an ESLint rule; the Mote rule is `js/no-unused-vars` (`mote migrate \
             --from eslint <config>` converts a whole ESLint configuration)"
        );
        assert!(equivalent("@typescript-eslint/no-explicit-any")
            .contains("the Mote rule is `js/no-explicit-any` ("));
        assert!(equivalent("complexity").contains(
            "the Mote rule is `js/cyclomatic-complexity`, with the limit in its parameter \
             `threshold` ("
        ));
        assert!(matches!(
            explain("import/no-cycle"),
            Some(ForeignRule::NoEquivalent(t)) if t.contains("ESLint plugin rule")
        ));
        // Mote ids and names shared by both languages are not ESLint's.
        assert_eq!(explain("js/no-unused-varz"), None);
        assert_eq!(explain("java/foo"), None);
        assert_eq!(explain("cognitive-complexity"), None);
        assert_eq!(explain("no-such-thing"), None);
    }

    #[cfg(feature = "java")]
    #[test]
    fn pmd_names() {
        assert!(equivalent("EmptyCatchBlock").contains(
            "that is a PMD rule name; the Mote rule is `java/empty-catch-block` \
             (`mote migrate --from pmd <ruleset.xml>`"
        ));
        assert!(
            equivalent("category/java/errorprone.xml/EmptyCatchBlock").starts_with(
                "that is a PMD rule reference; the Mote rule is `java/empty-catch-block`"
            )
        );
        // Older names follow their renaming.
        assert!(equivalent("rulesets/java/imports.xml/UnusedImports").contains("`java/"));
        assert!(matches!(
            explain("category/java/bestpractices.xml/NoSuchPmdRule"),
            Some(ForeignRule::NoEquivalent(t)) if t.contains("no equivalent of PMD's `NoSuchPmdRule`")
        ));
        assert!(matches!(
            explain("category/java/bestpractices.xml"),
            Some(ForeignRule::NoEquivalent(t)) if t.contains("category or ruleset")
        ));
        assert!(matches!(
            explain("NoSuchPmdRule"),
            Some(ForeignRule::NoEquivalent(t)) if t.starts_with("that looks like a PMD rule name")
        ));
        assert_eq!(explain("ABC"), None);
    }
}
