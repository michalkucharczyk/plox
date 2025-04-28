//! Structures that are results of expansion.

#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(private_interfaces)]
#![allow(unused_variables)]
use crate::{
	error::Error,
	graph_config::{DataSource, GraphConfig, Line, LineParams, PanelParams, SharedGraphContext},
};
use chrono::NaiveDateTime;
use clap::Args;
use serde::{Deserialize, Serialize};
use std::{
	fmt::Display,
	fs,
	path::{Path, PathBuf},
	str::FromStr,
};
use tracing::info;

#[derive(Debug)]
pub struct ResolvedGraphConfig {
	pub panels: Vec<ResolvedPanel>,
}

impl ResolvedGraphConfig {
	pub fn all_lines(&self) -> impl Iterator<Item = &ResolvedLine> {
		self.panels.iter().flat_map(|panel| panel.lines.iter())
	}
}

#[derive(Debug, Default)]
pub struct ResolvedPanel {
	pub lines: Vec<ResolvedLine>,
	pub params: PanelParams,

	/// If panel was duplicated due to per-file-panels this will be set to the source file name.
	input_file_name: Option<PathBuf>,

	/// Final time range to use for this panel when plotting.
	///
	/// This is derived from the time ranges of all its lines, using the
	/// configured range mode (e.g. full span or best-fit intersection),
	/// and may be overridden by manual range provided in CLI.
	///
	/// Used to generate the `set xrange [...]` directive for Gnuplot.
	//todo: do something with pub
	pub(crate) time_range: Option<(NaiveDateTime, NaiveDateTime)>,
}

impl ResolvedPanel {
	pub fn new_with_lines(lines: Vec<ResolvedLine>) -> Self {
		Self { lines, ..Default::default() }
	}

	pub fn is_empty(&self) -> bool {
		self.lines.is_empty() || self.lines.iter().all(ResolvedLine::is_empty)
	}

	pub fn input_file(&self) -> &Option<PathBuf> {
		&self.input_file_name
	}

	pub fn title(&self) -> Vec<String> {
		match (&self.params.panel_title, &self.input_file_name) {
			(Some(panel_title), Some(input_file_name)) => {
				let file_stem = input_file_name
					.file_stem()
					.expect("filename is validated at this point")
					.to_string_lossy();
				vec![panel_title.clone(), format!("[{}]", file_stem)]
			},
			(Some(panel_title), None) => vec![panel_title.clone()],
			(None, Some(input_file_name)) => {
				let file_stem = input_file_name
					.file_stem()
					.expect("filename is validated at this point")
					.to_string_lossy();
				vec![format!("[{}]", file_stem)]
			},
			(None, None) => Default::default(),
		}
	}

	pub fn time_range(&self) -> &Option<(NaiveDateTime, NaiveDateTime)> {
		&self.time_range
	}

	pub fn set_time_range(&mut self, start: NaiveDateTime, end: NaiveDateTime) {
		self.time_range = Some((start, end));
	}
}

#[derive(Debug, Clone)]
pub struct ResolvedLine {
	pub line: Line,
	pub source: ResolvedSource,

	/// Optional path to a shared CSV file used for multiple lines with the same (guard,
	/// pattern/field).
	///
	/// This allows avoiding redundant log processing by reusing the output of a previously matched
	/// line, typically one using `PlotField`, across other compatible line data sources like
	/// [`DataSource::EventCount`] or [`DataSource::EventDelta`].
	shared_csv_file: Option<PathBuf>,

	///Indicates if line contains any data.
	///
	///Should be set just after processing input files.
	data_points_count: usize,

	/// Time range of this line's data, based on the earliest and latest
	/// timestamps parsed from its CSV file.
	///
	///`Some((start, end))` if the file contains valid timestamps,
	/// or `None` if the file is empty or failed to parse.
	///
	/// This is used for panel-level range calculations and alignment.
	//todo pub
	time_range: Option<(NaiveDateTime, NaiveDateTime)>,
}

impl ResolvedLine {
	pub fn is_empty(&self) -> bool {
		self.data_points_count == 0
	}

	pub fn from_explicit_name(line: Line, file_name: PathBuf) -> Self {
		Self {
			line,
			source: ResolvedSource::FileName(file_name),
			shared_csv_file: None,
			data_points_count: 0,
			time_range: None,
		}
	}

	fn try_from_populated_inputs(
		line: Line,
		poulated_input: Option<(usize, &PathBuf)>,
	) -> Option<Self> {
		match poulated_input {
			None => ResolvedSource::try_from_explicit(line.source()).map(|source| Self {
				line,
				source,
				shared_csv_file: None,
				data_points_count: 0,
				time_range: None,
			}),
			Some((file_id, file_name)) => {
				ResolvedSource::try_match_input(line.source(), file_id, file_name).map(|source| {
					Self {
						line,
						source,
						shared_csv_file: None,
						data_points_count: 0,
						time_range: None,
					}
				})
			},
		}
	}

	/// The name of final csv file to be used.
	pub fn shared_csv_filename(&self) -> Option<PathBuf> {
		self.shared_csv_file.clone()
	}

	pub fn expect_shared_csv_filename(&self) -> PathBuf {
		self.shared_csv_filename().expect("All shared CVS are resolved at this point.")
	}

	/// Set the name of final csv file to be used.
	pub fn set_shared_csv_filename(&mut self, path: &Path) {
		self.shared_csv_file = Some(path.to_path_buf());
	}

	pub fn set_data_points_count(&mut self, count: usize) {
		self.data_points_count = count;
	}

	pub fn set_time_range(&mut self, start: NaiveDateTime, end: NaiveDateTime) {
		self.time_range = Some((start, end));
	}

	pub fn time_range(&self) -> &Option<(NaiveDateTime, NaiveDateTime)> {
		&self.time_range
	}
}

/// Represents the fully resolved source of a log line after expansion.
///
/// This is used in the `ResolvedGraphConfig` to indicate which concrete input file
/// a line should be read from. It is the result of resolving any `LineSource`
/// (e.g. `file`, `file-id`, or all-files) into a concrete file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
	/// A line originally intended for all inputs, now bound to a specific input file.
	PopulatedInput {
		/// Index of the input in `SharedGraphContext::input_files`.
		index: usize,
		/// Resolved file path.
		path: PathBuf,
	},

	/// A line that explicitly used `--file <path>`, preserved as-is.
	FileName(PathBuf),

	/// A line that targeted `--file-id <N>`, resolved to the corresponding input file.
	FileId {
		/// Index of the file in the input list, as provided in graph config.
		index: usize,
		/// Resolved file path.
		path: PathBuf,
	},
}

impl ResolvedSource {
	pub fn file_name(&self) -> &PathBuf {
		match self {
			ResolvedSource::PopulatedInput { path, .. }
			| ResolvedSource::FileName(path)
			| ResolvedSource::FileId { path, .. } => path,
		}
	}
}

impl ResolvedSource {
	/// Attempts to resolve a `LineSource` into a `ResolvedSource` for a specific input file and its
	/// index.
	///
	/// Used during config expansion to assign sources to specific log files.
	///
	/// - Returns `Some` if the source matches the given input file (by ID or for all files).
	/// - Returns `None` if the line is not associated with provided input.
	fn try_match_input(
		source: LineSource,
		input_id: usize,
		input_file_name: &Path,
	) -> Option<Self> {
		match source {
			LineSource::FileId(id) if id == input_id => {
				Some(Self::FileId { index: input_id, path: input_file_name.to_path_buf() })
			},
			LineSource::AllInputFiles => {
				Some(Self::PopulatedInput { index: input_id, path: input_file_name.to_path_buf() })
			},
			_ => None,
		}
	}

	/// Attempts to resolve a line source if it's explicitly bound to a file name.
	fn try_from_explicit(source: LineSource) -> Option<Self> {
		match source {
			LineSource::FileName(name) => Some(Self::FileName(name.clone())),
			_ => None,
		}
	}
}

#[derive(Clone, PartialEq)]
enum LineSource {
	/// from `--file`
	FileName(PathBuf),
	/// from `--file-id`
	FileId(usize),
	/// fallback if neither is given
	AllInputFiles,
}

impl Line {
	fn source(&self) -> LineSource {
		match (&self.params.file_name, self.params.file_id) {
			(Some(path), _) => LineSource::FileName(PathBuf::from(path)),
			(None, Some(id)) => LineSource::FileId(id),
			_ => LineSource::AllInputFiles,
		}
	}
}

/// Expands a generic `GraphConfig` using the given `SharedGraphContext`, producing a fully resolved
/// `ResolvedGraphConfig`.
///
/// This function performs the following transformations:
/// - Duplicates panels if `per_file_panels` is enabled (one per input file) and panel contains any
///   line that requires input-file population,
/// - Resolves each line's source by:
///   - Using `.file` as-is if explicitly set
///   - Mapping `.file_id` to the corresponding entry in `ctx.input_files`
///   - Applying the line to all input files if no file or file_id is provided
///
/// The result is a `ResolvedGraphConfig` with each line assigned to an exact file and input index.
pub fn expand_graph_config(
	graph: &GraphConfig,
	ctx: &SharedGraphContext,
) -> Result<ResolvedGraphConfig, Error> {
	// todo!()
	let mut resolved_panels = vec![];

	if ctx.per_file_panels() {
		for panel in &graph.panels {
			let is_any_line_to_be_populated =
				panel.lines.iter().any(|l| l.source() == LineSource::AllInputFiles);

			if is_any_line_to_be_populated {
				let fixed_lines = panel
					.lines
					.iter()
					.flat_map(|line| match line.source() {
						LineSource::FileName(file) => {
							vec![ResolvedLine::from_explicit_name(line.clone(), file)]
						},

						LineSource::FileId(id) => vec![
							ResolvedLine::try_from_populated_inputs(
								line.clone(),
								Some((id, &ctx.input[id])),
							)
							.expect("Line shall be resolvable"),
						],
						_ => {
							vec![]
						},
					})
					.collect::<Vec<_>>();

				for (file_id, input_file) in ctx.input.iter().enumerate() {
					let resolved_lines = panel
						.lines
						.iter()
						.filter_map(|line| {
							if let LineSource::AllInputFiles = line.source() {
								ResolvedLine::try_from_populated_inputs(
									line.clone(),
									Some((file_id, input_file)),
								)
							} else {
								None
							}
						})
						.collect::<Vec<_>>();

					let mut lines = fixed_lines.clone();
					lines.extend(resolved_lines);

					resolved_panels.push(ResolvedPanel {
						params: panel.params.clone(),
						lines,
						time_range: None,
						input_file_name: Some(input_file.clone()),
					});
				}
			} else {
				let resolved_lines = panel
					.lines
					.iter()
					.flat_map(|line| match line.source() {
						LineSource::FileName(file) => {
							vec![ResolvedLine::from_explicit_name(line.clone(), file)]
						},

						LineSource::FileId(id) => vec![
							ResolvedLine::try_from_populated_inputs(
								line.clone(),
								Some((id, &ctx.input[id])),
							)
							.expect("Line shall be resolvable"),
						],
						_ => {
							panic!(
								"Should not be here. Lines to be populated are handled in other branch. (This is bug)."
							)
						},
					})
					.collect::<Vec<_>>();

				resolved_panels.push(ResolvedPanel {
					params: panel.params.clone(),
					lines: resolved_lines,
					..Default::default()
				});
			}
		}
	} else {
		for panel in &graph.panels {
			let resolved_lines = panel
				.lines
				.iter()
				.flat_map(|line| match line.source() {
					LineSource::FileName(file) => {
						vec![ResolvedLine::from_explicit_name(line.clone(), file)]
					},

					LineSource::FileId(_) | LineSource::AllInputFiles => ctx
						.input
						.iter()
						.enumerate()
						.filter_map(|(i, f)| {
							ResolvedLine::try_from_populated_inputs(line.clone(), Some((i, f)))
						})
						.collect(),
				})
				.collect::<Vec<_>>();

			resolved_panels.push(ResolvedPanel {
				params: panel.params.clone(),
				lines: resolved_lines,
				..Default::default()
			});
		}
	}

	Ok(ResolvedGraphConfig { panels: resolved_panels })
}

#[cfg(test)]
mod tests {
	use tracing::trace;

	use super::*;
	use crate::{
		graph_cli_builder,
		graph_config::{DEFAULT_TIMESTAMP_FORMAT, DataSource, Panel, TimestampFormat},
		logging::init_tracing_test,
	};

	impl Line {
		fn test_line_name(&self) -> String {
			match self.data_source {
				DataSource::EventValue { ref pattern, .. }
				| DataSource::EventCount { ref pattern, .. }
				| DataSource::EventDelta { ref pattern, .. }
				| DataSource::FieldValue { field: ref pattern, .. } => pattern.clone(),
			}
		}
	}

	macro_rules! check_lines {
		($resolved:expr, $expected_panels:expr, $panel_lens:expr, $file_names:expr, $line_names:expr) => {
			assert_eq!($resolved.panels.len(), $expected_panels);
			for (panel_index, &panel_len) in $panel_lens.iter().enumerate() {
				assert_eq!($resolved.panels[panel_index].lines.len(), panel_len);
				for ((line_index, &file_name), &line_name) in
					$file_names[panel_index].iter().enumerate().zip($line_names[panel_index].iter())
				{
					assert_eq!(
						$resolved.panels[panel_index].lines[line_index]
							.source
							.file_name()
							.to_string_lossy(),
						file_name
					);
					assert_eq!(
						$resolved.panels[panel_index].lines[line_index].line.test_line_name(),
						line_name
					);
				}
			}
		};
		($resolved:expr, $expected_panels:expr, $panel_lens:expr, $file_names:expr) => {
			tracing::trace!("resolved: {:#?}", $resolved);
			tracing::trace!(
				"resolved: {:#?}",
				$resolved
					.panels
					.iter()
					.map(|p| {
						p.lines
							.iter()
							.map(|l| l.source.file_name().to_string_lossy())
							.collect::<Vec<_>>()
					})
					.collect::<Vec<_>>()
			);
			assert_eq!($resolved.panels.len(), $expected_panels);
			for (panel_index, &panel_len) in $panel_lens.iter().enumerate() {
				assert_eq!($resolved.panels[panel_index].lines.len(), panel_len);
				for (line_index, &file_name) in $file_names[panel_index].iter().enumerate() {
					assert_eq!(
						$resolved.panels[panel_index].lines[line_index]
							.source
							.file_name()
							.to_string_lossy(),
						file_name
					);
				}
			}
		};
	}

	#[test]
	fn test_populate_to_panel_01() {
		let input = vec![
			"--input",
			"A,B",
			"--plot",
			"x",
			"--file-id",
			"0",
			"--plot",
			"y",
			"--file-id",
			"1",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(resolved, 1, vec![2], vec![vec!["A", "B"]], vec![vec!["x", "y"]]);
	}

	#[test]
	fn test_populate_to_panel_02() {
		let input = vec!["--input", "A,B,C", "--plot", "x", "--plot", "y"];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			1,
			vec![6],
			vec![vec!["A", "B", "C", "A", "B", "C"]],
			vec![vec!["x", "x", "x", "y", "y", "y"]]
		);
	}

	#[test]
	fn test_populate_to_panel_03() {
		let input = vec!["--input", "A,B,C", "--plot", "x", "--plot", "y", "--file-id", "1"];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			1,
			vec![4],
			vec![vec!["A", "B", "C", "B"]],
			vec![vec!["x", "x", "x", "y"]]
		);
	}

	#[test]
	fn test_populate_to_panel_04() {
		let input = vec!["--input", "A,B,C", "--plot", "x", "--file-id", "1", "--plot", "y"];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			1,
			vec![4],
			vec![vec!["B", "A", "B", "C"]],
			vec![vec!["x", "y", "y", "y"]]
		);
	}

	#[test]
	fn test_populate_to_panel_05() {
		let input = vec!["--input", "A,B,C", "--plot", "x", "--file-name", "E", "--plot", "y"];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			1,
			vec![4],
			vec![vec!["E", "A", "B", "C"]],
			vec![vec!["x", "y", "y", "y"]]
		);
	}

	#[test]
	fn test_populate_to_panel_06() {
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--plot", "x", "--file-name", "D", 
			"--plot", "y",
			"--panel",
			"--plot", "u", "--file-name", "E", 
			"--plot", "t"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			2,
			vec![4, 4],
			vec![vec!["D", "A", "B", "C"], vec!["E", "A", "B", "C"]],
			vec![vec!["x", "y", "y", "y"], vec!["u", "t", "t", "t"]]
		);
	}

	#[test]
	fn test_populate_to_panel_07() {
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--plot", "x", "--file-name", "D", 
			"--plot", "y", "--file-id", "1", 
			"--plot", "z",
			"--panel",
			"--plot", "u", "--file-name", "E", 
			"--plot", "t"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			2,
			vec![5, 4],
			vec![vec!["D", "B", "A", "B", "C"], vec!["E", "A", "B", "C"]],
			vec![vec!["x", "y", "z", "z", "z"], vec!["u", "t", "t", "t"]]
		);
	}

	#[test]
	fn test_populate_to_multiple_panels_01() {
		init_tracing_test();
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--per-file-panels",
			"--plot", "z",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		trace!("ctx: {ctx:#?}");
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			3,
			vec![1, 1, 1],
			vec![vec!["A"], vec!["B"], vec!["C"]],
			vec![vec!["z"], vec!["z"], vec!["z"]]
		);
	}

	#[test]
	fn test_populate_to_multiple_panels_02() {
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--per-file-panels",
			"--plot", "x",
			"--plot", "y",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			3,
			vec![2, 2, 2],
			vec![vec!["A", "A"], vec!["B", "B"], vec!["C", "C"]],
			vec![vec!["x", "y"], vec!["x", "y"], vec!["x", "y"]]
		);
	}

	#[test]
	fn test_populate_to_multiple_panels_03() {
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--per-file-panels",
			"--plot", "z",
			"--panel",
			"--plot", "x",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			6,
			vec![1, 1, 1, 1, 1, 1],
			vec![vec!["A"], vec!["B"], vec!["C"], vec!["A"], vec!["B"], vec!["C"]],
			vec![vec!["z"], vec!["z"], vec!["z"], vec!["x"], vec!["x"], vec!["x"]]
		);
	}

	#[test]
	fn test_populate_to_multiple_panels_04() {
		init_tracing_test();
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--per-file-panels",
			"--plot", "z",
			"--panel",
			"--plot", "x",
			"--plot", "y", "--file-id", "1",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		tracing::trace!("config: {:#?}", config);
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			6,
			vec![1, 1, 1, 2, 2, 2],
			vec![vec!["A"], vec!["B"], vec!["C"], vec!["B", "A"], vec!["B", "B"], vec!["B", "C"]],
			vec![vec!["z"], vec!["z"], vec!["z"], vec!["y", "x"], vec!["y", "x"], vec!["y", "x"]]
		);
	}

	#[test]
	fn test_populate_to_multiple_panels_05() {
		init_tracing_test();
		#[rustfmt::skip]
		let input = vec![
			"--input", "A,B,C", 
			"--per-file-panels",
			"--plot", "z",
			"--panel",
			"--plot", "x",
			"--plot", "y", "--file-name", "D",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		tracing::trace!("config: {:#?}", config);
		let resolved = expand_graph_config(&config, &ctx).unwrap();
		check_lines!(
			resolved,
			6,
			vec![1, 1, 1, 2, 2, 2],
			vec![vec!["A"], vec!["B"], vec!["C"], vec!["D", "A"], vec!["D", "B"], vec!["D", "C"]],
			vec![vec!["z"], vec!["z"], vec!["z"], vec!["y", "x"], vec!["y", "x"], vec!["y", "x"]]
		);
	}

	#[test]
	fn test_per_file_panel_flag() {
		init_tracing_test();
		#[rustfmt::skip]
		let input = vec![
			"--per-file-panels",
			"--plot", "x",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels_option(), Some(true));

		let input = vec!["--per-file-panels", "false", "--plot", "x"];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels_option(), Some(false));

		let input = vec!["--plot", "x"];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels_option(), None);
	}

	#[test]
	fn test_args_or_config_file() {
		init_tracing_test();

		#[rustfmt::skip]
		let input = vec![
			"--config", "test-files/config01.toml",
			"--per-file-panels",
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels_option(), Some(true));
		assert_eq!(ctx.per_file_panels(), true);

		#[rustfmt::skip]
		let input = vec![
			"--config", "test-files/config01-with-per-file-panel.toml"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels(), true);

		#[rustfmt::skip]
		let input = vec![
			"--config", "test-files/config01-with-per-file-panel.toml",
			"--per-file-panels", "false"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels(), false);

		#[rustfmt::skip]
		let input = vec![
			"--config", "test-files/config01-with-timestamp-format.toml"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(*ctx.timestamp_format(), TimestampFormat::from("%s"));

		#[rustfmt::skip]
		let input = vec![
			"--config", "test-files/config01-with-timestamp-format.toml",
			"--timestamp-format", "%j %I:%M:%S %p"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(*ctx.timestamp_format(), TimestampFormat::from("%j %I:%M:%S %p"));

		#[rustfmt::skip]
		let input = vec![
			"--config", "test-files/config01-with-timestamp-format-with-per-file-panel.toml",
			"--per-file-panels", "false",
			"--timestamp-format", "%j %I:%M:%S %p"
		];
		let (config, ctx) = graph_cli_builder::build_from_cli_args(input).unwrap();
		assert_eq!(ctx.per_file_panels(), false);
		assert_eq!(*ctx.timestamp_format(), TimestampFormat::from("%j %I:%M:%S %p"));
	}

	#[test]
	#[should_panic(expected = "unknown field")]
	fn test_bad_config_file() {
		let input = vec!["--config", "test-files/invalid-config.toml"];
		let res = graph_cli_builder::build_from_cli_args(input);
		if res.is_err() {
			panic!("{:?}", res.err());
		}
	}

	#[test]
	fn test_expand_graph_config_minimal() {
		let config = GraphConfig {
			panels: vec![Panel {
				params: PanelParams::default(),
				lines: vec![
					Line {
						data_source: DataSource::FieldValue {
							guard: None,
							field: "duration".into(),
						},
						params: LineParams::default(),
					},
					Line {
						data_source: DataSource::EventCount {
							guard: None,
							pattern: "ERROR".into(),
						},
						params: LineParams::default(),
					},
				],
			}],
		};

		let ctx = SharedGraphContext::new_with_input(vec!["log1.txt".into(), "log2.txt".into()]);

		let resolved = expand_graph_config(&config, &ctx).unwrap();
		assert_eq!(resolved.panels.len(), 1);
		assert_eq!(resolved.panels[0].lines.len(), 4); // 2 lines * 2 files
		assert_eq!(resolved.panels[0].lines[0].source.file_name().to_string_lossy(), "log1.txt");
		assert_eq!(resolved.panels[0].lines[1].source.file_name().to_string_lossy(), "log2.txt");
		assert_eq!(resolved.panels[0].lines[2].source.file_name().to_string_lossy(), "log1.txt");
		assert_eq!(resolved.panels[0].lines[3].source.file_name().to_string_lossy(), "log2.txt");
	}

	#[test]
	fn test_resolved_source_match_input() {
		let a = PathBuf::from("a");
		let b = PathBuf::from("b");
		let c = PathBuf::from("c");
		let x = PathBuf::from("x");

		let line_source_id = LineSource::FileId(3);
		let line_source_fn = LineSource::FileName(x.clone());
		let line_source_all = LineSource::AllInputFiles;

		assert_eq!(
			ResolvedSource::try_match_input(line_source_id.clone(), 3, &c)
				.unwrap()
				.file_name(),
			&c
		);
		assert_eq!(ResolvedSource::try_match_input(line_source_id.clone(), 2, &c).is_none(), true);

		assert_eq!(ResolvedSource::try_match_input(line_source_fn.clone(), 2, &c).is_none(), true);
		assert_eq!(
			ResolvedSource::try_from_explicit(line_source_fn.clone()).unwrap().file_name(),
			&x
		);

		assert_eq!(
			ResolvedSource::try_match_input(line_source_all.clone(), 1, &a)
				.unwrap()
				.file_name(),
			&a
		);
		assert_eq!(
			ResolvedSource::try_match_input(line_source_all.clone(), 2, &b)
				.unwrap()
				.file_name(),
			&b
		);
		assert_eq!(
			ResolvedSource::try_match_input(line_source_all.clone(), 3, &c)
				.unwrap()
				.file_name(),
			&c
		);
	}
}
