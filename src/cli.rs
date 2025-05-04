//! This tiny module defines the overall command-line interface for plox.
//! It sets up the top-level argument parser, wires in the subcommands, and handles user input.

use crate::graph_config::{DataSource, EventDeltaSpec, FieldCaptureSpec, InputFilesContext};
use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
	/// Global verbosity (-v , -vv)
	///
	/// Levels:
	///  - info enabled by default
	///  - -v for debug
	///  - -vv for trace
	#[arg(short = 'v', long, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Quiet mode, no output.
	#[arg(short = 'q', long, action = clap::ArgAction::SetTrue, default_value_t = false)]
	pub quiet: bool,

	#[command(subcommand)]
	pub command: CliCommand,
}

pub const EXTRA_HELP: &str = color_print::cstr!(
	r#"
<bold><underline>Line matching:</underline></bold>
- Firstly, if an expression is provided by the user, the guard is used to quickly filter out non-matching lines by comparing it with the line using strcmp.
- Secondly, the timestamp pattern is used to extract the timestamp.
- Thirdly, the field/pattern regex is applied.

Try `plox match-preview --verbose` to debug matching issues.

<bold><underline>Timestamp format:</underline></bold>
The tool is designed to parse timestamped logs. The timestamp format used in the log file shall be passed as the `--timestamp-format` parameter.

For the the exact format specifiers refer to: https://docs.rs/chrono/latest/chrono/format/strftime/index.html

<underline>Examples</underline>:
- "2025-04-03 11:32:48.027"  | "%Y-%m-%d %H:%M:%S%.3f"
- "08:26:13 AM"              | "%I:%M:%S %p"
- "2025 035 08:26:13 AM"     | "%Y %j %I:%M:%S %p"
- "035 08:26:13 AM"          | "%j %I:%M:%S %p"
- "[1577834199]"             | "[%s]"
- "1577834199"               | "%s"
- "Apr 20 08:26:13 AM"       | "%b %d %I:%M:%S %p"
- "[100.333]"                | not supported...

<bold><underline>Field regex:</underline></bold>
Regex pattern shall contain a single capture group for matching value only, or two
capture groups for matching value and unit.

Currently only time-related units are implemented (s,ms,us,ns) and all values are converted to miliseconds.
If catpure group for units is not provided, no conversion is made.

Regex pattern does not match the timestamp. Timestamp will be striped and the remainder
for the log line will matched against regex.

<underline>Examples</underline>:
- "duration"                       | matches "5s" in "duration=5s"
- "\bduration:([\d\.]+)(\w+)?"     | matches "5s" in log: "duration:5s"
- "\bvalue:([\d\.]+)?"             | matches "75" in log: "value:75" (no units)
- "^\s+(?:[\d\.]+\s+){3}([\d\.]+)" | matches 4th column (whitespace separated)
- "txs=\(\d+,\s+(\d+)\)"           | matches '124' in "txs=(99,124)
"#
);

#[derive(Debug, Subcommand)]
pub enum CliCommand {
	Stat(StatArgs),
	Cat(CatArgs),
}

/// Represents the different ways a line's data can be sourced from logs in order to display some stats.
#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum StatDataSource {
	/// Extract the time delta between consecutive occurrences of `pattern`.
	EventDelta(EventDeltaSpec),

	/// Extract a numeric field from logs.
	///
	/// This is the most common data source type.
	FieldValue(FieldCaptureSpec),
}

impl From<StatDataSource> for DataSource {
	fn from(value: StatDataSource) -> Self {
		match value {
			StatDataSource::FieldValue(spec) => DataSource::FieldValue(spec),
			StatDataSource::EventDelta(spec) => DataSource::EventDelta(spec),
		}
	}
}

/// Display extracted values only.
#[derive(Debug, Args)]
pub struct CatArgs {
	#[clap(flatten)]
	pub input_files_ctx: InputFilesContext,

	#[command(subcommand)]
	pub command: StatDataSource,
}

/// Display stats and histogram for extracted data.
#[derive(Debug, Args)]
pub struct StatArgs {
	#[clap(flatten)]
	pub input_files_ctx: InputFilesContext,

	/// Histogram buckets count
	#[arg(long, default_value_t = 10)]
	pub buckets_count: u64,

	/// Float precision and width to be used when printing bucket range
	#[clap(long, num_args = 2)]
	pub precision: Vec<usize>,

	#[command(subcommand)]
	pub command: StatDataSource,
}

pub fn build_cli() -> clap::Command {
	Cli::command()
		.subcommand(crate::graph_cli_builder::build_cli())
		.subcommand(crate::match_preview_cli_builder::build_cli())
}
