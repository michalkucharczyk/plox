//! This module builds the 'match-preview' subcommand, which helps users test their regex patterns.

use crate::data_source_cli_builder::build_data_source_cli;
use crate::graph_config::*;
use clap::{ArgMatches, Args, Command, CommandFactory, FromArgMatches, Parser};
use std::path::PathBuf;
use tracing::trace;

const LOG_TARGET: &str = "match_preview_cli_builder";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("CLI parsing error: {0}")]
	GeneralCliParseError(String),
	#[error("Missing line data source")]
	MissingLineDataSource,
	#[error("CLI parsing error: {0}")]
	GraphCliParseError(#[from] crate::data_source_cli_builder::Error),
}

#[derive(Debug)]
pub struct MatchPreviewConfig {
	pub data_source: DataSource,
	pub count: usize,
}

impl MatchPreviewConfig {
	/// Builds a `PreviewConfig` by parsing CLI arguments in the order they appear.
	pub fn try_from_matches(matches: &ArgMatches) -> Result<Self, Error> {
		trace!(target: LOG_TARGET, "try_from_matches: {:#?}", matches);

		// Process plots, events, events-counts and event-deltas
		let data_sources = DataSource::get_cli_ids();
		let mut data_source = None;
		for id in &data_sources {
			if let Some(mut plot_values) = matches.get_occurrences::<String>(id) {
				if let Some(plot_value) = plot_values.next() {
					let args: Vec<_> = plot_value.collect();
					data_source = Some(DataSource::try_from_flag(id, &args)?);
				}
			}
		}

		Ok(MatchPreviewConfig { data_source: data_source.expect("xxx"), count: 10 })
	}
}

#[derive(Parser, Debug)]
#[command(name = "dummy")]
struct DummyCliSharedMatchPreviewContext {
	#[command(flatten)]
	ctx: SharedMatchPreviewContext,
}

#[derive(Args, Debug)]
pub struct SharedMatchPreviewContext {
	/// Input file used for match preview.
	#[arg(long)]
	pub input: PathBuf,

	/// Number of lines to be matched agains the guard.
	#[arg(long, default_value_t = 5)]
	pub count: usize,

	/// The format of the timestamp which is used in logs.
	#[arg(long)]
	pub timestamp_format: Option<TimestampFormat>,

	/// Enable match preview verbose ouptut.
	#[arg(long, default_value_t = false)]
	pub verbose: bool,
}

impl SharedMatchPreviewContext {
	pub fn timestamp_format(&self) -> &TimestampFormat {
		self.timestamp_format.as_ref().unwrap_or(&DEFAULT_TIMESTAMP_FORMAT)
	}
}

/// Constructs the command-line interface (CLI) for the match preview command.
///
/// Refer to `[graph_cli_builder::build_cli]` for some more context.
pub fn build_cli() -> Command {
	let long_about = r#"
The 'match-preview' command allow to play with regex and debug matching them against the log file.

Supports:
TODO add some nice text
- ...
"#;

	let graph_cmd = Command::new("match-preview").about("... todo ...").long_about(long_about);

	let mut graph_config_cli = build_data_source_cli(graph_cmd);

	{
		let cmd = DummyCliSharedMatchPreviewContext::command();
		let args = cmd.get_arguments();

		for arg in args {
			let arg = arg.clone().help_heading("Match Preview Context");
			graph_config_cli = graph_config_cli.arg(&arg);
		}
	}

	//todo avoid copy:
	let after_help: &'static str = color_print::cstr!(
		r#"
<bold><underline>Plot Field regex</underline></bold>
Regex pattern shall contain a single capture group for matching value only, or two
capture groups for matching value and unit

Regex pattern does not match the timestamp. Timestamp will be striped and the remainder
for the log line will matched agains regex.

<underline>Examples</underline>:
- <bold>"duration"</bold>                       - matches "5s" in "duration=5s"
- <bold>"duration:([\d\.]+)(\w+)?"</bold>       - matches "5s" in log: "duration:5s"
- <bold>"^\s+(?:[\d\.]+\s+){3}([\d\.]+)"</bold> - matches 4th column (whitespace separated)
"#
	);
	graph_config_cli.after_long_help(after_help)
}

pub fn build_from_matches(
	matches: &ArgMatches,
) -> Result<(MatchPreviewConfig, SharedMatchPreviewContext), crate::error::Error> {
	let shared_graph_config =
		SharedMatchPreviewContext::from_arg_matches(matches).map_err(|e| {
			Error::GeneralCliParseError(format!(
				"SharedGraphContext Instantiation failed. This is bug. {}",
				e
			))
		})?;

	let config = MatchPreviewConfig::try_from_matches(matches)?;

	Ok((config, shared_graph_config))
}

/// Intended to be used in test.
pub fn build_from_cli_args(
	args: Vec<&'static str>,
) -> Result<(MatchPreviewConfig, SharedMatchPreviewContext), crate::error::Error> {
	let full_args: Vec<_> = ["graph"].into_iter().chain(args).collect();
	let matches = build_cli().try_get_matches_from(full_args.clone()).unwrap();
	build_from_matches(&matches)
}

#[cfg(test)]
mod tests {
	// use crate::logging::init_tracing_test;
	//
	// use super::*;
	// use std::path::Path;

	// #[test]
	// fn test_01() {
	// 	check_ok(
	// 		vec!["--plot", "c1", "d"],
	// 		"test-files/config01.toml",
	// 		GraphConfigBuilder::new()
	// 			.with_default_panel()
	// 			.with_line(
	// 				LineBuilder::new()
	// 					.with_plot_field_line(Some("c1".into()), "d".into())
	// 					.build()
	// 					.unwrap(),
	// 			)
	// 			.build(),
	// 	);
	// }
}
