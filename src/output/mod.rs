pub mod json;
pub mod plain;
pub mod progress;
pub mod table;

use crate::error::TeamsError;
use crate::models::common::{Envelope, Metadata};
use serde::Serialize;
use std::io::IsTerminal;
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

pub fn print_success<T: Serialize>(format: OutputFormat, data: &T, start: Instant) {
    let metadata = Metadata::new().with_duration(start.elapsed().as_millis() as u64);
    match format {
        OutputFormat::Json => {
            let envelope = Envelope::success(data, metadata);
            json::print(&envelope);
        }
        OutputFormat::Human => {
            let output = serde_json::to_string_pretty(data).unwrap_or_default();
            println!("{output}");
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
            println!("{output}");
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
            json::print(&envelope);
        }
        OutputFormat::Human | OutputFormat::Plain => {
            eprintln!("Error: {err}");
        }
    }
}
