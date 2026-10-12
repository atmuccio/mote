//! Subcommand implementations.

pub mod compare;
pub mod config;
pub mod config_foreign;
pub mod gate;
pub mod init;
pub mod mcp;
pub mod migrate;
pub mod rules;
pub mod scan;
pub mod triage;

use crate::{CliResult, Io};

/// `mote version`: first line is `mote <semver>` (the GitHub Action parses it).
pub fn version(io: &mut Io<'_>) -> CliResult {
    let plugins = crate::plugins::plugins();
    let langs: Vec<String> = plugins
        .iter()
        .map(|p| format!("{} {} (v{})", p.id(), p.display_name(), p.version()))
        .collect();
    let _ = writeln!(io.out, "mote {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(io.out, "engine {}", mote_engine::ENGINE_VERSION);
    let _ = writeln!(
        io.out,
        "languages: {}",
        if langs.is_empty() {
            "none".to_string()
        } else {
            langs.join(", ")
        }
    );
    Ok(crate::exit::OK)
}
