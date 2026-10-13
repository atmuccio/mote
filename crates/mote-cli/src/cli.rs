//! Command-line definitions (clap derive). See `docs/cli.md`.

use clap::{Args, Parser, Subcommand, ValueEnum};
use mote_core::Severity;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "mote",
    bin_name = "mote",
    version,
    about = "Fast code-quality scanner: smells, bugs, complexity, duplication — gated on new code",
    propagate_version = true,
    max_term_width = 100
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Only print errors (and the requested output)
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// More logging on stderr (-v info, -vv debug, -vvv trace; or set MOTE_LOG)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Analyze code (working tree, a git revision, staged changes, or a remote repo)
    Scan(Box<ScanArgs>),
    /// Apply the fixes of fixable findings: prints a diff, or --write applies it
    Fix(Box<FixArgs>),
    /// Re-evaluate the quality gate from a saved results file (e.g. adding coverage later)
    Gate(GateArgs),
    /// Compare two saved results files: added, fixed and changed findings, measure deltas
    Compare(CompareArgs),
    /// List rules or explain one
    Rules(RulesArgs),
    /// Validate, explain, or print the JSON Schema of .mote.yaml or of the results
    Config(ConfigArgs),
    /// Write a starter .mote.yaml (proposed excludes and compat sources; --from merges in a PMD or ESLint configuration)
    Init(InitArgs),
    /// Record, list and prune triage decisions (accepted findings, false positives)
    Triage(TriageArgs),
    /// Propose a .mote.yaml from a PMD ruleset or an ESLint configuration (prints it)
    Migrate(MigrateArgs),
    /// Run the MCP server on stdio (for AI agents)
    Mcp(McpArgs),
    /// Print version information
    Version,
}

/// Output format of `scan`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Jsonl,
    Sarif,
    Markdown,
    Github,
    Gitlab,
    Checkstyle,
    Junit,
    Html,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// Parse `HIGH` / `medium` / `low`.
pub fn parse_severity(s: &str) -> Result<Severity, String> {
    mote_report::parse_severity(s)
        .ok_or_else(|| format!("invalid severity `{s}` (expected HIGH, MEDIUM or LOW)"))
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Root of the scan (default: the current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    // --- source -------------------------------------------------------------------------------
    /// Scan a git revision straight from the object database (no checkout)
    #[arg(
        long,
        value_name = "REV",
        help_heading = "Source",
        conflicts_with = "staged"
    )]
    pub rev: Option<String>,
    /// Scan the staged (index) content — for pre-commit hooks
    #[arg(long, help_heading = "Source")]
    pub staged: bool,
    /// Shallow-clone URL into the cache and scan it (use --rev for a ref)
    #[arg(
        long,
        value_name = "URL",
        help_heading = "Source",
        conflicts_with_all = ["staged", "path"]
    )]
    pub remote: Option<String>,

    // --- new code -------------------------------------------------------------------------------
    /// Compare against merge-base(REV, head): findings become new / unchanged / worsened and the
    /// gate applies to new code
    #[arg(long, value_name = "REV", help_heading = "New code")]
    pub base: Option<String>,
    /// Only analyze and report files changed vs --base
    #[arg(long, requires = "base", help_heading = "New code")]
    pub changed_only: bool,
    /// Compare with a saved results file (`scan --json`) instead of a git base: findings not in
    /// it are new; conditions that need a line diff are skipped
    #[arg(
        long,
        value_name = "FILE",
        help_heading = "New code",
        conflicts_with_all = ["base", "changed_only"]
    )]
    pub baseline: Option<PathBuf>,

    // --- selection ------------------------------------------------------------------------------
    /// Only these paths: globs or path prefixes (repeatable)
    #[arg(long, value_name = "GLOB", help_heading = "Selection")]
    pub include: Vec<String>,
    /// Config file (default: <root>/.mote.yaml if present)
    #[arg(long, value_name = "FILE", help_heading = "Selection")]
    pub config: Option<PathBuf>,
    /// Coverage reports: LCOV, JaCoCo XML, Cobertura XML (repeatable; globs allowed)
    #[arg(long, value_name = "GLOB", help_heading = "Selection")]
    pub coverage: Vec<String>,
    /// Triage file (default: `triage.file` of the config, `.mote-triage.yaml`)
    #[arg(long, value_name = "FILE", help_heading = "Selection")]
    pub triage: Option<PathBuf>,

    // --- output ---------------------------------------------------------------------------------
    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Text, help_heading = "Output")]
    pub format: Format,
    /// Write --format output to FILE instead of stdout
    #[arg(short, long, value_name = "FILE", help_heading = "Output")]
    pub output: Option<PathBuf>,
    /// Also write full results JSON (input for `mote gate`)
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub json: Option<PathBuf>,
    /// Also write one finding per line (JSONL)
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub jsonl: Option<PathBuf>,
    /// Also write SARIF 2.1.0
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub sarif: Option<PathBuf>,
    /// Lowest severity included in SARIF
    #[arg(long, value_name = "SEV", default_value = "LOW", value_parser = parse_severity, help_heading = "Output")]
    pub sarif_min_severity: Severity,
    /// Also write the Markdown summary (job summary / PR comment)
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub markdown: Option<PathBuf>,
    /// Also write a GitHub Checks API payload (annotations for new findings)
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub github_checks: Option<PathBuf>,
    /// Also write a GitLab Code Quality report
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub gitlab_codequality: Option<PathBuf>,
    /// Also write Checkstyle XML
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub checkstyle: Option<PathBuf>,
    /// Also write JUnit XML (the gate and one test suite per rule)
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub junit: Option<PathBuf>,
    /// Also write a self-contained HTML report
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub html: Option<PathBuf>,
    /// Also write a one-line metrics record (measures, grades, counts) to archive for trends
    #[arg(long, value_name = "FILE", help_heading = "Output")]
    pub summary_json: Option<PathBuf>,
    /// Also write a Bitbucket Code Insights report and annotations (report.json,
    /// annotations.json) into DIR
    #[arg(long, value_name = "DIR", help_heading = "Output")]
    pub bitbucket_insights: Option<PathBuf>,
    /// Show only new findings (text/jsonl/markdown/junit)
    #[arg(long, help_heading = "Output")]
    pub new_only: bool,
    /// Hide findings below SEV in text/jsonl/markdown/junit output
    #[arg(long, value_name = "SEV", value_parser = parse_severity, help_heading = "Output")]
    pub min_severity: Option<Severity>,
    /// Include suppressed findings in text output
    #[arg(long, help_heading = "Output")]
    pub show_suppressed: bool,
    /// Colorize text output
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, help_heading = "Output")]
    pub color: ColorChoice,

    // --- behavior -------------------------------------------------------------------------------
    /// Never fail on the gate (exit 0 unless an error occurs)
    #[arg(long, help_heading = "Behavior")]
    pub no_gate: bool,
    /// Exit 3 if any file could not be analyzed
    #[arg(long, help_heading = "Behavior")]
    pub strict: bool,
    /// Disable the analysis cache
    #[arg(long, help_heading = "Behavior")]
    pub no_cache: bool,
    /// Cache location (default: <root>/.mote/cache, or $MOTE_CACHE_DIR)
    #[arg(long, value_name = "DIR", help_heading = "Behavior")]
    pub cache_dir: Option<PathBuf>,
    /// Worker threads (default: all cores)
    #[arg(long, value_name = "N", help_heading = "Behavior")]
    pub threads: Option<usize>,
    /// Print phase and per-language timings to stderr
    #[arg(long, help_heading = "Behavior")]
    pub timings: bool,
}

#[derive(Debug, Args)]
pub struct FixArgs {
    /// Root of the working tree to fix (default: the current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Only the fixes of these rules (repeatable or comma-separated; default: every enabled rule
    /// that offers fixes)
    #[arg(long, value_name = "RULE_ID", value_delimiter = ',')]
    pub rule: Vec<String>,
    /// Apply the fixes to the files (default: print a unified diff and change nothing)
    #[arg(long)]
    pub write: bool,
    /// Only these paths: globs or path prefixes (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,
    /// Config file (default: <root>/.mote.yaml if present)
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Triage file (default: `triage.file` of the config, `.mote-triage.yaml`)
    #[arg(long, value_name = "FILE")]
    pub triage: Option<PathBuf>,
    /// Colorize the diff
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
    /// Disable the analysis cache
    #[arg(long)]
    pub no_cache: bool,
    /// Cache location (default: <root>/.mote/cache, or $MOTE_CACHE_DIR)
    #[arg(long, value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,
    /// Worker threads (default: all cores)
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,
    /// Not supported: `mote fix` works on the working tree only
    #[arg(long, value_name = "REV", hide = true)]
    pub rev: Option<String>,
    /// Not supported: `mote fix` works on the working tree only
    #[arg(long, hide = true)]
    pub staged: bool,
}

/// Output format of `gate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum GateFormat {
    Text,
    Markdown,
    Json,
}

#[derive(Debug, Args)]
pub struct GateArgs {
    /// Results file written by `mote scan --json`
    #[arg(long, value_name = "FILE")]
    pub results: PathBuf,
    /// Coverage reports: LCOV, JaCoCo XML, Cobertura XML (repeatable; globs allowed)
    #[arg(long, value_name = "GLOB")]
    pub coverage: Vec<String>,
    /// Config file with the gate settings (default: ./.mote.yaml if present)
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Evaluate only these gate conditions (repeatable or comma-separated), e.g. `coverage` in a
    /// job that adds coverage after the scan job already gated the findings
    #[arg(long, value_enum, value_name = "CONDITION", value_delimiter = ',')]
    pub only: Vec<GateCondition>,
    /// Output format
    #[arg(long, value_enum, default_value_t = GateFormat::Text)]
    pub format: GateFormat,
    /// Colorize text output
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

/// A quality-gate condition (`gate --only`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum GateCondition {
    /// New findings at or above `gate.fail_on` (all findings without a base)
    Severity,
    /// Duplication on new code (`gate.new_duplication_pct`)
    Duplication,
    /// Coverage on new code (`gate.new_coverage_pct`)
    Coverage,
    /// Worsened findings (`gate.worsened: fail`)
    Worsened,
    /// Maintainability and reliability grades (`gate.*_grade`)
    Grades,
}

impl GateCondition {
    /// Whether the engine's condition `name` is this one.
    pub fn matches(self, name: &str) -> bool {
        match self {
            GateCondition::Severity => name == "new_findings" || name == "findings",
            GateCondition::Duplication => name == "new_duplication_pct",
            GateCondition::Coverage => name == "new_coverage_pct",
            GateCondition::Worsened => name == "worsened",
            GateCondition::Grades => name.ends_with("_grade"),
        }
    }

    /// The name used on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            GateCondition::Severity => "severity",
            GateCondition::Duplication => "duplication",
            GateCondition::Coverage => "coverage",
            GateCondition::Worsened => "worsened",
            GateCondition::Grades => "grades",
        }
    }
}

/// Output format of `compare`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CompareFormat {
    Text,
    Markdown,
    Json,
}

#[derive(Debug, Args)]
pub struct CompareArgs {
    /// The older results file (`mote scan --json`)
    #[arg(value_name = "OLD")]
    pub old: PathBuf,
    /// The newer results file
    #[arg(value_name = "NEW")]
    pub new: PathBuf,
    /// Output format
    #[arg(long, value_enum, default_value_t = CompareFormat::Text)]
    pub format: CompareFormat,
    /// Findings listed per section in text and Markdown (JSON lists all)
    #[arg(long, value_name = "N", default_value_t = 50)]
    pub max_listed: usize,
    /// Write the output to FILE instead of stdout
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// Output format of `rules`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum RulesFormat {
    Text,
    Json,
    Markdown,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct RulesArgs {
    #[command(subcommand)]
    pub command: Option<RulesCommand>,
    /// Only rules of this language (e.g. java, js, mote)
    #[arg(long, value_name = "LANG")]
    pub language: Option<String>,
    /// Output format
    #[arg(long, value_enum, default_value_t = RulesFormat::Text)]
    pub format: RulesFormat,
    /// Include rules that are off by default
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Subcommand)]
pub enum RulesCommand {
    /// Description, parameters and examples of one rule
    Explain {
        #[arg(value_name = "RULE_ID")]
        rule_id: String,
        #[arg(long, value_enum, default_value_t = RulesFormat::Text)]
        format: RulesFormat,
    },
}

/// Output format of `config explain`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ExplainFormat {
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Check the configuration; exit 2 with readable errors if it is invalid
    Validate {
        /// Config file (default: ./.mote.yaml or ./.mote.yml)
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
    /// Print the JSON Schema of .mote.yaml (or, with --results, of the results JSON)
    Schema {
        /// The schema of the results JSON written by `mote scan --json` instead
        #[arg(long)]
        results: bool,
    },
    /// Effective rules, parameters and severities for a file
    Explain {
        /// File (relative to the current directory, which is the scan root)
        #[arg(value_name = "PATH")]
        path: String,
        /// Config file (default: ./.mote.yaml or ./.mote.yml)
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = ExplainFormat::Text)]
        format: ExplainFormat,
    },
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to write .mote.yaml into
    #[arg(value_name = "DIR", default_value = ".")]
    pub dir: PathBuf,
    /// Overwrite an existing configuration file
    #[arg(long)]
    pub force: bool,
    /// Only print the excludes proposed for third-party code copied into DIR (writes nothing)
    #[arg(long)]
    pub suggest_excludes: bool,
    /// Only print the `suppressions.compat` sources proposed from the suppressions of other
    /// tools found in DIR, with their counts (writes nothing)
    #[arg(long)]
    pub suggest_compat: bool,
    /// Merge the rules, severities, thresholds and ignores of another tool's configuration into
    /// the starter: `--from pmd [RULESET]` or `--from eslint [CONFIG]` (the file is looked for in
    /// DIR when not given; it is read as data, never run)
    #[arg(
        long,
        num_args = 1..=2,
        value_names = ["TOOL", "CONFIG"],
        conflicts_with_all = ["suggest_excludes", "suggest_compat"]
    )]
    pub from: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct TriageArgs {
    #[command(subcommand)]
    pub command: TriageCommand,
}

/// Where the triage file is.
#[derive(Debug, Args)]
pub struct TriageFileArgs {
    /// Triage file (default: `triage.file` of the config, `.mote-triage.yaml`)
    #[arg(long, value_name = "FILE")]
    pub file: Option<PathBuf>,
    /// Config file (default: ./.mote.yaml or ./.mote.yml)
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

/// A triage decision (`triage add --status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum TriageStatusArg {
    /// A real finding the team keeps as it is
    Accepted,
    /// The rule is wrong here
    FalsePositive,
}

/// Output format of `triage list`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum TriageFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
pub enum TriageCommand {
    /// Record a decision about one finding of a results file (replaces an earlier decision)
    Add {
        /// Finding id (12 hex digits) or fingerprint (32), as in the JSON/JSONL/SARIF output
        #[arg(value_name = "FINDING")]
        finding: String,
        /// The decision
        #[arg(long, value_enum)]
        status: TriageStatusArg,
        /// Why (required)
        #[arg(long, value_name = "TEXT")]
        reason: String,
        /// Results file written by `mote scan --json` that holds the finding
        #[arg(long, value_name = "FILE")]
        results: PathBuf,
        /// Who decided
        #[arg(long, value_name = "NAME")]
        by: Option<String>,
        /// Date of the decision (default: today, UTC)
        #[arg(long, value_name = "YYYY-MM-DD")]
        date: Option<String>,
        #[command(flatten)]
        file: TriageFileArgs,
    },
    /// List the decisions; with --results, whether each still matches a finding
    List {
        /// Results file written by `mote scan --json`
        #[arg(long, value_name = "FILE")]
        results: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = TriageFormat::Text)]
        format: TriageFormat,
        #[command(flatten)]
        file: TriageFileArgs,
    },
    /// Remove the entries a scan reported as stale (they match no finding any more)
    Prune {
        /// Results file written by `mote scan --json` with the triage file applied
        #[arg(long, value_name = "FILE")]
        results: PathBuf,
        /// Only print what would be removed
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        file: TriageFileArgs,
    },
}

/// The tool a configuration comes from (`migrate --from`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum MigrateFrom {
    /// A PMD ruleset XML file
    Pmd,
    /// An ESLint configuration: eslint.config.js (.mjs, .cjs, .ts, …), .eslintrc(.js|.cjs|.json|
    /// .yaml|.yml), package.json, or a JSON flat config
    Eslint,
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    /// The tool the configuration belongs to
    #[arg(long, value_enum)]
    pub from: MigrateFrom,
    /// The configuration file (JavaScript and TypeScript files are read statically, never run)
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
    /// Write the proposal to FILE instead of printing it
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Overwrite FILE when it exists
    #[arg(long, requires = "output")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct McpArgs {
    /// Default repository root for tools that don't name one (default: the current directory)
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_is_consistent() {
        Cli::command().debug_assert();
    }
}
