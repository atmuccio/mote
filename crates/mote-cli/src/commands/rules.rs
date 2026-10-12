//! `mote rules [--language L] [--format F] [--all]` and `mote rules explain <ID>`.

use crate::cli::{RulesArgs, RulesCommand, RulesFormat};
use crate::util::render;
use crate::{exit, plugins, CliError, CliResult, Io};
use mote_engine::{Registry, RuleInfo};

/// Every rule of the registry as `RuleInfo`, sorted by id.
pub fn catalog(registry: &Registry) -> Vec<RuleInfo> {
    let mut v: Vec<RuleInfo> = registry
        .catalog()
        .iter()
        .map(RuleInfo::from_entry)
        .collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

/// Look up a rule, with a "did you mean" hint.
pub fn find_rule(registry: &Registry, id: &str) -> Result<RuleInfo, String> {
    registry.rule_info(id).ok_or_else(|| {
        let ids = registry.rule_ids();
        match mote_engine::config::did_you_mean(id, &ids) {
            Some(s) => format!("unknown rule `{id}` (did you mean {s}?)"),
            None => format!("unknown rule `{id}` (list them with `mote rules --all`)"),
        }
    })
}

pub fn run(args: &RulesArgs, io: &mut Io<'_>) -> CliResult {
    let registry = plugins::registry()?;
    let out = match &args.command {
        Some(RulesCommand::Explain { rule_id, format }) => {
            let rule = find_rule(&registry, rule_id).map_err(CliError::usage)?;
            match format {
                RulesFormat::Text => render(|w| mote_report::rules::explain_text(&rule, w)),
                RulesFormat::Json => render(|w| {
                    serde_json::to_writer_pretty(&mut *w, &rule)?;
                    writeln!(w)
                }),
                RulesFormat::Markdown => mote_report::rules::explain_markdown(&rule).into_bytes(),
            }
        }
        None => {
            let rules: Vec<RuleInfo> = catalog(&registry)
                .into_iter()
                .filter(|r| args.all || r.default_enabled)
                .filter(|r| {
                    args.language
                        .as_deref()
                        .is_none_or(|l| r.language.eq_ignore_ascii_case(l))
                })
                .collect();
            if let Some(l) = &args.language {
                if rules.is_empty() && !catalog(&registry).iter().any(|r| &r.language == l) {
                    let mut langs: Vec<String> =
                        catalog(&registry).into_iter().map(|r| r.language).collect();
                    langs.sort();
                    langs.dedup();
                    return Err(CliError::usage(format!(
                        "unknown language `{l}` (available: {})",
                        langs.join(", ")
                    )));
                }
            }
            match args.format {
                RulesFormat::Text => render(|w| mote_report::rules::write_text(&rules, w)),
                RulesFormat::Json => render(|w| mote_report::rules::write_json(&rules, w)),
                RulesFormat::Markdown => {
                    let mut s = mote_report::rules::render_markdown(&rules);
                    s.push('\n');
                    s.into_bytes()
                }
            }
        }
    };
    io.out
        .write_all(&out)
        .map_err(|e| CliError::internal(format!("cannot write to stdout: {e}")))?;
    Ok(exit::OK)
}
