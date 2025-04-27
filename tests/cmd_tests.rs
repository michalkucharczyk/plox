use cmd_lib::spawn_with_output;

// Used for running commands visually pleasing in doc tests.
macro_rules! bash(
	( plox $($a:tt)* ) => {{
		let bin_path = env!("CARGO_BIN_EXE_plox");
		spawn_with_output!(
			$bin_path $($a)*
		)
		.expect("a process running. qed")
		.wait_with_output()
		.expect("to get output. qed.")
	}}
);

#[docify::export_content]
fn cmd_simple() -> String {
	bash!(
		plox graph
		  --input  some-playground/default.log
		  --output some-playground/default.png
		  --plot down x
	)
}

#[docify::export_content]
fn cmd_demo_lines() -> String {
	bash!(
		plox graph  --input some-playground/some.log --config some-playground/demo-lines.toml -o some-playground/demo-lines.png
	)
}

#[test]
fn test_cmd_simple() {
	cmd_simple();
}

#[test]
fn test_cmd_demo_lines() {
	cmd_demo_lines();
}
