use crate::graph_config::*;
use clap::{Arg, ArgAction, ArgMatches, Args, Command, CommandFactory, FromArgMatches, Parser};
use std::path::PathBuf;
use tracing::trace;

const LOG_TARGET: &str = "match_preview_cli_builder";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("CLI parsing error: {0}")]
	GeneralCliParseError(String),
	#[error("Missing line data source")]
	MissingLineDataSource,
	// should extract DataSource parsing errors into separate type
	#[error("Nested error: {0}")]
	NestedCliError(#[from] crate::graph_cli_builder::Error),
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
			if let Some(plot_values) = matches.get_occurrences::<String>(id) {
				for plot_value in plot_values {
					let args: Vec<_> = plot_value.collect();
					data_source = Some(DataSource::try_from_flag(id, &args)?);
					break;
				}
			}
		}

		Ok(MatchPreviewConfig { data_source: data_source.expect("xxx"), count: 10 })
	}
}

fn extract_help_multiline(args: &[Arg]) -> String {
	args.iter()
		.filter_map(|arg| {
			arg.get_long_help()
				.or(arg.get_help())
				.map(|h| format!("  <{}>: {}", arg.get_id(), h))
		})
		.collect::<Vec<_>>()
		.join("\n")
}

fn extract_num_args_and_names(args: &[Arg]) -> (usize, usize, Vec<String>) {
	let mut value_names = vec![];
	let mut required_count = 0;

	for a in args {
		value_names.push(a.get_id().to_string());
		if a.is_required_set() {
			required_count += 1;
		}
	}

	let total = args.len();
	(required_count, total, value_names)
}

/// Build args from subcommands' parameters and append to given base command
fn build_data_source_cli(mut base: Command) -> Command {
	let dummy_data_source_subcommand = DummyDataSourceSubcommand::command();
	for sub in dummy_data_source_subcommand.get_subcommands() {
		let sub_name = sub.get_name().to_string();
		let sub_args: Vec<Arg> = sub.get_arguments().cloned().collect();
		let sub_help = sub.get_about().unwrap_or_default();
		let field_help = extract_help_multiline(&sub_args);
		let (min_args, max_args, value_names) = extract_num_args_and_names(&sub_args);

		let full_help = if field_help.is_empty() {
			sub_help.to_string()
		} else {
			format!("{sub_help}\n{field_help}\n")
		};

		let flag = Arg::new(sub_name.clone())
			.long(&sub_name)
			.num_args(min_args..=max_args)
			.action(ArgAction::Append)
			.value_names(&value_names)
			.help(sub_help.to_string())
			.long_help(full_help)
			.next_line_help(true)
			.help_heading("Data Sources");

		base = base.arg(flag);
	}

	base
}

/// Dummy helper wrapper for `CommandFactory`
///
/// Used for injecting [`DataSource`] args and their parameters.
#[derive(Parser, Debug)]
#[command(name = "dummy")]
struct DummyDataSourceSubcommand {
	#[command(subcommand)]
	line: DataSource,
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
		SharedMatchPreviewContext::from_arg_matches(&matches).map_err(|e| {
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
	let full_args: Vec<_> = ["graph"].into_iter().chain(args.into_iter()).collect();
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
