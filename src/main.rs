mod api;
mod auth;
mod cli;
mod config;
mod error;
mod listen;
mod models;
mod output;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;
use crate::output::OutputFormat;

#[tokio::main]
async fn main() {
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
        .with_ansi(!cli.no_color)
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
    if let Err(e) = cli::run(cli, &config).await {
        let format = OutputFormat::detect(crate::config::resolve_output_format(
            output_format_flag.as_deref(),
            &config,
        ));
        output::print_error(format, &e);
        std::process::exit(e.exit_code());
    }
}
