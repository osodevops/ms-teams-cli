use clap::Subcommand;
use std::io::Read;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::{Result, TeamsError};
use crate::models::message::{
    ChatMessageAttachment, ChatMessageMention, ChatMessageMentioned, ChatMessageUser, ItemBody,
    SendMessageRequest,
};
use crate::models::user::User;
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
        /// Image file to send inline, like a pasted screenshot (repeatable)
        #[arg(long)]
        image: Vec<String>,
        /// File to upload and attach (repeatable; needs a Files.ReadWrite scope)
        #[arg(long)]
        attach: Vec<String>,
        /// User to @mention (repeatable): an Entra object ID or UPN
        #[arg(long, value_name = "USER")]
        mention: Vec<String>,
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
        #[arg(required_unless_present = "message", conflicts_with = "message")]
        message_id: Option<String>,
        /// Message ID
        #[arg(
            long = "message",
            alias = "message-id",
            required_unless_present = "message_id",
            conflicts_with = "message_id"
        )]
        message: Option<String>,
        /// Include an inventory of attachments and inline images
        #[arg(long)]
        with_attachments: bool,
    },
    /// List or download message attachments and inline images
    Attachments {
        #[command(subcommand)]
        command: super::message_attachments::AttachmentsCommand,
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
        #[arg(long, visible_alias = "message")]
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
        /// Image file to send inline, like a pasted screenshot (repeatable)
        #[arg(long)]
        image: Vec<String>,
        /// File to upload and attach (repeatable; needs a Files.ReadWrite scope)
        #[arg(long)]
        attach: Vec<String>,
    },
    /// Add a reaction to a channel or chat message
    #[command(
        override_usage = "teams message react (--team <TEAM> --channel <CHANNEL> | --chat <CHAT>) --message-id <MESSAGE_ID> <REACTION>"
    )]
    React {
        /// Team ID (for channel messages)
        #[arg(long, required_unless_present = "chat", requires = "channel")]
        team: Option<String>,
        /// Channel ID (for channel messages)
        #[arg(long, required_unless_present = "chat", requires = "team")]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long, conflicts_with_all = ["team", "channel"])]
        chat: Option<String>,
        /// Message ID
        #[arg(long, visible_alias = "message")]
        message_id: String,
        /// Reaction name (like, heart, laugh, surprised, sad, angry, eyes) or emoji character
        #[arg(
            required_unless_present = "reaction_flag",
            conflicts_with = "reaction_flag"
        )]
        reaction: Option<String>,
        /// Reaction name (like, heart, laugh, surprised, sad, angry, eyes) or emoji character
        #[arg(
            long = "reaction",
            value_name = "REACTION",
            required_unless_present = "reaction",
            conflicts_with = "reaction"
        )]
        reaction_flag: Option<String>,
    },
    /// Remove a reaction from a channel or chat message
    #[command(
        override_usage = "teams message unreact (--team <TEAM> --channel <CHANNEL> | --chat <CHAT>) --message-id <MESSAGE_ID> <REACTION>"
    )]
    Unreact {
        /// Team ID (for channel messages)
        #[arg(long, required_unless_present = "chat", requires = "channel")]
        team: Option<String>,
        /// Channel ID (for channel messages)
        #[arg(long, required_unless_present = "chat", requires = "team")]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long, conflicts_with_all = ["team", "channel"])]
        chat: Option<String>,
        /// Message ID
        #[arg(long, visible_alias = "message")]
        message_id: String,
        /// Reaction name (like, heart, laugh, surprised, sad, angry, eyes) or emoji character
        #[arg(
            required_unless_present = "reaction_flag",
            conflicts_with = "reaction_flag"
        )]
        reaction: Option<String>,
        /// Reaction name (like, heart, laugh, surprised, sad, angry, eyes) or emoji character
        #[arg(
            long = "reaction",
            value_name = "REACTION",
            required_unless_present = "reaction",
            conflicts_with = "reaction"
        )]
        reaction_flag: Option<String>,
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
        #[arg(required_unless_present = "message", conflicts_with = "message")]
        message_id: Option<String>,
        /// Message ID to pin
        #[arg(
            long = "message",
            alias = "message-id",
            required_unless_present = "message_id",
            conflicts_with = "message_id"
        )]
        message: Option<String>,
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
        #[arg(
            required_unless_present = "pinned_message",
            conflicts_with = "pinned_message"
        )]
        pinned_message_id: Option<String>,
        /// Pinned message ID to remove
        #[arg(
            long = "pinned-message-id",
            value_name = "PINNED_MESSAGE_ID",
            required_unless_present = "pinned_message_id",
            conflicts_with = "pinned_message_id"
        )]
        pinned_message: Option<String>,
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
        #[arg(required_unless_present = "message", conflicts_with = "message")]
        message_id: Option<String>,
        /// Message ID
        #[arg(
            long = "message",
            alias = "message-id",
            required_unless_present = "message_id",
            conflicts_with = "message_id"
        )]
        message: Option<String>,
    },
    /// Update a message
    Update {
        /// Team ID (for channel messages; requires --channel)
        #[arg(long, requires = "channel", conflicts_with = "chat")]
        team: Option<String>,
        /// Channel ID (for channel messages; requires --team)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long, required_unless_present = "team")]
        chat: Option<String>,
        /// Message ID
        #[arg(required_unless_present = "message", conflicts_with = "message")]
        message_id: Option<String>,
        /// Message ID
        #[arg(
            long = "message",
            alias = "message-id",
            required_unless_present = "message_id",
            conflicts_with = "message_id"
        )]
        message: Option<String>,
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
    let token = auth::resolve_token(profile).await?;
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
            image,
            attach,
            mention,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Sending Teams messages")?;

            // An adaptive card counts as media: the body only has to carry the
            // attachment marker, so requiring --body alongside it is noise.
            let has_media = !image.is_empty() || !attach.is_empty() || adaptive_card.is_some();
            // A mention alone is a valid non-empty body, so it lifts the
            // media-only exception the same way --image/--attach do.
            let content = resolve_body_or_media(body, stdin, has_media || !mention.is_empty())?;
            let identities = resolve_mentions(&client, &mention).await?;
            ensure_no_raw_at_markup(&content_type, &content)?;
            let mut req = build_send_request(content, &content_type, adaptive_card.as_deref())?;
            apply_mentions(&mut req, &identities)?;

            let msg = if let Some(chat_id) = chat {
                super::message_media::apply_media(
                    &client,
                    &mut req,
                    &image,
                    &attach,
                    super::message_media::AttachDestination::Chat,
                )
                .await?;
                api::messages::send_chat_message(&client, &chat_id, &req).await?
            } else {
                let team_id = team.ok_or_else(|| {
                    TeamsError::InvalidInput(
                        "--team and --channel are required for channel messages, or use --chat"
                            .into(),
                    )
                })?;
                let channel_id = channel.ok_or_else(|| {
                    TeamsError::InvalidInput("--channel is required for channel messages".into())
                })?;
                super::message_media::apply_media(
                    &client,
                    &mut req,
                    &image,
                    &attach,
                    super::message_media::AttachDestination::Channel {
                        team_id: &team_id,
                        channel_id: &channel_id,
                    },
                )
                .await?;
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
                let channel_id = channel
                    .ok_or_else(|| TeamsError::InvalidInput("--channel is required".into()))?;
                api::messages::list_channel_messages(&client, &team_id, &channel_id, pagination)
                    .await?
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
            message,
            with_attachments,
        } => {
            let start = Instant::now();
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let msg =
                api::messages::get_channel_message(&client, &team, &channel, &message_id).await?;
            if with_attachments {
                let message_ref = api::messages::MessageRef::Channel {
                    team_id: team,
                    channel_id: channel,
                    message_id,
                };
                let hosted = api::messages::list_hosted_contents(&client, &message_ref).await?;
                let items = crate::models::attachment_inventory::build_inventory(&msg, &hosted);
                let mut value = serde_json::to_value(&msg).map_err(|e| TeamsError::ApiError {
                    status: 0,
                    message: format!("Failed to serialize message: {e}"),
                })?;
                value["attachment_items"] = serde_json::to_value(items).unwrap_or_default();
                output::print_success(format, &value, start);
            } else {
                output::print_success(format, &msg, start);
            }
            Ok(())
        }

        MessageCommand::Attachments { command } => {
            super::message_attachments::run(command, &client, format).await
        }

        MessageCommand::Reply {
            team,
            channel,
            message_id,
            body,
            stdin,
            content_type,
            image,
            attach,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Replying to Teams messages")?;
            let has_media = !image.is_empty() || !attach.is_empty();
            let content = resolve_body_or_media(body, stdin, has_media)?;
            let mut req = build_send_request(content, &content_type, None)?;
            super::message_media::apply_media(
                &client,
                &mut req,
                &image,
                &attach,
                super::message_media::AttachDestination::Channel {
                    team_id: &team,
                    channel_id: &channel,
                },
            )
            .await?;
            let msg = api::messages::reply_to_message(&client, &team, &channel, &message_id, &req)
                .await?;
            output::print_success(format, &msg, start);
            Ok(())
        }

        MessageCommand::React {
            team,
            channel,
            chat,
            message_id,
            reaction,
            reaction_flag,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Reacting to Teams messages")?;
            let reaction = resolve_id(reaction, reaction_flag, "--reaction or <REACTION>")?;
            let reaction_type = reaction_type_for(&reaction);
            if let Some(chat_id) = chat {
                api::messages::set_chat_reaction(&client, &chat_id, &message_id, &reaction_type)
                    .await?;
            } else {
                let (team_id, channel_id) = require_channel(team, channel)?;
                api::messages::set_reaction(
                    &client,
                    &team_id,
                    &channel_id,
                    &message_id,
                    &reaction_type,
                )
                .await?;
            }
            let result = serde_json::json!({"status": "reaction_set", "reaction": reaction});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Unreact {
            team,
            channel,
            chat,
            message_id,
            reaction,
            reaction_flag,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Removing Teams message reactions")?;
            let reaction = resolve_id(reaction, reaction_flag, "--reaction or <REACTION>")?;
            let reaction_type = reaction_type_for(&reaction);
            if let Some(chat_id) = chat {
                api::messages::unset_chat_reaction(&client, &chat_id, &message_id, &reaction_type)
                    .await?;
            } else {
                let (team_id, channel_id) = require_channel(team, channel)?;
                api::messages::unset_reaction(
                    &client,
                    &team_id,
                    &channel_id,
                    &message_id,
                    &reaction_type,
                )
                .await?;
            }
            let result = serde_json::json!({"status": "reaction_removed", "reaction": reaction});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Pin {
            team,
            channel,
            message_id,
            message,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Pinning Teams messages")?;
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let pinned = api::messages::pin_message(&client, &team, &channel, &message_id).await?;
            output::print_success(format, &pinned, start);
            Ok(())
        }

        MessageCommand::Unpin {
            team,
            channel,
            pinned_message_id,
            pinned_message,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Unpinning Teams messages")?;
            let pinned_message_id = resolve_id(
                pinned_message_id,
                pinned_message,
                "--pinned-message-id or <PINNED_MESSAGE_ID>",
            )?;
            api::messages::unpin_message(&client, &team, &channel, &pinned_message_id).await?;
            let result = serde_json::json!({"status": "unpinned"});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Delete {
            team,
            channel,
            message_id,
            message,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Deleting Teams messages")?;
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            api::messages::delete_message(&client, &team, &channel, &message_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Update {
            team,
            channel,
            chat,
            message_id,
            message,
            body,
            content_type,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Updating Teams messages")?;
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let req = build_send_request(body, &content_type, None)?;
            let target = if let Some(chat_id) = chat {
                api::messages::MessageRef::Chat {
                    chat_id,
                    message_id: message_id.clone(),
                }
            } else {
                let (team_id, channel_id) = require_channel(team, channel)?;
                api::messages::MessageRef::Channel {
                    team_id,
                    channel_id,
                    message_id: message_id.clone(),
                }
            };
            api::messages::update_message(&client, &target, &req).await?;
            // Graph answers a delegated edit with no content, so the message is
            // fetched back to show the new text. The edit has already been
            // applied by this point, so a failed read-back is reported alongside
            // a successful update rather than as a failed command.
            let msg = match api::messages::get_message(&client, &target).await {
                Ok(updated) => {
                    serde_json::to_value(updated).map_err(|e| TeamsError::Other(e.into()))?
                }
                Err(err) => {
                    tracing::warn!("Message updated, but reading it back failed: {err}");
                    serde_json::json!({
                        "id": message_id,
                        "updated": true,
                        "readBackError": err.to_string(),
                    })
                }
            };
            output::print_success(format, &msg, start);
            Ok(())
        }
    }
}

pub(crate) fn resolve_id(
    positional: Option<String>,
    named: Option<String>,
    expected: &str,
) -> Result<String> {
    match (positional, named) {
        (Some(_), Some(_)) => Err(TeamsError::InvalidInput(format!(
            "Provide only one of {expected}"
        ))),
        (Some(id), None) | (None, Some(id)) => Ok(id),
        (None, None) => Err(TeamsError::InvalidInput(format!(
            "Missing required message identifier: {expected}"
        ))),
    }
}

/// Reaction names mapped to the unicode character Microsoft Graph expects.
///
/// Graph's setReaction/unsetReaction only accept a unicode character in the
/// request body; the legacy names (`like`, `heart`, ...) appear on reads for
/// backward compatibility but are rejected on writes with HTTP 400
/// ("Unicode 'like' in the payload is not supported"). The characters for the
/// six classic Teams reactions were confirmed against a live tenant via the
/// `displayName` Graph returns for them (`Like`, `Heart`, `Laugh`,
/// `Surprised`, `Sad`, `Angry`); note that `❤` without the variation
/// selector and `😢`/`😡` map to different reactions. Anything unlisted — an
/// unknown name, or an emoji character supplied directly — passes through
/// untouched.
const REACTION_UNICODE: &[(&str, &str)] = &[
    // Classic Teams reactions
    ("like", "👍"),
    ("heart", "❤️"),
    ("laugh", "😆"),
    ("surprised", "😮"),
    ("sad", "🙁"),
    ("angry", "😠"),
    // Common aliases
    ("thumbsup", "👍"),
    ("thumbsdown", "👎"),
    ("eyes", "👀"),
    ("tada", "🎉"),
    ("rocket", "🚀"),
    ("fire", "🔥"),
];

fn reaction_type_for(reaction: &str) -> String {
    REACTION_UNICODE
        .iter()
        .find(|(name, _)| *name == reaction)
        .map_or_else(|| reaction.to_string(), |(_, emoji)| (*emoji).to_string())
}

/// Unwraps the team/channel pair for the channel branch. Clap rejects an
/// incomplete pair during parsing, so this is the residual unwrap rather than
/// the primary check; the wording matches `message list` for the case where a
/// future caller reaches it.
fn require_channel(team: Option<String>, channel: Option<String>) -> Result<(String, String)> {
    let team_id = team.ok_or_else(|| {
        TeamsError::InvalidInput("--team and --channel required, or use --chat".into())
    })?;
    let channel_id =
        channel.ok_or_else(|| TeamsError::InvalidInput("--channel is required".into()))?;
    Ok((team_id, channel_id))
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

/// A body is required unless the message carries media (`--image`/`--attach`),
/// in which case it may be empty.
fn resolve_body_or_media(body: Option<String>, stdin: bool, has_media: bool) -> Result<String> {
    if body.is_none() && !stdin && has_media {
        return Ok(String::new());
    }
    resolve_body(body, stdin)
}

fn build_send_request(
    content: String,
    content_type: &str,
    adaptive_card_path: Option<&str>,
) -> Result<SendMessageRequest> {
    let mut req = SendMessageRequest {
        body: ItemBody {
            content_type: Some(content_type.to_string()),
            content: Some(content),
        },
        attachments: None,
        hosted_contents: None,
        mentions: None,
    };

    if let Some(path) = adaptive_card_path {
        let card_json = std::fs::read_to_string(path).map_err(|e| {
            TeamsError::InvalidInput(format!("Failed to read adaptive card file: {e}"))
        })?;
        // Validate JSON
        serde_json::from_str::<serde_json::Value>(&card_json)
            .map_err(|e| TeamsError::InvalidInput(format!("Invalid adaptive card JSON: {e}")))?;

        // Graph requires the body to reference each attachment by id, or it
        // rejects the whole message with "Body does not contain marker for
        // attachment with Id ...". The id is generated here, so the caller
        // cannot add the marker themselves.
        //
        // The marker is markup, so the body has to be HTML regardless of what
        // was asked for. Promote it the way the --attach path does — escaped
        // and wrapped — rather than concatenating onto text that may itself
        // contain `<` or `&`.
        super::message_media::ensure_html_body(&mut req);
        let id = uuid::Uuid::new_v4().to_string();
        let mut body = req.body.content.take().unwrap_or_default();
        body.push_str(&super::message_media::attachment_tag(&id));
        req.body.content = Some(body);
        req.attachments = Some(vec![ChatMessageAttachment {
            id: Some(id),
            content_type: Some("application/vnd.microsoft.card.adaptive".to_string()),
            content: Some(card_json),
            content_url: None,
            name: None,
            thumbnail_url: None,
            teams_app_id: None,
        }]);
    }

    Ok(req)
}

/// A user resolved from Graph and validated for use in a `--mention`.
#[derive(Debug, Clone, PartialEq)]
struct MentionIdentity {
    id: String,
    display_name: String,
}

/// Resolve each `--mention` value (object ID or UPN) through Graph, require a
/// canonical id and display name for each, and de-duplicate by object ID
/// keeping the first occurrence.
async fn resolve_mentions(
    client: &GraphClient,
    mentions: &[String],
) -> Result<Vec<MentionIdentity>> {
    let mut resolved = Vec::with_capacity(mentions.len());
    for value in mentions {
        let user = api::users::get_user(client, value).await?;
        resolved.push(validate_mention_user(&user, value)?);
    }
    Ok(dedup_mentions(resolved))
}

/// A mention needs both the canonical object ID (for the JSON identity) and
/// the display name (for the HTML element); without either Graph would strip
/// or misrender it.
fn validate_mention_user(user: &User, requested: &str) -> Result<MentionIdentity> {
    match (&user.id, &user.display_name) {
        (Some(id), Some(name)) if !id.is_empty() && !name.is_empty() => Ok(MentionIdentity {
            id: id.clone(),
            display_name: name.clone(),
        }),
        _ => Err(TeamsError::InvalidInput(format!(
            "--mention '{requested}' resolved to a user with no canonical object ID or \
             display name; a Teams mention needs both"
        ))),
    }
}

/// Two different inputs (say, a UPN and an object ID) can resolve to the same
/// person; one mention per canonical object ID, first flag wins.
fn dedup_mentions(identities: Vec<MentionIdentity>) -> Vec<MentionIdentity> {
    let mut seen = std::collections::HashSet::new();
    identities
        .into_iter()
        .filter(|identity| seen.insert(identity.id.clone()))
        .collect()
}

/// The `<at id="N">Display Name</at>` prefix, in flag order, with IDs
/// assigned contiguously from zero. Display names are HTML-escaped here but
/// stored unescaped in the JSON `mentions` array.
fn mention_html_prefix(identities: &[MentionIdentity]) -> String {
    identities
        .iter()
        .enumerate()
        .map(|(i, identity)| {
            format!(
                r#"<at id="{}">{}</at>"#,
                i,
                escape_html_text(&identity.display_name)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Synchronize the request's body and `mentions` array so each `<at id="N">`
/// element has a matching entry — both halves are required for Graph to keep
/// the mention. A plain-text body is safely promoted to HTML (escaped, line
/// breaks preserved as `<br>`); an explicit HTML body is kept as supplied,
/// with only the mention prefix prepended.
fn apply_mentions(req: &mut SendMessageRequest, identities: &[MentionIdentity]) -> Result<()> {
    if identities.is_empty() {
        return Ok(());
    }
    let content_type = req.body.content_type.as_deref().unwrap_or_default();
    if !matches!(content_type, "text" | "html") {
        return Err(TeamsError::InvalidInput(format!(
            "--mention requires a text or html body, not '{content_type}'"
        )));
    }

    let prefix = mention_html_prefix(identities);
    let existing = req.body.content.take().unwrap_or_default();
    let promoted = content_type == "text";
    if promoted {
        req.body.content_type = Some("html".to_string());
    }
    req.body.content = Some(if existing.is_empty() {
        prefix
    } else {
        format!(
            "{prefix} {}",
            if promoted {
                escape_body_text(&existing)
            } else {
                existing
            }
        )
    });

    req.mentions = Some(
        identities
            .iter()
            .enumerate()
            .map(|(i, identity)| ChatMessageMention {
                id: i as i32,
                mention_text: identity.display_name.clone(),
                mentioned: ChatMessageMentioned {
                    user: Some(ChatMessageUser {
                        id: Some(identity.id.clone()),
                        display_name: Some(identity.display_name.clone()),
                        user_identity_type: Some("aadUser".to_string()),
                    }),
                },
            })
            .collect(),
    );
    Ok(())
}

/// Raw `<at>` markup in an explicit HTML body is not a real mention — Graph
/// renders or strips it as ordinary text. Fail before posting anything rather
/// than send a message that only looks like it tagged someone.
fn ensure_no_raw_at_markup(content_type: &str, content: &str) -> Result<()> {
    if content_type.eq_ignore_ascii_case("html") && contains_at_element(content) {
        return Err(TeamsError::InvalidInput(
            "The HTML body contains raw <at> markup, which is not a real Teams mention. \
             Remove it and tag people with --mention USER instead."
                .into(),
        ));
    }
    Ok(())
}

/// True when the markup contains an actual `<at ...>` / `<at>` opening tag.
/// `<attachment>` shares the three-letter prefix but its next character is a
/// letter, which rules it out.
fn contains_at_element(html: &str) -> bool {
    let lower = html.to_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<at") {
        let idx = from + pos;
        match lower[idx + 3..].chars().next() {
            Some(c) if c.is_ascii_alphanumeric() => from = idx + 3,
            _ => return true,
        }
    }
    false
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_body_text(text: &str) -> String {
    escape_html_text(text).replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_card(dir: &std::path::Path) -> String {
        let path = dir.join("card.json");
        std::fs::write(
            &path,
            r#"{"type":"AdaptiveCard","version":"1.5","body":[]}"#,
        )
        .unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn adaptive_card_body_carries_the_attachment_marker() {
        let dir = std::env::temp_dir().join("teams-cli-card-marker");
        std::fs::create_dir_all(&dir).unwrap();
        let card = write_card(&dir);

        let req = build_send_request("hello".to_string(), "text", Some(&card)).unwrap();

        let id = req.attachments.as_ref().unwrap()[0]
            .id
            .clone()
            .expect("attachment id");
        let body = req.body.content.unwrap();

        // Graph rejects the message unless the body references the attachment.
        assert!(
            body.contains(&format!(r#"<attachment id="{id}"></attachment>"#)),
            "body did not reference attachment {id}: {body}"
        );
        // the caller's text is kept, wrapped for the HTML body the marker requires
        assert!(
            body.starts_with("<p>hello</p>"),
            "caller's body was dropped: {body}"
        );

        // The marker is markup, so the body must be html even when text was asked for.
        assert_eq!(req.body.content_type.as_deref(), Some("html"));
    }

    /// Promoting a text body to HTML by concatenation would send `a < b & c` as
    /// markup, and Teams would swallow it. The --attach path already escapes on
    /// promotion; the card path has to do the same.
    #[test]
    fn a_text_body_is_escaped_when_the_card_promotes_it_to_html() {
        let dir = std::env::temp_dir().join("teams-cli-card-escape");
        std::fs::create_dir_all(&dir).unwrap();
        let card = write_card(&dir);

        let req = build_send_request("a < b & \"c\"".to_string(), "text", Some(&card)).unwrap();
        let body = req.body.content.unwrap();

        assert!(
            body.contains("a &lt; b &amp; &quot;c&quot;"),
            "caller's text reached the body unescaped: {body}"
        );
        assert_eq!(req.body.content_type.as_deref(), Some("html"));
        // the marker itself must stay real markup
        let id = req.attachments.as_ref().unwrap()[0].id.clone().unwrap();
        assert!(
            body.ends_with(&format!(r#"<attachment id="{id}"></attachment>"#)),
            "{body}"
        );
    }

    /// An explicit --content-type html is the caller's own markup and is left alone.
    #[test]
    fn an_html_body_is_not_re_escaped_by_the_card_path() {
        let dir = std::env::temp_dir().join("teams-cli-card-html");
        std::fs::create_dir_all(&dir).unwrap();
        let card = write_card(&dir);

        let req = build_send_request("<b>bold</b>".to_string(), "html", Some(&card)).unwrap();
        let body = req.body.content.unwrap();

        assert!(body.starts_with("<b>bold</b>"), "{body}");
        assert_eq!(req.body.content_type.as_deref(), Some("html"));
    }

    /// The card alone is a complete message: the body only has to carry the marker.
    #[test]
    fn a_card_without_a_body_sends_just_the_marker() {
        let dir = std::env::temp_dir().join("teams-cli-card-only");
        std::fs::create_dir_all(&dir).unwrap();
        let card = write_card(&dir);

        let req = build_send_request(String::new(), "text", Some(&card)).unwrap();
        let id = req.attachments.as_ref().unwrap()[0].id.clone().unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(format!(r#"<attachment id="{id}"></attachment>"#).as_str())
        );
    }

    #[test]
    fn without_a_card_the_body_and_content_type_are_untouched() {
        let req = build_send_request("plain".to_string(), "text", None).unwrap();
        assert_eq!(req.body.content.as_deref(), Some("plain"));
        assert_eq!(req.body.content_type.as_deref(), Some("text"));
        assert!(req.attachments.is_none());
    }

    #[test]
    fn named_reaction_becomes_unicode() {
        assert_eq!(reaction_type_for("eyes"), "👀");
    }

    fn identity(id: &str, name: &str) -> MentionIdentity {
        MentionIdentity {
            id: id.into(),
            display_name: name.into(),
        }
    }

    fn html_request(body: &str) -> SendMessageRequest {
        SendMessageRequest {
            body: ItemBody {
                content_type: Some("html".into()),
                content: Some(body.into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        }
    }

    /// One `--mention` produces HTML element `<at id="0">` matched by JSON
    /// mention id 0 — both halves must agree or Graph drops the mention.
    #[test]
    fn one_mention_matches_html_and_json_ids() {
        let mut req = html_request("<p>Please review</p>");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie Daniels")]).unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Sophie Daniels</at> <p>Please review</p>"#)
        );
        let mentions = req.mentions.as_ref().unwrap();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].id, 0);
        assert_eq!(mentions[0].mention_text, "Sophie Daniels");
        let user = mentions[0].mentioned.user.as_ref().unwrap();
        assert_eq!(user.id.as_deref(), Some("oid-1"));
        assert_eq!(user.display_name.as_deref(), Some("Sophie Daniels"));
        assert_eq!(user.user_identity_type.as_deref(), Some("aadUser"));
    }

    #[test]
    fn multiple_mentions_keep_flag_order_with_contiguous_ids() {
        let mut req = html_request("Existing body");
        apply_mentions(
            &mut req,
            &[
                identity("oid-a", "Sophie Daniels"),
                identity("oid-b", "Abraham Ingersoll"),
            ],
        )
        .unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(
                r#"<at id="0">Sophie Daniels</at> <at id="1">Abraham Ingersoll</at> Existing body"#
            )
        );
        let mentions = req.mentions.as_ref().unwrap();
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].id, 0);
        assert_eq!(
            mentions[0].mentioned.user.as_ref().unwrap().id.as_deref(),
            Some("oid-a")
        );
        assert_eq!(mentions[1].id, 1);
        assert_eq!(
            mentions[1].mentioned.user.as_ref().unwrap().id.as_deref(),
            Some("oid-b")
        );
    }

    #[test]
    fn duplicate_object_ids_collapse_to_one_mention() {
        let deduped = dedup_mentions(vec![
            identity("same-oid", "First Name"),
            identity("other-oid", "Other Person"),
            identity("same-oid", "Second Name"),
        ]);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].display_name, "First Name");
        assert_eq!(deduped[1].display_name, "Other Person");
    }

    #[test]
    fn display_names_are_escaped_in_html_only() {
        let mut req = html_request("body");
        apply_mentions(&mut req, &[identity("oid-1", "R&D <Lead>")]).unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">R&amp;D &lt;Lead&gt;</at> body"#)
        );
        let user = req.mentions.as_ref().unwrap()[0]
            .mentioned
            .user
            .as_ref()
            .unwrap();
        assert_eq!(req.mentions.as_ref().unwrap()[0].mention_text, "R&D <Lead>");
        assert_eq!(user.display_name.as_deref(), Some("R&D <Lead>"));
    }

    /// The default text experience: the caller never learns Graph needs HTML;
    /// their text is escaped, line breaks become `<br>`, and the body is sent
    /// as html.
    #[test]
    fn text_body_is_promoted_to_html_with_line_breaks_intact() {
        let mut req = SendMessageRequest {
            body: ItemBody {
                content_type: Some("text".into()),
                content: Some("line one\nline <two> & three".into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        };
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();

        assert_eq!(req.body.content_type.as_deref(), Some("html"));
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Sophie</at> line one<br>line &lt;two&gt; &amp; three"#)
        );
        assert_eq!(req.mentions.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn explicit_html_body_is_preserved_apart_from_the_prefix() {
        let mut req = html_request("<p><b>bold</b> &amp; more</p>");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();

        assert_eq!(req.body.content_type.as_deref(), Some("html"));
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Sophie</at> <p><b>bold</b> &amp; more</p>"#)
        );
    }

    /// A mention by itself is a valid non-empty body.
    #[test]
    fn mention_alone_becomes_the_whole_body() {
        let mut req = SendMessageRequest {
            body: ItemBody {
                content_type: Some("text".into()),
                content: Some(String::new()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        };
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Sophie</at>"#)
        );
    }

    /// Sends without mentions keep current behaviour byte for byte: no
    /// transformation, no `mentions` property in the wire payload.
    #[test]
    fn no_mentions_leaves_the_request_untouched() {
        let mut text_req = SendMessageRequest {
            body: ItemBody {
                content_type: Some("text".into()),
                content: Some("plain & simple\nbody".into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        };
        apply_mentions(&mut text_req, &[]).unwrap();
        assert_eq!(text_req.body.content_type.as_deref(), Some("text"));
        assert_eq!(
            text_req.body.content.as_deref(),
            Some("plain & simple\nbody")
        );
        assert!(text_req.mentions.is_none());
        assert!(serde_json::to_value(&text_req)
            .unwrap()
            .get("mentions")
            .is_none());
    }

    #[test]
    fn raw_at_markup_without_mention_flag_is_rejected() {
        let err = ensure_no_raw_at_markup("html", r#"<p><at id="0">Sophie</at> please review</p>"#)
            .unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)), "{err:?}");
        assert!(err.to_string().contains("--mention"), "{err}");

        // Uppercase markup is caught too.
        assert!(ensure_no_raw_at_markup("HTML", "<AT>Sophie</AT>").is_err());
    }

    /// `<attachment>` shares the `<at` prefix but is legitimate media markup,
    /// not a pseudo-mention.
    #[test]
    fn at_detection_ignores_attachment_tags_and_text_bodies() {
        assert!(!contains_at_element(
            r#"<attachment id="ABC"></attachment>"#
        ));
        assert!(!contains_at_element("<p>chat about @at &lt;at&gt;</p>"));
        // A bare "<at" with no closing shape is still treated as a mention
        // attempt; failing closed beats posting a pseudo-mention.
        assert!(contains_at_element("plain text mentioning <at"));
        assert!(contains_at_element(r#"<at id="0">x</at>"#));
        assert!(contains_at_element("<at>"));
    }

    #[test]
    fn mention_rejects_non_text_content_types() {
        let mut req = html_request("body");
        req.body.content_type = Some("unknown".into());
        let err = apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)), "{err:?}");
    }

    /// A resolved user without an object ID or display name cannot be turned
    /// into a mention, so the command fails before posting anything.
    #[test]
    fn incomplete_resolved_user_is_invalid_input() {
        fn graph_user(id: &str, name: &str) -> User {
            serde_json::from_value(serde_json::json!({ "id": id, "displayName": name })).unwrap()
        }

        let requested = "sophie@example.com";
        for (id, name) in [
            (serde_json::Value::Null, serde_json::json!("Sophie")),
            (serde_json::json!(""), serde_json::json!("Sophie")),
            (serde_json::json!("oid"), serde_json::Value::Null),
            (serde_json::json!("oid"), serde_json::json!("")),
        ] {
            let user: User = serde_json::from_value(serde_json::json!({
                "id": id,
                "displayName": name
            }))
            .unwrap();
            let err = validate_mention_user(&user, requested).unwrap_err();
            assert!(matches!(err, TeamsError::InvalidInput(_)), "{err:?}");
            assert!(err.to_string().contains(requested), "{err}");
        }

        let ok = validate_mention_user(&graph_user("oid", "Sophie"), requested).unwrap();
        assert_eq!(ok.id, "oid");
        assert_eq!(ok.display_name, "Sophie");
    }

    /// Resolution goes through the existing `api::users::get_user` wrapper
    /// (`GET /users/{id-or-upn}`); the pure halves it feeds — validation and
    /// de-duplication — are covered by the tests above.
    #[test]
    fn resolved_identities_flow_through_validation_and_dedup() {
        let resolved = dedup_mentions(vec![
            validate_mention_user(
                &serde_json::from_value::<User>(serde_json::json!({
                    "id": "oid-a",
                    "displayName": "Sophie Daniels"
                }))
                .unwrap(),
                "sophie@contoso.com",
            )
            .unwrap(),
            validate_mention_user(
                &serde_json::from_value::<User>(serde_json::json!({
                    "id": "oid-a",
                    "displayName": "Sophie Daniels"
                }))
                .unwrap(),
                "32cbca05-dc05-454f-b0f3-072f331d4c97",
            )
            .unwrap(),
        ]);
        assert_eq!(resolved, vec![identity("oid-a", "Sophie Daniels")]);
    }

    #[test]
    fn emoji_reaction_passes_through() {
        assert_eq!(reaction_type_for("👀"), "👀");
        assert_eq!(reaction_type_for("💘"), "💘");
    }

    #[test]
    fn classic_names_become_unicode() {
        for (name, emoji) in [
            ("like", "👍"),
            ("heart", "❤️"),
            ("laugh", "😆"),
            ("surprised", "😮"),
            ("sad", "🙁"),
            ("angry", "😠"),
        ] {
            assert_eq!(reaction_type_for(name), emoji, "{name}");
        }
    }

    #[test]
    fn unknown_name_passes_through() {
        assert_eq!(reaction_type_for("fist bump"), "fist bump");
    }

    #[test]
    fn require_channel_reports_missing_team() {
        let err = require_channel(None, Some("channel".into())).unwrap_err();
        assert!(err.to_string().contains("--chat"), "{err}");
    }
}
