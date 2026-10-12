//! Fixture-driven rule tests. `MOTE_FIXTURE=<rule-name>` runs only matching fixtures.

use mote_core::testing::{assert_all_rules_robust, assert_fixtures};
use mote_lang_java::JavaPlugin;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[test]
fn fixtures() {
    assert_fixtures(&JavaPlugin::new(), &fixtures_dir());
}

#[test]
fn all_rules_are_robust_on_every_fixture() {
    assert_all_rules_robust(&JavaPlugin::new(), &fixtures_dir());
}
