pub mod json;
pub mod plain;
pub mod progress;
pub mod table;

use crate::error::TeamsError;
use crate::models::common::{Envelope, Metadata};
use serde::Serialize;
use std::io::{self, IsTerminal, Write};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Human,
    Plain,
}

impl OutputFormat {
    pub fn detect(flag: Option<&str>) -> Self {
        match flag {
            Some("json") => Self::Json,
            Some("human") | Some("table") => Self::Human,
            Some("plain") | Some("text") => Self::Plain,
            _ => {
                if std::io::stdout().is_terminal() {
                    Self::Human
                } else {
                    Self::Json
                }
            }
        }
    }
}

/// Write bytes to stdout, treating a closed pipe as normal early termination.
pub fn write_stdout(bytes: &[u8]) {
    handle_stdout_result(try_write_stdout(bytes));
}

/// Write a line to stdout, treating a closed pipe as normal early termination.
pub fn write_stdout_line(line: &str) {
    handle_stdout_result(try_write_stdout_line(line));
}

fn try_write_stdout(bytes: &[u8]) -> io::Result<()> {
    io::stdout().lock().write_all(bytes)
}

fn try_write_stdout_line(line: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(line.as_bytes())?;
    stdout.write_all(b"\n")
}

fn handle_stdout_result(result: io::Result<()>) {
    if let Err(err) = result {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }

        eprintln!("Error: Failed to write to stdout: {err}");
        std::process::exit(1);
    }
}

pub fn print_success<T: Serialize>(format: OutputFormat, data: &T, start: Instant) {
    let metadata = Metadata::new().with_duration(start.elapsed().as_millis() as u64);
    match format {
        OutputFormat::Json => {
            let envelope = Envelope::success(data, metadata);
            json::print(&envelope);
        }
        OutputFormat::Human => {
            let output = serde_json::to_string_pretty(data).unwrap_or_default();
            write_stdout_line(&output);
        }
        OutputFormat::Plain => {
            plain::print_object(data);
        }
    }
}

pub fn print_success_list<T: Serialize>(format: OutputFormat, data: &[T], start: Instant) {
    let metadata = Metadata::new().with_duration(start.elapsed().as_millis() as u64);
    match format {
        OutputFormat::Json => {
            let envelope = Envelope::success(data, metadata);
            json::print(&envelope);
        }
        OutputFormat::Human => {
            let output = serde_json::to_string_pretty(data).unwrap_or_default();
            write_stdout_line(&output);
        }
        OutputFormat::Plain => {
            plain::print_list(data);
        }
    }
}

pub fn print_error(format: OutputFormat, err: &TeamsError) {
    let metadata = Metadata::new();
    match format {
        OutputFormat::Json => {
            let envelope = Envelope::<()>::error(err.error_code(), err.to_string(), metadata);
            let output = json::serialize(&envelope);
            if let Err(write_err) = try_write_stdout_line(&output) {
                if write_err.kind() != io::ErrorKind::BrokenPipe {
                    eprintln!("Error: Failed to write error response to stdout: {write_err}");
                }
            }
        }
        OutputFormat::Human | OutputFormat::Plain => {
            eprintln!("Error: {err}");
        }
    }
}
