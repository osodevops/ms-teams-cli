use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::channel::{ChannelCreateRequest, ChannelUpdateRequest};
use crate::models::member::AddMemberRequest;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum ChannelCommand {
    /// List channels in a team
    List {
        /// Team ID
        team_id: String,
    },
    /// Get a channel by ID
    Get {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
    },
    /// Create a new channel
    Create {
        /// Team ID
        team_id: String,
        /// Channel display name
        #[arg(long)]
        name: String,
        /// Channel description
        #[arg(long)]
        description: Option<String>,
        /// Membership type (standard, private, shared)
        #[arg(long, alias = "type", default_value = "standard")]
        membership_type: String,
    },
    /// Update a channel
    Update {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
        /// New display name
        #[arg(long)]
        name: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a channel
    Delete {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
    },
    /// Channel member operations
    Members {
        #[command(subcommand)]
        command: ChannelMemberCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ChannelMemberCommand {
    /// List channel members
    List {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
    },
    /// Add a member to a channel
    Add {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
        /// User ID to add
        #[arg(long)]
        user_id: String,
        /// Role (owner or member)
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// Remove a member from a channel
    Remove {
        /// Team ID
        team_id: String,
        /// Channel ID
        channel_id: String,
        /// Membership ID to remove
        member_id: String,
    },
}

pub async fn run(
    cmd: ChannelCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        ChannelCommand::List { team_id } => {
            let start = Instant::now();
            let channels = api::channels::list_channels(&client, &team_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Type", "Email"];
                let rows: Vec<Vec<String>> = channels
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone().unwrap_or_default(),
                            c.display_name.clone().unwrap_or_default(),
                            c.membership_type.clone().unwrap_or_default(),
                            c.email.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &channels, start);
            }
            Ok(())
        }

        ChannelCommand::Get {
            team_id,
            channel_id,
        } => {
            let start = Instant::now();
            let channel = api::channels::get_channel(&client, &team_id, &channel_id).await?;
            output::print_success(format, &channel, start);
            Ok(())
        }

        ChannelCommand::Create {
            team_id,
            name,
            description,
            membership_type,
        } => {
            let start = Instant::now();
            let req = ChannelCreateRequest {
                display_name: name,
                description,
                membership_type: Some(membership_type),
            };
            let channel = api::channels::create_channel(&client, &team_id, &req).await?;
            output::print_success(format, &channel, start);
            Ok(())
        }

        ChannelCommand::Update {
            team_id,
            channel_id,
            name,
            description,
        } => {
            let start = Instant::now();
            let req = ChannelUpdateRequest {
                display_name: name,
                description,
            };
            let channel =
                api::channels::update_channel(&client, &team_id, &channel_id, &req).await?;
            output::print_success(format, &channel, start);
            Ok(())
        }

        ChannelCommand::Delete {
            team_id,
            channel_id,
        } => {
            let start = Instant::now();
            api::channels::delete_channel(&client, &team_id, &channel_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        ChannelCommand::Members { command } => {
            run_members(command, &client, format, pagination).await
        }
    }
}

async fn run_members(
    cmd: ChannelMemberCommand,
    client: &GraphClient,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    match cmd {
        ChannelMemberCommand::List {
            team_id,
            channel_id,
        } => {
            let start = Instant::now();
            let members =
                api::channels::list_members(client, &team_id, &channel_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Roles", "Email"];
                let rows: Vec<Vec<String>> = members
                    .iter()
                    .map(|m| {
                        vec![
                            m.id.clone().unwrap_or_default(),
                            m.display_name.clone().unwrap_or_default(),
                            m.roles.as_ref().map(|r| r.join(", ")).unwrap_or_default(),
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

        ChannelMemberCommand::Add {
            team_id,
            channel_id,
            user_id,
            role,
        } => {
            let start = Instant::now();
            let roles = if role == "member" { vec![] } else { vec![role] };
            let req = AddMemberRequest::new(&user_id, roles);
            let member = api::channels::add_member(client, &team_id, &channel_id, &req).await?;
            output::print_success(format, &member, start);
            Ok(())
        }

        ChannelMemberCommand::Remove {
            team_id,
            channel_id,
            member_id,
        } => {
            let start = Instant::now();
            api::channels::remove_member(client, &team_id, &channel_id, &member_id).await?;
            let result = serde_json::json!({"status": "removed"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
