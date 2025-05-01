//! Integration tests for the plox CLI, making sure the tool behaves as expected end-to-end.
//! These tests run real commands with sample inputs and check that the outputs are correct.
//! Also serves as live examples which are directly inluded into SAMPLE.md file.

use cmd_lib::spawn_with_output;

// Used for running commands visually pleasing in doc tests.
macro_rules! bash(
	( plox $($a:tt)* ) => {{
		let bin_path = env!("CARGO_BIN_EXE_plox");
		let output = spawn_with_output!(
			$bin_path -vv $($a)*
		)
		.expect("a process running. qed")
		.wait_with_output();

		// tracing::error!("output: {:#?}", output);
		output.unwrap()
	}}
);

#[docify::export_content]
fn cmd_simple() -> String {
	bash!(
		plox graph
		  --input  some-playground/default.log
		  --output some-playground/default.png
		  --plot om_module x
	)
}

#[docify::export_content]
fn cmd_simple_panels() -> String {
	bash!(
		plox graph
		  --input  some-playground/some.log
		  --output some-playground/panels.png
		  --timestamp-format "[%s]"
		  --plot om_module x
		  --panel
		  --plot x_module x01
		  --plot x_module x02
		  --plot x_module x03
		  --panel
		  --event-count foo_module SOME_EVENT
		  --event foo_module SOME_EVENT 1.0 --yaxis y2 --style points
	)
}

#[docify::export_content]
fn cmd_demo_lines() -> String {
	bash!(
		plox graph
		  --input  some-playground/some.log
		  --output some-playground/demo-lines.png
		  --config some-playground/demo-lines.toml
	)
}
#[docify::export_content]
fn cmd_regex() -> String {
	bash!(
		plox graph
		  --input  some-playground/default.log
		  --output some-playground/regex.png
		  --plot yam_module r#"y=\([\d\.]+,\s*([\d\.]+)\)"#
		  --title "1st tuple item"
		  --plot yam_module r#"y=\(([\d\.]+),\s*[\d\.]+\)"#
		  --title "2nd tuple item"
	)
}

#[docify::export_content]
fn cmd_deltas_and_count() -> String {
	bash!(
		plox graph
		  --input some-playground/default.log
		  --output some-playground/deltas.png
		  --event-delta foo_module "SOME_EVENT" --yaxis-scale log
		  --style points --marker-size 7 --marker-color olive --marker-type diamond
		  --event-count foo_module "SOME_EVENT" --style steps --yaxis y2
	)
}

#[docify::export_content]
fn cmd_simple_panels_two_files() -> String {
	bash!(
		plox graph
		  --input  some-playground/default.log,some-playground/default-other.log
		  --output some-playground/panels-two-files.png
		  --per-file-panels
		  --plot om_module x
		  --panel
		  --plot x_module x01
		  --plot x_module x02
		  --plot x_module x03
		  --panel
		  --event-count foo_module SOME_EVENT
		  --event foo_module SOME_EVENT 1.0 --yaxis y2 --style points
	)
}

#[docify::export_content]
fn cmd_demo_lines_two_files() -> String {
	bash!(
		plox graph
		  --input  some-playground/default.log
		  --input  some-playground/default-other.log
		  --output some-playground/demo-lines-two-files.png
		  --timestamp-format "%Y-%m-%d %H:%M:%S%.3f"
		  --per-file-panels
		  --config some-playground/demo-lines.toml
	)
}

#[test]
fn test_cmd_simple() {
	cmd_simple();
}

#[test]
fn test_cmd_regex() {
	cmd_regex();
}

#[test]
fn test_cmd_deltas_and_count() {
	// plox::logging::init_tracing(false, 2);
	cmd_deltas_and_count();
}

#[test]
fn test_cmd_simple_panels() {
	cmd_simple_panels();
}

#[test]
fn test_cmd_demo_lines() {
	cmd_demo_lines();
}

#[test]
fn test_cmd_simple_panels_two_files() {
	cmd_simple_panels_two_files();
}

#[test]
fn test_cmd_demo_lines_two_files() {
	cmd_demo_lines_two_files();
}
