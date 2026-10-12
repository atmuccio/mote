//! Unit-test helpers.

pub use crate::fake_plugin::FakePlugin;
use crate::registry::Registry;
use mote_core::LanguagePlugin;

/// Build a registry, panicking on error.
pub fn registry(plugins: Vec<Box<dyn LanguagePlugin>>) -> Registry {
    Registry::new(plugins).expect("registry")
}
