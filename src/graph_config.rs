//! User facing declaration of the graph config.
//!
//! Definition of structures representing graph configurations as written by users, raw input.
//! These configs, usually written in TOML (or provided as CLI options), describe panels, fields, and layout choices.
//! This module handles parsing them into Rust types and preparing them for further processing.

use crate::error::Error;
use annotate_snippets::{Level, Renderer, Snippet};
use chrono::NaiveDateTime;
use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
	borrow::Cow,
	fmt::Display,
	fs,
	path::{Path, PathBuf},
	str::FromStr,
};
use strum::EnumIter;
use toml::de::Error as TomlError;
use tracing::{error, info};

/// A complete graph configuration composed of one or more [`Panel`]s.
///
/// Each [`Panel`] is drawn horizontally in the final output, and each
/// panel may contain multiple lines of data.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphConfig {
	/// The list of panels in this graph.
	pub panels: Vec<Panel>,
}

/// The default format of the timestamp which is used in logs.
///
/// For exact format specifiers refer to: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>
pub const DEFAULT_TIMESTAMP_STR: &str = "%Y-%m-%d %H:%M:%S%.3f";
pub const DEFAULT_TIMESTAMP_FORMAT: TimestampFormat =
	TimestampFormat::DateTime(Cow::Borrowed(DEFAULT_TIMESTAMP_STR));

/// Represents user provided timestamp.
///
/// Shall be compatible with chrono strftime format.
/// For exact format specifiers refer to: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>
#[derive(Clone, PartialEq, Debug, Serialize)]
pub enum TimestampFormat {
	/// Time stmap format contains date specifier
	///
	/// Can be parsed by NaiveDateTime.
	DateTime(Cow<'static, str>),
	/// Time stmap format does not contain any date specifier.
	///
	/// Shall be parsed by NativeTime.
	Time(Cow<'static, str>),
}

impl TimestampFormat {
	pub fn as_str(&self) -> &str {
		match self {
			TimestampFormat::DateTime(cow) => cow.as_ref(),
			TimestampFormat::Time(cow) => cow.as_ref(),
		}
	}
}

impl From<&str> for TimestampFormat {
	fn from(s: &str) -> Self {
		if Self::format_contains_date(s) {
			TimestampFormat::DateTime(Cow::Owned(s.into()))
		} else {
			TimestampFormat::Time(Cow::Owned(s.into()))
		}
	}
}

impl<'de> Deserialize<'de> for TimestampFormat {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Ok(Self::from(s.as_str()))
	}
}

impl TimestampFormat {
	fn format_contains_date(fmt: &str) -> bool {
		//https://docs.rs/chrono/latest/chrono/format/strftime/index.html
		const DATE_SPECIFIERS: [&str; 25] = [
			"%Y", "%C", "%y", "%q", "%m", "%b", "%B", "%h", "%d", "%e", "%a", "%A", "%w", "%u",
			"%U", "%W", "%G", "%g", "%V", "%j", "%D", "%x", "%F", "%v", "%s",
		];
		DATE_SPECIFIERS.iter().any(|&s| fmt.contains(s))
	}
}

/// Global graph context shared across all panels and lines.
///
/// This includes shared configuration such as input log files and layout preferences.
/// It is used during graph config resolution to assign each line to a specific input file.
///
/// # Resolution Behavior
///
/// - `--input <a.log> <b.log>` sets the global list of input log files.
/// - `--per-file-panels` duplicates all panels once per input file.
///   - Lines **without** a file or file-id will be assigned to the file from input file list,
///   - Lines **with** an explicit `--file` or `--file-id` remain unchanged and appear in all
///     panels.
///
/// This context is injected when converting from a basic [`GraphConfig`] into a
/// fully-resolved [`crate::resolved_graph_config::ResolvedGraphConfig`] with concrete log sources.
#[derive(Args, Debug, Serialize, Deserialize, Default)]
pub struct SharedGraphContext {
	/// Input log files to be processed.
	/// Comma-separated list of input log files to be processed.
	#[arg(long, short = 'i', value_delimiter = ',', help_heading = "Input files")]
	#[serde(skip)]
	pub input: Vec<PathBuf>,

	/// When enabled, creates a separate panel for each input file.
	///
	/// If any panel contains lines that are not explicitly bound to a file (i.e. no `file_name` or
	/// `file_id` set), that panel will be duplicated once per input file. Each duplicated panel
	/// will contain lines resolved to a specific file from the input list.
	///
	/// Panels and lines that already target specific files are unaffected by this option.
	#[arg(long, num_args(0..=1), default_value = None, help_heading = "Panels layout",  default_missing_value = "true")]
	per_file_panels: Option<bool>,

	/// The format of the timestamp which is used in logs.
	///
	/// For exact format specifiers refer to: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>
	///
	/// [default: '%Y-%m-%d %H:%M:%S%.3f']
	#[arg(
		long,
		default_value = None,
		help_heading = "Input files",
	)]
	timestamp_format: Option<TimestampFormat>,

	/// Forces regeneration of the CSV cache by re-parsing the log files.
	#[arg(
		long = "force-csv-regen",
		short = 'f',
		default_value_t = false,
		help_heading = "Output files"
	)]
	#[serde(skip)]
	pub force_csv_regen: bool,

	/// Additionally writes the current graph configuration to a file in TOML format.
	#[arg(
		long = "write-config",
		short = 'w',
		value_name = "CONFIG-FILE",
		help_heading = "Output files"
	)]
	pub output_config_path: Option<PathBuf>,

	/// Path to the output PNG graph file.
	///
	/// The corresponding `.gnuplot` script will be written alongside it, using the same filename
	/// with a different extension. Ignored if `--inline-output` is set.
	///
	/// If nothing is provided `graph.png` and `graph.gnuplot` in current directory will be stored.
	#[arg(long, short = 'o', value_name = "FILE", help_heading = "Output files")]
	pub output: Option<PathBuf>,

	/// Output filename to be placed in a location derived from the input log file paths.
	///
	/// Location of file is automatically resolved as follow:
	/// - If a single log file is provided, the output goes next to it.
	/// - If multiple log files are used, the output goes to their common ancestor directory.
	///
	/// This option is a convenience shortcut: only the directory is inferred — the filename must
	/// be provided here.
	///
	/// Overrides `--output` if both are set.
	#[arg(
		long,
		value_name = "FILE",
		value_parser = validate_standalone_filename,
		help_heading = "Output files"
	)]
	pub inline_output: Option<PathBuf>,

	/// Directory to store parsed CSV cache files.
	/// The full path of each log file is mirrored inside this directory to avoid name collisions.
	/// If not set, a `.plox/` directory is created next to each log file to store its cache.
	#[arg(long, value_name = "DIR", help_heading = "Output files")]
	#[serde(skip)]
	pub cache_dir: Option<PathBuf>,

	/// Strategy for aligning time ranges across all panels.
	///
	/// This determines how time-axis (x) ranges are handled when plotting.
	#[arg(long, value_enum, conflicts_with = "time_range", help_heading = "Panels layout")]
	pub panel_alignment_mode: Option<PanelAlignmentModeArg>,

	/// Optional override for the global time range used in the graph.
	///
	/// Can be specified as either:
	/// - A relative range in `[0.0, 1.0]`,
	/// - Two timestamp strings.
	///
	/// Timestamp strings must be compatible with the `--timestamp-format`.
	///
	/// Conflicts with `--panel-alignment-mode`, and implies global alignment.
	#[arg(
		long,
		value_parser = TimeRangeArg::parse_time_range,
		conflicts_with = "panel_alignment_mode",
		help_heading = "Panels layout"
	)]
	#[serde(skip)]
	pub time_range: Option<TimeRangeArg>,
}

impl SharedGraphContext {
	pub fn new_with_input(input: Vec<PathBuf>) -> Self {
		Self { input, ..Default::default() }
	}

	pub fn timestamp_format(&self) -> &TimestampFormat {
		self.timestamp_format.as_ref().unwrap_or(&DEFAULT_TIMESTAMP_FORMAT)
	}

	#[cfg(test)]
	pub fn per_file_panels_option(&self) -> Option<bool> {
		self.per_file_panels
	}

	pub fn per_file_panels(&self) -> bool {
		self.per_file_panels.unwrap_or(false)
	}

	/// Intended to merge context given on CLI with one read from file
	pub fn merge_with_other(&mut self, other: Self) {
		macro_rules! set_if_none {
			($field:ident) => {
				if self.$field.is_none() {
					self.$field = other.$field;
				}
			};
		}

		set_if_none!(per_file_panels);
		set_if_none!(timestamp_format);
	}
}

/// A panel that holds multiple [`Line`]s in the same horizontal space.
///
/// Panels are typically stacked vertically, so each panel is drawn on a separate row.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Panel {
	/// The list of lines to draw on this panel.
	pub lines: Vec<Line>,

	#[serde(flatten)]
	pub params: PanelParams,
}

/// A single line (or data series) to be plotted on a panel.
///
/// It includes a [`DataSource`] to describe the data source (e.g. plotting a field vs
/// an event pattern), as well as various styling and configuration details
/// (e.g. color, axis).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
	/// The logical data source or type of this line.
	#[serde(flatten)]
	pub data_source: DataSource,

	#[serde(flatten)]
	pub params: LineParams,
}

impl Line {
	pub fn new_with_data_source(data_source: DataSource) -> Self {
		Line { data_source, params: LineParams::default() }
	}
}

#[derive(Default, Clone, Args, Debug, Serialize, Deserialize, PartialEq)]
pub struct LineParams {
	/// Optionally overrides source log file.
	///
	/// Assigns a specific file to the line
	#[arg(long)]
	pub file_name: Option<PathBuf>,

	/// Optionally specifies the index of input file.
	///
	/// Assigns the line to the nth input from `--input` (index starting at 0)
	#[arg(long)]
	pub file_id: Option<usize>,

	/// Optional title of the line. Will be placed on legend.
	#[arg(long)]
	pub title: Option<String>,

	/// The style of the plotted line
	#[arg(long, default_value = "lines")]
	#[serde(default)]
	pub style: PlotStyle,

	/// The width of the line
	#[arg(long)]
	pub line_width: Option<LineWidth>,

	/// The color of the line.
	#[arg(long)]
	pub line_color: Option<Color>,

	/// The dash type.
	#[arg(long)]
	pub dash_style: Option<DashStyle>,

	/// Which Y-axis this line should use, if applicable (e.g. primary or secondary).
	#[arg(long)]
	pub yaxis: Option<YAxis>,

	/// The marker type.
	#[arg(long)]
	pub marker_type: Option<MarkerType>,

	/// The color of the marker (if markers are enabled).
	#[arg(long)]
	pub marker_color: Option<Color>,

	/// The size of the marker
	#[arg(long, default_value_t = MarkerSize::default())]
	#[serde(default = "MarkerSize::default")]
	pub marker_size: MarkerSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineWidth(pub f64);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarkerSize(pub f64);

impl Display for LineWidth {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f64::fmt(&self.0, f)
	}
}

impl Display for MarkerSize {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f64::fmt(&self.0, f)
	}
}

impl Default for LineWidth {
	fn default() -> Self {
		Self(1.0)
	}
}

impl Default for MarkerSize {
	fn default() -> Self {
		Self(2.0)
	}
}

impl FromStr for MarkerSize {
	type Err = String;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let l = s.parse::<f64>().map_err(|e| format!("MarkerSize parse error:{}", e))?;
		if l <= 0.0 {
			return Err(format!("MarkerSize: invalid value {l}"));
		}
		Ok(Self(l))
	}
}

impl FromStr for LineWidth {
	type Err = String;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let l = s.parse::<f64>().map_err(|e| format!("LineWidth parse error:{}", e))?;
		if l <= 0.0 {
			return Err(format!("LineWidth: invalid value {l}"));
		}
		Ok(Self(l))
	}
}

#[derive(Default, Clone, Args, Debug, Serialize, Deserialize, PartialEq)]
pub struct PanelParams {
	/// Title displayed above the panel
	#[arg(long)]
	pub panel_title: Option<String>,

	/// Height ratio (relative to other panels)
	#[arg(long)]
	pub height: Option<f64>,

	/// Y-axis scale (linear or log)
	#[arg(long)]
	pub yaxis_scale: Option<AxisScale>,

	/// Show legend.
	///
	/// Legend will be shown if not provided.
	#[arg(long)]
	pub legend: Option<bool>,

	/// Panel range mode.
	///
	/// How panel time range shall be generated.
	#[arg(long)]
	pub time_range_mode: Option<PanelRangeMode>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AxisScale {
	Linear,
	Log,
}

/// Represents the different ways a line's data can be sourced from logs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Subcommand)]
#[serde(tag = "data_source", rename_all = "snake_case")]
pub enum DataSource {
	/// Plot a fixed numerical value (`yvalue`) whenever `pattern` appears in logs.
	#[clap(name = "event")]
	EventValue {
		/// Optional guard string to quickly filter out log lines using `strcmp`
		guard: Option<String>,
		/// Substring or regex pattern to match in log lines.
		pattern: String,
		/// The fixed value to plot each time `pattern` is found.
		yvalue: f64,
	},

	/// Plot a cumulative count of `pattern` occurrences over time.
	EventCount {
		/// Optional guard string to quickly filter out log lines using `strcmp`
		guard: Option<String>,
		/// Substring or regex pattern to match in log lines.
		pattern: String,
	},

	/// Plot the time delta between consecutive occurrences of `pattern`.
	EventDelta {
		/// Optional guard string to quickly filter out log lines using `strcmp`
		guard: Option<String>,
		/// Substring or regex pattern to match in log lines.
		pattern: String,
	},

	/// Plot a numeric field from logs.
	///
	/// This is the most common data source type.
	#[serde(untagged)]
	#[clap(name = "plot")]
	FieldValue {
		/// Optional guard string to quickly filter out log lines using `strcmp`
		guard: Option<String>,
		/// The name of the field to parse as numeric or regex.
		/// Refer to "Plot Field Regex" help section for more details.
		field: String,
	},
}

impl DataSource {
	pub fn new_event_value(guard: Option<String>, pattern: String, yvalue: f64) -> Self {
		DataSource::EventValue { guard, pattern, yvalue }
	}

	pub fn new_event_count(guard: Option<String>, pattern: String) -> Self {
		DataSource::EventCount { guard, pattern }
	}

	pub fn new_event_delta(guard: Option<String>, pattern: String) -> Self {
		DataSource::EventDelta { guard, pattern }
	}

	pub fn new_plot_field(guard: Option<String>, field: String) -> Self {
		DataSource::FieldValue { guard, field }
	}
}

/// Which Y-axis to plot a line against.
///
/// Typically, a graph can have two Y-axes:
/// - The **primary** axis (left side) -> [`YAxis::Y`]
/// - The **secondary** axis (right side) -> [`YAxis::Y2`]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum YAxis {
	/// Primary Y-axis (the left side).
	Y,
	/// Secondary Y-axis (the right side).
	Y2,
}

/// Predefined set of colors for gnuplot lines and markers.
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Deserialize, Serialize, EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum Color {
	Red,
	Blue,
	DarkGreen,
	Purple,
	Cyan,
	Goldenrod,
	Brown,
	Olive,
	Navy,
	Violet,
	Coral,
	Salmon,
	SteelBlue,
	DarkMagenta,
	DarkCyan,
	DarkYellow,
	DarkTurquoise,
	Yellow,
	Black,
	Magenta,
	Orange,
	Green,
	DarkOrange,
}

/// Predefined marker symbols for gnuplot plots.
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Deserialize, Serialize, EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum MarkerType {
	Dot,
	TriangleFilled,
	SquareFilled,
	DiamondFilled,
	Plus,
	Cross,
	Circle,
	X,
	Triangle,
	Square,
	Diamond,
}

impl FromStr for MarkerType {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		<MarkerType as ValueEnum>::from_str(s, true).map_err(|_| format!("Bad MarkerType: {}", s))
	}
}

/// Plot styles for gnuplot
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Deserialize, Serialize, Default, EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum PlotStyle {
	#[default]
	Lines,
	Steps,
	Points,
	LinesPoints,
}

impl FromStr for PlotStyle {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		<PlotStyle as ValueEnum>::from_str(s, true).map_err(|_| format!("Bad PlotStyle: {}", s))
	}
}

/// Dash (line-type) styles for gnuplot
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Deserialize, Serialize, Default, EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum DashStyle {
	#[default]
	Solid,
	Dashed,
	Dotted,
	DashDot,
	LongDash,
}

impl FromStr for DashStyle {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		<DashStyle as ValueEnum>::from_str(s, true).map_err(|_| format!("Bad DashStyle: {}", s))
	}
}

impl FromStr for Color {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		<Color as ValueEnum>::from_str(s, true).map_err(|_| format!("Bad Color: {}", s))
	}
}

impl From<&str> for Color {
	fn from(s: &str) -> Self {
		<Self as FromStr>::from_str(s).expect("Failed to convert &str to Color")
	}
}
impl From<&str> for MarkerType {
	fn from(s: &str) -> Self {
		<Self as FromStr>::from_str(s).expect("Failed to convert &str to MarkerType")
	}
}

fn validate_standalone_filename(s: &str) -> Result<PathBuf, String> {
	let path = PathBuf::from(s);
	if path.components().count() != 1 || !path.is_relative() {
		Err(format!("Name '{s}' must be a filename only, without any directories"))
	} else {
		Ok(path)
	}
}

/// Defines how the time range for each panel is computed from its lines.
///
/// This determines the `time_range` for every panel, based on the `time_range` values of the lines
/// it contains.
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PanelRangeMode {
	/// Use the full span of all line ranges (min start, max end).
	#[default]
	Full,

	/// Use the overlapping time window of all lines (max start, min end).
	BestFit,
}

/// Controls how panels are aligned on the X-axis during plotting.
///
/// This setting determines whether each panel uses its own time range,
/// or whether all panels are synchronized to a shared range.
///
/// After computing each panel's local time range, this setting determines
/// whether to preserve them independently or override them to ensure
/// consistent alignment (e.g. for side-by-side comparison).
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum PanelAlignmentMode {
	/// Align all panels using the **full combined** time range from all data.
	///
	/// Ensures that the time axis covers the entire time span of all lines,
	/// even if some panels have sparse data.
	#[default]
	SharedFull,

	/// Use each panel's own computed time range.
	///
	/// No alignment is applied — panels may have different time axes.
	PerPanel,

	/// Align all panels using the **overlapping** portion of their time ranges.
	///
	/// Useful for comparing synchronized events across sources.
	/// If there is no overlap, no alignment is applied.
	SharedOverlap,

	/// Use a fixed time range explicitly provided via `--time-range`.
	///
	/// Overrides all computed ranges.
	Fixed(NaiveDateTime, NaiveDateTime),
}

/// Clap wrapper for [`PanelAlignmentMode`]
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum PanelAlignmentModeArg {
	#[default]
	SharedFull,
	PerPanel,
	SharedOverlap,
}

/// Represents a user-defined time range override provided via `--time-range`.
///
/// This can be used to zoom in or constrain the graph to a specific time window.
/// The variant determines how to interpret the input:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeRangeArg {
	/// Relative zoom: values between 0.0 and 1.0
	Relative(f64, f64),

	/// Absolute date-time values, parsed using TimestampFormat
	AbsoluteDateTime(String, String),
}

impl TimeRangeArg {
	pub fn parse_time_range(s: &str) -> Result<TimeRangeArg, String> {
		let pieces: Vec<&str> = s.split(',').map(str::trim).collect();
		if pieces.len() != 2 {
			return Err("Expected two values separated by a comma".into());
		}

		if let (Ok(a), Ok(b)) = (pieces[0].parse::<f64>(), pieces[1].parse::<f64>()) {
			if !(0.0..=1.0).contains(&a) || !(0.0..=1.0).contains(&b) || a >= b {
				return Err("Relative range must be between 0.0 and 1.0, and start < end".into());
			}
			return Ok(TimeRangeArg::Relative(a, b));
		}

		Ok(TimeRangeArg::AbsoluteDateTime(pieces[0].into(), pieces[1].into()))
	}
}

impl GraphConfig {
	pub fn save_to_file(self: &GraphConfig, config_path: &Path) -> Result<(), Error> {
		let toml_string = toml::to_string(self).expect("Failed to convert GraphConfig to TOML");
		fs::write(config_path, toml_string)
			.map(|_| info!("Config saved successfully: {:?}.", config_path))
			.map_err(|e| Error::IoError(format!("{:?}", config_path), e))
	}

	pub fn load_from_file(path: &Path) -> Result<Self, Error> {
		let content = fs::read_to_string(path).map_err(|error| {
			error!(?error, "Reading toml error");
			Error::IoError(format!("{}", path.display()), error)
		})?;
		toml::from_str(&content).map_err(|e| {
			let r = annotate_toml_error(&e, &content, &path.display().to_string());
			error!("{r}");
			e.into()
		})
	}
}

pub fn annotate_toml_error(err: &TomlError, source: &str, filename: &str) -> String {
	if let Some(span) = err.span() {
		let snippet = Snippet::source(source)
			.line_start(1)
			.origin(filename)
			.fold(true)
			.annotation(Level::Error.span(span.clone()).label(err.message()));
		let title = format!("Failed to parse {filename}");
		let message = Level::Error.title(&title).snippet(snippet);
		format!("{}", Renderer::styled().render(message))
	} else {
		err.to_string()
	}
}
