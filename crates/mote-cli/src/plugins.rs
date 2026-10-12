//! Compiled-in language plugins (cargo features `java`, `js`).

use mote_core::LanguagePlugin;
use mote_engine::{Registry, RegistryError};
use std::sync::Arc;

/// The language plugins built into this binary.
#[allow(clippy::vec_init_then_push)] // pushes are feature-gated
pub fn plugins() -> Vec<Box<dyn LanguagePlugin>> {
    #[allow(unused_mut)]
    let mut v: Vec<Box<dyn LanguagePlugin>> = Vec::new();
    #[cfg(feature = "java")]
    v.push(Box::new(mote_lang_java::JavaPlugin::default()));
    #[cfg(feature = "js")]
    v.push(Box::new(
        // Analysis runs on the engine's big-stack pool; declaring it lets the plugin skip its
        // nesting pre-scan for all but the largest files.
        mote_lang_js::JsPlugin::default().with_analysis_stack(mote_engine::ANALYSIS_STACK_BYTES),
    ));
    #[cfg(feature = "test-plugin")]
    v.push(Box::new(crate::test_plugin::TestPlugin));
    v
}

/// Registry of the built-in plugins and their rules. Configuration errors name the Mote
/// rule for ESLint and PMD rule names written in rule maps (the `mote migrate` tables).
pub fn registry() -> Result<Arc<Registry>, RegistryError> {
    Registry::new(plugins()).map(|r| {
        Arc::new(r.with_foreign_rule_names(Box::new(crate::commands::config_foreign::ForeignNames)))
    })
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "java")]
    #[test]
    fn java_release_parameter_names_agree() {
        // The engine passes the Java release to the plugin as a rule parameter by this name.
        assert_eq!(
            mote_engine::JAVA_RELEASE_PARAM,
            mote_lang_java::JAVA_RELEASE_PARAM
        );
        assert_eq!(
            mote_engine::java_release::JAVA,
            mote_core::LanguagePlugin::id(&mote_lang_java::JavaPlugin::default())
        );
    }
}
