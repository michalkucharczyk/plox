use crate::{
	graph_config::{
		PanelAlignmentMode, PanelAlignmentModeArg, PanelRangeMode, SharedGraphContext,
		TimeRangeArg, TimestampFormat,
	},
	logging::APPV,
	resolved_graph_config::{ResolvedGraphConfig, ResolvedPanel},
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::{
	fs::File,
	io::{self, BufRead, BufReader},
	path::PathBuf,
};
use tracing::{debug, trace};

const LOG_TARGET: &str = "range";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("I/O error: {0}")]
	IoError(#[from] io::Error),
	#[error("Error while accesing file '{0}': {1}")]
	FileError(PathBuf, io::Error),
	#[error("Aligning ranges error: {0}")]
	Generic(String),
	#[error("Error while parsing CVS date: {0} (this is bug)")]
	CvsDateParseError(#[from] chrono::ParseError),
	#[error("Empty ranges for all lines. No data or bad timestamp or bad regex?")]
	EmptyRangeError,
}

fn csv_range_from_file(path: &PathBuf) -> Result<Option<(NaiveDateTime, NaiveDateTime)>, Error> {
	fn parse_timestamp(date: &str, time: &str) -> Result<NaiveDateTime, Error> {
		let dt = format!("{} {}", date.trim(), time.trim());
		//todo: clean up date
		// This is how [`LineProcessor::process`] stores the date and time.
		let ts = NaiveDateTime::parse_and_remainder(&dt, "%Y-%m-%d %H:%M:%S%.f")?;
		Ok(ts.0)
	}

	let mut lines = BufReader::new(File::open(path)?).lines();

	let Some(start_line) = lines.nth(1) else { return Ok(None) };
	let start_line = start_line.map_err(|e| Error::FileError(path.clone(), e))?;

	let (start_date, start_time) = start_line
		.split_once(',')
		.ok_or_else(|| Error::Generic("Malformed start line".into()))?;

	let start = parse_timestamp(start_date, start_time)?;

	let Some(end_line) = lines.last() else { return Ok(Some((start, start))) };
	let end_line = end_line.map_err(|e| Error::FileError(path.clone(), e))?;

	let (end_date, end_time) = end_line
		.split_once(',')
		.ok_or_else(|| Error::Generic("Malformed start line".into()))?;

	let end = parse_timestamp(end_date, end_time)?;

	Ok(Some((start, end)))
}

impl ResolvedGraphConfig {
	/// Sets the time range for every line in the config.
	///
	/// This reads a shared cvs files, and extracts the time range for every line in config.
	/// Requires the CSV files to be resolved.
	pub fn populate_line_ranges(&mut self) -> Result<(), Error> {
		for panel in &mut self.panels {
			for line in &mut panel.lines {
				if let Some(range) = csv_range_from_file(&line.expect_shared_csv_filename())? {
					line.set_time_range(range.0, range.1);
				} else {
					debug!(target:LOG_TARGET, "empty CSV time range for line: {:#?}", line);
				}
			}
		}

		Ok(())
	}

	/// Returns a global range for all lines.
	pub fn global_time_range(&self) -> Result<(NaiveDateTime, NaiveDateTime), Error> {
		let mut starts = Vec::new();
		let mut ends = Vec::new();
		for line in self.all_lines() {
			if let Some((start, end)) = line.time_range() {
				starts.push(*start);
				ends.push(*end);
			}
		}

		if starts.is_empty() || ends.is_empty() {
			return Err(Error::EmptyRangeError);
		}

		Ok((*starts.iter().min().unwrap(), *ends.iter().max().unwrap()))
	}
}

impl ResolvedPanel {
	fn resolve_time_range(&mut self) {
		let mut starts = Vec::new();
		let mut ends = Vec::new();

		for line in &self.lines {
			if let Some((start, end)) = line.time_range() {
				starts.push(start);
				ends.push(end);
			}
		}

		if starts.is_empty() || ends.is_empty() {
			trace!(target:APPV, "empty range for panel: {:?}", self);
			return;
		}

		let (min, max) = (*starts.iter().min().unwrap(), *ends.iter().max().unwrap());

		let (start, end) = match self.params.time_range_mode.unwrap_or_default() {
			PanelRangeMode::Full => (min, max),
			PanelRangeMode::BestFit => {
				let (start, end) = (*starts.iter().max().unwrap(), *ends.iter().min().unwrap());
				if start < end { (start, end) } else { (min, max) }
			},
		};
		self.set_time_range(*start, *end);
	}
}

fn resolve_panels_ranges_inner(
	config: &mut ResolvedGraphConfig,
	align_mode: PanelAlignmentMode,
) -> Result<(), Error> {
	// Compute panel ranges based on csv files for indiviueal lines
	for panel in &mut config.panels {
		panel.resolve_time_range();
	}

	match align_mode {
		PanelAlignmentMode::PerPanel => { /* no change */ },
		PanelAlignmentMode::SharedFull => {
			let global_start =
				config.panels.iter().filter_map(|p| *p.time_range()).map(|r| r.0).min();
			let global_end =
				config.panels.iter().filter_map(|p| *p.time_range()).map(|r| r.1).max();

			if let (Some(start), Some(end)) = (global_start, global_end) {
				debug!(target: APPV, "PanelAlignmentMode::AlignSum found range {:?} - {:?}", global_start, global_end);
				if global_start < global_end {
					for panel in &mut config.panels {
						panel.set_time_range(start, end);
					}
				} else {
					panic!("should not happen. (this is bug?)");
				}
			}
		},
		PanelAlignmentMode::SharedOverlap => {
			let global_start =
				config.panels.iter().filter_map(|p| *p.time_range()).map(|r| r.0).max();
			let global_end =
				config.panels.iter().filter_map(|p| *p.time_range()).map(|r| r.1).min();

			if let (Some(start), Some(end)) = (global_start, global_end) {
				trace!(target: APPV, "PanelAlignmentMode::AlignIntersection found range {:?} - {:?}", global_start, global_end);
				if global_start < global_end {
					for panel in &mut config.panels {
						panel.set_time_range(start, end);
					}
				} else {
					debug!(target: APPV, "PanelAlignmentMode::AlignIntersection empty, no range adjustment made {:?} - {:?}", global_start, global_end);
					/* no change */
				}
			}
		},
		PanelAlignmentMode::Fixed(start, end) => {
			for panel in &mut config.panels {
				panel.set_time_range(start, end);
			}
		},
	}

	Ok(())
}

impl TimeRangeArg {
	/// Resolve this time range argument into actual timestamps,
	/// using the global data range and timestamp format provided by the user.
	pub fn resolve(
		&self,
		total_range: (NaiveDateTime, NaiveDateTime),
		format: &TimestampFormat,
	) -> Result<(NaiveDateTime, NaiveDateTime), Error> {
		fn scale_duration(duration: chrono::Duration, frac: f64) -> chrono::Duration {
			let micros = duration.num_microseconds().unwrap_or(0);
			let scaled = (micros as f64 * frac).round() as i64;
			chrono::Duration::microseconds(scaled)
		}

		match self {
			TimeRangeArg::Relative(start_frac, end_frac) => {
				if !(0.0..=1.0).contains(start_frac)
					|| !(0.0..=1.0).contains(end_frac)
					|| start_frac >= end_frac
				{
					panic!("should already be verified. (this is bug)");
				}
				let duration = total_range.1 - total_range.0;
				let start = total_range.0 + scale_duration(duration, *start_frac);
				let end = total_range.0 + scale_duration(duration, *end_frac);
				Ok((start, end))
			},

			TimeRangeArg::AbsoluteDateTime(a, b) => match format {
				TimestampFormat::DateTime(fmt) => {
					let start = NaiveDateTime::parse_from_str(a, fmt)?;
					let end = NaiveDateTime::parse_from_str(b, fmt)?;
					Ok((start, end))
				},
				TimestampFormat::Time(fmt) => {
					let t0 = NaiveTime::parse_from_str(a, fmt)?;
					let t1 = NaiveTime::parse_from_str(b, fmt)?;
					let base_date = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(); // safe fallback
					Ok((base_date.and_time(t0), base_date.and_time(t1)))
				},
			},
		}
	}
}

impl SharedGraphContext {
	pub fn resolved_alignment_mode(
		&self,
		total_range: (NaiveDateTime, NaiveDateTime),
	) -> Result<PanelAlignmentMode, Error> {
		if let Some(time_range) = &self.time_range {
			let resolved = time_range.resolve(total_range, self.timestamp_format())?;
			return Ok(PanelAlignmentMode::Fixed(resolved.0, resolved.1));
		}

		Ok(match self.panel_alignment_mode {
			Some(PanelAlignmentModeArg::SharedOverlap) => PanelAlignmentMode::SharedOverlap,
			Some(PanelAlignmentModeArg::SharedFull) => PanelAlignmentMode::SharedFull,
			Some(PanelAlignmentModeArg::PerPanel) | None => PanelAlignmentMode::PerPanel,
		})
	}
}

pub fn resolve_panels_ranges(
	config: &mut ResolvedGraphConfig,
	shared_context: &SharedGraphContext,
) -> Result<(), Error> {
	config.populate_line_ranges()?;
	let global_range = config.global_time_range()?;
	let panel_alignment_mode = shared_context.resolved_alignment_mode(global_range)?;

	debug!(target: APPV, "Global total range {:?}", global_range);
	debug!(target: APPV, "Resolved panel alignment mode {:?}", panel_alignment_mode);

	resolve_panels_ranges_inner(config, panel_alignment_mode)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		graph_config::{DataSource, Line},
		logging::init_tracing_test,
		resolved_graph_config::ResolvedLine,
	};
	use chrono::NaiveDate;

	impl ResolvedLine {
		fn new_with_range(line: Line, start: NaiveDateTime, end: NaiveDateTime) -> Self {
			let mut s = Self::from_explicit_name(line, "dummy".into());
			s.set_time_range(start, end);
			s
		}
	}

	fn build_resolved_graph_config(lines: Vec<ResolvedLine>) -> ResolvedGraphConfig {
		ResolvedGraphConfig { panels: vec![ResolvedPanel::new_with_lines(lines)] }
	}

	fn build_resolved_graph_config_multi_panel(
		vec_of_lines: Vec<Vec<ResolvedLine>>,
	) -> ResolvedGraphConfig {
		let panels = vec_of_lines
			.into_iter()
			.map(|lines| ResolvedPanel::new_with_lines(lines))
			.collect();
		ResolvedGraphConfig { panels }
	}

	fn plot_line(start: NaiveDateTime, end: NaiveDateTime) -> ResolvedLine {
		ResolvedLine::new_with_range(
			Line::new_with_data_source(DataSource::new_plot_field(
				Some("dummy".into()),
				"dummy".into(),
			)),
			start,
			end,
		)
	}

	#[test]
	fn two_lines_full() {
		let mut config = build_resolved_graph_config(vec![
			plot_line(
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 10, 00).unwrap(),
			),
			plot_line(
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
			),
		]);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedOverlap).unwrap();

		for panel in config.panels {
			assert_eq!(
				panel.time_range.unwrap().0,
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
			);
			assert_eq!(
				panel.time_range.unwrap().1,
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
			);
		}
	}

	#[test]
	fn two_lines_best_fit() {
		let mut config = build_resolved_graph_config(vec![
			plot_line(
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 10, 00).unwrap(),
			),
			plot_line(
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
			),
		]);

		config.panels[0].params.time_range_mode = Some(PanelRangeMode::BestFit);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedOverlap).unwrap();

		for panel in config.panels {
			assert_eq!(
				panel.time_range.unwrap().0,
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
			);
			assert_eq!(
				panel.time_range.unwrap().1,
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 10, 00).unwrap(),
			);
		}
	}

	#[test]
	fn two_lines_best_fit_no_overlap_fallback() {
		let mut config = build_resolved_graph_config(vec![
			plot_line(
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 10, 00).unwrap(),
			),
			plot_line(
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 20, 56).unwrap(),
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
			),
		]);

		config.panels[0].params.time_range_mode = Some(PanelRangeMode::BestFit);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedOverlap).unwrap();

		// fallback to full
		for panel in config.panels {
			assert_eq!(
				panel.time_range.unwrap().0,
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
			);
			assert_eq!(
				panel.time_range.unwrap().1,
				NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
			);
		}
	}

	#[test]
	fn two_lines_x_two_panels_independent() {
		let mut config = build_resolved_graph_config_multi_panel(vec![
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 30, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
				),
			],
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 10, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 20, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 10, 00).unwrap(),
				),
			],
		]);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::PerPanel).unwrap();

		assert_eq!(
			config.panels[0].time_range.unwrap().0,
			NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
		);
		assert_eq!(
			config.panels[0].time_range.unwrap().1,
			NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
		);
		assert_eq!(
			config.panels[1].time_range.unwrap().0,
			NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 56).unwrap(),
		);
		assert_eq!(
			config.panels[1].time_range.unwrap().1,
			NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 10, 00).unwrap(),
		);
	}

	#[test]
	fn two_lines_x_two_panels_align_global_no_overlap() {
		let mut config = build_resolved_graph_config_multi_panel(vec![
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 00, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(12, 30, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
				),
			],
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 10, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 20, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 56).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 10, 00).unwrap(),
				),
			],
		]);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedOverlap).unwrap();

		assert_eq!(
			config.panels[0].time_range.unwrap().0,
			NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(11, 00, 56).unwrap(),
		);
		assert_eq!(
			config.panels[0].time_range.unwrap().1,
			NaiveDate::from_ymd_opt(2025, 5, 17).unwrap().and_hms_opt(13, 10, 00).unwrap(),
		);
		assert_eq!(
			config.panels[1].time_range.unwrap().0,
			NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 56).unwrap(),
		);
		assert_eq!(
			config.panels[1].time_range.unwrap().1,
			NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 10, 00).unwrap(),
		);
	}

	#[test]
	fn two_lines_x_two_panels_align_best_fit_global_overlap() {
		init_tracing_test();
		let mut config = build_resolved_graph_config_multi_panel(vec![
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 30, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 00, 00).unwrap(),
				),
			],
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(10, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(15, 00, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 10, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 20, 00).unwrap(),
				),
			],
		]);

		config.panels[0].params.time_range_mode = Some(PanelRangeMode::BestFit);
		config.panels[1].params.time_range_mode = Some(PanelRangeMode::BestFit);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedOverlap).unwrap();
		for panel in config.panels {
			assert_eq!(
				panel.time_range.unwrap().0,
				NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 10, 00).unwrap(),
			);
			assert_eq!(
				panel.time_range.unwrap().1,
				NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 20, 00).unwrap(),
			);
		}
	}

	#[test]
	fn two_lines_x_two_panels_align_full_global_overlap() {
		init_tracing_test();
		let mut config = build_resolved_graph_config_multi_panel(vec![
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 30, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 00, 00).unwrap(),
				),
			],
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(10, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(15, 00, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 10, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 20, 00).unwrap(),
				),
			],
		]);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedOverlap).unwrap();
		for panel in config.panels {
			assert_eq!(
				panel.time_range.unwrap().0,
				NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 00).unwrap(),
			);
			assert_eq!(
				panel.time_range.unwrap().1,
				NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 00, 00).unwrap(),
			);
		}
	}

	#[test]
	fn two_lines_x_two_panels_align_full_shared() {
		init_tracing_test();
		let mut config = build_resolved_graph_config_multi_panel(vec![
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(12, 30, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(13, 00, 00).unwrap(),
				),
			],
			vec![
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(20, 00, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(21, 00, 00).unwrap(),
				),
				plot_line(
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(20, 10, 00).unwrap(),
					NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(20, 20, 00).unwrap(),
				),
			],
		]);

		resolve_panels_ranges_inner(&mut config, PanelAlignmentMode::SharedFull).unwrap();

		for panel in config.panels {
			assert_eq!(
				panel.time_range.unwrap().0,
				NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(11, 00, 00).unwrap(),
			);
			assert_eq!(
				panel.time_range.unwrap().1,
				NaiveDate::from_ymd_opt(2025, 5, 16).unwrap().and_hms_opt(21, 00, 00).unwrap(),
			);
		}
	}

	#[test]
	fn populate_range_from_csv_file() {
		init_tracing_test();
		let mut line = ResolvedLine::from_explicit_name(
			Line::new_with_data_source(DataSource::new_plot_field(
				Some("dummy".into()),
				"dummy".into(),
			)),
			"dummy".into(),
		);

		line.set_shared_csv_filename(&PathBuf::from("./test-files/some-data.csv"));
		let mut config = build_resolved_graph_config(vec![line]);
		config.populate_line_ranges().unwrap();

		assert_eq!(
			config.panels[0].lines[0].time_range().unwrap().1,
			NaiveDate::from_ymd_opt(2025, 4, 22)
				.unwrap()
				.and_hms_milli_opt(20, 18, 38, 118)
				.unwrap(),
		);

		assert_eq!(
			config.panels[0].lines[0].time_range().unwrap().0,
			NaiveDate::from_ymd_opt(2025, 4, 22)
				.unwrap()
				.and_hms_milli_opt(20, 17, 00, 194)
				.unwrap(),
		);
	}
}
