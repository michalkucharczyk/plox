#![doc = include_str!("../README.md")]

#[cfg(feature = "generate-readme")]
docify::compile_markdown!("SAMPLE.docify.md", "SAMPLE.md");

#[cfg(feature = "generate-readme")]
plox_macros::plox_process_doc!("SAMPLE.md");

#[cfg(feature = "generate-readme")]
docify::compile_markdown!("README.docify.md", "README.md");

#[cfg(feature = "generate-readme")]
plox_macros::plox_process_doc!("README.md");

pub mod align_ranges;
pub mod cli;
pub mod data_source_cli_builder;
pub mod error;
pub mod gnuplot;
pub mod graph_cli_builder;
pub mod graph_config;
pub mod logging;
pub mod match_preview_cli_builder;
pub mod process_log;
pub mod resolved_graph_config;
mod utils;
