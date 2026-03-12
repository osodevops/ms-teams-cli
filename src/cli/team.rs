use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::member::AddMemberRequest;
use crate::models::team::{TeamCloneRequest, TeamCreateRequest, TeamUpdateRequest};
use crate::output::{self, progress, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    /// List joined teams
    List,
    /// Get a team by ID
    Get {
        /// Team ID
        id: String,
    },
    /// Create a new team
    Create {
        /// Display name for the team
        #[arg(long)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// Update a team
    Update {
        /// Team ID
        id: String,
        /// New display name
        #[arg(long)]
        name: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a team
    Delete {
        /// Team ID
        id: String,
    },
    /// Clone a team
    Clone {
        /// Source team ID
        id: String,
        /// Display name for the cloned team
        #[arg(long)]
        name: String,
        /// Parts to clone (comma-separated: apps,tabs,settings,channels,members)
        #[arg(long, default_value = "apps,tabs,settings,channels,members")]
        parts: String,
        /// Visibility (public or private)
        #[arg(long, default_value = "private")]
        visibility: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// Archive a team
    Archive {
        /// Team ID
        id: String,
    },
    /// Unarchive a team
    Unarchive {
        /// Team ID
        id: String,
    },
    /// Team member operations
    Members {
        #[command(subcommand)]
        command: TeamMemberCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum TeamMemberCommand {
    /// List team members
    List {
        /// Team ID
        team_id: String,
    },
    /// Add a member to a team
    Add {
        /// Team ID
        team_id: String,
        /// User ID to add
        #[arg(long)]
        user_id: String,
        /// Roles (e.g., owner, member)
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// Remove a member from a team
    Remove {
        /// Team ID
        team_id: String,
        /// Membership ID to remove
        member_id: String,
    },
}

pub async fn run(
    cmd: TeamCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        TeamCommand::List => {
            let start = Instant::now();
            let teams = api::teams::list_joined_teams(&client, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Description", "Archived"];
                let rows: Vec<Vec<String>> = teams
                    .iter()
                    .map(|t| {
                        vec![
                            t.id.clone().unwrap_or_default(),
                            t.display_name.clone().unwrap_or_default(),
                            t.description.clone().unwrap_or_default(),
                            t.is_archived
                                .map(|a| a.to_string())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &teams, start);
            }
            Ok(())
        }

        TeamCommand::Get { id } => {
            let start = Instant::now();
            let team = api::teams::get_team(&client, &id).await?;
            output::print_success(format, &team, start);
            Ok(())
        }

        TeamCommand::Create { name, description } => {
            let start = Instant::now();
            let pb = progress::spinner("Creating team...");
            let req = TeamCreateRequest::new(name, description);
            let location = api::teams::create_team(&client, &req).await?;
            pb.finish_and_clear();
            let result = serde_json::json!({
                "status": "accepted",
                "message": "Team creation initiated (async operation)",
                "operation_url": location,
            });
            output::print_success(format, &result, start);
            Ok(())
        }

        TeamCommand::Update {
            id,
            name,
            description,
        } => {
            let start = Instant::now();
            let req = TeamUpdateRequest {
                display_name: name,
                description,
            };
            let team = api::teams::update_team(&client, &id, &req).await?;
            output::print_success(format, &team, start);
            Ok(())
        }

        TeamCommand::Delete { id } => {
            let start = Instant::now();
            api::teams::delete_team(&client, &id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        TeamCommand::Clone {
            id,
            name,
            parts,
            visibility,
            description,
        } => {
            let start = Instant::now();
            let pb = progress::spinner("Cloning team...");
            let req = TeamCloneRequest {
                display_name: name,
                parts_to_clone: parts,
                visibility,
                description,
            };
            let location = api::teams::clone_team(&client, &id, &req).await?;
            pb.finish_and_clear();
            let result = serde_json::json!({
                "status": "accepted",
                "message": "Team clone initiated (async operation)",
                "operation_url": location,
            });
            output::print_success(format, &result, start);
            Ok(())
        }

        TeamCommand::Archive { id } => {
            let start = Instant::now();
            api::teams::archive_team(&client, &id).await?;
            let result = serde_json::json!({"status": "archived"});
            output::print_success(format, &result, start);
            Ok(())
        }

        TeamCommand::Unarchive { id } => {
            let start = Instant::now();
            api::teams::unarchive_team(&client, &id).await?;
            let result = serde_json::json!({"status": "unarchived"});
            output::print_success(format, &result, start);
            Ok(())
        }

        TeamCommand::Members { command } => {
            run_members(command, &client, format, pagination).await
        }
    }
}

async fn run_members(
    cmd: TeamMemberCommand,
    client: &GraphClient,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    match cmd {
        TeamMemberCommand::List { team_id } => {
            let start = Instant::now();
            let members = api::teams::list_members(client, &team_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Roles", "Email"];
                let rows: Vec<Vec<String>> = members
                    .iter()
                    .map(|m| {
                        vec![
                            m.id.clone().unwrap_or_default(),
                            m.display_name.clone().unwrap_or_default(),
                            m.roles
                                .as_ref()
                                .map(|r| r.join(", "))
                                .unwrap_or_default(),
                            m.email.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &members, start);
            }
            Ok(())
        }

        TeamMemberCommand::Add {
            team_id,
            user_id,
            role,
        } => {
            let start = Instant::now();
            let roles = if role == "member" {
                vec![]
            } else {
                vec![role]
            };
            let req = AddMemberRequest::new(&user_id, roles);
            let member = api::teams::add_member(client, &team_id, &req).await?;
            output::print_success(format, &member, start);
            Ok(())
        }

        TeamMemberCommand::Remove {
            team_id,
            member_id,
        } => {
            let start = Instant::now();
            api::teams::remove_member(client, &team_id, &member_id).await?;
            let result = serde_json::json!({"status": "removed"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
