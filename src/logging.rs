use tracing_subscriber::{EnvFilter, fmt};

use crate::process_log::MATCH_PREVIEW;

/// Tracing target for verbose (-v -vv -vvv) cross-application messages.
pub const APPV: &str = "appverbose";

#[cfg(test)]
pub(crate) fn init_tracing_test() {
	use std::sync::Once;
	static INIT: Once = Once::new();
	INIT.call_once(|| {
		init_tracing(false, 0);
	});
}

pub fn init_tracing(quiet: bool, verbosity: u8) {
	use tracing_subscriber::prelude::*;
	if std::env::var("RUST_LOG").is_ok() {
		let rust_log_env = std::env::var("RUST_LOG").unwrap_or_default();
		let preview_given = rust_log_env.contains(MATCH_PREVIEW);
		let mut full_filter = EnvFilter::new(&rust_log_env);

		if !preview_given {
			let directive = format!("{}=off", MATCH_PREVIEW);
			full_filter = full_filter.add_directive(directive.parse().unwrap());
		}

		let subscriber = tracing_subscriber::registry()
			.with(fmt::layer().with_target(true))
			.with(full_filter);

		tracing::subscriber::set_global_default(subscriber)
			.expect("Failed to set tracing subscriber");
	} else {
		let level = match (quiet, verbosity) {
			(true, _) => None,
			(false, 0) => Some("info"),
			(false, 1) => Some("debug"),
			(false, _) => Some("trace"),
		};

		let env_filter = if let Some(level) = level {
			EnvFilter::new(format!("warn,{}={level}", APPV))
		} else {
			EnvFilter::new("warn")
		};

		let fmt_layer = fmt::layer().without_time().with_target(false).with_level(true);

		let subscriber = tracing_subscriber::registry().with(fmt_layer).with(env_filter);
		tracing::subscriber::set_global_default(subscriber)
			.expect("Failed to set tracing subscriber");
	};
}

// fn testing() {
// 	error!(target: "some", "some, error");
// 	warn!(target: "some", "some, warn");
// 	info!(target: "some", "some, info");
// 	debug!(target: "some", "some, debug");
// 	trace!(target: "some", "some, trace");
//
// 	error!(target: logging::APPV, "appv, error");
// 	warn!(target:  logging::APPV, "appv, warn");
// 	info!(target:  logging::APPV, "appv, info");
// 	debug!(target: logging::APPV, "appv, debug");
// 	trace!(target: logging::APPV, "appv, trace");
//
// 	return Ok(());
// }
