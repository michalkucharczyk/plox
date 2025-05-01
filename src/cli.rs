//! This tiny module defines the overall command-line interface for plox.
//! It sets up the top-level argument parser, wires in the subcommands, and handles user input.

use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
	/// Global verbosity (-v , -vv)
	///
	/// Levels:
	///  - info enabled by default
	///  - -v for debug
	///  - -vv for trace
	#[arg(short = 'v', long, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Quiet mode, no output.
	#[arg(short = 'q', long, action = clap::ArgAction::SetTrue, default_value_t = false)]
	pub quiet: bool,

	#[command(subcommand)]
	pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
	//todo histogram, etc..
	Test1(Test1Args),
	Test2(Test2Args),
}

/// some help for test1
#[derive(Debug, Args)]
pub struct Test1Args {
	/// xxxx xxx xx
	#[arg(long)]
	pub force: bool,
}

/// some help for test2
#[derive(Debug, Args)]
pub struct Test2Args {
	/// uuuu uuu
	#[arg(long)]
	pub force1: bool,
}

pub fn build_cli() -> clap::Command {
	Cli::command()
		.subcommand(crate::graph_cli_builder::build_cli())
		.subcommand(crate::match_preview_cli_builder::build_cli())
}
