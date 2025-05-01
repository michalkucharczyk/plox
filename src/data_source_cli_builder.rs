//! Common utils for building data source related args and parsing them.

use std::{
	num::{ParseFloatError, ParseIntError},
	str::ParseBoolError,
};

use crate::graph_config::*;
use clap::{Arg, ArgAction, Command, CommandFactory, Parser};

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("CLI parsing error: {0}")]
	GeneralCliParseError(String),
	#[error("Parse int error: {0}")]
	ParseIntError(#[from] ParseIntError),
	#[error("Parse int error: {0}")]
	ParseBoolError(#[from] ParseBoolError),
	#[error("Parse float error: {0}")]
	ParseFloatError(#[from] ParseFloatError),
}

impl DataSource {
	//clenaup this mess
	const CLI_NAME_PLOT_FIELD: &str = "plot";
	const CLI_NAME_EVENT: &str = "event";
	const CLI_NAME_EVENT_COUNT: &str = "event-count";
	const CLI_NAME_EVENT_DELTA: &str = "event-delta";

	pub fn get_cli_ids() -> Vec<String> {
		DummyDataSourceSubcommand::command()
			.get_subcommands()
			.map(|sc| sc.get_name().to_string().clone())
			.collect()
	}
}

impl DataSource {
	// note:
	// we cannot use TypedValueParser (what would be cute) for DataSource because parse_ref is only
	// receiving a single parameter from: '--plot x y' invocation (by desing in clap), so we cannot
	// build a right instance.
	//
	// This could be worked around by specifying '--plot "x y"' but it is not convinient.
	// So manual parsing is required.
	pub fn try_from_flag(id: &str, val: &[&String]) -> Result<Self, Error> {
		Ok(match id {
			Self::CLI_NAME_EVENT => match val.len() {
				2 => DataSource::EventValue {
					guard: None,
					pattern: val[0].to_string(),
					yvalue: val[1].parse::<f64>()?,
				},
				3 => DataSource::EventValue {
					guard: Some(val[0].to_string()),
					pattern: val[1].to_string(),
					yvalue: val[2].parse::<f64>()?,
				},
				_ => {
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					)));
				},
			},
			Self::CLI_NAME_PLOT_FIELD => match val.len() {
				1 => DataSource::FieldValue { guard: None, field: val[0].to_string() },
				2 => DataSource::FieldValue {
					guard: Some(val[0].to_string()),
					field: val[1].to_string(),
				},
				_ => {
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					)));
				},
			},
			Self::CLI_NAME_EVENT_COUNT => match val.len() {
				1 => DataSource::EventCount { guard: None, pattern: val[0].to_string() },
				2 => DataSource::EventCount {
					guard: Some(val[0].to_string()),
					pattern: val[1].to_string(),
				},
				_ => {
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					)));
				},
			},
			Self::CLI_NAME_EVENT_DELTA => match val.len() {
				1 => DataSource::EventDelta { guard: None, pattern: val[0].to_string() },
				2 => DataSource::EventDelta {
					guard: Some(val[0].to_string()),
					pattern: val[1].to_string(),
				},
				_ => {
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					)));
				},
			},
			_ => {
				return Err(Error::GeneralCliParseError(format!(
					"Unknown DataSource id:{}. This is bug",
					id
				)));
			},
		})
	}
}

/// Dummy helper wrapper for `CommandFactory`
///
/// Used for injecting DataSource args and their parameters.
#[derive(Parser, Debug)]
#[command(name = "dummy")]
pub struct DummyDataSourceSubcommand {
	#[command(subcommand)]
	line: DataSource,
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

/// Build args from DataSource subcommands' parameters and append to given base command
pub fn build_data_source_cli(mut base: Command) -> Command {
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
			.help_heading("Data sources - plotted line types");

		base = base.arg(flag);
	}

	base
}
