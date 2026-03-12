use clap::Subcommand;
use std::io::Read;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::{Result, TeamsError};
use crate::models::message::{ChatMessageAttachment, ItemBody, SendMessageRequest};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum MessageCommand {
    /// Send a message to a channel or chat
    Send {
        /// Team ID (for channel messages)
        #[arg(long)]
        team: Option<String>,
        /// Channel ID (for channel messages)
        #[arg(long)]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long)]
        chat: Option<String>,
        /// Message body text
        #[arg(long)]
        body: Option<String>,
        /// Read message body from stdin
        #[arg(long)]
        stdin: bool,
        /// Content type: text or html
        #[arg(long, default_value = "text")]
        content_type: String,
        /// Path to adaptive card JSON file
        #[arg(long)]
        adaptive_card: Option<String>,
    },
    /// List messages in a channel or chat
    List {
        /// Team ID (for channel messages)
        #[arg(long)]
        team: Option<String>,
        /// Channel ID (for channel messages)
        #[arg(long)]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long)]
        chat: Option<String>,
    },
    /// Get a specific message
    Get {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID
        message_id: String,
    },
    /// Reply to a channel message
    Reply {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID to reply to
        #[arg(long)]
        message_id: String,
        /// Reply body text
        #[arg(long)]
        body: Option<String>,
        /// Read message body from stdin
        #[arg(long)]
        stdin: bool,
        /// Content type: text or html
        #[arg(long, default_value = "text")]
        content_type: String,
    },
    /// Add a reaction to a message (beta)
    React {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID
        #[arg(long)]
        message_id: String,
        /// Reaction type (e.g., like, heart, laugh, surprised, sad, angry)
        reaction: String,
    },
    /// Remove a reaction from a message (beta)
    Unreact {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID
        #[arg(long)]
        message_id: String,
        /// Reaction type to remove
        reaction: String,
    },
    /// Pin a message in a channel
    Pin {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID to pin
        message_id: String,
    },
    /// Unpin a message from a channel
    Unpin {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Pinned message ID to remove
        pinned_message_id: String,
    },
    /// Delete a message
    Delete {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID
        message_id: String,
    },
    /// Update a message
    Update {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Message ID
        message_id: String,
        /// New message body
        #[arg(long)]
        body: String,
        /// Content type: text or html
        #[arg(long, default_value = "text")]
        content_type: String,
    },
}

pub async fn run(
    cmd: MessageCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        MessageCommand::Send {
            team,
            channel,
            chat,
            body,
            stdin,
            content_type,
            adaptive_card,
        } => {
            let start = Instant::now();

            let content = resolve_body(body, stdin)?;
            let req = build_send_request(content, &content_type, adaptive_card.as_deref())?;

            let msg = if let Some(chat_id) = chat {
                api::messages::send_chat_message(&client, &chat_id, &req).await?
            } else {
                let team_id = team.ok_or_else(|| {
                    TeamsError::InvalidInput("--team and --channel are required for channel messages, or use --chat".into())
                })?;
                let channel_id = channel.ok_or_else(|| {
                    TeamsError::InvalidInput("--channel is required for channel messages".into())
                })?;
                api::messages::send_channel_message(&client, &team_id, &channel_id, &req).await?
            };
            output::print_success(format, &msg, start);
            Ok(())
        }

        MessageCommand::List {
            team,
            channel,
            chat,
        } => {
            let start = Instant::now();

            let messages = if let Some(chat_id) = chat {
                api::messages::list_chat_messages(&client, &chat_id, pagination).await?
            } else {
                let team_id = team.ok_or_else(|| {
                    TeamsError::InvalidInput("--team and --channel required, or use --chat".into())
                })?;
                let channel_id = channel.ok_or_else(|| {
                    TeamsError::InvalidInput("--channel is required".into())
                })?;
                api::messages::list_channel_messages(&client, &team_id, &channel_id, pagination).await?
            };

            if format == OutputFormat::Human {
                let headers = vec!["ID", "From", "Body Preview", "Date"];
                let rows: Vec<Vec<String>> = messages
                    .iter()
                    .map(|m| {
                        let from = m
                            .from
                            .as_ref()
                            .and_then(|f| f.user.as_ref())
                            .and_then(|u| u.display_name.clone())
                            .unwrap_or_default();
                        let body_preview = m
                            .body
                            .as_ref()
                            .and_then(|b| b.content.as_ref())
                            .map(|c| {
                                let clean: String = c.chars().take(60).collect();
                                clean
                            })
                            .unwrap_or_default();
                        vec![
                            m.id.clone().unwrap_or_default(),
                            from,
                            body_preview,
                            m.created_date_time.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &messages, start);
            }
            Ok(())
        }

        MessageCommand::Get {
            team,
            channel,
            message_id,
        } => {
            let start = Instant::now();
            let msg =
                api::messages::get_channel_message(&client, &team, &channel, &message_id).await?;
            output::print_success(format, &msg, start);
            Ok(())
        }

        MessageCommand::Reply {
            team,
            channel,
            message_id,
            body,
            stdin,
            content_type,
        } => {
            let start = Instant::now();
            let content = resolve_body(body, stdin)?;
            let req = build_send_request(content, &content_type, None)?;
            let msg =
                api::messages::reply_to_message(&client, &team, &channel, &message_id, &req)
                    .await?;
            output::print_success(format, &msg, start);
            Ok(())
        }

        MessageCommand::React {
            team,
            channel,
            message_id,
            reaction,
        } => {
            let start = Instant::now();
            api::messages::set_reaction(&client, &team, &channel, &message_id, &reaction).await?;
            let result = serde_json::json!({"status": "reaction_set", "reaction": reaction});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Unreact {
            team,
            channel,
            message_id,
            reaction,
        } => {
            let start = Instant::now();
            api::messages::unset_reaction(&client, &team, &channel, &message_id, &reaction)
                .await?;
            let result = serde_json::json!({"status": "reaction_removed", "reaction": reaction});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Pin {
            team,
            channel,
            message_id,
        } => {
            let start = Instant::now();
            let pinned =
                api::messages::pin_message(&client, &team, &channel, &message_id).await?;
            output::print_success(format, &pinned, start);
            Ok(())
        }

        MessageCommand::Unpin {
            team,
            channel,
            pinned_message_id,
        } => {
            let start = Instant::now();
            api::messages::unpin_message(&client, &team, &channel, &pinned_message_id).await?;
            let result = serde_json::json!({"status": "unpinned"});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Delete {
            team,
            channel,
            message_id,
        } => {
            let start = Instant::now();
            api::messages::delete_message(&client, &team, &channel, &message_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Update {
            team,
            channel,
            message_id,
            body,
            content_type,
        } => {
            let start = Instant::now();
            let req = build_send_request(body, &content_type, None)?;
            let msg =
                api::messages::update_message(&client, &team, &channel, &message_id, &req).await?;
            output::print_success(format, &msg, start);
            Ok(())
        }
    }
}

fn resolve_body(body: Option<String>, stdin: bool) -> Result<String> {
    if stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| TeamsError::InvalidInput(format!("Failed to read stdin: {e}")))?;
        Ok(buf.trim_end().to_string())
    } else {
        body.ok_or_else(|| TeamsError::InvalidInput("--body or --stdin is required".into()))
    }
}

fn build_send_request(
    content: String,
    content_type: &str,
    adaptive_card_path: Option<&str>,
) -> Result<SendMessageRequest> {
    let attachments = if let Some(path) = adaptive_card_path {
        let card_json = std::fs::read_to_string(path).map_err(|e| {
            TeamsError::InvalidInput(format!("Failed to read adaptive card file: {e}"))
        })?;
        // Validate JSON
        serde_json::from_str::<serde_json::Value>(&card_json).map_err(|e| {
            TeamsError::InvalidInput(format!("Invalid adaptive card JSON: {e}"))
        })?;
        Some(vec![ChatMessageAttachment {
            id: Some(uuid::Uuid::new_v4().to_string()),
            content_type: Some("application/vnd.microsoft.card.adaptive".to_string()),
            content: Some(card_json),
            name: None,
        }])
    } else {
        None
    };

    Ok(SendMessageRequest {
        body: ItemBody {
            content_type: Some(content_type.to_string()),
            content: Some(content),
        },
        attachments,
    })
}
