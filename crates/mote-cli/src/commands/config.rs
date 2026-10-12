//! `mote config validate|schema|explain`.

use crate::cli::{ConfigArgs, ConfigCommand, ExplainFormat};
use crate::commands::scan::scanner_error;
use crate::util::{self, render};
use crate::{exit, plugins, CliError, CliResult, Io};
use mote_engine::scan::EffectiveRules;
use mote_engine::{Config, ReleaseOrigin, Scanner};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// `--config FILE`, else `.mote.yaml` / `.mote.yml` in `dir`. Returns the config and the
/// file it came from.
pub fn load(explicit: Option<&Path>, dir: &Path) -> Result<(Config, Option<PathBuf>), CliError> {
    match explicit {
        Some(p) => Ok((Config::load(p)?, Some(p.to_path_buf()))),
        None => Ok(Config::discover(dir)?),
    }
}

/// Effective rules of a file as JSON (shared with the MCP `explain_config` tool).
pub fn effective_json(path: &str, e: &EffectiveRules) -> Value {
    let mut rules = Map::new();
    for (id, r) in &e.rules {
        let v = if r.params.is_empty() {
            json!(r.severity.as_str())
        } else {
            json!({"severity": r.severity.as_str(), "params": r.params})
        };
        rules.insert(id.clone(), v);
    }
    let mut out = json!({
        "path": path,
        "language": e.language,
        "is_test": e.is_test,
        "excluded": e.excluded,
        "rules": rules,
    });
    if e.language == mote_engine::java_release::JAVA {
        out["java_release"] = match &e.java_release {
            Some(r) => {
                let mut v = json!({ "release": r.release });
                match &r.origin {
                    ReleaseOrigin::Config => v["source"] = json!("config"),
                    ReleaseOrigin::BuildFile { path, setting } => {
                        v["source"] = json!(path);
                        v["setting"] = json!(setting);
                    }
                }
                v
            }
            None => Value::Null,
        };
    }
    out
}

/// The Java release line of `config explain`.
fn java_release_line(e: &EffectiveRules) -> Option<String> {
    (e.language == mote_engine::java_release::JAVA).then(|| match &e.java_release {
        Some(r) => format!("java release: {} ({})", r.release, r.origin),
        None => "java release: unknown (no build file declares it; set `java.release` in the \
                 configuration)"
            .to_string(),
    })
}

pub fn run(args: &ConfigArgs, io: &mut Io<'_>) -> CliResult {
    match &args.command {
        ConfigCommand::Schema { results } => {
            let schema = if *results {
                mote_engine::ScanResult::json_schema()
            } else {
                Config::json_schema()
            };
            let out = render(|w| {
                serde_json::to_writer_pretty(&mut *w, &schema)?;
                writeln!(w)
            });
            io.out
                .write_all(&out)
                .map_err(|e| CliError::internal(e.to_string()))?;
            Ok(exit::OK)
        }
        ConfigCommand::Validate { config } => {
            let cwd = util::cwd()?;
            // Every problem of the file at once (parse and validation), each at its line and
            // column — the same message `scan` gives.
            let (cfg, origin) = load(config.as_deref(), &cwd)?;
            let registry = plugins::registry()?;
            cfg.validate(&registry)
                .map_err(|e| crate::config_error(e, origin.as_deref()))?;
            let _ = match origin {
                Some(p) => writeln!(io.out, "{}: configuration is valid", p.display()),
                None => writeln!(
                    io.out,
                    "no .mote.yaml found in {}; the defaults apply (create one with `mote init`)",
                    cwd.display()
                ),
            };
            Ok(exit::OK)
        }
        ConfigCommand::Explain {
            path,
            config,
            format,
        } => {
            let cwd = util::cwd()?;
            let (cfg, origin) = load(config.as_deref(), &cwd)?;
            let registry = plugins::registry()?;
            let origin = origin.map(|p| p.display().to_string());
            let scanner = Scanner::new(registry, cfg, &cwd)
                .map_err(|e| scanner_error(e, origin.as_deref()))?;
            let rel = mote_engine::source::relativize(&cwd, path).ok_or_else(|| {
                CliError::usage(format!("{path} is outside the current directory"))
            })?;
            let eff = scanner
                .effective_rules(&rel)
                .ok_or_else(|| CliError::usage(format!("no language plugin handles {rel}")))?;
            let out = match format {
                ExplainFormat::Json => render(|w| {
                    serde_json::to_writer_pretty(&mut *w, &effective_json(&rel, &eff))?;
                    writeln!(w)
                }),
                ExplainFormat::Text => render(|w| {
                    writeln!(w, "{rel}")?;
                    writeln!(
                        w,
                        "  language: {} · test code: {} · excluded: {}",
                        eff.language,
                        if eff.is_test { "yes" } else { "no" },
                        if eff.excluded { "yes" } else { "no" }
                    )?;
                    if let Some(o) = &origin {
                        writeln!(w, "  config: {o}")?;
                    }
                    if let Some(line) = java_release_line(&eff) {
                        writeln!(w, "  {line}")?;
                    }
                    writeln!(w, "  {} rules enabled:", eff.rules.len())?;
                    let width = eff.rules.keys().map(String::len).max().unwrap_or(0);
                    for (id, r) in &eff.rules {
                        let params: Vec<String> = r
                            .params
                            .iter()
                            .map(|(k, v)| format!("{k}={}", mote_report::rules::param_value(v)))
                            .collect();
                        writeln!(
                            w,
                            "    {id:<width$}  {:<6}  {}",
                            r.severity.as_str(),
                            params.join(" ")
                        )?;
                    }
                    Ok(())
                }),
            };
            io.out
                .write_all(&out)
                .map_err(|e| CliError::internal(e.to_string()))?;
            Ok(exit::OK)
        }
    }
}
