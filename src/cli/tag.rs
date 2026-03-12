use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::tag::{AddTagMemberRequest, CreateTagMemberEntry, CreateTagRequest, UpdateTagRequest};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum TagCommand {
    /// List tags in a team
    List {
        /// Team ID
        team_id: String,
    },
    /// Get a tag by ID
    Get {
        /// Team ID
        team_id: String,
        /// Tag ID
        tag_id: String,
    },
    /// Create a tag
    Create {
        /// Team ID
        team_id: String,
        /// Tag display name
        #[arg(long)]
        name: String,
        /// Comma-separated user IDs to add as members
        #[arg(long)]
        members: Option<String>,
    },
    /// Update a tag
    Update {
        /// Team ID
        team_id: String,
        /// Tag ID
        tag_id: String,
        /// New display name
        #[arg(long)]
        name: String,
    },
    /// Delete a tag
    Delete {
        /// Team ID
        team_id: String,
        /// Tag ID
        tag_id: String,
    },
    /// Add a member to a tag
    AddMember {
        /// Team ID
        team_id: String,
        /// Tag ID
        tag_id: String,
        /// User ID to add
        #[arg(long)]
        user: String,
    },
    /// Remove a member from a tag
    RemoveMember {
        /// Team ID
        team_id: String,
        /// Tag ID
        tag_id: String,
        /// User ID to remove
        #[arg(long)]
        user: String,
    },
}

pub async fn run(
    cmd: TagCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        TagCommand::List { team_id } => {
            let start = Instant::now();
            let tags = api::tags::list_tags(&client, &team_id, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Display Name", "Description", "Member Count"];
                let rows: Vec<Vec<String>> = tags
                    .iter()
                    .map(|t| {
                        vec![
                            t.id.clone().unwrap_or_default(),
                            t.display_name.clone().unwrap_or_default(),
                            t.description.clone().unwrap_or_default(),
                            t.member_count
                                .map(|c| c.to_string())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &tags, start);
            }
            Ok(())
        }

        TagCommand::Get { team_id, tag_id } => {
            let start = Instant::now();
            let tag = api::tags::get_tag(&client, &team_id, &tag_id).await?;
            output::print_success(format, &tag, start);
            Ok(())
        }

        TagCommand::Create {
            team_id,
            name,
            members,
        } => {
            let start = Instant::now();
            let member_entries: Vec<CreateTagMemberEntry> = members
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|uid| CreateTagMemberEntry {
                    user_id: uid.trim().to_string(),
                })
                .collect();
            let req = CreateTagRequest {
                display_name: name,
                members: member_entries,
            };
            let tag = api::tags::create_tag(&client, &team_id, &req).await?;
            output::print_success(format, &tag, start);
            Ok(())
        }

        TagCommand::Update {
            team_id,
            tag_id,
            name,
        } => {
            let start = Instant::now();
            let req = UpdateTagRequest {
                display_name: name,
            };
            let tag = api::tags::update_tag(&client, &team_id, &tag_id, &req).await?;
            output::print_success(format, &tag, start);
            Ok(())
        }

        TagCommand::Delete { team_id, tag_id } => {
            let start = Instant::now();
            api::tags::delete_tag(&client, &team_id, &tag_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        TagCommand::AddMember {
            team_id,
            tag_id,
            user,
        } => {
            let start = Instant::now();
            let req = AddTagMemberRequest {
                user_id: user,
            };
            let member =
                api::tags::add_tag_member(&client, &team_id, &tag_id, &req).await?;
            output::print_success(format, &member, start);
            Ok(())
        }

        TagCommand::RemoveMember {
            team_id,
            tag_id,
            user,
        } => {
            let start = Instant::now();
            api::tags::remove_tag_member(&client, &team_id, &tag_id, &user).await?;
            let result = serde_json::json!({"status": "removed"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
