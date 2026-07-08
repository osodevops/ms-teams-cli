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
    /// Resolve a name or email to user candidates, degrading with available
    /// scopes: exact lookup, then people search, then a roster sweep over
    /// shared group/meeting chats
    Resolve {
        /// Display name, email address, UPN, or user object ID
        query: String,
        /// Maximum group/meeting chats to scan in the roster-sweep fallback
        #[arg(long, default_value_t = 200)]
        max_chats: u64,
    },
}

pub async fn run(
    cmd: UserCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile).await?;
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
            let users = api::users::list_users(&client, filter.as_deref(), pagination).await?;

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

        UserCommand::Resolve { query, max_chats } => {
            let start = Instant::now();
            let result = api::resolve::resolve_user(&client, &query, max_chats).await?;

            if result.candidates.is_empty() {
                let attempted: Vec<String> = result
                    .stages
                    .iter()
                    .map(|s| format!("{}={}", s.stage, s.status))
                    .collect();
                return Err(crate::error::TeamsError::NotFound(format!(
                    "no user matching '{}' ({})",
                    query,
                    attempted.join(", ")
                )));
            }

            if format == OutputFormat::Human {
                let headers = vec![
                    "ID",
                    "Display Name",
                    "Email",
                    "UPN",
                    "Job Title",
                    "Department",
                    "Via",
                ];
                let rows: Vec<Vec<String>> = result
                    .candidates
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone().unwrap_or_default(),
                            c.display_name.clone().unwrap_or_default(),
                            c.mail.clone().unwrap_or_default(),
                            c.user_principal_name.clone().unwrap_or_default(),
                            c.job_title.clone().unwrap_or_default(),
                            c.department.clone().unwrap_or_default(),
                            c.via.clone(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success(format, &result, start);
            }
            Ok(())
        }
    }
}
