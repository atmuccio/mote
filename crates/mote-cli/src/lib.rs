//! The `mote` command-line tool and MCP server.
//!
//! [`run`] executes one command line against injected output streams and returns the exit code
//! (0 gate passed, 1 gate failed, 2 usage/config error, 3 internal error), so the whole CLI is
//! testable in-process. `main.rs` only installs the allocator and calls [`main`].

pub mod cli;
mod commands;
pub mod mcp;
pub mod plugins;
#[cfg(feature = "test-plugin")]
pub mod test_plugin;
mod util;

use clap::Parser;
use cli::{Cli, Command};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

/// Exit codes.
pub mod exit {
    pub const OK: i32 = 0;
    pub const GATE_FAILED: i32 = 1;
    pub const USAGE: i32 = 2;
    pub const INTERNAL: i32 = 3;
}

/// Output streams of one invocation.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    /// Whether `out` is an interactive terminal (enables `--color auto`).
    pub out_is_tty: bool,
}

/// A failure that ends the command with an exit code and a message on stderr.
#[derive(Debug)]
pub struct CliError {
    pub code: i32,
    pub message: String,
}

impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        CliError {
            code: exit::USAGE,
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        CliError {
            code: exit::INTERNAL,
            message: message.into(),
        }
    }
}

impl From<mote_engine::EngineError> for CliError {
    fn from(e: mote_engine::EngineError) -> Self {
        use mote_engine::EngineError as E;
        match e {
            E::Config(c) => CliError::from(c),
            E::Invalid(m) => CliError::usage(m),
            // A mistyped revision or a directory outside git is the user's to fix.
            E::Git(
                g @ (mote_engine::git::GitError::UnknownRev(_)
                | mote_engine::git::GitError::NotARepo(_)),
            ) => CliError::usage(g.to_string()),
            E::Git(g) => CliError::internal(g.to_string()),
            other => CliError::internal(other.to_string()),
        }
    }
}

impl From<mote_engine::ConfigError> for CliError {
    fn from(e: mote_engine::ConfigError) -> Self {
        config_error(e, None)
    }
}

/// The usage error of an invalid configuration, the same for every command (`config validate`,
/// `scan`, `gate`, …): a configuration that does not parse also gets the validation problems of
/// the rest of it (unknown rule ids, parameters, globs), so one message lists every problem of
/// the file; `origin` names the file when the error does not; the file is shown relative to the
/// current directory when it is below it (`.mote.yaml:3:5: …`).
pub(crate) fn config_error(
    e: mote_engine::ConfigError,
    origin: Option<&std::path::Path>,
) -> CliError {
    let mut e = match plugins::registry() {
        Ok(registry) => e.complete(&registry),
        Err(_) => e,
    };
    if e.path.is_none() {
        e.path = origin.map(std::path::Path::to_path_buf);
    }
    e.path = e.path.map(|p| shown_path(&p));
    CliError::usage(e.to_string())
}

/// `path` relative to the current directory when it is below it, without `./`.
fn shown_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let below_cwd = std::env::current_dir().ok().and_then(|cwd| {
        path.strip_prefix(cwd)
            .ok()
            .map(std::path::Path::to_path_buf)
    });
    let rel = below_cwd.unwrap_or_else(|| path.to_path_buf());
    let cleaned: std::path::PathBuf = rel
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    if cleaned.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        cleaned
    }
}

impl From<mote_engine::RegistryError> for CliError {
    fn from(e: mote_engine::RegistryError) -> Self {
        CliError::internal(format!("plugin registry: {e}"))
    }
}

pub type CliResult<T = i32> = Result<T, CliError>;

/// When the process entered `main` (for `--timings`).
pub static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn init_logging(quiet: bool, verbose: u8) {
    use tracing_subscriber::EnvFilter;
    let default = if quiet {
        "error"
    } else {
        match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };
    let filter = std::env::var("MOTE_LOG")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .and_then(|v| EnvFilter::try_new(v).ok())
        .unwrap_or_else(|| EnvFilter::new(default));
    // `try_init`: tests call `run` many times in one process.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .try_init();
}

/// Run one command line (`args[0]` is the program name). Never panics on user input.
pub fn run<I, T>(args: I, io: &mut Io<'_>) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let code = e.exit_code();
            let text = if io.out_is_tty {
                e.render().ansi().to_string()
            } else {
                e.render().to_string()
            };
            if code == 0 {
                let _ = write!(io.out, "{text}");
            } else {
                let _ = write!(io.err, "{text}");
            }
            return if code == 0 { exit::OK } else { exit::USAGE };
        }
    };
    init_logging(cli.quiet, cli.verbose);
    let result = match cli.command {
        Command::Scan(args) => commands::scan::run(&args, cli.quiet, io),
        Command::Gate(args) => commands::gate::run(&args, io),
        Command::Compare(args) => commands::compare::run(&args, cli.quiet, io),
        Command::Rules(args) => commands::rules::run(&args, io),
        Command::Config(args) => commands::config::run(&args, io),
        Command::Init(args) => commands::init::run(&args, io),
        Command::Triage(args) => commands::triage::run(&args, io),
        Command::Migrate(args) => commands::migrate::run(&args, cli.quiet, io),
        Command::Mcp(args) => commands::mcp::run(&args, io),
        Command::Version => commands::version(io),
    };
    let _ = io.out.flush();
    match result {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(io.err, "error: {}", e.message);
            e.code
        }
    }
}

/// Swallows `BrokenPipe` on stdout (e.g. `mote scan | head`) so the exit code still reflects
/// the scan.
struct Stdout(io::BufWriter<io::StdoutLock<'static>>);

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.write(buf) {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(buf.len()),
            r => r,
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self.0.flush() {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            r => r,
        }
    }
}

/// Stack of the thread that runs the command.
///
/// The process main thread only waits: on Windows its stack is 1 MB, too small for deeply nested
/// input. Commands (setup, discovery, reduce, the MCP server) run on a thread with this stack;
/// plugin analysis runs on the engine's worker pool, never on either of these threads.
pub const COMMAND_STACK_SIZE: usize = 8 << 20;

/// Name of the thread that runs the command.
pub const COMMAND_THREAD: &str = "mote-cli";

/// Entry point of the binary: runs the command on a [`COMMAND_STACK_SIZE`] thread.
pub fn main() -> ExitCode {
    let _ = START.set(std::time::Instant::now());
    let args: Vec<OsString> = std::env::args_os().collect();
    let fallback = args.clone();
    let code = match std::thread::Builder::new()
        .name(COMMAND_THREAD.into())
        .stack_size(COMMAND_STACK_SIZE)
        .spawn(move || run_process(args))
    {
        Ok(handle) => handle.join().unwrap_or_else(|_| {
            let _ = writeln!(
                io::stderr(),
                "error: internal error (panic); please report this with the command line used"
            );
            exit::INTERNAL
        }),
        // Cannot create threads at all: run here rather than not at all.
        Err(_) => run_process(fallback),
    };
    ExitCode::from(code.clamp(0, 255) as u8)
}

/// Run one command line against the process's stdout/stderr.
fn run_process(args: Vec<OsString>) -> i32 {
    let out_is_tty = io::stdout().is_terminal();
    let mut out = Stdout(io::BufWriter::with_capacity(1 << 16, io::stdout().lock()));
    let mut err = io::stderr();
    let code = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
            out_is_tty,
        };
        run(args, &mut io)
    }))
    .unwrap_or_else(|_| {
        let _ = writeln!(
            io::stderr(),
            "error: internal error (panic); please report this with the command line used"
        );
        exit::INTERNAL
    });
    let _ = out.flush();
    code
}
