use proc_macro::TokenStream;
use syn::{LitStr, parse::Parse, parse_macro_input};

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
pub fn plox_process_doc(input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(input as Args);
	let input_path = args.input.value();

	let content = std::fs::read_to_string(&input_path)
		.unwrap_or_else(|_| panic!("Failed to read {}", input_path));

	let stripped = strip_bash_macro(&content);

	std::fs::write(&input_path, stripped)
		.unwrap_or_else(|_| panic!("Failed to write {}", input_path));

	TokenStream::new()
}

struct Args {
	input: LitStr,
}

impl Parse for Args {
	fn parse(tokens: syn::parse::ParseStream) -> syn::Result<Self> {
		let input: LitStr = tokens.parse()?;
		Ok(Args { input })
	}
}
