use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::chat::{ChatCreateRequest, ChatUpdateRequest};
use crate::models::member::AddMemberRequest;
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum ChatCommand {
    /// List your chats
    List,
    /// Get a chat by ID
    Get {
        /// Chat ID
        id: String,
    },
    /// Create a new chat
    Create {
        /// Chat type: oneOnOne or group
        #[arg(long, default_value = "group")]
        chat_type: String,
        /// Topic for the chat (group chats only)
        #[arg(long)]
        topic: Option<String>,
        /// User IDs to add as members (comma-separated)
        #[arg(long, value_delimiter = ',')]
        members: Vec<String>,
    },
    /// Update a chat topic
    Update {
        /// Chat ID
        id: String,
        /// New topic
        #[arg(long)]
        topic: String,
    },
    /// Hide a chat
    Hide {
        /// Chat ID
        chat_id: String,
        /// Your user ID
        #[arg(long)]
        user_id: String,
    },
    /// Unhide a chat
    Unhide {
        /// Chat ID
        chat_id: String,
        /// Your user ID
        #[arg(long)]
        user_id: String,
    },
    /// Chat member operations
    Members {
        #[command(subcommand)]
        command: ChatMemberCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ChatMemberCommand {
    /// List chat members
    List {
        /// Chat ID
        chat_id: String,
    },
    /// Add a member to a chat
    Add {
        /// Chat ID
        chat_id: String,
        /// User ID to add
        #[arg(long)]
        user_id: String,
        /// Role (owner or member)
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// Remove a member from a chat
    Remove {
        /// Chat ID
        chat_id: String,
        /// Membership ID to remove
        member_id: String,
    },
}

pub async fn run(
    cmd: ChatCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        ChatCommand::List => {
            let start = Instant::now();
            let chats = api::chats::list_chats(&client, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Topic", "Type", "Last Updated"];
                let rows: Vec<Vec<String>> = chats
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone().unwrap_or_default(),
                            c.topic.clone().unwrap_or_default(),
                            c.chat_type.clone().unwrap_or_default(),
                            c.last_updated_date_time.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &chats, start);
            }
            Ok(())
        }

        ChatCommand::Get { id } => {
            let start = Instant::now();
            let chat = api::chats::get_chat(&client, &id).await?;
            output::print_success(format, &chat, start);
            Ok(())
        }

        ChatCommand::Create {
            chat_type,
            topic,
            members,
        } => {
            let start = Instant::now();
            let member_reqs: Vec<AddMemberRequest> = members
                .iter()
                .map(|uid| AddMemberRequest::new(uid, vec![]))
                .collect();
            let req = ChatCreateRequest {
                chat_type,
                topic,
                members: member_reqs,
            };
            let chat = api::chats::create_chat(&client, &req).await?;
            output::print_success(format, &chat, start);
            Ok(())
        }

        ChatCommand::Update { id, topic } => {
            let start = Instant::now();
            let req = ChatUpdateRequest {
                topic: Some(topic),
            };
            let chat = api::chats::update_chat(&client, &id, &req).await?;
            output::print_success(format, &chat, start);
            Ok(())
        }

        ChatCommand::Hide { chat_id, user_id } => {
            let start = Instant::now();
            api::chats::hide_chat(&client, &chat_id, &user_id).await?;
            let result = serde_json::json!({"status": "hidden"});
            output::print_success(format, &result, start);
            Ok(())
        }

        ChatCommand::Unhide { chat_id, user_id } => {
            let start = Instant::now();
            api::chats::unhide_chat(&client, &chat_id, &user_id).await?;
            let result = serde_json::json!({"status": "unhidden"});
            output::print_success(format, &result, start);
            Ok(())
        }

        ChatCommand::Members { command } => {
            run_members(command, &client, format, pagination).await
        }
    }
}

async fn run_members(
    cmd: ChatMemberCommand,
    client: &GraphClient,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    match cmd {
        ChatMemberCommand::List { chat_id } => {
            let start = Instant::now();
            let members = api::chats::list_members(client, &chat_id, pagination).await?;
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

        ChatMemberCommand::Add {
            chat_id,
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
            let member = api::chats::add_member(client, &chat_id, &req).await?;
            output::print_success(format, &member, start);
            Ok(())
        }

        ChatMemberCommand::Remove {
            chat_id,
            member_id,
        } => {
            let start = Instant::now();
            api::chats::remove_member(client, &chat_id, &member_id).await?;
            let result = serde_json::json!({"status": "removed"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
