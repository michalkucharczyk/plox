use crate::graph_config::*;
use clap::{
	value_parser, Arg, ArgAction, ArgMatches, Command, CommandFactory, FromArgMatches, Parser,
	ValueEnum,
};
use serde::{Deserialize, Serialize};
use std::{
	collections::BTreeMap,
	num::{ParseFloatError, ParseIntError},
	path::{Path, PathBuf},
	str::{FromStr, ParseBoolError},
};
use tracing::{error, trace};

const LOG_TARGET: &str = "graph_cli_builder";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Parse int error: {0}")]
	ParseIntError(#[from] ParseIntError),
	#[error("Parse int error: {0}")]
	ParseBoolError(#[from] ParseBoolError),
	#[error("Parse float error: {0}")]
	ParseFloatError(#[from] ParseFloatError),
	#[error("CLI parsing error: {0}")]
	GeneralCliParseError(String),
	#[error("Unknown panel param {0:?}")]
	UnknownPanelParam(String),
	#[error("Invalid line source {0:?}")]
	InvalidLineSource(String),
	#[error("Missing line data source")]
	MissingLineDataSource,
	#[error("Unknown line param {0:?}")]
	UnknownLineParam(String),
}

impl From<String> for Error {
	fn from(error: String) -> Self {
		Error::GeneralCliParseError(error)
	}
}

/// Helper for deserializing a GraphConfig which may contain extra options from
/// [`SharedGraphContext`]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphConfigWithContext {
	#[serde(flatten)]
	pub config: GraphConfig,
	#[serde(flatten)]
	pub context: SharedGraphContext,
}

impl GraphConfigWithContext {
	pub fn load_from_file(path: &Path) -> Result<Self, crate::error::Error> {
		let content = std::fs::read_to_string(path).map_err(|error| {
			error!(?error, "Reading toml error");
			crate::error::Error::IoError(format!("{}", path.display()), error)
		})?;
		toml::from_str(&content).map_err(|e| {
			let r = annotate_toml_error(&e, &content, &path.display().to_string());
			error!("{r}");
			e.into()
		})
	}
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
				_ =>
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					))),
			},
			Self::CLI_NAME_PLOT_FIELD => match val.len() {
				1 => DataSource::FieldValue { guard: None, field: val[0].to_string() },
				2 => DataSource::FieldValue {
					guard: Some(val[0].to_string()),
					field: val[1].to_string(),
				},
				_ =>
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					))),
			},
			Self::CLI_NAME_EVENT_COUNT => match val.len() {
				1 => DataSource::EventCount { guard: None, pattern: val[0].to_string() },
				2 => DataSource::EventCount {
					guard: Some(val[0].to_string()),
					pattern: val[1].to_string(),
				},
				_ =>
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					))),
			},
			Self::CLI_NAME_EVENT_DELTA => match val.len() {
				1 => DataSource::EventDelta { guard: None, pattern: val[0].to_string() },
				2 => DataSource::EventDelta {
					guard: Some(val[0].to_string()),
					pattern: val[1].to_string(),
				},
				_ =>
					return Err(Error::GeneralCliParseError(format!(
						"Bad parameter count ({}) for {}. This is bug.",
						val.len(),
						id
					))),
			},
			_ =>
				return Err(Error::GeneralCliParseError(format!(
					"Unknown DataSource id:{}. This is bug",
					id
				))),
		})
	}
}

/// A builder for incrementally constructing a [`Line`].
///
/// This builder allows you to specify the line's data source via [`DataSource`]
/// (e.g. [`DataSource::FieldValue`], [`DataSource::EventValue`]) and then apply
/// styling or configuration parameters (e.g. color, axis) via [`LineParams`].
#[derive(Debug, Default)]
pub struct LineBuilder {
	/// Optional core data source for the line.
	line: Option<DataSource>,
	params: LineParams,
}

impl LineBuilder {
	/// Create a new empty builder.
	fn new() -> Self {
		Self { ..Self::default() }
	}

	/// Set the data source for this line to a particular [`DataSource`].
	///
	/// This overwrites any previously set data source.
	fn line(mut self, data_source: DataSource) -> Self {
		self.line = Some(data_source);
		self
	}

	/// Apply one styling/config parameter.
	///
	/// This can be called multiple times to layer parameters. If a parameter
	/// is duplicated, the last call wins.
	fn apply_param(mut self, param: LineParam) -> Self {
		match param {
			LineParam::LineColor(c) => self.params.line_color = Some(c),
			LineParam::YAxis(y) => self.params.yaxis = Some(y),
			LineParam::MarkerType(mt) => self.params.marker_type = Some(mt),
			LineParam::MarkerColor(mc) => self.params.marker_color = Some(mc),
			LineParam::InputFileName(name) => self.params.file_name = Some(name),
			LineParam::InputFileId(id) => self.params.file_id = Some(id),
			LineParam::PlotStyle(style) => self.params.style = style,
			LineParam::LineWidth(w) => self.params.line_width = Some(w),
			LineParam::MarkerSize(w) => self.params.marker_size = w,
			LineParam::DashStyle(s) => self.params.dash_style = Some(s),
			LineParam::Title(s) => self.params.title = Some(s),
		}
		self
	}

	/// Finalize and return the fully constructed [`Line`], if a data source was set.
	///
	/// Returns [`None`] if no [`DataSource`] was specified.
	fn build(self) -> Result<Line, Error> {
		if self.params.file_name.is_some() && self.params.file_id.is_some() {
			return Err(Error::InvalidLineSource(format!(
				"file-name {} and file-id {} cannot be used together.",
				self.params.file_name.unwrap().display(),
				self.params.file_id.unwrap()
			)));
		}
		self.line
			.ok_or(Error::MissingLineDataSource)
			.map(|data_source| Line { data_source, params: self.params })
	}
}

/// A builder for incrementally constructing a [`Panel`].
///
/// This builder allows to specify configuration parameters via applying [`PanelParams`].
#[derive(Debug, Default)]
pub struct PanelBuilder {
	lines: Vec<Line>,
	params: PanelParams,
}

impl PanelBuilder {
	/// Create a new empty builder.
	fn new() -> Self {
		Self::default()
	}

	/// Apply one styling/config parameter.
	///
	/// This can be called multiple times to set different parameters. If a parameter
	/// is defined multiple times, the last value takes precedence.
	fn apply_param(mut self, param: PanelParam) -> Self {
		match param {
			PanelParam::PanelTitle(t) => self.params.panel_title = Some(t),
			PanelParam::Height(h) => self.params.height = Some(h),
			PanelParam::YAxisScale(ys) => self.params.yaxis_scale = Some(ys),
			PanelParam::Legend(l) => self.params.legend = Some(l),
			PanelParam::TimeRangeMode(r) => self.params.time_range_mode = Some(r),
		}
		self
	}

	/// Sets lines contained within panel.
	fn with_lines(mut self, lines: Vec<Line>) -> Self {
		self.lines = lines.clone();
		self
	}

	/// Finalize and return the constructed [`Panel`].
	fn build(self) -> Panel {
		Panel { lines: self.lines, params: self.params }
	}
}

#[derive(Debug)]
enum Event {
	NewPanel,
	NewLine(DataSource),
	ApplyLineParam(LineParam),
	ApplyPanelParam(PanelParam),
}

/// Represents a styling or configuration parameter that can be applied to a line.
///
/// These parameters do not change the data source; rather, they adjust how the line
/// is drawn (color, axis, marker style), or from which log file the data is taken.
///
/// Intended to be used while parsing the command line, for event based config building.
#[derive(Debug, PartialEq)]
enum LineParam {
	/// See: [`LineParams::file_name`]
	InputFileName(PathBuf),

	/// See: [`LineParams::title`]
	Title(String),

	/// See: [`LineParams::file_id`]
	InputFileId(usize),

	/// See: [`LineParams::style`]
	PlotStyle(PlotStyle),

	/// See: [`LineParams::line_width`]
	LineWidth(LineWidth),

	/// See: [`LineParams::line_color`]
	LineColor(Color),

	/// See: [`LineParams::dash_style`]
	DashStyle(DashStyle),

	/// See: [`LineParams::yaxis`]
	YAxis(YAxis),

	/// See: [`LineParams::marker_type`]
	MarkerType(MarkerType),

	/// See: [`LineParams::marker_color`]
	MarkerColor(Color),

	/// See: [`LineParams::marker_size`]
	MarkerSize(MarkerSize),
}

impl LineParam {
	fn from_flag(flag: &str, val: &[String]) -> Result<Self, Error> {
		Ok(match flag {
			"title" => Self::Title(val[0].clone()),
			"file_name" => Self::InputFileName(PathBuf::from(&val[0])),
			"file_id" => Self::InputFileId(val[0].parse::<usize>()?),
			"style" => Self::PlotStyle(<PlotStyle as ValueEnum>::from_str(&val[0], false)?),
			"line_width" => Self::LineWidth(LineWidth::from_str(&val[0])?),
			"line_color" => Self::LineColor(<Color as ValueEnum>::from_str(&val[0], false)?),
			"dash_style" => Self::DashStyle(<DashStyle as ValueEnum>::from_str(&val[0], false)?),
			"yaxis" => Self::YAxis(YAxis::from_str(&val[0], false)?),
			"marker_type" => Self::MarkerType(<MarkerType as ValueEnum>::from_str(&val[0], false)?),
			"marker_color" => Self::MarkerColor(<Color as ValueEnum>::from_str(&val[0], false)?),
			"marker_size" => Self::MarkerSize(MarkerSize::from_str(&val[0])?),
			_ => Err(Error::UnknownLineParam(flag.to_string()))?,
		})
	}
}

#[derive(Debug, PartialEq)]
enum PanelParam {
	/// See: [`PanelParams::panel_title`]
	PanelTitle(String),

	/// See: [`PanelParams::height`]
	Height(f64),

	/// See: [`PanelParams::yaxis_scale`]
	YAxisScale(AxisScale),

	/// See: [`PanelParams::legend`]
	Legend(bool),

	/// See: [`PanelParams::time_range_mode`]
	TimeRangeMode(PanelRangeMode),
}

impl PanelParam {
	fn from_flag(flag: &str, val: &[String]) -> Result<Self, Error> {
		Ok(match flag {
			"panel_title" => Self::PanelTitle(val[0].to_string()),
			"height" => Self::Height(val[0].parse::<f64>()?),
			"yaxis_scale" => Self::YAxisScale(AxisScale::from_str(&val[0], false)?),
			"legend" => Self::Legend(val[0].parse::<bool>()?),
			"time_range_mode" => Self::TimeRangeMode(PanelRangeMode::from_str(&val[0], false)?),
			_ => Err(Error::UnknownPanelParam(flag.to_string()))?,
		})
	}
}

impl GraphConfig {
	fn parse_params_for_command<F>(
		command: Command,
		matches: &ArgMatches,
		mut build_event: F,
	) -> Result<(), Error>
	where
		F: FnMut(usize, &str, &[String]) -> Result<(), Error>,
	{
		// Process each line parameter flag
		let line_args_ids = {
			let arg_ids: Vec<_> = command.get_arguments().map(|arg| arg.get_id().clone()).collect();
			arg_ids
		};

		for id in line_args_ids {
			trace!(target: LOG_TARGET, "processing id: {:?}", id);
			if let Some(values) = matches.get_raw_occurrences(id.as_str()) {
				let entries = matches.indices_of(id.as_str()).unwrap();
				for (index, val) in entries.zip(values.clone()) {
					let param_args = val.into_iter().try_fold(Vec::new(), |mut acc, s| {
						let converted = s
							.to_str()
							.ok_or_else(|| {
								Error::GeneralCliParseError(format!(
									"Params string conversion (?) mess: {:?}",
									values
								))
							})?
							.to_string();
						acc.push(converted);
						Ok::<Vec<String>, Error>(acc)
					})?;

					build_event(index, id.as_str(), &param_args[..])?;
				}
			}
		}
		Ok(())
	}

	/// Builds a `GraphConfig` by parsing CLI arguments in the order they appear.
	///
	/// This function enables flexible, order-sensitive CLI composition by:
	/// - Tracking user-provided arguments as logical **events** (e.g. `--panel`, `--plot`)
	/// - Preserving original argument order using match indices
	/// - Incrementally constructing panels and lines using a builder-style approach
	///
	/// This is necessary because Clap alone cannot support ordered, repeated, multi-flag patterns
	/// like: `--plot ... --panel --event ... --plot ...`.  
	/// By interpreting arguments as a linear sequence of graphing instructions,
	/// this method supports expressive CLI layouts without sacrificing ergonomics or structure.
	///
	/// Used internally to construct a `GraphConfig` from `clap::ArgMatches`.
	pub fn try_from_matches(matches: &ArgMatches) -> Result<Self, Error> {
		let mut events: BTreeMap<usize, Event> = BTreeMap::new();

		trace!(target: LOG_TARGET, "try_from_matches: {:#?}", matches);

		// Index panels
		if let Some(indices) = matches.indices_of("panel") {
			trace!(target: LOG_TARGET, "panel indices: {:#?}", indices);
			for i in indices {
				events.insert(i, Event::NewPanel);
			}
		}

		// Process plots, events, events-counts and event-deltas
		let all_data_sources = DataSource::get_cli_ids();
		for id in &all_data_sources {
			if let Some(plot_values) = matches.get_occurrences::<String>(id) {
				let mut indices = matches.indices_of(id).unwrap();
				for plot_value in plot_values {
					let args: Vec<_> = plot_value.collect();

					let args_len = args.len();
					let index = indices.nth(args_len - 1).unwrap();
					events.insert(index, Event::NewLine(DataSource::try_from_flag(id, &args)?));
				}
			}
		}

		Self::parse_params_for_command(
			DummyCliLineArgs::command(),
			matches,
			|index, id, param_args| -> Result<(), Error> {
				let param = LineParam::from_flag(id, param_args)?;
				events.insert(index, Event::ApplyLineParam(param));
				Ok(())
			},
		)?;

		Self::parse_params_for_command(
			DummyCliPanelArgs::command(),
			matches,
			|index, id, param_args| -> Result<(), Error> {
				let param = PanelParam::from_flag(id, param_args)?;
				events.insert(index, Event::ApplyPanelParam(param));
				Ok(())
			},
		)?;

		//todo: could be refactored to some nicer flow.
		let mut panels = vec![];
		let mut current_lines = vec![];
		let mut current_line_builder: Option<LineBuilder> = None;
		let mut current_panel_builder: Option<PanelBuilder> = Some(PanelBuilder::new());

		trace!(target: LOG_TARGET, ?events, "building graph config");
		for (_, event) in events {
			match event {
				Event::NewPanel => {
					if let Some(line) = current_line_builder.take().map(|b| b.build()) {
						current_lines.push(line?);
					}

					if let Some(panel_builder) = current_panel_builder.take() {
						panels.push(panel_builder.with_lines(current_lines).build());
						current_panel_builder = Some(PanelBuilder::new());
					}

					current_lines = vec![];
				},
				Event::NewLine(data_source) => {
					if let Some(line) = current_line_builder.take().map(|b| b.build()) {
						current_lines.push(line?);
					}
					current_line_builder = Some(LineBuilder::new().line(data_source));
				},
				Event::ApplyLineParam(param) =>
					if let Some(builder) = current_line_builder {
						current_line_builder = Some(builder.apply_param(param))
					} else {
						return Err(Error::GeneralCliParseError(format!(
							"Line parameter {:?} has no associated line.",
							param
						)));
					},
				Event::ApplyPanelParam(param) =>
					if let Some(builder) = current_panel_builder {
						current_panel_builder = Some(builder.apply_param(param))
					} else {
						return Err(Error::GeneralCliParseError(format!(
							"Panel parameter {:?} has no associated panel.",
							param
						)));
					},
			}
		}

		if let Some(line) = current_line_builder.take().map(|b| b.build()) {
			current_lines.push(line?);
		}

		if !current_lines.is_empty() {
			if let Some(panel_builder) = current_panel_builder.take() {
				panels.push(panel_builder.with_lines(current_lines).build());
			} else {
				return Err(Error::GeneralCliParseError(
					"No panel builder left? Logic error.".into(),
				));
			}
		}

		Ok(GraphConfig { panels })
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
			.help_heading("Data sources - plotted line types");

		base = base.arg(flag);
	}

	base
}

/// Dummy helper wrapper for `CommandFactory`
///
/// Used for injecting DataSource args and their parameters.
#[derive(Parser, Debug)]
#[command(name = "dummy")]
struct DummyDataSourceSubcommand {
	#[command(subcommand)]
	line: DataSource,
}

/// Dummy helper wrapper for `CommandFactory`
///
/// Used for injecting line parameters args.
#[derive(Parser, Debug)]
#[command(name = "dummy")]
struct DummyCliLineArgs {
	#[command(flatten)]
	line_args: LineParams,
}

/// Dummy helper wrapper for `CommandFactory`
///
/// Used for injecting panel parameters args.
#[derive(Parser, Debug)]
#[command(name = "dummy")]
struct DummyCliPanelArgs {
	#[command(flatten)]
	panel_args: PanelParams,
}

#[derive(Parser, Debug)]
#[command(name = "dummy")]
struct DummyCliSharedGraphContext {
	#[command(flatten)]
	ctx: SharedGraphContext,
}

/// Constructs the command-line interface (CLI) for the graph command.
///
/// This CLI setup uses a custom strategy to reuse argument definitions and documentation
/// from `clap`-derived enums and structs, while building a flat, flag-based CLI interface.
///
/// - [`DataSource`] is defined as an enum with `#[derive(Subcommand)]`, where each variant (e.g.
///   `EventValue`, `PlotField`) holds documented arguments.
/// - We extract the auto-generated `Command` from Clap using `.command()` and pull out each
///   subcommand’s fields (clap `Arg`s).
/// - These are restructured into regular `--flag <args>` format by preserving:
///   - Help text (`.get_help()`)
///   - Field names as value names (`.get_id()`)
///   - Required/optional status to determine `num_args`
///
/// We also extract argument definitions from additional `#[derive(Args)]` structs (like
/// [`LineParams`]) and inject them into the final `Command` using the same technique.
///
/// ## Why this is needed:
/// - Clap does not support using enums directly for `--flag <args>` style flags.
/// - It also cannot handle repeated flags (e.g. `--plot ... --panel --plot ... --event ...`) in a
///   way that preserves **argument order**, which is important for many use cases like sequential
///   log analysis or layered graphing.
/// - Structs alone cannot express multiple positional groups, or interleaved repeated arguments.
/// - We work around this by:
///   - Using `ArgAction::Append` to collect repeated values
///   - Manually tracking CLI argument **positions** (via `matches.indices_of(...)`) to reconstruct
///     the original user input order (see [`GraphConfig::try_from_matches`]).
///
/// This pattern avoids duplication of documentation, keeps CLI definitions clean,
/// and enables flexible composition of arguments from multiple sources.
pub fn build_cli() -> Command {
	let long_about = r#"
The 'graph' command parses timestamped log files and plots numeric fields, regex captures, events, or deltas over time.

Supports:
- Regex-based value extraction,
- Named fields with optional guards,
- Multiple panels and file-aware layouts.
"#;

	let graph_cmd = Command::new("graph")
		.about("Extract and plot structured data from logs.")
		.long_about(long_about);

	let mut graph_config_cli = build_data_source_cli(graph_cmd);

	// merge all line arguments [`LineParams`]
	{
		let cmd = DummyCliLineArgs::command();
		let args = cmd.get_arguments();

		for arg in args {
			let arg = arg.clone().action(ArgAction::Append).help_heading("Line Options");
			graph_config_cli = graph_config_cli.arg(&arg);
		}
	}
	{
		let cmd = DummyCliPanelArgs::command();
		let args = cmd.get_arguments();

		for arg in args {
			let arg = arg.clone().action(ArgAction::Append).help_heading("Panel Options");
			graph_config_cli = graph_config_cli.arg(&arg);
		}
	}

	{
		let cmd = DummyCliSharedGraphContext::command();
		let args = cmd.get_arguments();

		for arg in args {
			let arg = arg.clone();
			graph_config_cli = graph_config_cli.arg(&arg);
		}
	}

	let graph_config_cli = graph_config_cli
		.arg(
			// Note: flags don't track the position of each occurrence, so we need to emulate
			// flags with value-less options to get the same result.
			Arg::new("panel")
				.long("panel")
				.value_parser(value_parser!(bool))
				.default_missing_value("true")
				.action(ArgAction::Append)
				.num_args(0)
				.help_heading("Panel Options")
				.help("Add new panel to graph"),
		)
		.arg(
			Arg::new("config")
				.long("config")
				.short('c')
				.value_name("FILE")
				.help_heading("Input files")
				.help("Path to TOML config file containing panels layout."),
		);
	let after_help: &'static str = color_print::cstr!(
		r#"
<bold><underline>Field regex:</underline></bold>
Regex pattern shall contain a single capture group for matching value only, or two
capture groups for matching value and unit.

Regex pattern does not match the timestamp. Timestamp will be striped and the remainder
for the log line will matched against regex.

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
) -> Result<(GraphConfig, SharedGraphContext), crate::error::Error> {
	let mut shared_graph_config = SharedGraphContext::from_arg_matches(matches).map_err(|e| {
		Error::GeneralCliParseError(format!(
			"SharedGraphContext Instantiation failed. This is bug. {}",
			e
		))
	})?;

	let config = if let Some(config_path) = matches.get_one::<String>("config") {
		let GraphConfigWithContext { config, context } =
			GraphConfigWithContext::load_from_file(Path::new(config_path))?;
		shared_graph_config.merge_with_other(context);
		config
	} else {
		GraphConfig::try_from_matches(matches)?
	};

	Ok((config, shared_graph_config))
}

/// Intended to be used in test.
#[cfg(test)]
pub fn build_from_cli_args(
	args: Vec<&'static str>,
) -> Result<(GraphConfig, SharedGraphContext), crate::error::Error> {
	let full_args: Vec<_> = ["graph"].into_iter().chain(args.into_iter()).collect();
	let matches = build_cli().try_get_matches_from(full_args.clone()).unwrap();
	build_from_matches(&matches)
}

#[cfg(test)]
mod tests {
	use crate::logging::init_tracing_test;

	use super::*;
	use std::path::Path;

	pub struct GraphConfigBuilder {
		panels: Vec<Panel>,
		current_panel: Option<Panel>,
	}

	impl GraphConfigBuilder {
		pub fn new() -> Self {
			GraphConfigBuilder { panels: Vec::new(), current_panel: None }
		}

		pub fn with_panel(mut self, panel: Panel) -> Self {
			if let Some(panel) = self.current_panel.take() {
				self.panels.push(panel);
			}
			self.current_panel = Some(panel);
			self
		}

		pub fn with_default_panel(mut self) -> Self {
			if let Some(panel) = self.current_panel.take() {
				self.panels.push(panel);
			}
			self.current_panel = Some(Panel { lines: Vec::new(), params: Default::default() });
			self
		}

		pub fn with_line(mut self, line: Line) -> Self {
			if let Some(ref mut panel) = self.current_panel {
				panel.lines.push(line);
			} else {
				// If there's no current panel, start a new one and add the line
				self.current_panel = Some(Panel { lines: vec![line], params: Default::default() });
			}
			self
		}

		pub fn build(mut self) -> GraphConfig {
			if let Some(panel) = self.current_panel {
				self.panels.push(panel);
			}
			GraphConfig { panels: self.panels }
		}
	}

	impl LineBuilder {
		pub fn with_event_count_line(mut self, guard: Option<String>, pattern: String) -> Self {
			self.line = Some(DataSource::EventCount { guard, pattern });
			self
		}

		pub fn with_event_value_line(
			mut self,
			guard: Option<String>,
			pattern: String,
			yvalue: f64,
		) -> Self {
			self.line = Some(DataSource::EventValue { guard, pattern, yvalue });
			self
		}

		pub fn with_plot_field_line(mut self, guard: Option<String>, field: String) -> Self {
			self.line = Some(DataSource::FieldValue { guard, field });
			self
		}
	}

	#[test]
	fn test_01() {
		check_ok(
			vec!["--plot", "c1", "d"],
			"test-files/config01.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("c1".into()), "d".into())
						.build()
						.unwrap(),
				)
				.build(),
		);
	}
	#[test]
	fn test_02() {
		check_ok(
			vec!["--event-count", "d"],
			"test-files/config02.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_event_count_line(None, "d".into()).build().unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_03() {
		check_ok(
			vec!["--event-count", "c1", "d"],
			"test-files/config03.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_event_count_line(Some("c1".into()), "d".into())
						.build()
						.unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_04() {
		check_ok(
			vec!["--event", "d", "101.1"],
			"test-files/config04.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_event_value_line(None, "d".into(), 101.1f64)
						.build()
						.unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_05() {
		check_ok(
			vec!["--event", "c1", "d", "101.1"],
			"test-files/config05.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_event_value_line(Some("c1".into()), "d".into(), 101.1f64)
						.build()
						.unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_06() {
		check_ok(
			vec!["--plot", "c1", "d", "--plot", "xxx"],
			"test-files/config06.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("c1".into()), "d".into())
						.build()
						.unwrap(),
				)
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "xxx".into()).build().unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_07() {
		check_ok(
			vec![
				"--plot", "1", "--panel", "--plot", "2", "--panel", "--plot", "3", "--panel",
				"--plot", "4",
			],
			"test-files/config07.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "1".into()).build().unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "2".into()).build().unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "3".into()).build().unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "4".into()).build().unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_08() {
		check_ok(
			vec![
				"--plot", "c1", "d", "--plot", "x", "y", "--panel", "--plot", "1", "A", "--plot",
				"2", "--panel", "--plot", "3", "--plot", "4", "B", "--panel", "--plot", "5",
				"--plot", "6",
			],
			"test-files/config08.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("c1".into()), "d".into())
						.build()
						.unwrap(),
				)
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("x".into()), "y".into())
						.build()
						.unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("1".into()), "A".into())
						.build()
						.unwrap(),
				)
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "2".into()).build().unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "3".into()).build().unwrap(),
				)
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("4".into()), "B".into())
						.build()
						.unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "5".into()).build().unwrap(),
				)
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "6".into()).build().unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_09() {
		check_ok(
			vec!["--plot", "c1", "d", "--plot", "x", "y", "--panel", "--plot", "e"],
			"test-files/config09.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("c1".into()), "d".into())
						.build()
						.unwrap(),
				)
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("x".into()), "y".into())
						.build()
						.unwrap(),
				)
				.with_default_panel()
				.with_line(
					LineBuilder::new().with_plot_field_line(None, "e".into()).build().unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_10() {
		check_ok(
			vec!["--plot", "c1", "d", "--line-color", "red"],
			"test-files/config10.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("c1".into()), "d".into())
						.apply_param(LineParam::LineColor("red".into()))
						.build()
						.unwrap(),
				)
				.build(),
		)
	}
	#[test]
	fn test_11() {
		check_ok(
			vec!["--plot", "c1", "d", "--line-color", "red", "--file-id", "12"],
			"test-files/config11.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("c1".into()), "d".into())
						.apply_param(LineParam::LineColor("red".into()))
						.apply_param(LineParam::InputFileId(12))
						.build()
						.unwrap(),
				)
				.build(),
		)
	}

	#[test]
	fn test_12() {
		check_ok(
			vec![
				"--event",
				"duration",
				"666.0",
				"--file-name",
				"x.log",
				"--yaxis",
				"y2",
				"--line-color",
				"red",
				"--marker-type",
				"circle",
				"--marker-color",
				"blue",
			],
			"test-files/config12.toml",
			GraphConfigBuilder::new()
				.with_default_panel()
				.with_line(
					LineBuilder::new()
						.with_event_value_line(None, "duration".into(), 666.0)
						.apply_param(LineParam::LineColor("red".into()))
						.apply_param(LineParam::MarkerType("circle".into()))
						.apply_param(LineParam::MarkerColor("blue".into()))
						.apply_param(LineParam::YAxis(YAxis::Y2))
						.apply_param(LineParam::InputFileName("x.log".into()))
						.build()
						.unwrap(),
				)
				.build(),
		)
	}

	#[test]
	fn test_13() {
		check_ok(
			vec![
				"--panel-title",
				"A nice title",
				"--height",
				"0.3",
				"--yaxis-scale",
				"log",
				"--legend",
				"true",
				"--event",
				"duration",
				"666.0",
			],
			"test-files/config13.toml",
			GraphConfigBuilder::new()
				.with_panel(
					PanelBuilder::new()
						.apply_param(PanelParam::PanelTitle("A nice title".into()))
						.apply_param(PanelParam::Height(0.3))
						.apply_param(PanelParam::YAxisScale(AxisScale::Log))
						.apply_param(PanelParam::Legend(true))
						.build(),
				)
				.with_line(
					LineBuilder::new()
						.with_event_value_line(None, "duration".into(), 666.0)
						.build()
						.unwrap(),
				)
				.build(),
		)
	}

	#[rustfmt::skip]
	fn test_14_input() -> Vec<&'static str> {
		vec![
			//panel 1
			"--panel-title", "Another title", "--height", "0.3", "--yaxis-scale", "log", "--legend", "true",
			//line 1
			"--event", "duration", "666.0",
				"--file-name", "x.log",
				"--title", "LineTitle",
				"--style", "lines-points",
				"--line-width", "2.4",
				"--line-color", "red",
				"--dash-style", "dash-dot",
				"--yaxis", "y2",
				"--marker-type", "circle",
				"--marker-color", "blue",
				"--marker-size", "5.0",
			//line 2
			"--event", "duration", "777.0",
				"--file-name", "y.log",
				"--yaxis", "y",
				"--line-color", "blue",
				"--marker-type", "square",
				"--marker-color", "yellow",
			//panel 2
			"--panel", "--panel-title", "panel2", "--height", "0.5", "--yaxis-scale", "linear", "--legend", "false",
			// line 1
			"--plot", "xxx", "yyy",
				"--file-name", "plot1.log",
				"--yaxis", "y",
				"--line-color", "red",
				"--marker-type", "circle",
				"--marker-color", "blue",
			// line 2
			"--event-count", "duration",
				"--file-name", "plot2.log",
				"--yaxis", "y2",
				"--line-color", "dark-turquoise",
				"--marker-type", "dot",
				"--marker-color", "black",
		]
	}

	#[test]
	fn test_14_combo() {
		init_tracing_test();
		check_ok(
			test_14_input(),
			"test-files/config14.toml",
			GraphConfigBuilder::new()
				.with_panel(
					PanelBuilder::new()
						.apply_param(PanelParam::PanelTitle("Another title".into()))
						.apply_param(PanelParam::Height(0.3))
						.apply_param(PanelParam::YAxisScale(AxisScale::Log))
						.apply_param(PanelParam::Legend(true))
						.build(),
				)
				.with_line(
					LineBuilder::new()
						.with_event_value_line(None, "duration".into(), 666.0)
						.apply_param(LineParam::InputFileName("x.log".into()))
						.apply_param(LineParam::Title("LineTitle".into()))
						.apply_param(LineParam::PlotStyle(PlotStyle::LinesPoints))
						.apply_param(LineParam::LineWidth(LineWidth(2.4)))
						.apply_param(LineParam::LineColor(Color::Red))
						.apply_param(LineParam::DashStyle(DashStyle::DashDot))
						.apply_param(LineParam::YAxis(YAxis::Y2))
						.apply_param(LineParam::MarkerType(MarkerType::Circle))
						.apply_param(LineParam::MarkerColor(Color::Blue))
						.apply_param(LineParam::MarkerSize(MarkerSize(5.0)))
						.build()
						.unwrap(),
				)
				.with_line(
					LineBuilder::new()
						.with_event_value_line(None, "duration".into(), 777.0)
						.apply_param(LineParam::LineColor("blue".into()))
						.apply_param(LineParam::MarkerType("square".into()))
						.apply_param(LineParam::MarkerColor("yellow".into()))
						.apply_param(LineParam::YAxis(YAxis::Y))
						.apply_param(LineParam::InputFileName("y.log".into()))
						.build()
						.unwrap(),
				)
				.with_panel(
					PanelBuilder::new()
						.apply_param(PanelParam::PanelTitle("panel2".into()))
						.apply_param(PanelParam::Height(0.5))
						.apply_param(PanelParam::YAxisScale(AxisScale::Linear))
						.apply_param(PanelParam::Legend(false))
						.build(),
				)
				.with_line(
					LineBuilder::new()
						.with_plot_field_line(Some("xxx".into()), "yyy".into())
						.apply_param(LineParam::LineColor("red".into()))
						.apply_param(LineParam::MarkerType("circle".into()))
						.apply_param(LineParam::MarkerColor("blue".into()))
						.apply_param(LineParam::YAxis(YAxis::Y))
						.apply_param(LineParam::InputFileName("plot1.log".into()))
						.build()
						.unwrap(),
				)
				.with_line(
					LineBuilder::new()
						.with_event_count_line(None, "duration".into())
						.apply_param(LineParam::LineColor("dark-turquoise".into()))
						.apply_param(LineParam::MarkerType("dot".into()))
						.apply_param(LineParam::MarkerColor("black".into()))
						.apply_param(LineParam::YAxis(YAxis::Y2))
						.apply_param(LineParam::InputFileName("plot2.log".into()))
						.build()
						.unwrap(),
				)
				.build(),
		)
	}

	#[test]
	#[should_panic(expected = "invalid value")]
	fn test_e00() {
		check_err(vec!["--plot", "c1", "d", "--line-color", "red", "--file-id", "12x"])
	}

	#[test]
	#[should_panic(expected = "invalid value")]
	fn test_e01() {
		check_err(vec!["--plot", "c1", "d", "--line-color", "red", "--yaxis", "y3"])
	}

	#[test]
	#[should_panic(expected = "Invalid line source")]
	fn test_e02() {
		check_err(vec!["--plot", "c1", "d", "--file-id", "1", "--file-name", "x.log"])
	}

	fn check_err(args: Vec<&str>) {
		let full_args: Vec<_> = ["graph"].iter().chain(args.iter()).cloned().collect();
		let matches = build_cli().try_get_matches_from(full_args.clone());
		trace!("matches: {:#?}", matches);
		if let Ok(matches) = matches {
			let parsed = GraphConfig::try_from_matches(&matches);
			trace!("parsed: {:#?}", parsed);
			panic!("{}", parsed.err().unwrap());
		} else {
			panic!("{}", matches.err().unwrap().render());
		}
	}

	fn check_ok(args: Vec<&str>, config_file: &str, expected: GraphConfig) {
		let full_args: Vec<_> = ["graph"].iter().chain(args.iter()).cloned().collect();
		let matches = build_cli().try_get_matches_from(full_args.clone()).unwrap();
		let parsed = GraphConfig::try_from_matches(&matches).unwrap();

		parsed.save_to_file(&Path::new("/tmp/parsed.toml")).unwrap();
		expected.save_to_file(&Path::new("/tmp/expected.toml")).unwrap();

		if !Path::new(config_file).exists() {
			parsed.save_to_file(&Path::new(config_file)).unwrap();
		}
		let loaded = GraphConfig::load_from_file(&Path::new(config_file)).unwrap();
		trace!("loaded: {:#?}", loaded);
		trace!("parsed: {:#?}", parsed);
		trace!("expect: {:#?}", expected);
		trace!("{:#?}", full_args.join(" "));
		assert_eq!(parsed, expected);
		assert_eq!(loaded, expected);
	}
}
