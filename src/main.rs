mod api;
mod auth;
mod cli;
mod config;
mod error;
mod help_json;
mod listen;
mod models;
mod output;

use clap::{CommandFactory, Parser};
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;
use crate::output::OutputFormat;

#[tokio::main]
async fn main() {
    // Handled before clap parses, because the root requires a subcommand and
    // `teams --help-json` deliberately has none. The docs site calls this on the
    // released binary to regenerate its command reference.
    if std::env::args_os().any(|arg| arg == "--help-json") {
        let dump = help_json::build(Cli::command());
        match serde_json::to_string_pretty(&dump) {
            Ok(json) => output::write_stdout_line(&json),
            Err(e) => {
                eprintln!("Failed to render help JSON: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let cli = Cli::parse();

    let output_format_flag = cli.output.clone();

    // Initialise tracing (logs go to stderr)
    let filter = match cli.verbose {
        0 => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(log_colors(
            cli.no_color,
            std::env::var_os("NO_COLOR"),
            std::io::IsTerminal::is_terminal(&std::io::stderr()),
        ))
        .init();

    // Load config
    let config = match config::load_config(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            let format = OutputFormat::detect(output_format_flag.as_deref());
            output::print_error(format, &e);
            std::process::exit(e.exit_code());
        }
    };

    // Run the command
    let result = match auth::keyring::check_token_store() {
        Ok(()) => cli::run(cli, &config).await,
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        let format = OutputFormat::detect(crate::config::resolve_output_format(
            output_format_flag.as_deref(),
            &config,
        ));
        output::print_error(format, &e);
        std::process::exit(e.exit_code());
    }
}

/// Whether log lines on stderr carry ANSI colour: not when `--no-color` or a
/// non-empty `NO_COLOR` asks otherwise, nor when stderr is a file or pipe,
/// where the codes would land in a log as text.
fn log_colors(
    no_color: bool,
    no_color_env: Option<std::ffi::OsString>,
    stderr_is_terminal: bool,
) -> bool {
    let no_color_env = no_color_env.is_some_and(|v| !v.is_empty());
    !no_color && !no_color_env && stderr_is_terminal
}

#[cfg(test)]
mod tests {
    use super::log_colors;

    #[test]
    fn log_colors_only_on_a_terminal_without_a_no_color_request() {
        assert!(log_colors(false, None, true));
        assert!(!log_colors(false, None, false));
        assert!(!log_colors(true, None, true));
        assert!(!log_colors(false, Some("1".into()), true));
        assert!(log_colors(false, Some("".into()), true));
    }
}
