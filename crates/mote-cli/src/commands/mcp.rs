//! `mote mcp [--root DIR]`: the MCP server on stdin/stdout.

use crate::cli::McpArgs;
use crate::{exit, plugins, util, CliError, CliResult, Io};

pub fn run(args: &McpArgs, io: &mut Io<'_>) -> CliResult {
    let registry = plugins::registry()?;
    let root = match &args.root {
        Some(r) if r.is_absolute() => r.clone(),
        Some(r) => util::cwd()?.join(r),
        None => util::cwd()?,
    };
    if !root.is_dir() {
        return Err(CliError::usage(format!(
            "{}: not a directory",
            root.display()
        )));
    }
    tracing::info!("MCP server ready (root {})", root.display());
    let mut server = crate::mcp::Server::new(registry, root);
    let stdin = std::io::stdin();
    crate::mcp::serve(stdin.lock(), io.out, &mut server)
        .map_err(|e| CliError::internal(format!("MCP transport error: {e}")))?;
    Ok(exit::OK)
}
