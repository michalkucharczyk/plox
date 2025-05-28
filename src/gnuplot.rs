//! This module handles the creation of gnuplot scripts and graph images.
//!
//! It generates scripts for gnuplot to produce visual representations based on
//! resolved configurations and parsed data. Additionally, it manages the execution
//! of gnuplot and the saving of resulting graph images.

use crate::{
	graph_config::{
		AxisScale, Color, DashStyle, GraphFullContext, MarkerType, OutputFilePaths, PlotStyle,
		YAxis,
	},
	logging::APPV,
	resolved_graph_config::{ResolvedGraphConfig, ResolvedLine},
};
use std::{
	fs::File,
	io::{self, Write},
	path::{Path, PathBuf},
	process::{Command, ExitStatus},
};
use tracing::{debug, info, warn};

const LOG_TARGET: &str = "gnuplot";

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("I/O error: {0}")]
	IoError(#[from] io::Error),
	#[error("CSV data files not resolved properly (internal bug) for line: {0:#?}")]
	CvsFilesResolutionError(Box<ResolvedLine>),
	#[error("Looks like '{0}' command is not available: {1}")]
	GnuplotCommandNotAvailable(String, io::Error),
	#[error("gnuplot execution error: '{0}' / {1}")]
	GnuplotExecution(String, io::Error),
	#[error("gnuplot non-zero exit code: '{0}', stdout:'{1}', stderr:'{2}")]
	GnuplotNonZeroExitCode(ExitStatus, String, String),
	#[error("Error while creating gnuplot script '{0}': {1}")]
	ScriptCreationError(PathBuf, io::Error),
	#[error("Incorrect input files (this is bug).")]
	IncorrectOutputFiles,
}

impl MarkerType {
	/// Returns the gnuplot marker specification, e.g., `pt 7`.
	pub fn to_gnuplot(&self) -> &'static str {
		match self {
			MarkerType::Dot => "pt 7",
			MarkerType::Plus => "pt 1",
			MarkerType::Cross => "pt 3",
			MarkerType::Circle => "pt 6",
			MarkerType::Triangle => "pt 8",
			MarkerType::TriangleFilled => "pt 9",
			MarkerType::Square => "pt 4",
			MarkerType::SquareFilled => "pt 5",
			MarkerType::Diamond => "pt 12",
			MarkerType::DiamondFilled => "pt 13",
			MarkerType::X => "pt 2",
		}
	}
}

impl Color {
	/// Returns the gnuplot color specification, e.g. `lc rgb "red"`.
	pub fn to_gnuplot(&self) -> &'static str {
		match self {
			Color::Red => "lc rgb \"red\"",
			Color::Blue => "lc rgb \"blue\"",
			Color::Green => "lc rgb \"green\"",
			Color::Orange => "lc rgb \"orange\"",
			Color::Purple => "lc rgb \"purple\"",
			Color::Cyan => "lc rgb \"cyan\"",
			Color::Magenta => "lc rgb \"magenta\"",
			Color::Goldenrod => "lc rgb \"goldenrod\"",
			Color::Brown => "lc rgb \"brown\"",
			Color::Olive => "lc rgb \"olive\"",
			Color::Navy => "lc rgb \"navy\"",
			Color::DarkGreen => "lc rgb \"dark-green\"",
			Color::DarkOrange => "lc rgb \"dark-orange\"",
			Color::Violet => "lc rgb \"violet\"",
			Color::Coral => "lc rgb \"coral\"",
			Color::Salmon => "lc rgb \"salmon\"",
			Color::SteelBlue => "lc rgb \"steelblue\"",
			Color::DarkMagenta => "lc rgb \"dark-magenta\"",
			Color::DarkCyan => "lc rgb \"dark-cyan\"",
			Color::DarkYellow => "lc rgb \"dark-yellow\"",
			Color::DarkTurquoise => "lc rgb \"dark-turquoise\"",
			Color::Yellow => "lc rgb \"yellow\"",
			Color::Black => "lc rgb \"black\"",
		}
	}
}

impl PlotStyle {
	/// Returns the gnuplot style snippet, e.g. `"with linespoints"`
	pub fn to_gnuplot(&self) -> &'static str {
		match self {
			PlotStyle::Lines => "with lines",
			PlotStyle::Steps => "with steps",
			PlotStyle::Points => "with points",
			PlotStyle::LinesPoints => "with linespoints",
		}
	}
}
impl DashStyle {
	/// Returns the gnuplot dash (line type) snippet, e.g. `"lt 2"`
	pub fn to_gnuplot(&self) -> &'static str {
		match self {
			DashStyle::Solid => "dt 1",
			DashStyle::Dashed => "dt 2",
			DashStyle::Dotted => "dt 3",
			DashStyle::DashDot => "dt 4",
			DashStyle::LongDash => "dt 5",
		}
	}
}

use strum::IntoEnumIterator;

#[derive(Debug, Clone, Copy)]
struct Style {
	color: Color,
	dash: DashStyle,
	marker: MarkerType,
}

impl Style {
	pub fn line_style(&self, i: usize) -> String {
		format!(
			"set linetype {} {} {} {} lw 2.0 ps 4.0",
			i,
			self.color.to_gnuplot(),
			self.dash.to_gnuplot(),
			self.marker.to_gnuplot()
		)
	}
}

fn build_default_styles() -> Vec<Style> {
	let mut styles = Vec::new();
	for dash in DashStyle::iter() {
		for (color, marker) in Color::iter().zip(MarkerType::iter().cycle()) {
			styles.push(Style { color, dash, marker });
		}
	}
	styles
}

/// Write a gnuplot script to the given output path based on the graph configuration.
///
/// # Arguments
/// * `config` - The full graph configuration (panels and lines).
/// * `output_script_path` - The path where the .gnu file will be written.
/// * `output_image_path` - The path to the output PNG file.
pub fn write_gnuplot_script(
	config: &ResolvedGraphConfig,
	context: &GraphFullContext,
	output_script_path: &PathBuf,
	output_image_path: &Path,
) -> Result<(), Error> {
	let mut file = File::create(output_script_path)
		.map_err(|e| Error::ScriptCreationError(output_script_path.clone(), e))?;
	let num_non_empty_panels = config.panels.iter().filter(|p| !p.is_empty()).count();
	let plot_margin = 0.005;
	let plot_height = 1.0 / num_non_empty_panels as f64 - plot_margin;

	let has_multiple_input_files = context.input().len() > 1;

	//write to gnuplot script wrapper
	macro_rules! gpwr {
	    ($dst:expr, $($arg:tt)*) => ({
	        writeln!($dst, $($arg)*).map_err(|e| Error::ScriptCreationError(output_script_path.clone(),e))
	    });
	}

	gpwr!(file, "set terminal pngcairo enhanced font 'arial,10' fontscale 3.0 size 7560, 5500")?;
	gpwr!(file, "set output '{}'", output_image_path.display())?;

	{
		let styles = build_default_styles().into_iter().take(20);
		for (i, style) in styles.enumerate() {
			gpwr!(file, "{}", style.line_style(i + 1))?;
		}
	}

	gpwr!(file, "set datafile separator ','")?;
	gpwr!(file, "set xdata time")?;
	gpwr!(file, "set timefmt '%Y-%m-%dT%H:%M:%S'")?;
	gpwr!(file, "set format x '%H:%M:%S'")?;
	gpwr!(file, "set mxtics 10")?;
	gpwr!(file, "set grid xtics mxtics")?;
	gpwr!(file, "set grid ytics mytics")?;
	gpwr!(file, "set ytics nomirror")?;
	gpwr!(file, "set key noenhanced")?;
	gpwr!(file, "set multiplot")?;
	gpwr!(file, "set lmargin at screen 0.035")?;
	gpwr!(file, "set rmargin at screen 0.975")?;

	gpwr!(file, "combine_datetime(date_col,time_col) = strcol(date_col) . 'T' . strcol(time_col)")?;

	let mut i = 0;
	for panel in config.panels.iter().rev() {
		debug!(target:LOG_TARGET,"drawing: {:#?}",panel);
		if panel.is_empty() {
			continue;
		}

		let y_position = plot_height * i as f64;
		i += 1;
		gpwr!(file, "set origin 0.0,{}", y_position)?;
		gpwr!(file, "set size 1.0,{}", plot_height)?;
		gpwr!(file, "unset label")?;
		{
			let mut x = -0.03;
			for (i, title_line) in panel.title().into_iter().enumerate() {
				let font = if i == 0 { "arial bold,10" } else { "arial,8" };
				gpwr!(
					file,
					"set label '{title_line}' at graph {x},0.5 rotate by 90 center font\"{font}\" noenhanced",
				)?;
				x += 0.005;
			}
		}

		match panel.params.yaxis_scale {
			Some(AxisScale::Linear) | None => gpwr!(file, "unset logscale y")?,
			Some(AxisScale::Log) => gpwr!(file, "set logscale y 10")?,
		}

		if panel.lines.iter().any(|line| matches!(line.line.params.yaxis, Some(YAxis::Y2))) {
			gpwr!(file, "set y2tics nomirror")?;
			gpwr!(file, "set my2tics 10")?;
		};

		if let Some((start, end)) = panel.time_range {
			let format = "%Y-%m-%dT%H:%M:%S"; // must match `set timefmt`
			gpwr!(file, "set xrange [\"{}\":\"{}\"]", start.format(format), end.format(format))?;
		}

		for (j, line) in panel.lines.iter().enumerate() {
			let csv_data_path = line
				.shared_csv_filename()
				.ok_or(Error::CvsFilesResolutionError(Box::new(line.clone())))?;
			gpwr!(file, "csv_data_file_{j:04} = '{}'", csv_data_path.display())?;
		}

		gpwr!(file, "plot \\")?;
		for (j, line) in panel.lines.iter().enumerate() {
			// build style parts
			let mut style_parts: Vec<String> = Vec::new();

			// plot style (lines/steps/points/linespoints)
			style_parts.push(line.line.params.style.to_gnuplot().into());
			if let Some(dash_style) = &line.line.params.dash_style {
				style_parts.push(dash_style.to_gnuplot().into());
			}

			if let Some(line_width) = &line.line.params.line_width {
				style_parts.push(format!("lw {}", line_width));
			}

			if let Some(color) = &line.line.params.line_color {
				style_parts.push(color.to_gnuplot().into());
			}

			if matches!(line.line.params.style, PlotStyle::LinesPoints | PlotStyle::Points) {
				// markers
				if let Some(marker) = &line.line.params.marker_type {
					style_parts.push(marker.to_gnuplot().into());
				}
				style_parts.push(format!("ps {}", line.line.params.marker_size));

				if let Some(mcol) = &line.line.params.marker_color {
					style_parts.push(mcol.to_gnuplot().into());
				}
			}

			// axis selection
			let axis = match line.line.params.yaxis.as_ref().unwrap_or(&YAxis::Y) {
				YAxis::Y2 => "axes x1y2",
				YAxis::Y => "axes x1y1",
			};
			style_parts.push(axis.into());

			let style = if style_parts.is_empty() {
				"with lines axes x1y1".to_string()
			} else {
				style_parts.join(" ")
			};

			write!(
				file,
				"   csv_data_file_{j:04} using (combine_datetime('date','time')):'{}' {} title '{}'",
				line.csv_data_column_for_plot(),
				style,
				line.title(has_multiple_input_files),
			)?;

			if j != panel.lines.len() - 1 {
				gpwr!(file, ", \\")?;
			} else {
				gpwr!(file, "")?;
			}
		}
		gpwr!(file, "unset y2tics")?;
		gpwr!(file, "unset my2tics")?;
	}

	gpwr!(file, "unset multiplot")?;
	Ok(())
}

fn path_to_display(path: &Path) -> &Path {
	let Ok(cwd) = std::env::current_dir() else {
		return path;
	};
	path.strip_prefix(&cwd).unwrap_or(path)
}

/// Write gnuplot script and immediately execute it with `gnuplot`.
pub fn run_gnuplot(config: &ResolvedGraphConfig, context: &GraphFullContext) -> Result<(), Error> {
	let OutputFilePaths::Gnuplot((image_path, script_path)) = context.get_graph_output_path()
	else {
		return Err(Error::IncorrectOutputFiles);
	};

	write_gnuplot_script(config, context, &script_path, &image_path)?;
	let script_path = if context.output_graph_ctx.display_absolute_paths {
		script_path
	} else {
		path_to_display(&script_path).to_path_buf()
	};
	info!(target:APPV,"Script saved: {}", script_path.display());

	if std::env::var("PLOX_SKIP_GNUPLOT").is_ok() {
		info!(target:APPV, "PLOX_SKIP_GNUPLOT is set, skipping gnuplot execution and image generation.");
		return Ok(());
	}

	const GNUPLOT_CMD: &str = "gnuplot";

	Command::new(GNUPLOT_CMD)
		.output()
		.map_err(|e| Error::GnuplotCommandNotAvailable(GNUPLOT_CMD.into(), e))?;

	let output = Command::new(GNUPLOT_CMD).arg(&script_path).output()?;

	if !output.status.success() {
		return Err(Error::GnuplotNonZeroExitCode(
			output.status,
			String::from_utf8_lossy(&output.stdout).to_string(),
			String::from_utf8_lossy(&output.stderr).to_string(),
		));
	}

	let image_path = if context.output_graph_ctx.display_absolute_paths {
		image_path
	} else {
		path_to_display(&image_path).to_path_buf()
	};
	info!(target:APPV,"Image  saved: {}", image_path.display());

	if !output.stdout.is_empty() {
		debug!(target:APPV,"--- gnuplot stdout ---");
		debug!(target:APPV,"\n{}", String::from_utf8_lossy(&output.stdout));
	}

	if !output.stderr.is_empty() {
		debug!(target:APPV,"--- gnuplot stderr ---");
		debug!(target:APPV,"\n{}", String::from_utf8_lossy(&output.stderr));
	}

	let do_not_open =
		context.output_graph_ctx.do_not_display || std::env::var("PLOX_DO_NOT_DISPLAY").is_ok();

	if !do_not_open {
		let cmd = if let Ok(viewer_cmd_path) = std::env::var("PLOX_IMAGE_VIEWER") {
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
			cmd.arg(image_path);
			if let Err(e) = cmd.status() {
				warn!(target:APPV,"Displaying image with command: '{cmd:?}' failed {e}.");
			}
		};
	} else {
		debug!(target:APPV,"Displaying image disabled.");
	}
	Ok(())
}
