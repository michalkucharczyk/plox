use crate::graph_config::{AxisScale, Color, DashStyle, MarkerSize, MarkerType, PlotStyle, YAxis};
use crate::{
	graph_config::{GraphFullContext, OutputFilePaths},
	logging::APPV,
	resolved_graph_config::{ResolvedGraphConfig, ResolvedLine},
};
use csv::ReaderBuilder;
use plotly::{
	Scatter,
	common::{DashType, Line, LineShape, Marker, MarkerSymbol, Mode},
};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::{fs::File, io};
use std::{io::BufReader, num::ParseFloatError};
use tracing::warn;
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

impl Color {
	pub fn to_plotly(&self) -> &'static str {
		match self {
			Color::Red => "red",
			Color::Blue => "blue",
			Color::Green => "green",
			Color::Orange => "orange",
			Color::Purple => "purple",
			Color::Cyan => "cyan",
			Color::Magenta => "magenta",
			Color::Goldenrod => "goldenrod",
			Color::Brown => "brown",
			Color::Olive => "olive",
			Color::Navy => "navy",
			Color::DarkGreen => "darkgreen",
			Color::DarkOrange => "darkorange",
			Color::Violet => "violet",
			Color::Coral => "coral",
			Color::Salmon => "salmon",
			Color::SteelBlue => "steelblue",
			Color::DarkMagenta => "darkmagenta",
			Color::DarkCyan => "darkcyan",
			Color::DarkYellow => "gold", // Plotly has no darkyellow
			Color::DarkTurquoise => "darkturquoise",
			Color::Yellow => "yellow",
			Color::Black => "black",
		}
	}
}

impl DashStyle {
	pub fn to_plotly(&self) -> DashType {
		match self {
			DashStyle::Solid => DashType::Solid,
			DashStyle::Dashed => DashType::Dash,
			DashStyle::Dotted => DashType::Dot,
			DashStyle::DashDot => DashType::DashDot,
			DashStyle::LongDash => DashType::LongDash,
		}
	}
}

impl MarkerType {
	pub fn to_plotly(&self) -> MarkerSymbol {
		match self {
			MarkerType::Dot => MarkerSymbol::Circle,
			MarkerType::Plus => MarkerSymbol::Cross,
			MarkerType::Cross => MarkerSymbol::X,
			MarkerType::Circle => MarkerSymbol::Circle,
			MarkerType::Triangle => MarkerSymbol::TriangleUp,
			MarkerType::TriangleFilled => MarkerSymbol::TriangleUp,
			MarkerType::Square => MarkerSymbol::Square,
			MarkerType::SquareFilled => MarkerSymbol::Square,
			MarkerType::Diamond => MarkerSymbol::Diamond,
			MarkerType::DiamondFilled => MarkerSymbol::Diamond,
			MarkerType::X => MarkerSymbol::X,
		}
	}
}

impl From<MarkerSize> for usize {
	fn from(val: MarkerSize) -> Self {
		(val.0).round() as usize
	}
}

#[derive(Serialize)]
struct PanelTemplateInput {
	id: String,
	title: String,
	traces_json: String,
	yaxis_scale: String,
}

fn build_trace(
	context: &GraphFullContext,
	line: &ResolvedLine,
) -> Result<Scatter<String, f64>, Error> {
	let csv_path = line
		.shared_csv_filename()
		.ok_or(Error::CvsFilesResolutionError(Box::new(line.clone())))?;

	let (timestamps, values) = read_csv(&csv_path, line.csv_data_column_for_plot())?;

	let mut trace = Scatter::new(timestamps, values)
		.mode(plotly::common::Mode::Markers)
		.name(line.title(context.input().len() > 1));

	let style = &line.line.params.style;
	trace = trace.mode(match style {
		PlotStyle::Lines => Mode::Lines,
		PlotStyle::Steps => Mode::Lines, // Plotly doesn't support 'steps' directly, needs `line.shape`
		PlotStyle::Points => Mode::Markers,
		PlotStyle::LinesPoints => Mode::LinesMarkers,
	});

	let mut line_style = Line::new();

	if let Some(width) = line.line.params.line_width {
		let w: f64 = width.into();
		line_style = line_style.width(w * 0.5);
	} else {
		line_style = line_style.width(0.5);
	}

	if let Some(color) = &line.line.params.line_color {
		line_style = line_style.color(color.to_plotly()); // See below for helper
	}

	if let Some(dash) = &line.line.params.dash_style {
		line_style = line_style.dash(dash.to_plotly());
	}

	if matches!(style, PlotStyle::Steps) {
		line_style = line_style.shape(LineShape::Hv); // horizontal-vertical steps
	}

	trace = trace.line(line_style);

	if matches!(style, PlotStyle::Points | PlotStyle::LinesPoints) {
		let mut marker = Marker::new().size(Into::<usize>::into(line.line.params.marker_size));

		if let Some(mt) = &line.line.params.marker_type {
			marker = marker.symbol(mt.to_plotly());
		}

		if let Some(mc) = &line.line.params.marker_color {
			marker = marker.color(mc.to_plotly());
		}

		trace = trace.marker(marker);
	};

	match line.line.params.yaxis.as_ref().unwrap_or(&YAxis::Y) {
		YAxis::Y2 => trace = trace.y_axis("y2"),
		YAxis::Y => trace = trace.y_axis("y"),
	};

	Ok(*trace)
}

pub fn write_plotly_html_inner(
	config: &ResolvedGraphConfig,
	context: &GraphFullContext,
) -> Result<PathBuf, Error> {
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
			traces.push(build_trace(context, line)?);
		}

		let traces_json = serde_json::to_string(&traces)?;
		panels.push(PanelTemplateInput {
			id,
			traces_json,
			title: panel.title().join(" | ").to_string(),
			yaxis_scale: match panel.params.yaxis_scale {
				Some(AxisScale::Linear) | None => "linear".to_string(),
				Some(AxisScale::Log) => "log".to_string(),
			},
		});
	}

	let raw_template = include_str!("../templates/plotly_template.html"); // relative to this Rust file
	let rendered = minijinja::render!(raw_template,
			panels => panels
	);

	std::fs::write(&html_path, rendered)?;
	info!(target:APPV,"HTML saved: {}", html_path.display());

	Ok(html_path)
}

pub fn write_plotly_html(
	config: &ResolvedGraphConfig,
	context: &GraphFullContext,
) -> Result<(), Error> {
	let html_path = write_plotly_html_inner(config, context)?;

	let do_not_open =
		context.output_graph_ctx.do_not_display || std::env::var("PLOX_DO_NOT_DISPLAY").is_ok();

	if !do_not_open {
		let cmd = if let Ok(viewer_cmd_path) = std::env::var("PLOX_BROWSER") {
			Some(Command::new(viewer_cmd_path))
		} else {
			#[cfg(target_os = "linux")]
			{
				Some(Command::new("xdg-open"))
			}
			#[cfg(not(target_os = "linux"))]
			{
				None
			}
		};

		if let Some(mut cmd) = cmd {
			cmd.arg(html_path);
			if let Err(e) = cmd.status() {
				warn!(target:APPV,"Displaying generated html page with command: '{cmd:?}' failed {e}.");
			}
		};
	} else {
		debug!(target:APPV,"Displaying html page disabled.");
	}

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
