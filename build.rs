fn main() {
	#[cfg(feature = "generate-readme")]
	{
		println!("cargo:rerun-if-changed=SAMPLE.docify.md");

		docify::compile_markdown!("SAMPLE.docify.md", "SAMPLE.local.md");

		let doc =
			std::fs::read_to_string("SAMPLE.local.md").expect("Failed to read SAMPLE.local.md");

		let stripped = strip_bash_macro(&doc);
		let rewritten = rewrite_img_src(&stripped);

		std::fs::write("SAMPLE.local.md", stripped).expect("Failed to write SAMPLE.md");
		std::fs::write("SAMPLE.md", rewritten).expect("Failed to write SAMPLE.md");
	}
}

#[cfg(feature = "generate-readme")]
fn rewrite_img_src(input: &str) -> String {
	let re = regex::Regex::new(r#"<img\s+[^>]*src="\s*(some-playground/[^">]+\.png)\s*"[^>]*>"#)
		.unwrap();

	re.replace_all(input, |caps: &regex::Captures| {
		let local_path = &caps[1];
		let remote_path = local_path.replacen(
			"some-playground/",
			"https://raw.githubusercontent.com/michalkucharczyk/plox/main/some-playground/",
			1,
		);
		caps[0].replace(local_path, &remote_path)
	})
	.to_string()
}

#[cfg(feature = "generate-readme")]
fn strip_bash_macro(input: &str) -> String {
	let re = regex::Regex::new(r#"bash!\(\s*((?:.|\n)*?)\s*\)"#).unwrap();
	re.replace_all(input, |caps: &regex::Captures| caps[1].to_string()).to_string()
}
