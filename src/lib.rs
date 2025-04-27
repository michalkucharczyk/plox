#![doc = include_str!("../SAMPLE.md")]

#[cfg(feature = "generate-readme")]
docify::compile_markdown!("SAMPLE.docify.md", "SAMPLE.local.md");

#[cfg(feature = "generate-readme")]
plox_macros::plox_apply_fix!("SAMPLE.local.md", "SAMPLE.md");

pub mod align_ranges;
pub mod cli;
pub mod error;
pub mod gnuplot;
pub mod graph_cli_builder;
pub mod graph_config;
pub mod logging;
pub mod match_preview_cli_builder;
pub mod process_log;
pub mod resolved_graph_config;
