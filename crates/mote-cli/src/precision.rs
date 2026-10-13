//! Measured precision of the rules, embedded at build time from `data/rule-precision.json`
//! (regenerated for each release from the latest benchmark measurement). Shown by
//! `mote rules explain`, `mote rules --format json|markdown` and the MCP `rule` tool.

use mote_report::precision::PrecisionData;
use std::sync::OnceLock;

const DATA: &str = include_str!("../data/rule-precision.json");

/// The embedded data (`None` only if the file were malformed, which a test rules out).
pub fn data() -> Option<&'static PrecisionData> {
    static CELL: OnceLock<Option<PrecisionData>> = OnceLock::new();
    CELL.get_or_init(|| PrecisionData::parse(DATA).ok())
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::RuleStatus;

    /// The test plugin (`tl/`) is not part of the product.
    fn product_rules() -> Vec<mote_engine::RuleInfo> {
        let registry = crate::plugins::registry().unwrap();
        crate::commands::rules::catalog(&registry)
            .into_iter()
            .filter(|r| r.language != "tl")
            .collect()
    }

    #[test]
    fn embedded_data_parses() {
        let d = PrecisionData::parse(DATA).expect("data/rule-precision.json");
        assert_eq!(d.format_version, 1);
        assert!(d.repositories > 0);
        assert_eq!(d.measured.len(), 10, "YYYY-MM-DD: {}", d.measured);
        assert!(data().is_some());
    }

    /// Catches stale data: a rule renamed or removed since the measurement, or a stable rule
    /// without an entry. New rules start incubating and become stable only after their findings
    /// are measured, so the data covers every stable rule; the maintainers regenerate it from the
    /// latest benchmark measurement.
    #[cfg(all(feature = "java", feature = "js"))]
    #[test]
    fn embedded_data_matches_the_rules() {
        let d = data().unwrap();
        let rules = product_rules();
        let ids: std::collections::HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
        let unknown: Vec<&String> = d
            .rules
            .keys()
            .filter(|id| !ids.contains(id.as_str()))
            .collect();
        assert!(
            unknown.is_empty(),
            "data/rule-precision.json has rules that no longer exist: {unknown:?}"
        );
        let missing: Vec<&str> = rules
            .iter()
            .filter(|r| r.status == RuleStatus::Stable && d.rule(&r.id).is_none())
            .map(|r| r.id.as_str())
            .collect();
        assert!(
            missing.is_empty(),
            "stable rules without an entry in data/rule-precision.json: {missing:?} \
             (a new rule starts as incubating; it becomes stable only after its findings are \
             measured, and the data is regenerated with that measurement)"
        );
        for (id, p) in &d.rules {
            assert_eq!(p.labeled, p.tp + p.fp, "{id}");
            assert!(p.tp + p.fp + p.noise <= p.findings, "{id}");
            assert_eq!(
                p.precision.is_some(),
                p.state == mote_report::precision::PrecisionState::Measured,
                "{id}"
            );
        }
    }
}
