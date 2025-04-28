use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse::Parse, parse_macro_input};

fn rewrite_img_src(input: &str) -> String {
	let re =
		regex::Regex::new(r#"<img\s+[^>]*src="\s*(some-playground/[^">]+)\s*"[^>]*>"#).unwrap();

	re.replace_all(input, |caps: &regex::Captures| {
		let local_path = &caps[1];
		let remote_path = local_path.replacen(
			"some-playground/",
			"https://github.com/michalkucharczyk/plox/blob/master/some-playground/",
			1,
		);
		caps[0].replace(local_path, &remote_path)
	})
	.to_string()
}

fn strip_bash_macro(input: &str) -> String {
	let re = regex::Regex::new(r"(?ms)^\s*bash!\(\s*(.*?)\s*\)\s*$").unwrap();
	let strip_raw_string = regex::Regex::new("r#\\\"(.*?)\\\"#").unwrap();
	re.replace_all(input, |caps: &regex::Captures| {
		let content = &caps[1];

		let lines: Vec<&str> = content.lines().collect();

		if lines.len() <= 1 {
			content.trim().to_string()
		} else {
			let mut out = String::new();
			for (i, line) in lines.iter().enumerate() {
				let trimmed = line.trim_end();
				let lx = strip_raw_string.replace_all(trimmed, r#""$1""#);

				out.push_str(&lx);
				if i + 1 != lines.len() {
					out.push_str(" \\");
				}
				out.push('\n');
			}
			out
		}
	})
	.to_string()
}

#[proc_macro]
pub fn plox_apply_fix(input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(input as Args);
	let input_path = args.input.value();
	let output_path = args.output.value();

	let content = std::fs::read_to_string(&input_path)
		.unwrap_or_else(|_| panic!("Failed to read {}", input_path));

	let stripped = strip_bash_macro(&content);
	let rewritten = rewrite_img_src(&stripped);

	std::fs::write(&input_path, stripped)
		.unwrap_or_else(|_| panic!("Failed to write {}", input_path));

	std::fs::write(&output_path, rewritten)
		.unwrap_or_else(|_| panic!("Failed to write {}", output_path));

	// TokenStream::new() // nothing returned
	quote!().into()
}

struct Args {
	input: LitStr,
	output: LitStr,
}

impl Parse for Args {
	fn parse(tokens: syn::parse::ParseStream) -> syn::Result<Self> {
		let input: LitStr = tokens.parse()?;
		let _: syn::Token![,] = tokens.parse()?;
		let output: LitStr = tokens.parse()?;
		Ok(Args { input, output })
	}
}
