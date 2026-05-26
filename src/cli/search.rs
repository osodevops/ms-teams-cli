use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum SearchCommand {
    /// Search messages
    Messages {
        /// Search query string
        #[arg(required_unless_present = "query_flag", conflicts_with = "query_flag")]
        query: Option<String>,
        /// Search query string
        #[arg(
            long = "query",
            value_name = "QUERY",
            required_unless_present = "query",
            conflicts_with = "query"
        )]
        query_flag: Option<String>,
        /// Maximum number of results
        #[arg(long)]
        top: Option<u64>,
    },
    /// Search users (people)
    Users {
        /// Search query string
        #[arg(required_unless_present = "query_flag", conflicts_with = "query_flag")]
        query: Option<String>,
        /// Search query string
        #[arg(
            long = "query",
            value_name = "QUERY",
            required_unless_present = "query",
            conflicts_with = "query"
        )]
        query_flag: Option<String>,
        /// Maximum number of results
        #[arg(long)]
        top: Option<u64>,
    },
    /// Search your joined teams by display name
    Teams {
        /// Search query string
        #[arg(required_unless_present = "query_flag", conflicts_with = "query_flag")]
        query: Option<String>,
        /// Search query string
        #[arg(
            long = "query",
            value_name = "QUERY",
            required_unless_present = "query",
            conflicts_with = "query"
        )]
        query_flag: Option<String>,
    },
}

pub async fn run(
    cmd: SearchCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        SearchCommand::Messages {
            query,
            query_flag,
            top,
        } => {
            let start = Instant::now();
            let query = resolve_query(query, query_flag)?;
            let result = api::search::search_messages(&client, &query, top).await?;

            if format == OutputFormat::Human {
                let hits = extract_hits(&result);
                let headers = vec!["Rank", "Summary", "Hit ID"];
                let rows: Vec<Vec<String>> = hits
                    .iter()
                    .map(|h| {
                        vec![
                            h.rank.map(|r| r.to_string()).unwrap_or_default(),
                            h.summary.clone().unwrap_or_default(),
                            h.hit_id.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success(format, &result, start);
            }
            Ok(())
        }

        SearchCommand::Users {
            query,
            query_flag,
            top,
        } => {
            let start = Instant::now();
            let query = resolve_query(query, query_flag)?;
            let result = api::search::search_users(&client, &query, top).await?;

            if format == OutputFormat::Human {
                let hits = extract_hits(&result);
                let headers = vec!["Hit ID", "Summary"];
                let rows: Vec<Vec<String>> = hits
                    .iter()
                    .map(|h| {
                        vec![
                            h.hit_id.clone().unwrap_or_default(),
                            h.summary.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success(format, &result, start);
            }
            Ok(())
        }

        SearchCommand::Teams { query, query_flag } => {
            let start = Instant::now();
            let query = resolve_query(query, query_flag)?;
            let teams = api::search::search_teams(&client, &query).await?;

            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Description"];
                let rows: Vec<Vec<String>> = teams
                    .iter()
                    .map(|t| {
                        vec![
                            t.id.clone().unwrap_or_default(),
                            t.display_name.clone().unwrap_or_default(),
                            t.description.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &teams, start);
            }
            Ok(())
        }
    }
}

fn resolve_query(positional: Option<String>, flag: Option<String>) -> Result<String> {
    match (positional, flag) {
        (Some(_), Some(_)) => Err(crate::error::TeamsError::InvalidInput(
            "Provide only one of --query or <QUERY>".into(),
        )),
        (Some(query), None) | (None, Some(query)) => Ok(query),
        (None, None) => Err(crate::error::TeamsError::InvalidInput(
            "Missing required search query: --query or <QUERY>".into(),
        )),
    }
}

fn extract_hits(
    result: &crate::models::search::SearchResponse,
) -> Vec<&crate::models::search::SearchHit> {
    result
        .value
        .iter()
        .flat_map(|rs| rs.hits_containers.iter().flatten())
        .flat_map(|hc| hc.hits.iter().flatten())
        .collect()
}
