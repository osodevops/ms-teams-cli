use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    /// Get current authenticated user's profile
    Me,
    /// Get a user by ID or UPN
    Get {
        /// User ID or user principal name (email)
        id: String,
    },
    /// List users in the organization
    List {
        /// OData filter expression
        #[arg(long)]
        filter: Option<String>,
    },
}

pub async fn run(
    cmd: UserCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        UserCommand::Me => {
            let start = Instant::now();
            let user = api::users::get_me(&client).await?;
            output::print_success(format, &user, start);
            Ok(())
        }

        UserCommand::Get { id } => {
            let start = Instant::now();
            let user = api::users::get_user(&client, &id).await?;
            output::print_success(format, &user, start);
            Ok(())
        }

        UserCommand::List { filter } => {
            let start = Instant::now();
            let users =
                api::users::list_users(&client, filter.as_deref(), pagination).await?;

            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Email", "Job Title"];
                let rows: Vec<Vec<String>> = users
                    .iter()
                    .map(|u| {
                        vec![
                            u.id.clone().unwrap_or_default(),
                            u.display_name.clone().unwrap_or_default(),
                            u.mail
                                .clone()
                                .or_else(|| u.user_principal_name.clone())
                                .unwrap_or_default(),
                            u.job_title.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &users, start);
            }
            Ok(())
        }
    }
}
