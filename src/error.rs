//! Error handling for the plox project.
//!
//! It defines the main `Error` type, wraps lower-level errors, and ensures consistent reporting.
//! Intended to provide clear, friendly messages when something goes wrong.

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("CLI parsing error: {0}")]
	CliParseError(#[from] crate::graph_cli_builder::Error),

	//todo: clean-up
	#[error("CLI parsing error: {0}")]
	CliParseError2(#[from] crate::match_preview_cli_builder::Error),

	#[error("I/O error: path: '{0}' error:{1}")]
	IoError(String, io::Error),

	#[error("Toml error. {0}")]
	TomlError(#[from] toml::de::Error),

	#[error("Other error. {0}")]
	Other(#[from] Box<dyn std::error::Error + Send + Sync>),

	#[error("GNU plot script error. {0}")]
	GnuPlotCreationError(#[from] crate::gnuplot::Error),

	#[error("Logs processing error. {0}")]
	LogProcessing(#[from] crate::process_log::Error),

	#[error("Time ranges resolution error. {0}")]
	TimeRangesResolution(#[from] crate::align_ranges::Error),

	#[error("Plotly generation error. {0}")]
	PlotlyError(#[from] crate::plotly_backend::Error),
}
