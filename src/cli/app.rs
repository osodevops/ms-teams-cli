use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::app::InstallAppRequest;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum AppCommand {
    /// List installed apps in a team
    List {
        /// Team ID
        team_id: String,
    },
    /// Install an app in a team
    Install {
        /// Team ID
        team_id: String,
        /// App catalog ID
        #[arg(long)]
        app_id: String,
    },
    /// Uninstall an app from a team
    Uninstall {
        /// Team ID
        team_id: String,
        /// Installation ID
        #[arg(long)]
        app_id: String,
    },
}

pub async fn run(
    cmd: AppCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile).await?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        AppCommand::List { team_id } => {
            let start = Instant::now();
            let apps = api::apps::list_team_apps(&client, &team_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["Installation ID", "App Name", "Version", "Distribution"];
                let rows: Vec<Vec<String>> = apps
                    .iter()
                    .map(|a| {
                        let app = a.teams_app.as_ref();
                        let def = a.teams_app_definition.as_ref();
                        vec![
                            a.id.clone().unwrap_or_default(),
                            app.and_then(|a| a.display_name.clone()).unwrap_or_default(),
                            def.and_then(|d| d.version.clone()).unwrap_or_default(),
                            app.and_then(|a| a.distribution_method.clone())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &apps, start);
            }
            Ok(())
        }

        AppCommand::Install { team_id, app_id } => {
            let start = Instant::now();
            let req = InstallAppRequest::new(&app_id);
            api::apps::install_team_app(&client, &team_id, &req).await?;
            let result = serde_json::json!({"status": "installed"});
            output::print_success(format, &result, start);
            Ok(())
        }

        AppCommand::Uninstall { team_id, app_id } => {
            let start = Instant::now();
            api::apps::uninstall_team_app(&client, &team_id, &app_id).await?;
            let result = serde_json::json!({"status": "uninstalled"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
