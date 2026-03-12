use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::app::{CreateTabRequest, TabConfiguration};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum TabCommand {
    /// List tabs in a channel
    List {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
    },
    /// Create a tab in a channel
    Create {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
        /// App catalog ID
        #[arg(long)]
        app_id: String,
        /// Tab display name
        #[arg(long)]
        name: String,
        /// Content URL for the tab
        #[arg(long)]
        content_url: String,
    },
    /// Delete a tab from a channel
    Delete {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
        /// Tab ID
        #[arg(long)]
        tab_id: String,
    },
}

pub async fn run(
    cmd: TabCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        TabCommand::List {
            team_id,
            channel_id,
        } => {
            let start = Instant::now();
            let tabs =
                api::apps::list_tabs(&client, &team_id, &channel_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["Tab ID", "Display Name", "App Name", "Web URL"];
                let rows: Vec<Vec<String>> = tabs
                    .iter()
                    .map(|t| {
                        vec![
                            t.id.clone().unwrap_or_default(),
                            t.display_name.clone().unwrap_or_default(),
                            t.teams_app
                                .as_ref()
                                .and_then(|a| a.display_name.clone())
                                .unwrap_or_default(),
                            t.web_url.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &tabs, start);
            }
            Ok(())
        }

        TabCommand::Create {
            team_id,
            channel_id,
            app_id,
            name,
            content_url,
        } => {
            let start = Instant::now();
            let req = CreateTabRequest {
                display_name: name,
                teams_app_bind: format!(
                    "https://graph.microsoft.com/v1.0/appCatalogs/teamsApps/{}",
                    app_id
                ),
                configuration: TabConfiguration {
                    entity_id: None,
                    content_url: Some(content_url),
                    website_url: None,
                    remove_url: None,
                },
            };
            let tab =
                api::apps::create_tab(&client, &team_id, &channel_id, &req).await?;
            output::print_success(format, &tab, start);
            Ok(())
        }

        TabCommand::Delete {
            team_id,
            channel_id,
            tab_id,
        } => {
            let start = Instant::now();
            api::apps::delete_tab(&client, &team_id, &channel_id, &tab_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
