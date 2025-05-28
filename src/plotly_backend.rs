use crate::{
	graph_config::GraphFullContext,
	resolved_graph_config::{ResolvedGraphConfig, ResolvedLine},
};
use csv::ReaderBuilder;
use plotly::common::{Mode, Title};
use plotly::{Layout, Plot, Scatter};
use std::path::Path;
use std::{fs::File, io};
use std::{io::BufReader, num::ParseFloatError};

const LOG_TARGET: &str = "gnuplot";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("I/O error: {0}")]
	IoError(#[from] io::Error),
	#[error("CVS error: {0}")]
	CsvError(#[from] csv::Error),
	#[error("General error")]
	GeneralError,
	#[error("CSV data files not resolved properly (internal bug) for line: {0:#?}")]
	CvsFilesResolutionError(Box<ResolvedLine>),
	#[error("Parse float error: {0}")]
	ParseFloatError(#[from] ParseFloatError),
}

pub fn write_plotly_html(
	config: &ResolvedGraphConfig,
	context: &GraphFullContext,
	output_html_path: &Path,
) -> Result<(), Error> {
	let mut plot = Plot::new();

	for (panel_idx, panel) in config.panels.iter().enumerate() {
		if panel.is_empty() {
			continue;
		}

		for (_line_idx, line) in panel.lines.iter().enumerate() {
			let csv_path = line
				.shared_csv_filename()
				.ok_or(Error::CvsFilesResolutionError(Box::new(line.clone())))?;

			let (timestamps, values) = read_csv(&csv_path, line.csv_data_column_for_plot())?;

			let trace = Scatter::new(timestamps, values)
				.mode(Mode::Lines)
				.name(line.title(context.input().len() > 1));

			plot.add_trace(trace);
		}

		let panel_title = if panel.title().is_empty() {
			format!("Panel {}", panel_idx + 1)
		} else {
			panel.title().join("\n")
		};
		let layout = Layout::new().title(Title::with_text(&panel_title));
		plot.set_layout(layout);
	}

	plot.write_html(output_html_path);
	Ok(())
}

fn read_csv(csv_path: &Path, value_column: &str) -> Result<(Vec<String>, Vec<f64>), Error> {
	let file = File::open(csv_path)?;
	let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(BufReader::new(file));

	let headers = rdr.headers()?.clone();
	let date_idx = headers.iter().position(|h| h == "date").ok_or(Error::GeneralError)?;
	let time_idx = headers.iter().position(|h| h == "time").ok_or(Error::GeneralError)?;
	let value_idx = headers.iter().position(|h| h == value_column).ok_or(Error::GeneralError)?;

	let mut timestamps = Vec::new();
	let mut values = Vec::new();

	for record in rdr.records() {
		let record = record?;
		let d = record.get(date_idx).ok_or(Error::GeneralError)?.to_string();
		let t = record.get(time_idx).ok_or(Error::GeneralError)?.to_string();
		let val_str = record.get(value_idx).ok_or(Error::GeneralError)?;
		let val = val_str.parse::<f64>()?;

		timestamps.push(d + " " + &t);
		values.push(val);
	}

	Ok((timestamps, values))
}
