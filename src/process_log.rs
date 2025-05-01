//! The heart of the log processing engine.
//! This module parses raw log files, extracts timestamps and numeric fields, and prepares the data for plotting.
//! It supports value extraction, event counting, deltas, and outputs intermediate CSV caches.

use crate::{
	graph_config::{DataSource, SharedGraphContext, TimestampFormat},
	logging::APPV,
	match_preview_cli_builder::{MatchPreviewConfig, SharedMatchPreviewContext},
	resolved_graph_config::{ResolvedGraphConfig, ResolvedLine},
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, ParseError, TimeDelta};
use regex::Regex;
use std::{
	collections::HashMap,
	fs::{self, File},
	io::{self, BufRead, BufReader, Write},
	path::{Path, PathBuf},
	time::UNIX_EPOCH,
};
use tracing::{Level, debug, info, trace, warn};
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt, layer::SubscriberExt};

const LOG_TARGET: &str = "csv";
pub const MATCH_PREVIEW: &str = "match-preview";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Regex error: {0}")]
	Regex(#[from] regex::Error),

	#[error("I/O Error while accessing file: '{0}': '{1}'")]
	FileIoError(PathBuf, io::Error),

	#[error("Invalid input file '{0}': '{1}'")]
	InvalidInputFile(PathBuf, String),

	#[error("Field regex shall have 1 or 2 capture groups. Regex: {0}")]
	RegexCapturesGroupsInvalidCount(String),

	#[error("User provided time range parsing error: {0}")]
	TimeRangeParsingError(#[from] ParseError),

	#[error("Timestamp extraction failed: file:'{0}' format:'{1:?}', line:'{2}' ")]
	TimestampExtractionFailure(PathBuf, TimestampFormat, String),
}

impl Error {
	fn new_file_io_error(f: &Path, e: io::Error) -> Self {
		Self::FileIoError(f.to_path_buf(), e)
	}
}

/// Stores match-specific state (e.g. counters or timestamps).
#[derive(Debug)]
struct ProcessingState {
	count: u64,
	last_timestamp: Option<ExtractedNaiveDateTime>,
}

/// Single record extracted from a matching log line, with some extra stats.
#[derive(Debug)]
struct LogRecord {
	pub date: Option<String>,
	pub time: String,
	pub value: f64,
	pub count: u64,
	pub diff: Option<f64>,
}

#[derive(Debug)]
struct LineProcessor {
	data_source: DataSource,
	pub regex: Regex,
	pub state: ProcessingState,
	pub records: Vec<LogRecord>,
	pub output_path: Option<PathBuf>,
	pub timestamp_format: TimestampFormat,
	timestamp_extraction_failure_count: usize,
	input_file_name: PathBuf,
}

impl LineProcessor {
	pub fn from_data_source(
		data_source: DataSource,
		output_path: Option<PathBuf>,
		timestamp_format: TimestampFormat,
		input_file_name: PathBuf,
	) -> Result<Self, Error> {
		let regex = data_source.compile_regex()?;
		Ok(Self {
			data_source,
			regex,
			output_path,
			timestamp_format,
			state: ProcessingState::new(),
			records: Vec::new(),
			timestamp_extraction_failure_count: 0,
			input_file_name,
		})
	}

	/// Parses timestamp prefix from the line.
	///
	/// Returns the timestamp and remainder.
	fn extract_timestamp<'a>(
		&self,
		line: &'a str,
	) -> Result<(ExtractedNaiveDateTime, &'a str), ParseError> {
		let result = self.timestamp_format.extract_timestamp(line);
		trace!(target:MATCH_PREVIEW, timestamp_format=?self.timestamp_format, "extract_timestamp");
		// trace!(target:MATCH_PREVIEW, line,  "extract_timestamp");
		debug!(target:MATCH_PREVIEW, result=?result.map(|r|r.0), "extract_timestamp");
		result
	}

	fn handle_timestamp_extraction_failure(&mut self, line: &str) -> Result<(), Error> {
		self.timestamp_extraction_failure_count += 1;

		if self.timestamp_extraction_failure_count > 3 {
			warn!(target:APPV, log_line = line,
				timestamp_format=?self.timestamp_format,
				"Timestamp extraction failed for {} lines. Exiting.", self.timestamp_extraction_failure_count);
			Err(Error::TimestampExtractionFailure(
				self.input_file_name.clone(),
				self.timestamp_format.clone(),
				line.to_string(),
			))
		} else {
			Ok(())
		}
	}

	pub fn guard_matches(&self, log_line: &str) -> bool {
		self.data_source.guard().as_ref().map(|g| log_line.contains(g)).unwrap_or(true)
	}

	pub fn try_match<'a>(
		&mut self,
		line: &'a str,
	) -> Result<(bool, Option<(regex::Captures<'a>, ExtractedNaiveDateTime)>), Error> {
		if self.guard_matches(line) {
			if tracing::event_enabled!(target:MATCH_PREVIEW, Level::TRACE) {
				trace!(target:MATCH_PREVIEW, "try_match: line:\"{line}\"");
			} else {
				info!(target:MATCH_PREVIEW, "try_match: line:\"{line}\"");
			}
			if let Ok((timestamp, remainder)) = self.extract_timestamp(line) {
				let captures = self.regex.captures(remainder).map(|capture| (capture, timestamp));

				if tracing::event_enabled!(Level::TRACE) {
					trace!(target:MATCH_PREVIEW, "try_match remainder={remainder} regex={:#?} captures={captures:#?}", self.regex);
				} else {
					debug!(target:MATCH_PREVIEW, "try_match: line remainder: \"{remainder}\"");
					if let Some((captures, _)) = &captures {
						if let Some(c) = captures.get(1) {
							debug!(target:MATCH_PREVIEW, "try_match: (value) captures[1]={c:?}");
						};
						if let Some(c) = captures.get(2) {
							debug!(target:MATCH_PREVIEW, "try_match:  (unit) captures[2]={c:?}");
						};
					} else {
						debug!(target:MATCH_PREVIEW, "try_match: no matches...");
					}
				}

				Ok((true, captures))
			} else {
				self.handle_timestamp_extraction_failure(line)?;
				Ok((true, None))
			}
		} else {
			Ok((false, None))
		}
	}

	pub fn process(&mut self, caps: regex::Captures, timestamp: ExtractedNaiveDateTime) {
		let date = timestamp.date().map(|d| d.format("%Y-%m-%d").to_string());
		let time = timestamp.time().format("%H:%M:%S%.3f").to_string();
		let count = self.state.next_count();
		let diff = self.state.compute_delta(timestamp);

		let mut value = 1.0;

		match &self.data_source {
			DataSource::EventValue { yvalue, .. } => value = *yvalue,
			DataSource::EventCount { .. } | DataSource::EventDelta { .. } => (),
			DataSource::FieldValue { .. } => {
				let raw_val = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
				let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("");
				value = match normalize_value(raw_val, unit) {
					Some(v) => v,
					None => {
						//add conversion warning (if conversion enabled)
						return;
					},
				};
			},
		}

		self.records.push(LogRecord { date, time, value, count, diff });
	}

	fn write_csv(&self) -> Result<(), Error> {
		let filename = self.expect_output_path();
		let mut file =
			File::create(filename).map_err(|e| Error::FileIoError(filename.clone(), e))?;
		match self.timestamp_format {
			TimestampFormat::Time(_) => {
				writeln!(file, "date,time,value,count,delta")
					.map_err(|e| Error::FileIoError(filename.clone(), e))?;
				for r in &self.records {
					//todo: clean up date
					writeln!(
						file,
						"2025-01-01,{},{},{},{}",
						r.time,
						r.value,
						r.count,
						r.diff.unwrap_or(0.0)
					)
					.map_err(|e| Error::new_file_io_error(filename, e))?;
				}
			},
			TimestampFormat::DateTime(_) => {
				writeln!(file, "date,time,value,count,delta")
					.map_err(|e| Error::new_file_io_error(filename, e))?;
				for r in &self.records {
					writeln!(
						file,
						"{},{},{},{},{}",
						r.date.as_ref().expect("date should be set"),
						r.time,
						r.value,
						r.count,
						r.diff.unwrap_or(0.0)
					)
					.map_err(|e| Error::new_file_io_error(filename, e))?;
				}
			},
		};

		Ok(())
	}

	pub fn expect_output_path(&self) -> &PathBuf {
		self.output_path
			.as_ref()
			.expect("output_path is expected to be set (this is bug")
	}
}

impl ResolvedLine {
	pub fn regex_filename_tag(&self) -> String {
		self.line.data_source.regex_filename_tag()
	}

	pub fn match_token(&self) -> String {
		self.line.data_source.match_token()
	}

	pub fn title(&self, multi_input_files: bool) -> String {
		let file_stem = self
			.source
			.file_name()
			.file_stem()
			.expect("filename is validated at this point")
			.to_string_lossy();
		let title = self.line.params.title.clone().unwrap_or(self.line.data_source.title());
		if multi_input_files { format!("{} ({})", title, file_stem) } else { title }
	}

	pub fn source_file_name(&self) -> &PathBuf {
		self.source.file_name()
	}

	pub fn guard(&self) -> &Option<String> {
		self.line.data_source.guard()
	}

	pub fn csv_data_column_for_plot(&self) -> &'static str {
		self.line.data_source.csv_data_column_for_plot()
	}
}

impl DataSource {
	/// Returns a regex tag used in CSV filename.
	pub fn regex_filename_tag(&self) -> String {
		urlencoding::encode(&self.regex_pattern()).to_string()
	}

	pub fn title(&self) -> String {
		match &self {
			DataSource::FieldValue { .. } => format!("value of {}", self.pattern()),
			DataSource::EventValue { .. } => format!("presence of {}", self.pattern()),
			DataSource::EventCount { .. } => format!("count of {}", self.pattern()),
			DataSource::EventDelta { .. } => format!("delta {}", self.pattern()),
		}
	}

	fn match_token(&self) -> String {
		match &self {
			// DataSource::EventValue { pattern, yvalue, .. } => format!("{}_{}", pattern, yvalue),
			DataSource::EventValue { pattern, .. }
			| DataSource::EventCount { pattern, .. }
			| DataSource::EventDelta { pattern, .. } => pattern.clone(),
			DataSource::FieldValue { field, .. } => field.clone(),
		}
	}

	fn pattern(&self) -> String {
		match &self {
			DataSource::EventValue { pattern, .. }
			| DataSource::EventCount { pattern, .. }
			| DataSource::EventDelta { pattern, .. } => pattern.clone(),
			DataSource::FieldValue { field, .. } => field.clone(),
		}
	}

	/// Checks if regex pattern is valid.
	///
	/// For [`DataSource::FieldValue`] it checks if regex pattern contains a correct number of captures groups.
	/// Otherwise no validation is performed and any pattern is assumed to be correct.
	fn validate_field_regex(&self) -> Result<bool, Error> {
		if let DataSource::FieldValue { field, .. } = &self {
			if let Ok(regex) = Regex::new(field) {
				let captures_len = regex.captures_len() - 1;
				if (1..=2).contains(&captures_len) {
					return Ok(true);
				}
				if captures_len > 2 {
					return Err(Error::RegexCapturesGroupsInvalidCount(field.clone()));
				}
			}
		}
		Ok(false)
	}

	fn is_field_valid_regex(&self) -> bool {
		self.validate_field_regex().unwrap_or(false)
	}

	/// Returns actual regex pattern that will be used for matching events and extracting values.
	fn regex_pattern(&self) -> String {
		match &self {
			DataSource::EventValue { pattern, .. }
			| DataSource::EventCount { pattern, .. }
			| DataSource::EventDelta { pattern, .. } => pattern.clone(),
			DataSource::FieldValue { field, .. } => {
				if self.is_field_valid_regex() {
					field.clone()
				} else {
					format!(r"\b{}=([\d\.]+)(\w+)?", regex::escape(field))
				}
			},
		}
	}

	pub fn compile_regex(&self) -> Result<Regex, Error> {
		self.validate_field_regex()?;
		Regex::new(&self.regex_pattern()).map_err(Into::into)
	}

	pub fn guard(&self) -> &Option<String> {
		match &self {
			DataSource::EventValue { guard, .. }
			| DataSource::EventCount { guard, .. }
			| DataSource::EventDelta { guard, .. }
			| DataSource::FieldValue { guard, .. } => guard,
		}
	}

	pub fn csv_data_column_for_plot(&self) -> &'static str {
		match &self {
			DataSource::FieldValue { .. } | DataSource::EventValue { .. } => "value",
			DataSource::EventCount { .. } => "count",
			DataSource::EventDelta { .. } => "delta",
		}
	}
}

impl ResolvedLine {
	/// Returns if CSV file can be shared.
	///
	/// Some data (like count or delta) can be use other's line results, and do not required
	/// dedicated file processing.
	pub fn can_csv_file_be_shared(&self) -> bool {
		matches!(
			&self.line.data_source,
			DataSource::EventCount { .. } | DataSource::EventDelta { .. }
		)
	}

	/// Generates a unique and consistent CSV filename for this line.
	///
	/// The filename is built from:
	/// - The input log file name
	/// - An optional guard string
	/// - A regex- or field-based identifier specific to the line data source
	///
	/// This naming strategy ensures that multiple lines using the same pattern and guard
	/// will map to the same CSV file, enabling output reuse and avoiding redundant processing.
	pub fn get_csv_filename(&self) -> PathBuf {
		let tag = self.regex_filename_tag();
		let core = match &self.line.data_source {
			DataSource::EventValue { yvalue, .. } => format!("value_{yvalue}_{tag}"),
			DataSource::EventCount { .. } => format!("count_{tag}"),
			DataSource::EventDelta { .. } => format!("delta_{tag}"),
			DataSource::FieldValue { .. } => tag,
		};

		let log_name = self
			.source_file_name()
			.file_name()
			.expect("file path shall be given")
			.to_string_lossy();

		let ts = fs::metadata(self.source_file_name())
			.and_then(|m| m.modified())
			.map_err(|_| ())
			.and_then(|t| t.duration_since(UNIX_EPOCH).map_err(|_| ()))
			.map(|d| d.as_secs().to_string())
			.unwrap_or_else(|_| "nots".to_string());

		PathBuf::from(if let Some(guard) = self.line.data_source.guard() {
			format!("{log_name}_{ts}__{guard}__{core}.csv")
		} else {
			format!("{log_name}_{ts}__{core}.csv")
		})
	}
}

/// Result will contain exactly the lines that needs to be processed against the log.
/// It will be deduplicated
fn propagate_shared_csv_files<F>(
	config: &mut ResolvedGraphConfig,
	shared_context: &SharedGraphContext,
	get_cache_dir: F,
) -> Result<HashMap<PathBuf, ResolvedLine>, Error>
where
	F: Fn(&SharedGraphContext, &PathBuf) -> Result<PathBuf, Error>,
{
	type MatchKey = (Option<String>, String, PathBuf);

	let mut grouped_lines: HashMap<MatchKey, Vec<&mut ResolvedLine>> = HashMap::new();

	for panel in &mut config.panels {
		for line in &mut panel.lines {
			let guard = line.guard().clone();
			let token = line.match_token();
			let input = line.source_file_name().clone();

			grouped_lines.entry((guard, token, input)).or_default().push(line);
		}
	}

	trace!(target: LOG_TARGET,  "propagete_shared_csv_files {:#?}", grouped_lines);

	let mut canonicals: HashMap<PathBuf, ResolvedLine> = Default::default();

	for ((_, _, input_filename), mut lines) in grouped_lines {
		for line in &mut lines {
			let output_dir = get_cache_dir(shared_context, &input_filename)?;

			let csv_output_path = output_dir.join(line.get_csv_filename());
			line.set_shared_csv_filename(&csv_output_path);
		}

		let canonical = lines
			.iter()
			.find(|l| matches!(l.line.data_source, DataSource::FieldValue { .. }))
			.or(lines
				.iter()
				.find(|l| matches!(l.line.data_source, DataSource::EventValue { .. })))
			.unwrap_or(&lines[0]);

		canonicals.insert((*canonical).expect_shared_csv_filename(), (*canonical).clone());

		let shared_path = canonical.expect_shared_csv_filename();

		trace!(target: LOG_TARGET,  "propagete_shared_csv_files canonical {:#?}", shared_path);

		for line in lines {
			if line.can_csv_file_be_shared() {
				line.set_shared_csv_filename(&shared_path);
			} else {
				canonicals
					.entry(line.expect_shared_csv_filename())
					.or_insert_with(|| (*line).clone());
			}
		}
	}

	trace!(target: LOG_TARGET,  "propagete_shared_csv_files cannonicals: {:#?}", canonicals);

	Ok(canonicals)
}

/// Processes a log file and writes CSVs based on the graph config.
pub fn process_inputs(
	config: &mut ResolvedGraphConfig,
	shared_context: &SharedGraphContext,
) -> Result<(), Error> {
	let mut canonical_lines =
		propagate_shared_csv_files(config, shared_context, |shared_context, input_file_name| {
			shared_context.get_cache_dir(input_file_name)
		})?;

	trace!(target: LOG_TARGET,  "after propagete_shared_csv_files {:#?}", config);

	// input_log_file ->  map( output_path -> processor)
	let mut processors: HashMap<PathBuf, HashMap<PathBuf, LineProcessor>> = Default::default();

	for line in config.all_lines() {
		let csv_output_path = line.expect_shared_csv_filename();

		let output_dir: PathBuf = csv_output_path
			.parent()
			.expect("CSV file shall be resolved to path with at least one parent")
			.into();
		if !output_dir.exists() {
			std::fs::create_dir_all(&output_dir)
				.map_err(|e| Error::new_file_io_error(&output_dir, e))?;
		}

		if !shared_context.force_csv_regen && Path::new(&csv_output_path).exists() {
			debug!(
				target: APPV,
				"Using cached file for regex: {} file: {}",
				line.line.data_source.regex_pattern(),
				csv_output_path.display(),
			);
			continue;
		}

		if let Some(canonical_line) = canonical_lines.remove(&csv_output_path) {
			let processor = LineProcessor::from_data_source(
				canonical_line.line.data_source.clone(),
				Some(csv_output_path),
				shared_context.timestamp_format().clone(),
				canonical_line.source_file_name().clone(),
			)?;

			processors
				.entry(canonical_line.source_file_name().clone())
				.or_default()
				.entry(processor.expect_output_path().clone())
				.or_insert(processor);
		}
	}

	trace!(target: LOG_TARGET,  "process_inputs readers: {:#?}", processors);

	// Iterate over log lines
	for (log_file_name, mut processors) in processors {
		if !log_file_name.is_file() {
			return Err(Error::InvalidInputFile(log_file_name, "Not a regular file".to_string()));
		}
		let input_file =
			File::open(&log_file_name).map_err(|e| Error::new_file_io_error(&log_file_name, e))?;
		let reader = BufReader::new(input_file);
		for line in reader.lines().map_while(Result::ok) {
			for processor in &mut processors.values_mut() {
				if let (_, Some((captures, timestamp))) = processor.try_match(&line)? {
					processor.process(captures, timestamp);
				}
			}
		}
		// Write all output files
		for (_, processor) in processors {
			assert_eq!(log_file_name, processor.input_file_name);
			if processor.records.len() == 0 {
				warn!(
					target:APPV,
					input_file = ?log_file_name.display(),
					guard = ?processor.data_source.guard(),
					regex = processor.data_source.regex_pattern(),
					"No matches."
				);
			} else {
				debug!(
					target:APPV,
					"Processed input file: {}, regex: {}, matched {}, cache file: {}",
					log_file_name.display(),
					processor.data_source.regex_pattern(),
					processor.records.len(),
					processor.expect_output_path().display()
				);
			}
			processor.write_csv()?;
		}
	}

	config.resolve_data_points_count()?;

	Ok(())
}

pub fn regex_match_preview(
	config: MatchPreviewConfig,
	context: SharedMatchPreviewContext,
) -> Result<(), Error> {
	let env_filter = if context.verbose {
		EnvFilter::new(format!("warn,{}=trace", MATCH_PREVIEW))
	} else {
		EnvFilter::new(format!("warn,{}=debug", MATCH_PREVIEW))
	};

	let preview_layer = fmt::layer().without_time().with_target(false).with_level(true);
	let preview_subscriber = Registry::default().with(preview_layer.with_filter(env_filter));

	tracing::subscriber::with_default(preview_subscriber, || {
		regex_match_preview_inner(config, context)
	})
}

pub fn regex_match_preview_inner(
	config: MatchPreviewConfig,
	context: SharedMatchPreviewContext,
) -> Result<(), Error> {
	let mut processor = LineProcessor::from_data_source(
		config.data_source.clone(),
		None,
		context.timestamp_format().clone(),
		context.input.clone(),
	)?;

	let input_file =
		File::open(&context.input).map_err(|e| Error::FileIoError(context.input.clone(), e))?;
	let reader = BufReader::new(input_file);
	let mut matched_count = 0;

	info!(target:MATCH_PREVIEW, "input file: {}", context.input.display());
	if let Some(guard) = config.data_source.guard().as_ref() {
		info!(target:MATCH_PREVIEW, "guard: {guard}")
	};
	info!(target:MATCH_PREVIEW, "regex pattern: {}", config.data_source.regex_pattern());
	info!(target:MATCH_PREVIEW, "timestamp pattern: {:?}", context.timestamp_format);

	for line in reader.lines().map_while(Result::ok) {
		let (guard_matched, captured) = processor.try_match(&line)?;
		if guard_matched {
			if let Some((captures, timestamp)) = captured {
				processor.process(captures, timestamp);
				info!(target:MATCH_PREVIEW, "matched: {:#?}", processor.records.last());
			}

			matched_count += 1;
		}
		if matched_count >= context.count {
			break;
		}
	}

	if matched_count == 0 {
		if let Some(guard) = config.data_source.guard() {
			warn!(target:MATCH_PREVIEW, "No lines matched against guard: '{:?}'", guard);
			warn!(target:MATCH_PREVIEW, "Is it correctly configured?");
		}
	}
	Ok(())
}

impl ResolvedGraphConfig {
	pub fn resolve_data_points_count(&mut self) -> Result<(), Error> {
		for panel in &mut self.panels {
			for line in &mut panel.lines {
				let file_path = line.expect_shared_csv_filename();
				let file =
					File::open(&file_path).map_err(|e| Error::new_file_io_error(&file_path, e))?;
				let reader = io::BufReader::new(file);

				line.set_data_points_count(reader.lines().count() - 1);
			}
		}
		Ok(())
	}
}

/// Converts value+unit to milliseconds.
fn normalize_value(value: &str, unit: &str) -> Option<f64> {
	let base: f64 = value.parse().ok()?;
	match unit {
		"s" => Some(base * 1000.0),
		"ms" => Some(base),
		"us" | "µs" => Some(base / 1000.0),
		"ns" => Some(base / 1000000.0),
		"microseconds" => Some(base / 1000.0),
		_ => Some(base),
	}
}

impl ProcessingState {
	fn new() -> Self {
		Self { count: 0, last_timestamp: None }
	}

	fn next_count(&mut self) -> u64 {
		self.count += 1;
		self.count
	}

	fn compute_delta(&mut self, current: ExtractedNaiveDateTime) -> Option<f64> {
		let diff = self
			.last_timestamp
			.map(|prev| current.signed_duration_since(prev).num_milliseconds() as f64);
		self.last_timestamp = Some(current);
		diff
	}
}

#[derive(Clone, Copy, Debug)]
pub enum ExtractedNaiveDateTime {
	DateTime(NaiveDateTime),
	Time(NaiveTime),
}

impl ExtractedNaiveDateTime {
	fn date(&self) -> Option<NaiveDate> {
		match self {
			Self::Time(_) => None,
			Self::DateTime(v) => Some(v.date()),
		}
	}

	fn time(&self) -> NaiveTime {
		match self {
			Self::Time(v) => *v,
			Self::DateTime(v) => v.time(),
		}
	}
	pub const fn signed_duration_since(self, rhs: ExtractedNaiveDateTime) -> TimeDelta {
		match (self, rhs) {
			(Self::Time(v), Self::Time(rhs)) => v.signed_duration_since(rhs),
			(Self::DateTime(v), Self::DateTime(rhs)) => v.signed_duration_since(rhs),
			_ => panic!("should not happen"),
		}
	}
}

impl TimestampFormat {
	fn extract_timestamp<'a>(
		&self,
		line: &'a str,
	) -> Result<(ExtractedNaiveDateTime, &'a str), ParseError> {
		Ok(match self {
			TimestampFormat::Time(fmt) => NaiveTime::parse_and_remainder(line, fmt)
				.map(|v| (ExtractedNaiveDateTime::Time(v.0), v.1))?,
			TimestampFormat::DateTime(fmt) => {
				let mut parsed = chrono::format::Parsed::new();
				let remainder = chrono::format::parse_and_remainder(
					&mut parsed,
					line,
					chrono::format::StrftimeItems::new(fmt),
				)?;

				trace!(target:MATCH_PREVIEW, ?parsed, "extract_timestamp");

				let dt = match parsed.to_naive_datetime_with_offset(0) {
					Ok(dt) => dt,
					_ => {
						//hack: this may need some rethink / clean up
						//todo: clean up date
						if parsed.year().is_none() {
							parsed.set_year(2025)?;
						}
						parsed.to_naive_datetime_with_offset(0)?
					},
				};

				(ExtractedNaiveDateTime::DateTime(dt), remainder)

				// NaiveDateTime::parse_and_remainder(line, fmt)
				// 	.map(|v| (ExtractedNaiveDateTime::DateTime(v.0), v.1))?
			},
		})
	}
}

impl SharedGraphContext {
	fn common_path_ancestor(paths: &[PathBuf]) -> Option<PathBuf> {
		let canonicalized: Result<Vec<_>, _> = paths.iter().map(|p| p.canonicalize()).collect();
		Self::common_path_ancestor_inner(&canonicalized.ok()?)
	}

	fn common_path_ancestor_inner(paths: &[PathBuf]) -> Option<PathBuf> {
		if paths.is_empty() {
			return None;
		}

		let mut iter = paths.iter();
		let first = iter.next()?;

		let mut components: Vec<_> =
			first.parent().expect("shall be full path here").components().collect();

		for path in iter {
			let mut new_components = Vec::new();
			for (a, b) in components
				.iter()
				.zip(path.parent().expect("shall be full path here").components())
			{
				if a == &b {
					new_components.push(*a);
				} else {
					break;
				}
			}
			if new_components.is_empty() {
				return None;
			}
			components = new_components;
		}

		let ancestor = components.iter().fold(PathBuf::new(), |mut acc, comp| {
			acc.push(comp.as_os_str());
			acc
		});

		Some(ancestor)
	}

	/// Returns tuple containging the path to the image and the path to the gnuplot script
	pub fn get_graph_output_path(&self) -> (PathBuf, PathBuf) {
		if let Some(ref output_file) = self.inline_output {
			let common_ancestor =
				Self::common_path_ancestor(&self.input).unwrap_or_else(|| PathBuf::from("./"));
			let image_path = common_ancestor.join(output_file);
			let gnuplot_path = image_path.with_extension("gnuplot");
			(image_path, gnuplot_path)
		} else {
			let def = PathBuf::from("graph.png");
			let output_file = self.output.as_ref().unwrap_or(&def);
			let image_path = PathBuf::from(".").join(output_file);
			let gnuplot_path = image_path.with_extension("gnuplot");
			(image_path, gnuplot_path)
		}
	}

	/// Returns the configured root directory for storing cache files, if provided by the user.
	///
	/// This corresponds to the `--cache-dir` CLI option. If `None`, per-log `.plox/` directories
	/// will be used instead. The returned path does not include any log-specific subdirectories.
	fn get_cache_root(&self) -> &Option<PathBuf> {
		&self.cache_dir
	}

	/// Returns the directory where the cache file for a given log file should be stored.
	///
	/// If a global `--cache-dir` is provided, the full canonical path of the log file is
	/// reproduced as a subdirectory inside it. For example:
	///   log: `/var/log/app/debug.log`
	///   cache-dir: `~/.cache/plox`
	///   result: `~/.cache/plox/var/log/app/`
	///
	/// If no `--cache-dir` is given, a `.plox/` directory is created next to the log file:
	///   log: `./logs/debug.log`
	///   result: `./logs/.plox/`
	///
	/// The log file must exist and be canonicalizable; otherwise this function returns an error.
	pub fn get_cache_dir(&self, log_file: &Path) -> Result<PathBuf, Error> {
		let log_file_path =
			log_file.canonicalize().map_err(|e| Error::new_file_io_error(log_file, e))?; // fails if file doesn't exist
		self.get_cache_dir_inner(&log_file_path)
	}

	fn get_cache_dir_inner(&self, log_file_path: &Path) -> Result<PathBuf, Error> {
		assert!(log_file_path.is_absolute());
		if let Some(root) = self.get_cache_root() {
			// Strip leading `/` to build a relative path under the root
			let relative = log_file_path.strip_prefix("/").unwrap_or(log_file_path);
			Ok(root.join(relative).parent().unwrap_or(root).to_path_buf())
		} else {
			let log_dir = log_file_path.parent().unwrap_or_else(|| Path::new("."));
			Ok(log_dir.join(".plox"))
		}
	}
}

#[cfg(test)]
mod tests {
	use chrono::{NaiveDate, NaiveTime};

	use crate::{
		graph_config::{DEFAULT_TIMESTAMP_FORMAT, Line},
		logging::init_tracing_test,
		resolved_graph_config::ResolvedPanel,
	};

	use super::*;

	fn build_resolved_graph_config(lines: Vec<ResolvedLine>) -> ResolvedGraphConfig {
		ResolvedGraphConfig { panels: vec![ResolvedPanel::new_with_lines(lines)] }
	}

	fn event_line(
		input_file: &'static str,
		guard: Option<&'static str>,
		field: &'static str,
		yvalue: f64,
	) -> ResolvedLine {
		ResolvedLine::from_explicit_name(
			Line::new_with_data_source(DataSource::new_event_value(
				guard.map(Into::into),
				field.into(),
				yvalue,
			)),
			PathBuf::from(input_file),
		)
	}

	fn event_delta_line(
		input_file: &'static str,
		guard: Option<&'static str>,
		field: &'static str,
	) -> ResolvedLine {
		ResolvedLine::from_explicit_name(
			Line::new_with_data_source(DataSource::new_event_delta(
				guard.map(Into::into),
				field.into(),
			)),
			PathBuf::from(input_file),
		)
	}

	fn event_count_line(
		input_file: &'static str,
		guard: Option<&'static str>,
		field: &'static str,
	) -> ResolvedLine {
		ResolvedLine::from_explicit_name(
			Line::new_with_data_source(DataSource::new_event_count(
				guard.map(Into::into),
				field.into(),
			)),
			PathBuf::from(input_file),
		)
	}

	fn plot_line(
		input_file: &'static str,
		guard: Option<&'static str>,
		field: &'static str,
	) -> ResolvedLine {
		ResolvedLine::from_explicit_name(
			Line::new_with_data_source(DataSource::new_plot_field(
				guard.map(Into::into),
				field.into(),
			)),
			PathBuf::from(input_file),
		)
	}

	fn check_output_and_config(
		config: ResolvedGraphConfig,
		output: HashMap<PathBuf, ResolvedLine>,
		expected_output_len: usize,
		allow_shared_lines_in_output: bool,
	) {
		// Make sure output len is correct
		assert_eq!(
			output.len(),
			expected_output_len,
			"Output len mismatch e:{}/a:{}",
			expected_output_len,
			output.len()
		);
		// Make sure all shared names are set in config
		for value in config.all_lines() {
			value.expect_shared_csv_filename();
		}
		// Make sure all keys are matched to the files in values.
		for (output_file_name, canonical) in &output {
			assert_eq!(*output_file_name, canonical.expect_shared_csv_filename());
			if !allow_shared_lines_in_output {
				assert!(!canonical.can_csv_file_be_shared(), "no shared lines allowed in output");
			}
		}
		// Ensure no duplicate values from method: get_shared_csv_filename()
		let mut seen_filenames = std::collections::HashSet::new();
		for value in output.values() {
			let filename = value.expect_shared_csv_filename();
			assert!(
				seen_filenames.insert(filename.clone()),
				"Duplicate filename detected: {}",
				filename.display()
			);
		}
		//check that shared names are properly propagated to the lines that can accept shared file.
		for line in config.all_lines() {
			let mut allowed_canonical_names = vec![];
			for (output_file_name, canonical) in &output {
				if line.match_token() == canonical.match_token()
					&& line.guard() == canonical.guard()
					&& line.source_file_name() == canonical.source_file_name()
				{
					allowed_canonical_names.push(output_file_name.clone());
				}
			}
			assert!(allowed_canonical_names.contains(&line.expect_shared_csv_filename()));
		}
		//make sure that all shared files from config are in output
		for line in config.all_lines() {
			let shared_csv_file = line.expect_shared_csv_filename();
			assert!(
				output.contains_key(&shared_csv_file),
				"Output should contain shared_csv_file: {}",
				shared_csv_file.display()
			);
		}
	}
	fn call_propagate_shared_csv_files(
		config: &mut ResolvedGraphConfig,
	) -> Result<HashMap<PathBuf, ResolvedLine>, Error> {
		let shared_context = SharedGraphContext::new_with_input(vec![PathBuf::from("input.log")]);
		propagate_shared_csv_files(config, &shared_context, |_, _| {
			Ok(PathBuf::from("/some/out/dir"))
		})
	}

	#[test]
	fn test_csv_resolution_00() {
		init_tracing_test();
		let mut config =
			build_resolved_graph_config(vec![plot_line("input.log", Some("guard"), "duration")]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 1, false);
	}

	#[test]
	fn test_csv_resolution_00a() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard"), "duration"),
			event_count_line("input.log", Some("guard"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 1, false);
	}

	#[test]
	fn test_csv_resolution_00b() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard"), "duration"),
			event_count_line("input.log", Some("guard"), "duration"),
			event_delta_line("input.log", Some("guard"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 1, false);
	}

	#[test]
	fn test_csv_resolution_01() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard0"), "duration"),
			plot_line("input.log", Some("guard1"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 2, false);
	}

	#[test]
	fn test_csv_resolution_03() {
		init_tracing_test();

		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard0"), "duration"),
			event_line("input.log", Some("guard1"), "duration", 100.0),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 2, false);
	}

	#[test]
	fn test_csv_resolution_04() {
		init_tracing_test();

		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard0"), "duration"),
			event_line("input.log", Some("guard1"), "duration", 100.0),
			event_count_line("input.log", Some("guard1"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 2, false);
	}

	#[test]
	fn test_csv_resolution_05() {
		init_tracing_test();

		let mut config = build_resolved_graph_config(vec![event_count_line(
			"input.log",
			Some("guard1"),
			"duration",
		)]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 1, true);
	}

	#[test]
	fn test_csv_resolution_06() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			event_count_line("input.log", Some("guard1"), "duration1"),
			event_delta_line("input.log", Some("guard1"), "duration2"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 2, true);
	}

	#[test]
	fn test_csv_resolution_07() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard1"), "duration"),
			event_line("input.log", Some("guard1"), "duration", 100.0),
			event_count_line("input.log", Some("guard1"), "duration"),
			event_delta_line("input.log", Some("guard1"), "duration"),
			plot_line("input.log", Some("guard2"), "duration"),
			event_line("input.log", Some("guard2"), "duration", 100.0),
			event_count_line("input.log", Some("guard2"), "duration"),
			event_delta_line("input.log", Some("guard2"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 4, false);
	}

	#[test]
	fn test_csv_resolution_08() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			plot_line("input1.log", Some("guard"), "duration"),
			event_line("input1.log", Some("guard"), "duration", 100.0),
			event_count_line("input1.log", Some("guard"), "duration"),
			event_delta_line("input1.log", Some("guard"), "duration"),
			plot_line("input2.log", Some("guard"), "duration"),
			event_line("input2.log", Some("guard"), "duration", 100.0),
			event_count_line("input2.log", Some("guard"), "duration"),
			event_delta_line("input2.log", Some("guard"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 4, false);
	}

	#[test]
	fn test_csv_resolution_09() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			event_count_line("input1.log", Some("guard1"), "duration"),
			event_delta_line("input1.log", Some("guard2"), "duration"),
			event_count_line("input2.log", Some("guard1"), "duration"),
			event_delta_line("input2.log", Some("guard2"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 4, true);
	}

	#[test]
	fn test_csv_resolution_10() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			plot_line("input.log", Some("guard"), "duration"),
			plot_line("input.log", Some("guard"), "duration"),
			plot_line("input.log", Some("guard"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 1, false);
	}

	#[test]
	fn test_csv_resolution_11() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			event_line("input.log", Some("guard"), "duration", 1.0),
			event_line("input.log", Some("guard"), "duration", 2.0),
			event_line("input.log", Some("guard"), "duration", 3.0),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 3, false);
	}

	#[test]
	fn test_csv_resolution_12() {
		init_tracing_test();
		let mut config = build_resolved_graph_config(vec![
			event_count_line("input.log", Some("guard"), "duration"),
			event_count_line("input.log", Some("guard"), "duration"),
			event_count_line("input.log", Some("guard"), "duration"),
		]);
		let output = call_propagate_shared_csv_files(&mut config).unwrap();
		check_output_and_config(config, output, 1, true);
	}

	#[test]
	fn test_line_processing_00() {
		init_tracing_test();
		let log_line = "2025-04-03 11:32:48.027 INFO main: operation duration=12.5ms";

		let resolved_line = plot_line("input.log", Some("operation"), "duration");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			DEFAULT_TIMESTAMP_FORMAT,
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);
		processor.process(captures, timestamp);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 12.5);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_single_line_check() {
		init_tracing_test();
		let log_line = "2025-04-03 11:32:48.027 INFO main: operation duration:12.5us, val:127.0ms";
		let resolved_line = plot_line("input.log", Some("operation"), r"duration:([\d\.]+)(\w+)?");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			DEFAULT_TIMESTAMP_FORMAT,
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);
		processor.process(captures, timestamp);
		tracing::info!("{:#?}", timestamp);

		let d = NaiveDate::from_ymd_opt(2025, 4, 3).unwrap();
		let t = NaiveTime::from_hms_milli_opt(11, 32, 48, 27).unwrap();

		assert_eq!(timestamp.date().unwrap(), d);
		assert_eq!(timestamp.time(), t);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 0.0125);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_date_format_no_year2() {
		init_tracing_test();
		let log_line = "Apr 20 08:26:13 AM  1000     25131   6737.00      3.17 817575604 3179060   2.41  polkadot-parach";
		let resolved_line =
			plot_line("input.log", Some("polkadot-parach"), r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"%b %d %I:%M:%S %p".into(),
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);

		let d = NaiveDate::from_ymd_opt(2025, 4, 20).unwrap();
		let t = NaiveTime::from_hms_opt(8, 26, 13).unwrap();
		assert_eq!(timestamp.date().unwrap(), d);
		assert_eq!(timestamp.time(), t);

		processor.process(captures, timestamp);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 3.17);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_date_format_seconds_since_epoch() {
		init_tracing_test();
		let log_line = "[1577834199]  1000     25131   6737.00      3.17 817575604 3179060   2.41  polkadot-parach";
		let resolved_line =
			plot_line("input.log", Some("polkadot-parach"), r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"[%s]".into(),
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);

		let d = NaiveDate::from_ymd_opt(2019, 12, 31).unwrap();
		let t = NaiveTime::from_hms_opt(23, 16, 39).unwrap();
		assert_eq!(timestamp.date().unwrap(), d);
		assert_eq!(timestamp.time(), t);

		processor.process(captures, timestamp);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 3.17);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	#[ignore]
	fn test_line_processing_date_format_seconds_since_epoch2() {
		init_tracing_test();
		let log_line = "[636152.333]  1000     25131   6737.00      3.17 817575604 3179060   2.41  polkadot-parach";
		let resolved_line =
			plot_line("input.log", Some("polkadot-parach"), r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"[%s.3f]".into(),
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);

		let d = NaiveDate::from_ymd_opt(2019, 12, 31).unwrap();
		let t = NaiveTime::from_hms_opt(23, 16, 39).unwrap();
		assert_eq!(timestamp.date().unwrap(), d);
		assert_eq!(timestamp.time(), t);

		processor.process(captures, timestamp);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 3.17);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_date_format_no_year() {
		init_tracing_test();
		let log_line = "035 08:26:13 AM  1000     25131   6737.00      3.17 817575604 3179060   2.41  polkadot-parach";
		let resolved_line =
			plot_line("input.log", Some("polkadot-parach"), r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"%j %I:%M:%S %p".into(),
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);

		let d = NaiveDate::from_ymd_opt(2025, 2, 4).unwrap();
		let t = NaiveTime::from_hms_opt(8, 26, 13).unwrap();
		assert_eq!(timestamp.date().unwrap(), d);
		assert_eq!(timestamp.time(), t);

		processor.process(captures, timestamp);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 3.17);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_date_format() {
		init_tracing_test();
		let log_line = "2025 035 08:26:13 AM  1000     25131   6737.00      3.17 817575604 3179060   2.41  polkadot-parach";
		let resolved_line =
			plot_line("input.log", Some("polkadot-parach"), r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"%Y %j %I:%M:%S %p".into(),
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);

		let d = NaiveDate::from_ymd_opt(2025, 2, 4).unwrap();
		let t = NaiveTime::from_hms_opt(8, 26, 13).unwrap();
		assert_eq!(timestamp.date().unwrap(), d);
		assert_eq!(timestamp.time(), t);

		processor.process(captures, timestamp);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 3.17);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_time_only() {
		init_tracing_test();
		let log_line = "08:26:13 AM  1000     25131   6737.00      3.17 817575604 3179060   2.41  polkadot-parach";
		let resolved_line =
			plot_line("input.log", Some("polkadot-parach"), r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"%I:%M:%S %p".into(),
			"input.log".into(),
		)
		.unwrap();

		assert!(processor.guard_matches(log_line));
		let (g, matched) = processor.try_match(log_line).unwrap();
		let (captures, timestamp) = matched.unwrap();
		assert!(g);
		processor.process(captures, timestamp);

		let t = NaiveTime::from_hms_opt(8, 26, 13).unwrap();

		assert!(timestamp.date().is_none());
		assert_eq!(timestamp.time(), t);

		assert_eq!(processor.records.len(), 1);
		let record = &processor.records[0];
		assert_eq!(record.value, 3.17);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
	}

	#[test]
	fn test_line_processing_multi_line_field() {
		init_tracing_test();
		let log_lines = [
			"2025-04-03 11:32:48.027 INFO main: operation duration=1.5",
			"2025-04-03 11:32:48.054 INFO main: operation duration=2.5",
			"2025-04-03 11:32:49.054 INFO main: operation duration=3.5",
			"2025-04-03 11:33:49.154 INFO main: operation duration=4.5",
			"2025-04-04 11:33:49.154 INFO main: operation duration=2.5",
		];

		let resolved_line = plot_line("input.log", Some("operation"), r"duration");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			DEFAULT_TIMESTAMP_FORMAT,
			"input.log".into(),
		)
		.unwrap();

		for log_line in log_lines {
			assert!(processor.guard_matches(log_line));
			let (g, matched) = processor.try_match(log_line).unwrap();
			let (captures, timestamp) = matched.unwrap();
			assert!(g);
			processor.process(captures, timestamp);
		}

		assert_eq!(processor.records.len(), 5);
		let record = &processor.records[0];
		assert_eq!(record.value, 1.5);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
		let record = &processor.records[1];
		assert_eq!(record.value, 2.5);
		assert_eq!(record.count, 2);
		assert_eq!(record.diff.unwrap(), 27.0);
		let record = &processor.records[2];
		assert_eq!(record.value, 3.5);
		assert_eq!(record.count, 3);
		assert_eq!(record.diff.unwrap(), 1000.0);
		let record = &processor.records[3];
		assert_eq!(record.value, 4.5);
		assert_eq!(record.count, 4);
		assert_eq!(record.diff.unwrap(), 60100.0);
		let record = &processor.records[4];
		assert_eq!(record.value, 2.5);
		assert_eq!(record.count, 5);
		assert_eq!(record.diff.unwrap(), 86400000.0);
	}

	#[test]
	fn test_line_processing_multi_line_regex() {
		init_tracing_test();
		let log_lines = [
			"2025-04-03 11:32:48.027 INFO main: operation duration:1.5ns, val:127.0",
			"2025-04-03 11:32:48.054 INFO main: operation duration:2.5us, val:127.0",
			"2025-04-03 11:32:49.054 INFO main: operation duration:3.5ms, val:127.0",
			"2025-04-03 11:33:49.154 INFO main: operation duration:4.5s, val:127.0",
			"2025-04-04 11:33:49.154 INFO main: operation duration:2.5s, val:127.0",
		];

		let resolved_line = plot_line("input.log", Some("operation"), r"duration:([\d\.]+)(\w+)?");

		let mut processor = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			DEFAULT_TIMESTAMP_FORMAT,
			"input.log".into(),
		)
		.unwrap();

		for log_line in log_lines {
			assert!(processor.guard_matches(log_line));
			let (g, matched) = processor.try_match(log_line).unwrap();
			let (captures, timestamp) = matched.unwrap();
			assert!(g);
			processor.process(captures, timestamp);
		}

		assert_eq!(processor.records.len(), 5);
		let record = &processor.records[0];
		assert_eq!(record.value, 1.5 / 1_000_000.0);
		assert_eq!(record.count, 1);
		assert_eq!(record.diff, None);
		let record = &processor.records[1];
		assert_eq!(record.value, 2.5 / 1000.0);
		assert_eq!(record.count, 2);
		assert_eq!(record.diff.unwrap(), 27.0);
		let record = &processor.records[2];
		assert_eq!(record.value, 3.5);
		assert_eq!(record.count, 3);
		assert_eq!(record.diff.unwrap(), 1000.0);
		let record = &processor.records[3];
		assert_eq!(record.value, 4500.0);
		assert_eq!(record.count, 4);
		assert_eq!(record.diff.unwrap(), 60100.0);
		let record = &processor.records[4];
		assert_eq!(record.value, 2500.0);
		assert_eq!(record.count, 5);
		assert_eq!(record.diff.unwrap(), 86400000.0);
	}

	#[test]
	fn test_line_processing_bad_regex() {
		//3 captures group are incorrect
		let r = r"^\s+(?:[\d\.]+\s+){3}([\d\.]+)([\d\.]+)([\d\.]+)";
		init_tracing_test();
		let resolved_line = plot_line("input.log", Some("polkadot-parach"), r);

		let err = LineProcessor::from_data_source(
			resolved_line.line.data_source,
			Some(PathBuf::from("output.csv")),
			"%Y %j %I:%M:%S %p".into(),
			"input.log".into(),
		)
		.unwrap_err();

		if let Error::RegexCapturesGroupsInvalidCount(x) = err {
			assert_eq!(x, r);
		} else {
			panic!("incorrect error value");
		}
	}

	#[test]
	fn test_common_path_ancestor() {
		init_tracing_test();
		let p1 = PathBuf::from("/a/b/c/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a/b"));
		let p1 = PathBuf::from("/a/b/d/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a/b/d"));
		let p1 = PathBuf::from("/a/b/d/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a/b/d"));
		let p1 = PathBuf::from("/a/c/d/log1");
		let p2 = PathBuf::from("/a/b/d/log2");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/a"));
		let p1 = PathBuf::from("/a/c/d/log1");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1]).unwrap();
		assert_eq!(r, PathBuf::from("/a/c/d/"));
		let p1 = PathBuf::from("/log1");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1]).unwrap();
		assert_eq!(r, PathBuf::from("/"));
		let p1 = PathBuf::from("/log1");
		let p2 = PathBuf::from("/log2");
		let r = SharedGraphContext::common_path_ancestor_inner(&[p1, p2]).unwrap();
		assert_eq!(r, PathBuf::from("/"));
	}
}
