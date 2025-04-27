fn main() {
	#[cfg(feature = "generate-readme")]
	{
		println!("cargo:rerun-if-changed=SAMPLE.docify.md");
		println!("cargo:rerun-if-changed=tests/cmd_tests.rs");
	}
}
