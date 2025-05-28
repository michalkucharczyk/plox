use crate::{
	graph_config::{GraphFullContext, OutputFilePaths},
	logging::APPV,
	resolved_graph_config::{ResolvedGraphConfig, ResolvedLine},
};
use csv::ReaderBuilder;
use plotly::Scatter;
use serde::Serialize;
use std::path::Path;
use std::{fs::File, io};
use std::{io::BufReader, num::ParseFloatError};
use tracing::{debug, info};

//todo:
// - logging
// - read_cvs unification with process_log::stat/cat
// - title - multi line better support
// - style
// - log - scale?
// - y2 axis?

const LOG_TARGET: &str = "plotly";

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
	#[error("JSON serialization error: {0}")]
	SerdeJsonError(#[from] serde_json::Error),
	#[error("Incorrect input files (this is bug).")]
	IncorrectOutputFiles,
}

#[derive(Serialize)]
struct PanelTemplateInput {
	id: String,
	title: String,
	traces_json: String,
}

pub fn write_plotly_html(
	config: &ResolvedGraphConfig,
	context: &GraphFullContext,
) -> Result<(), Error> {
	let OutputFilePaths::Plotly(html_path) = context.get_graph_output_path() else {
		return Err(Error::IncorrectOutputFiles);
	};

	let mut panels = vec![];

	for (panel_idx, panel) in config.panels.iter().enumerate() {
		if panel.is_empty() {
			continue;
		}
		let id = format!("plot{}", panel_idx);
		debug!(target:LOG_TARGET,"drawing {id}: {:#?}",panel);
		let mut traces = vec![];

		for line in &panel.lines {
			let csv_path = line
				.shared_csv_filename()
				.ok_or(Error::CvsFilesResolutionError(Box::new(line.clone())))?;

			let (timestamps, values) = read_csv(&csv_path, line.csv_data_column_for_plot())?;

			let trace = Scatter::new(timestamps, values)
				.mode(plotly::common::Mode::Markers)
				.name(line.title(context.input().len() > 1));

			traces.push(trace);
		}

		let traces_json = serde_json::to_string(&traces)?;
		panels.push(PanelTemplateInput {
			id,
			traces_json,
			title: panel.title().join(" | ").to_string(),
		});
	}

	let raw_template = include_str!("../templates/plotly_template.html"); // relative to this Rust file
	let rendered = minijinja::render!(raw_template,
			panels => panels
	);

	std::fs::write(&html_path, rendered)?;
	info!(target:APPV,"HTML saved: {}", html_path.display());
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
