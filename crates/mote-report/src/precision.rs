//! Measured precision of the rules: the data the CLI embeds (`data/rule-precision.json` of
//! `mote-cli`), and how `mote rules explain`, `mote rules --format json`, the MCP `rule` tool and
//! `docs/rules.md` show it.
//!
//! The findings of every rule on public open-source repositories are sampled and labeled true
//! positive, false positive or noise; precision is `tp / (tp + fp)`. A percentage is given only
//! when enough findings are labeled ([`PrecisionThresholds`]); otherwise only the counts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The measurement: when, on how many repositories, and one entry per rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrecisionData {
    pub format_version: u32,
    /// Date of the measured run (`YYYY-MM-DD`).
    pub measured: String,
    /// How the numbers were obtained, in one paragraph.
    pub methodology: String,
    /// The Mote version whose rule list the data covers.
    pub mote_version: String,
    /// Number of repositories scanned.
    pub repositories: u32,
    pub rules: BTreeMap<String, RulePrecision>,
    pub thresholds: PrecisionThresholds,
}

/// When labels are enough for a percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecisionThresholds {
    /// Labeled findings (true or false positives) needed for a percentage.
    pub min_labeled: u32,
    /// A rule with at most this many findings gets a percentage once every finding is labeled…
    pub rare_max_findings: u32,
    /// …and at least this many of them are true or false positives.
    pub rare_min_labeled: u32,
}

/// What the measurement says about one rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrecisionState {
    /// Enough labels: `precision` is set.
    Measured,
    /// Findings, but too few labeled for a percentage.
    TooFewLabels,
    /// No findings in the repositories.
    NoFindings,
    /// Deprecated at the time: not run on the repositories.
    NotRun,
}

/// One rule's counts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RulePrecision {
    pub state: PrecisionState,
    /// `tp / labeled` rounded down to three decimals, only when `state` is `measured`.
    pub precision: Option<f64>,
    pub tp: u32,
    pub fp: u32,
    /// True plus false positives among the labeled findings.
    pub labeled: u32,
    /// Findings labeled noise (code configuration should exclude): not part of the precision.
    pub noise: u32,
    /// Findings in the repositories (labeled or not).
    pub findings: u32,
}

/// The `precision` object of a rule in JSON outputs (`mote rules --format json`, `mote rules
/// explain --format json`, the MCP `rule` tool): the rule's entry, or `"state":
/// "not-measured-yet"` for a rule newer than the data, plus when and on how many repositories
/// it was measured.
#[derive(Clone, Debug, Serialize)]
pub struct PrecisionView<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<&'static str>,
    #[serde(flatten)]
    entry: Option<&'a RulePrecision>,
    measured: &'a str,
    repositories: u32,
}

impl RulePrecision {
    /// Every finding of the rule labeled (true or false positive, or noise).
    pub fn reviewed_all(&self) -> bool {
        self.findings > 0 && self.tp + self.fp + self.noise >= self.findings
    }
}

impl PrecisionData {
    /// Parse the JSON data file.
    pub fn parse(json: &str) -> Result<PrecisionData, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The entry of a rule (none for a rule newer than the measurement).
    pub fn rule(&self, id: &str) -> Option<&RulePrecision> {
        self.rules.get(id)
    }

    /// The `precision` object of a rule in JSON outputs.
    pub fn json_for(&self, id: &str) -> PrecisionView<'_> {
        let entry = self.rule(id);
        PrecisionView {
            state: entry.is_none().then_some("not-measured-yet"),
            entry,
            measured: &self.measured,
            repositories: self.repositories,
        }
    }

    /// The precision line of a rule. `standalone` adds when and where it was measured (for
    /// `mote rules explain`; the catalog says it once in its introduction); `markdown` sets the
    /// percentage in bold.
    pub fn line(&self, id: &str, standalone: bool, markdown: bool) -> String {
        let what = match self.rule(id) {
            None => format!(
                "not measured yet: the rule is newer than the last measurement ({})",
                self.measured
            ),
            Some(p) => match p.state {
                PrecisionState::Measured if p.labeled > 0 => {
                    let pct = percent(p.tp, p.labeled);
                    let pct = if markdown {
                        format!("**{pct}%**")
                    } else {
                        format!("{pct}%")
                    };
                    let every = if p.reviewed_all() {
                        ", every finding reviewed"
                    } else {
                        ""
                    };
                    let context = if standalone {
                        format!(
                            ", measured {} on {} open-source repositories",
                            self.measured, self.repositories
                        )
                    } else {
                        String::new()
                    };
                    format!(
                        "{pct} ({} of {} labeled findings correct{every}{context})",
                        p.tp, p.labeled
                    )
                }
                // Every finding labeled, still too few for a percentage: more labels cannot help
                // until the rule finds more code.
                PrecisionState::Measured | PrecisionState::TooFewLabels if p.reviewed_all() => {
                    if p.labeled > 0 {
                        format!(
                            "too few findings to measure ({} of {} labeled findings correct, every finding reviewed)",
                            p.tp, p.labeled
                        )
                    } else {
                        "too few findings to measure (every finding reviewed)".to_string()
                    }
                }
                PrecisionState::Measured | PrecisionState::TooFewLabels => {
                    if p.labeled > 0 {
                        format!(
                            "not enough labeled findings yet ({} of {} labeled findings correct so far)",
                            p.tp, p.labeled
                        )
                    } else {
                        "not enough labeled findings yet".to_string()
                    }
                }
                PrecisionState::NoFindings => {
                    "not measured: no findings in the benchmark repositories".to_string()
                }
                PrecisionState::NotRun => {
                    "not measured: deprecated rules are not run on the benchmark repositories"
                        .to_string()
                }
            },
        };
        format!("Measured precision: {what}")
    }

    /// The introduction of the catalog (`docs/rules.md`): method, limits, and how many of the
    /// listed rules have a percentage.
    pub fn intro_markdown<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> String {
        let (mut measured, mut few, mut none, mut newer, mut total) = (0, 0, 0, 0, 0);
        for id in ids {
            total += 1;
            match self.rule(id).map(|p| p.state) {
                Some(PrecisionState::Measured) => measured += 1,
                Some(PrecisionState::TooFewLabels) => few += 1,
                Some(PrecisionState::NoFindings | PrecisionState::NotRun) => none += 1,
                None => newer += 1,
            }
        }
        let t = &self.thresholds;
        let newer = match newer {
            0 => String::new(),
            1 => " One rule is newer than the measurement.".to_string(),
            n => format!(" {n} rules are newer than the measurement."),
        };
        format!(
            "## Measured precision\n\n\
Each rule has a *Measured precision* line. The findings of every rule on {repos} public \
open-source repositories at pinned commits (measured {date}) were sampled per rule, and each \
sampled finding was labeled correct, false positive, or noise. Precision is correct / (correct + \
false positive), rounded down. Of the {total} rules listed here: {measured} with a measured \
precision, {few} with findings but too few labeled ones for a percentage (their counts are \
shown), and {none} without findings in those repositories.{newer}\n\n\
Limits:\n\n\
- A finding is judged by the rule's own definition: it is correct when the code is what the rule \
describes, whether or not a team would choose to change it. Precision says how often a finding \
is right, not how many problems a rule catches.\n\
- Noise (findings in code that configuration should exclude, such as vendored or generated \
files) is counted apart and left out of the percentage.\n\
- A percentage needs at least {min} labeled findings, or, for a rule with at most {rare_max} \
findings in the repositories, every finding labeled and at least {rare_min} of them correct or \
false positive. Smaller samples show their counts only, and a percentage over few labels is \
still a rough figure.\n\
- Results on other code may differ.\n\n\
How the repositories were chosen and the findings labeled: \
[benchmark.md](benchmark.md#part-4-coverage-yardstick).\n",
            repos = self.repositories,
            date = self.measured,
            min = t.min_labeled,
            rare_max = t.rare_max_findings,
            rare_min = t.rare_min_labeled,
        )
    }
}

/// `tp / labeled` as a whole percentage, rounded down (199 of 200 is 99%, never 100%).
pub fn percent(tp: u32, labeled: u32) -> u32 {
    if labeled == 0 {
        return 0;
    }
    (u64::from(tp) * 100 / u64::from(labeled)) as u32
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small data set with one rule per state (ids of `testdata::with_base`).
    pub fn sample() -> PrecisionData {
        PrecisionData::parse(
            r#"{
  "format_version": 1,
  "measured": "2026-10-12",
  "methodology": "Sampled and labeled.",
  "mote_version": "0.1.0",
  "repositories": 34,
  "rules": {
    "java/empty-catch-block": {"findings": 120, "fp": 1, "labeled": 60, "noise": 2, "precision": 0.983, "state": "measured", "tp": 59},
    "java/self-assignment": {"findings": 7, "fp": 0, "labeled": 5, "noise": 2, "precision": 1, "state": "measured", "tp": 5},
    "java/todo-comment": {"findings": 400, "fp": 0, "labeled": 8, "noise": 0, "precision": null, "state": "too-few-labels", "tp": 8},
    "java/duplicated-block": {"findings": 0, "fp": 0, "labeled": 0, "noise": 0, "precision": null, "state": "no-findings", "tp": 0}
  },
  "thresholds": {"min_labeled": 20, "rare_max_findings": 50, "rare_min_labeled": 5}
}"#,
        )
        .unwrap()
    }

    #[test]
    fn lines_per_state() {
        let d = sample();
        assert_eq!(
            d.line("java/empty-catch-block", true, false),
            "Measured precision: 98% (59 of 60 labeled findings correct, measured 2026-10-12 on 34 open-source repositories)"
        );
        assert_eq!(
            d.line("java/empty-catch-block", false, true),
            "Measured precision: **98%** (59 of 60 labeled findings correct)"
        );
        assert_eq!(
            d.line("java/self-assignment", false, false),
            "Measured precision: 100% (5 of 5 labeled findings correct, every finding reviewed)"
        );
        assert_eq!(
            d.line("java/todo-comment", true, false),
            "Measured precision: not enough labeled findings yet (8 of 8 labeled findings correct so far)"
        );
        // Every finding labeled, too few of them.
        let mut rare = d.clone();
        let e = rare.rules.get_mut("java/todo-comment").unwrap();
        e.findings = 3;
        e.labeled = 2;
        e.tp = 2;
        e.noise = 1;
        assert_eq!(
            rare.line("java/todo-comment", true, false),
            "Measured precision: too few findings to measure (2 of 2 labeled findings correct, every finding reviewed)"
        );
        assert_eq!(
            d.line("java/duplicated-block", true, false),
            "Measured precision: not measured: no findings in the benchmark repositories"
        );
        assert_eq!(
            d.line("java/cognitive-complexity", true, false),
            "Measured precision: not measured yet: the rule is newer than the last measurement (2026-10-12)"
        );
    }

    #[test]
    fn json_objects() {
        use serde_json::Value;
        let d = sample();
        let json_for = |id: &str| serde_json::to_value(d.json_for(id)).unwrap();
        let v = json_for("java/empty-catch-block");
        assert_eq!(v["state"], "measured");
        assert_eq!(v["precision"], 0.983);
        assert_eq!(v["tp"], 59);
        assert_eq!(v["fp"], 1);
        assert_eq!(v["labeled"], 60);
        assert_eq!(v["noise"], 2);
        assert_eq!(v["findings"], 120);
        assert_eq!(v["measured"], "2026-10-12");
        assert_eq!(v["repositories"], 34);
        assert_eq!(json_for("java/todo-comment")["precision"], Value::Null);
        let v = json_for("java/cognitive-complexity");
        assert_eq!(v["state"], "not-measured-yet");
        assert_eq!(v["measured"], "2026-10-12");
        assert!(v.get("tp").is_none());
        // Field order: state first, when and where last.
        let text = serde_json::to_string(&d.json_for("java/empty-catch-block")).unwrap();
        assert_eq!(
            text,
            r#"{"state":"measured","precision":0.983,"tp":59,"fp":1,"labeled":60,"noise":2,"findings":120,"measured":"2026-10-12","repositories":34}"#
        );
        assert_eq!(
            serde_json::to_string(&d.json_for("java/cognitive-complexity")).unwrap(),
            r#"{"state":"not-measured-yet","measured":"2026-10-12","repositories":34}"#
        );
    }

    #[test]
    fn percent_rounds_down() {
        assert_eq!(percent(59, 60), 98);
        assert_eq!(percent(199, 200), 99);
        assert_eq!(percent(949, 1000), 94);
        assert_eq!(percent(20, 20), 100);
        assert_eq!(percent(0, 0), 0);
    }

    #[test]
    fn intro_counts_the_listed_rules() {
        let d = sample();
        let s = d.intro_markdown([
            "java/empty-catch-block",
            "java/todo-comment",
            "java/duplicated-block",
            "java/cognitive-complexity",
        ]);
        assert!(s.starts_with("## Measured precision\n\n"), "{s}");
        assert!(
            s.contains(
                "on 34 public open-source repositories at pinned commits (measured 2026-10-12)"
            ),
            "{s}"
        );
        assert!(
            s.contains("Of the 4 rules listed here: 1 with a measured precision, 1 with findings but too few"),
            "{s}"
        );
        assert!(
            s.contains(
                "1 without findings in those repositories. One rule is newer than the measurement."
            ),
            "{s}"
        );
        assert!(
            s.contains("at least 20 labeled findings, or, for a rule with at most 50"),
            "{s}"
        );
    }
}
