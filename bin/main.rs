use clap::Parser;
use plox::{
	align_ranges,
	cli::{Cli, CliCommand, Test1Args, Test2Args, build_cli},
	error::Error,
	gnuplot, graph_cli_builder,
	logging::{self, APPV},
	match_preview_cli_builder, process_log, resolved_graph_config,
};
use std::{process::ExitCode, time::Instant};
use tracing::{debug, error, info, trace};

fn main() -> ExitCode {
	match inner_main() {
		Err(Error::TomlError(_)) => ExitCode::FAILURE,
		Err(e) => {
			error!("{}", e);
			ExitCode::FAILURE
		},
		Ok(_) => ExitCode::SUCCESS,
	}
}

fn inner_main() -> Result<(), Error> {
	let matches = build_cli().get_matches();
	logging::init_tracing(matches.get_flag("quiet"), matches.get_count("verbose"));

	if let Some(graph_matches) = matches.subcommand_matches("match-preview") {
		let (config, shared_context) =
			match_preview_cli_builder::build_from_matches(graph_matches)?;
		info!(target:APPV, "Provided input preview config:{config:#?}");
		info!(target:APPV, "Provided SharedPreviewContext:{shared_context:#?}");
		process_log::regex_match_preview(config, shared_context).map_err(Into::<Error>::into)?;
	} else if let Some(graph_matches) = matches.subcommand_matches("graph") {
		let (config, shared_context) = graph_cli_builder::build_from_matches(graph_matches)?;

		trace!(target:APPV, "Provided input graph config:{config:#?}");
		trace!(target:APPV, "Provided SharedGraphContext:{shared_context:#?}");

		if let Some(ref output_config_path) = shared_context.output_config_path {
			config.save_to_file(output_config_path)?;
		}

		let mut resolved_config =
			resolved_graph_config::expand_graph_config(&config, &shared_context)?;

		let now = Instant::now();
		process_log::process_inputs(&mut resolved_config, &shared_context)
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
			CliCommand::Test1(Test1Args { force }) => {
				info!(target:APPV,"test1 {force:?}");
			},
			CliCommand::Test2(Test2Args { force1 }) => {
				info!(target:APPV,"test2 {force1:?}");
			},
		}
	}

	Ok(())
}
