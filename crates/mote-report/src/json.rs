//! JSON (the full `ScanResult`) and JSONL (one compact finding per line).

use crate::{status_str, Filter};
use mote_core::Finding;
use mote_engine::ScanResult;
use serde::Serialize;
use std::io::{self, Write};

/// Write the complete result as JSON (fields in declaration order; round-trips through
/// `serde_json::from_*::<ScanResult>`).
pub fn write_json(result: &ScanResult, pretty: bool, w: &mut dyn Write) -> io::Result<()> {
    if pretty {
        serde_json::to_writer_pretty(&mut *w, result)?;
    } else {
        serde_json::to_writer(&mut *w, result)?;
    }
    writeln!(w)
}

/// Parse a result written by [`write_json`].
pub fn read_json(text: &str) -> Result<ScanResult, serde_json::Error> {
    serde_json::from_str(text)
}

/// One JSONL line.
#[derive(Serialize)]
pub struct JsonlFinding<'a> {
    pub id: &'a str,
    pub rule: &'a str,
    pub severity: &'static str,
    pub quality: &'static str,
    pub path: &'a str,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub message: &'a str,
    pub status: &'static str,
    pub symbol: Option<&'a str>,
}

impl<'a> From<&'a Finding> for JsonlFinding<'a> {
    fn from(f: &'a Finding) -> Self {
        JsonlFinding {
            id: &f.id,
            rule: &f.rule_id,
            severity: f.severity.as_str(),
            quality: f.quality.as_str(),
            path: &f.path,
            line: f.range.start_line,
            col: f.range.start_col,
            end_line: f.range.end_line,
            end_col: f.range.end_col,
            message: &f.message,
            status: status_str(f.status),
            symbol: f.symbol.as_deref(),
        }
    }
}

/// Write the filtered findings, one compact JSON object per line.
pub fn write_jsonl(result: &ScanResult, filter: &Filter, w: &mut dyn Write) -> io::Result<()> {
    for f in filter.apply(result) {
        serde_json::to_writer(&mut *w, &JsonlFinding::from(f))?;
        w.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, with_base, without_base};
    use mote_core::Severity;

    #[test]
    fn json_round_trips() {
        for r in [with_base(), without_base()] {
            let mut a = Vec::new();
            write_json(&r, true, &mut a).unwrap();
            let back = read_json(std::str::from_utf8(&a).unwrap()).unwrap();
            let mut b = Vec::new();
            write_json(&back, true, &mut b).unwrap();
            assert_eq!(a, b);
        }
        let mut a = Vec::new();
        write_json(&with_base(), false, &mut a).unwrap();
        let text = String::from_utf8(a).unwrap();
        assert_eq!(text.lines().count(), 1);
        // stable field order: the declaration order of ScanResult
        let pos = |k: &str| text.find(&format!("\"{k}\":")).unwrap();
        assert!(pos("engine_version") < pos("source"));
        assert!(pos("findings") < pos("files"));
        assert!(pos("gate") < pos("stats"));
    }

    #[test]
    fn jsonl_snapshot_and_filter() {
        let mut out = Vec::new();
        write_jsonl(&with_base(), &Filter::default(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_snapshot("findings.jsonl", &text);
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v.as_object().unwrap().len(), 12);
            let keys = [
                "id", "rule", "severity", "quality", "path", "line", "col", "end_line", "end_col",
                "message", "status", "symbol",
            ];
            let positions: Vec<usize> = keys
                .iter()
                .map(|k| line.find(&format!("\"{k}\":")).unwrap())
                .collect();
            assert!(positions.windows(2).all(|w| w[0] < w[1]), "{line}");
        }
        assert_eq!(text.lines().count(), 5, "suppressed finding excluded");
        let mut out = Vec::new();
        let f = Filter {
            new_only: true,
            min_severity: Severity::Medium,
            show_suppressed: false,
        };
        write_jsonl(&with_base(), &f, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 3, "{text}");
        assert!(!text.contains("\"unchanged\""));
    }
}
