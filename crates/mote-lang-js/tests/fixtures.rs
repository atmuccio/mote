//! Fixture-driven rule tests (see `mote_core::testing` for the format).
//!
//! `MOTE_FIXTURE=<rule-name> cargo test -p mote-lang-js --test fixtures` runs one rule.

use mote_core::testing::{assert_all_rules_robust, assert_fixtures};
use mote_lang_js::JsPlugin;
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[test]
fn fixtures() {
    assert_fixtures(&JsPlugin::new(), &fixture_dir());
}

#[test]
fn all_rules_robust() {
    assert_all_rules_robust(&JsPlugin::new(), &fixture_dir());
}
