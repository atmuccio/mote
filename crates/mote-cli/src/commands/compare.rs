//! `mote compare OLD NEW`: what changed between two saved results (see
//! [`mote_report::compare`]). Exits 0 unless a file cannot be read (2).

use crate::cli::{CompareArgs, CompareFormat};
use crate::commands::scan::read_results;
use crate::util::write_file;
use crate::{exit, CliError, CliResult, Io};
use mote_report::compare::{self, CompareOptions};

pub fn run(args: &CompareArgs, quiet: bool, io: &mut Io<'_>) -> CliResult {
    let old = read_results(&args.old)?;
    let new = read_results(&args.new)?;
    let c = compare::compare(
        &args.old.to_string_lossy(),
        &old,
        &args.new.to_string_lossy(),
        &new,
    );
    let opts = CompareOptions {
        max_listed: args.max_listed,
    };
    let out = match args.format {
        CompareFormat::Text => compare::render_text(&c, &opts),
        CompareFormat::Markdown => compare::render_markdown(&c, &opts),
        CompareFormat::Json => {
            let mut s = serde_json::to_string_pretty(&compare::to_json(&c))
                .map_err(|e| CliError::internal(format!("cannot render JSON: {e}")))?;
            s.push('\n');
            s
        }
    };
    // Text carries the warnings itself; the other formats get them on stderr too.
    if !quiet && args.format != CompareFormat::Text {
        for w in &c.warnings {
            let _ = writeln!(io.err, "warning: {w}");
        }
    }
    match &args.output {
        Some(p) => write_file(p, out.as_bytes())?,
        None => io
            .out
            .write_all(out.as_bytes())
            .map_err(|e| CliError::internal(format!("cannot write to stdout: {e}")))?,
    }
    Ok(exit::OK)
}
