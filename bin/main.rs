use clap::Parser;
use plox::{
	align_ranges,
	cli::{CatArgs, Cli, CliCommand, StatArgs, build_cli},
	error::Error,
	gnuplot,
	graph_cli_builder::{self},
	graph_config::{GraphConfig, Line, Panel},
	logging::{self, APPV},
	match_preview_cli_builder, process_log, resolved_graph_config,
};
use std::{process::ExitCode, time::Instant};
use tracing::{debug, error, info, trace};

fn main() -> ExitCode {
	match inner_main() {
		Err(Error::TomlError(_)) => ExitCode::FAILURE,
		Err(Error::LogProcessing(crate::process_log::Error::TimestampExtractionFailure(
			file,
			ts,
			log,
		))) => {
			// error!("{:?}", e);
			error!("Error occured when extracting timestamp from '{}' log file", file.display());
			error!("Timestamp format given was: {ts:?}");
			error!("Line that failed:\n{log}");
			error!(
				"Try `plox graph --help` and check out the timestamp format section for more details and examples."
			);
			error!("Try `plox match-preview --verbose` to debug matching problems.");
			error!(
				"For exact format specifiers refer to: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>"
			);
			ExitCode::FAILURE
		},
		Err(e) => {
			error!("{}", e);
			ExitCode::FAILURE
		},
		Ok(_) => ExitCode::SUCCESS,
	}
}

fn inner_main() -> Result<(), Error> {
	let matches = build_cli().get_matches();
	let verbose_level = matches.get_count("verbose");
	logging::init_tracing(matches.get_flag("quiet"), verbose_level);

	if let Some(graph_matches) = matches.subcommand_matches("match-preview") {
		let (config, shared_context) =
			match_preview_cli_builder::build_from_matches(graph_matches)?;
		info!(target:APPV, "Provided input preview config:{config:#?}");
		info!(target:APPV, "Provided SharedPreviewContext:{shared_context:#?}");
		process_log::regex_match_preview(config, shared_context, verbose_level)
			.map_err(Into::<Error>::into)?;
	} else if let Some(graph_matches) = matches.subcommand_matches("graph") {
		let (config, shared_context) = graph_cli_builder::build_from_matches(graph_matches)?;

		trace!(target:APPV, "Provided input graph config:{config:#?}");
		trace!(target:APPV, "Provided SharedGraphContext:{shared_context:#?}");

		if let Some(output_config_path) = shared_context.output_config_path() {
			config.save_to_file(output_config_path)?;
		}

		let mut resolved_config =
			resolved_graph_config::expand_graph_config_with_ctx(&config, &shared_context)?;

		let now = Instant::now();
		process_log::process_inputs(&mut resolved_config, &shared_context.input_files_ctx)
			.map_err(Into::<Error>::into)?;
		debug!(target:APPV,"Input files processed in: {:?}", now.elapsed());

		let now = Instant::now();
		align_ranges::resolve_panels_ranges(&mut resolved_config, &shared_context)
			.map_err(Into::<Error>::into)?;
		debug!(target:APPV,"Ranges resolved in: {:?}", now.elapsed());

		let now = Instant::now();
		gnuplot::run_gnuplot(&resolved_config, &shared_context)?;
		debug!(target:APPV,"gnuplot done in: {:?}", now.elapsed());
	} else {
		//todo histogram, etc..
		let c = Cli::parse();
		match c.command {
			CliCommand::Cat(CatArgs { input_files_ctx, command: source }) => {
				let line = Line::new_with_data_source(source.into());
				let config =
					GraphConfig { panels: vec![Panel::builder().with_lines(vec![line]).build()] };
				let mut resolved_graph_config = resolved_graph_config::expand_graph_config(
					&config,
					input_files_ctx.input(),
					false,
				)?;
				process_log::process_inputs(&mut resolved_graph_config, &input_files_ctx)
					.map_err(Into::<Error>::into)?;

				process_log::display_values(&resolved_graph_config)?;
			},
			CliCommand::Stat(StatArgs {
				input_files_ctx,
				command: source,
				buckets_count,
				precision,
			}) => {
				let line = Line::new_with_data_source(source.into());
				let config =
					GraphConfig { panels: vec![Panel::builder().with_lines(vec![line]).build()] };
				let mut resolved_graph_config = resolved_graph_config::expand_graph_config(
					&config,
					input_files_ctx.input(),
					false,
				)?;
				process_log::process_inputs(&mut resolved_graph_config, &input_files_ctx)
					.map_err(Into::<Error>::into)?;

				let (precision, width) = if precision.len() == 2 {
					(Some(precision[0]), Some(precision[1]))
				} else {
					(None, None)
				};

				process_log::display_stats(
					&resolved_graph_config,
					buckets_count,
					precision,
					width,
				)?;
			},
		}
	}

	Ok(())
}
