//! Integration tests for the plox CLI, making sure the tool behaves as expected end-to-end.
//! These tests run real commands with sample inputs and check that the outputs are correct.
//! Also serves as live examples which are directly inluded into SAMPLE.md file.

use cmd_lib::{run_cmd, spawn_with_output};
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

// Used for running commands visually pleasing in doc tests.
macro_rules! bash(
	( plox $($a:tt)* ) => {{
		let bin_path = env!("CARGO_BIN_EXE_plox");
		let status = spawn_with_output!($bin_path -vv $($a)*)
			.expect("process running")
			.wait_with_output();

		if status.is_err() {
			// cmd_lib limitation: we can either have status code or stdout/stderr captured.
			// So let's re-run failed execution and print the output, so we know what failed.
			tracing::error!("Execution failed, rerunning with output captured");
			spawn_with_output!(
				$bin_path -vv $($a)*
			)
			.expect("process running")
			.wait_with_pipe(&mut |pipe| {
				BufReader::new(pipe)
					.lines()
					.filter_map(|line| line.ok())
					.for_each(|line| tracing::error!("{}", line));
				}).unwrap();
			panic!("Execution of plox failed.");
		}

		Default::default()
	}}
);

fn compare_files(file: &str) {
	let path1 = format!("tests/examples/{}", file);
	let path2 = format!("tests/.examples-expected/{}", file);
	compare_files_inner(&path1, &path2);
}

fn compare_files_inner(path1: &str, path2: &str) {
	let file1 = File::open(path1).unwrap();
	let file2 = File::open(path2).unwrap();

	let reader1 = BufReader::new(file1);
	let reader2 = BufReader::new(file2);

	for (line_num, (line1, line2)) in reader1.lines().zip(reader2.lines()).enumerate() {
		let line1 = line1.unwrap();
		let line2 = line2.unwrap();

		if line1.starts_with("csv_data_file_") && line2.starts_with("csv_data_file_") {
			let prefix1 = line1.split('=').next().unwrap_or("");
			let prefix2 = line2.split('=').next().unwrap_or("");

			if prefix1 != prefix2 {
				panic!(
					"Mismatch found at line {}: Expected line starting with '{}' but found '{}'",
					line_num + 1,
					prefix1,
					prefix2
				);
			}
		} else if line1 != line2 {
			panic!(
				"Mismatch found at line {}: Expected '{}' but found '{}'",
				line_num + 1,
				line1,
				line2
			);
		}
	}
}

#[docify::export_content]
fn cmd_simple() -> String {
	bash!(
		plox graph
		  --input  tests/examples/default.log
		  --output tests/examples/default.png
		  --plot om_module x
	)
}

#[test]
fn test_cmd_simple() {
	plox::logging::init_tracing_test();
	cmd_simple();
	compare_files("default.gnuplot");
}

#[docify::export_content]
fn cmd_simple_panels() -> String {
	bash!(
		plox graph
		  --input  tests/examples/some.log
		  --output tests/examples/panels.png
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

#[test]
fn test_cmd_simple_panels() {
	plox::logging::init_tracing_test();
	cmd_simple_panels();
	compare_files("panels.gnuplot");
}

#[docify::export_content]
fn cmd_demo_lines() -> String {
	bash!(
		plox graph
		  --input  tests/examples/some.log
		  --output tests/examples/demo-lines.png
		  --config tests/examples/demo-lines.toml
	)
}

#[test]
fn test_cmd_demo_lines() {
	plox::logging::init_tracing_test();
	cmd_demo_lines();
	compare_files("demo-lines.gnuplot");
}

#[docify::export_content]
fn cmd_regex() -> String {
	bash!(
		plox graph
		  --input  tests/examples/default.log
		  --output tests/examples/regex.png
		  --plot yam_module r#"y=\([\d\.]+,\s*([\d\.]+)\)"#
		  --title "1st tuple item"
		  --plot yam_module r#"y=\(([\d\.]+),\s*[\d\.]+\)"#
		  --title "2nd tuple item"
	)
}

#[test]
fn test_cmd_regex() {
	plox::logging::init_tracing_test();
	cmd_regex();
	compare_files("regex.gnuplot");
}

#[docify::export_content]
fn cmd_deltas_and_count() -> String {
	bash!(
		plox graph
		  --input tests/examples/default.log
		  --output tests/examples/deltas.png
		  --event-delta foo_module "SOME_EVENT" --yaxis-scale log
		  --style points --marker-size 7 --marker-color olive --marker-type diamond
		  --event-count foo_module "SOME_EVENT" --style steps --yaxis y2
	)
}

#[test]
fn test_cmd_deltas_and_count() {
	plox::logging::init_tracing_test();
	cmd_deltas_and_count();
	compare_files("deltas.gnuplot");
}

#[docify::export_content]
fn cmd_simple_panels_two_files() -> String {
	bash!(
		plox graph
		  --input  tests/examples/default.log,tests/examples/default-other.log
		  --output tests/examples/panels-two-files.png
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

#[test]
fn test_cmd_simple_panels_two_files() {
	plox::logging::init_tracing_test();
	cmd_simple_panels_two_files();
	compare_files("panels-two-files.gnuplot");
}

#[docify::export_content]
fn cmd_demo_lines_two_files() -> String {
	bash!(
		plox graph
		  --input  tests/examples/default.log
		  --input  tests/examples/default-other.log
		  --output tests/examples/demo-lines-two-files.png
		  --timestamp-format "%Y-%m-%d %H:%M:%S%.3f"
		  --per-file-panels
		  --config tests/examples/demo-lines.toml
	)
}

#[test]
fn test_cmd_demo_lines_two_files() {
	plox::logging::init_tracing_test();
	cmd_demo_lines_two_files();
	compare_files("demo-lines-two-files.gnuplot");
}
