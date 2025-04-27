use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse_macro_input, LitStr};

#[proc_macro]
pub fn plox_docify_embed(input: TokenStream) -> TokenStream {
	let path = syn::parse_macro_input!(input as syn::LitStr).value();
	let content = std::fs::read_to_string(path).expect("Failed to read markdown file.");

	let processed = strip_bash_and_fix_img(&content);

	let output = syn::LitStr::new(&processed, proc_macro2::Span::call_site());

	quote!(#output).into()
}

fn strip_bash_and_fix_img(input: &str) -> String {
	// strip bash!() container:
	let bash_re = regex::Regex::new(r#"bash!\(\s*((?:.|\n)*?)\s*\)"#).unwrap();
	let stripped = bash_re.replace_all(input, |caps: &regex::Captures| caps[1].to_string());

	// replace all local paths to github urls:
	let img_re =
		regex::Regex::new(r#"<img\s+[^>]*src="\s*(some-playground/[^">]+\.png)\s*"[^>]*>"#)
			.unwrap();

	img_re
		.replace_all(&stripped, |caps: &regex::Captures| {
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

#[proc_macro]
pub fn plox_docify_generate2(input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(input as Args);
	let input_path = args.input.value();
	let output_path = args.output.value();

	let content = std::fs::read_to_string(&input_path)
		.unwrap_or_else(|_| panic!("Failed to read {}", input_path));

	let processed = strip_bash_and_fix_img(&content);

	std::fs::write(&output_path, processed)
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
