use clap::Subcommand;
use std::io::Read;
use std::time::Instant;

use crate::api::messages::MessageRef;
use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::{Result, TeamsError};
use crate::models::message::{
    ChatMessage, ChatMessageAttachment, ChatMessageFrom, ChatMessageMention, ChatMessageMentioned,
    ChatMessageUser, ItemBody, SendMessageRequest,
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
        /// Subject line for a channel root message
        #[arg(long, requires = "channel", conflicts_with = "chat")]
        subject: Option<String>,
        /// Quote-reply to this chat message, as the Teams client's Reply does
        /// (repeatable; Graph accepts up to 5)
        #[arg(
            long,
            value_name = "MESSAGE_ID",
            requires = "chat",
            conflicts_with_all = ["team", "channel"]
        )]
        quote: Vec<String>,
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
        /// List the replies in this channel thread instead of the thread roots
        #[arg(
            long = "message-id",
            visible_alias = "message",
            requires = "channel",
            conflicts_with = "chat"
        )]
        message_id: Option<String>,
    },
    /// Get a specific message
    Get {
        /// Team ID
        #[arg(long)]
        team: String,
        /// Channel ID
        #[arg(long)]
        channel: String,
        /// Reply ID, to target a reply in a channel thread (MESSAGE_ID is then the parent post)
        #[arg(long)]
        reply: Option<String>,
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
    /// List, download or delete message attachments and inline images
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
        /// User to @mention (repeatable): an Entra object ID or UPN
        #[arg(long, value_name = "USER")]
        mention: Vec<String>,
    },
    /// Add a reaction to a channel or chat message
    #[command(
        override_usage = "teams message react (--team <TEAM> --channel <CHANNEL> [--reply <REPLY>] | --chat <CHAT>) --message-id <MESSAGE_ID> <REACTION>"
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
        /// Reply ID, to target a reply in a channel thread (MESSAGE_ID is then the parent post)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        reply: Option<String>,
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
        override_usage = "teams message unreact (--team <TEAM> --channel <CHANNEL> [--reply <REPLY>] | --chat <CHAT>) --message-id <MESSAGE_ID> <REACTION>"
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
        /// Reply ID, to target a reply in a channel thread (MESSAGE_ID is then the parent post)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        reply: Option<String>,
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
    /// Delete your own message (a soft delete; undo it with `message undelete`)
    #[command(
        override_usage = "teams message delete (--team <TEAM> --channel <CHANNEL> [--reply <REPLY>] | --chat <CHAT>) (MESSAGE_ID | --message <MESSAGE>) --yes"
    )]
    Delete {
        /// Team ID (for channel messages; requires --channel)
        #[arg(long, requires = "channel", conflicts_with = "chat")]
        team: Option<String>,
        /// Channel ID (for channel messages; requires --team)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long, required_unless_present = "team")]
        chat: Option<String>,
        /// Reply ID, to target a reply in a channel thread (MESSAGE_ID is then the parent post)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        reply: Option<String>,
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
        /// Confirm the deletion; without it the command exits with code 2 and sends nothing
        #[arg(long)]
        yes: bool,
    },
    /// Restore a message removed with `message delete`
    #[command(
        override_usage = "teams message undelete (--team <TEAM> --channel <CHANNEL> [--reply <REPLY>] | --chat <CHAT>) (MESSAGE_ID | --message <MESSAGE>)"
    )]
    Undelete {
        /// Team ID (for channel messages; requires --channel)
        #[arg(long, requires = "channel", conflicts_with = "chat")]
        team: Option<String>,
        /// Channel ID (for channel messages; requires --team)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long, required_unless_present = "team")]
        chat: Option<String>,
        /// Reply ID, to target a reply in a channel thread (MESSAGE_ID is then the parent post)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        reply: Option<String>,
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
    #[command(
        override_usage = "teams message update (--team <TEAM> --channel <CHANNEL> [--reply <REPLY>] | --chat <CHAT>) (MESSAGE_ID | --message <MESSAGE>) (--body <BODY> | --stdin) [--content-type <CONTENT_TYPE>] [--mention <USER>]..."
    )]
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
        /// Reply ID, to target a reply in a channel thread (MESSAGE_ID is then the parent post)
        #[arg(long, requires = "team", conflicts_with = "chat")]
        reply: Option<String>,
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
        #[arg(long, required_unless_present = "stdin", conflicts_with = "stdin")]
        body: Option<String>,
        /// Read the new message body from stdin
        #[arg(long)]
        stdin: bool,
        /// Content type: text or html
        #[arg(long, default_value = "text")]
        content_type: String,
        /// User to @mention (repeatable): an Entra object ID or UPN
        #[arg(long, value_name = "USER")]
        mention: Vec<String>,
    },
}

pub async fn run(
    cmd: MessageCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    // Deletion is refused before any token is resolved or request is made, so
    // a missing confirmation is an invalid-input error (exit code 2) in the
    // normal output envelope rather than an auth failure or a bare parse error.
    if let MessageCommand::Delete { yes: false, .. } = &cmd {
        return Err(TeamsError::InvalidInput(
            "Deleting a message needs explicit confirmation: add --yes".into(),
        ));
    }
    if let MessageCommand::Attachments {
        command: super::message_attachments::AttachmentsCommand::Delete { yes: false, .. },
    } = &cmd
    {
        return Err(TeamsError::InvalidInput(
            "Deleting attachments needs explicit confirmation: add --yes".into(),
        ));
    }

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
            subject,
            quote,
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
            let mut req = build_send_request_with_mentions(
                content,
                &content_type,
                adaptive_card.as_deref(),
                &identities,
            )?;
            req.subject = subject;

            let msg = if let Some(chat_id) = chat {
                super::message_media::apply_media(
                    &client,
                    &mut req,
                    &image,
                    &attach,
                    super::message_media::AttachDestination::Chat { chat_id: &chat_id },
                )
                .await?;
                if quote.is_empty() {
                    api::messages::send_chat_message(&client, &chat_id, &req).await?
                } else {
                    // replyWithQuote reads the body as HTML even when it is
                    // marked text, so plain text is escaped into HTML first or
                    // its `<` and `&` would be swallowed as markup.
                    super::message_media::ensure_html_body(&mut req);
                    api::messages::reply_with_quote(&client, &chat_id, &quote, &req).await?
                }
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
            message_id,
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
                match message_id {
                    Some(root_id) => {
                        api::messages::list_channel_message_replies(
                            &client,
                            &team_id,
                            &channel_id,
                            &root_id,
                            pagination,
                        )
                        .await?
                    }
                    None => {
                        api::messages::list_channel_messages(
                            &client,
                            &team_id,
                            &channel_id,
                            pagination,
                        )
                        .await?
                    }
                }
            };

            if format == OutputFormat::Human {
                let headers = vec!["ID", "From", "Subject", "Body Preview", "Date"];
                let rows: Vec<Vec<String>> = messages.items.iter().map(message_list_row).collect();
                output::table::print_table(headers, rows);
                output::print_more_hint(&messages);
            } else {
                output::print_success_page(format, &messages, start);
            }
            Ok(())
        }

        MessageCommand::Get {
            team,
            channel,
            reply,
            message_id,
            message,
            with_attachments,
        } => {
            let start = Instant::now();
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let message_ref =
                resolve_message_ref(Some(team), Some(channel), None, reply, message_id)?;
            let msg = api::messages::get_message(&client, &message_ref).await?;
            if with_attachments {
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
            mention,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Replying to Teams messages")?;
            let has_media = !image.is_empty() || !attach.is_empty();
            // A mention alone is a valid non-empty body, so it lifts the
            // requirement for --body or --stdin, exactly as it does on send.
            let content = resolve_body_or_media(body, stdin, has_media || !mention.is_empty())?;
            let identities = resolve_mentions(&client, &mention).await?;
            let mut req =
                build_send_request_with_mentions(content, &content_type, None, &identities)?;
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
            reply,
            message_id,
            reaction,
            reaction_flag,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Reacting to Teams messages")?;
            let reaction = resolve_id(reaction, reaction_flag, "--reaction or <REACTION>")?;
            let target = resolve_message_ref(team, channel, chat, reply, message_id)?;
            api::messages::set_reaction(&client, &target, &reaction_type_for(&reaction)).await?;
            let result = serde_json::json!({"status": "reaction_set", "reaction": reaction});
            output::print_success(format, &result, start);
            Ok(())
        }

        MessageCommand::Unreact {
            team,
            channel,
            chat,
            reply,
            message_id,
            reaction,
            reaction_flag,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Removing Teams message reactions")?;
            let reaction = resolve_id(reaction, reaction_flag, "--reaction or <REACTION>")?;
            let target = resolve_message_ref(team, channel, chat, reply, message_id)?;
            api::messages::unset_reaction(&client, &target, &reaction_type_for(&reaction)).await?;
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
            chat,
            reply,
            message_id,
            message,
            yes: _,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Deleting Teams messages")?;
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let target = resolve_message_ref(team, channel, chat, reply, message_id.clone())?;
            api::messages::soft_delete_message(&client, &target)
                .await
                .map_err(|err| {
                    with_channel_scope_hint(err, &target, client.token.scope.as_deref())
                })?;
            let msg = read_back(&client, &target, "deleted").await;
            output::print_success(format, &msg, start);
            Ok(())
        }

        MessageCommand::Undelete {
            team,
            channel,
            chat,
            reply,
            message_id,
            message,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Restoring Teams messages")?;
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let target = resolve_message_ref(team, channel, chat, reply, message_id.clone())?;
            api::messages::undo_soft_delete_message(&client, &target)
                .await
                .map_err(|err| {
                    with_channel_scope_hint(err, &target, client.token.scope.as_deref())
                })?;
            let msg = read_back(&client, &target, "restored").await;
            output::print_success(format, &msg, start);
            Ok(())
        }

        MessageCommand::Update {
            team,
            channel,
            chat,
            reply,
            message_id,
            message,
            body,
            stdin,
            content_type,
            mention,
        } => {
            let start = Instant::now();
            auth::require_delegated_token(&client.token, "Updating Teams messages")?;
            let message_id = resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let target = resolve_message_ref(team, channel, chat, reply, message_id)?;
            // Graph takes the body and the `mentions` array together on the
            // PATCH, so an edit that keeps or adds a mention re-sends both.
            let content = resolve_body(body, stdin)?;
            let identities = resolve_mentions(&client, &mention).await?;
            let req = build_send_request_with_mentions(content, &content_type, None, &identities)?;
            api::messages::update_message(&client, &target, &req)
                .await
                .map_err(|err| {
                    with_channel_scope_hint(err, &target, client.token.scope.as_deref())
                })?;
            let msg = read_back(&client, &target, "updated").await;
            output::print_success(format, &msg, start);
            Ok(())
        }
    }
}

/// Graph answers a delegated edit, delete or undelete with no content, so the
/// message is fetched back to show its new state (the edited text, or
/// `deletedDateTime` set or cleared). The mutation has already been applied by
/// this point, so a failed read-back is reported alongside the successful
/// change rather than as a failed command, with `outcome` naming the change.
async fn read_back(client: &GraphClient, target: &MessageRef, outcome: &str) -> serde_json::Value {
    read_back_at(client, &target.message_url(), target, outcome).await
}

async fn read_back_at(
    client: &GraphClient,
    url: &str,
    target: &MessageRef,
    outcome: &str,
) -> serde_json::Value {
    let fetched = api::messages::get_message_at(client, url)
        .await
        .and_then(|msg| serde_json::to_value(msg).map_err(|e| TeamsError::Other(e.into())));
    match fetched {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!("Message {outcome}, but reading it back failed: {err}");
            let message_id = match target {
                MessageRef::Channel { message_id, .. } | MessageRef::Chat { message_id, .. } => {
                    message_id
                }
                MessageRef::ChannelReply { reply_id, .. } => reply_id,
            };
            serde_json::json!({
                "id": message_id,
                outcome: true,
                "readBackError": err.to_string(),
            })
        }
    }
}

/// Channel edits and deletions need a delegated scope the default login does not
/// request, and Graph's 403 does not say so. Name the scope so the likely fix
/// is obvious. The hint is skipped for chat targets, and when the token
/// already carries the scope, since the 403 then has another cause (the
/// message is someone else's, or tenant policy forbids it).
fn with_channel_scope_hint(
    err: TeamsError,
    target: &MessageRef,
    scope: Option<&str>,
) -> TeamsError {
    let is_channel = !matches!(target, MessageRef::Chat { .. });
    let has_scope = scope.is_some_and(|scopes| {
        scopes
            .split_whitespace()
            .any(|s| s.eq_ignore_ascii_case("ChannelMessage.ReadWrite"))
    });
    match err {
        TeamsError::PermissionDenied(msg) if is_channel && !has_scope => {
            TeamsError::PermissionDenied(format!(
                "{msg} (channel messages likely need the ChannelMessage.ReadWrite delegated \
                 scope, which requires admin consent: have an administrator grant it with \
                 `teams auth consent-url --scopes ...`, then sign in again with \
                 `teams auth login --scopes ...` including it)"
            ))
        }
        other => other,
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

/// Turn the `--team`/`--channel`/`--chat`/`--reply` flags shared by the
/// message commands into a [`MessageRef`], rejecting mixed or incomplete
/// combinations clap's per-flag rules cannot express.
pub(crate) fn resolve_message_ref(
    team: Option<String>,
    channel: Option<String>,
    chat: Option<String>,
    reply: Option<String>,
    message_id: String,
) -> Result<MessageRef> {
    match (chat, team, channel) {
        (Some(chat_id), None, None) => {
            if reply.is_some() {
                return Err(TeamsError::InvalidInput(
                    "--reply only applies to channel messages".into(),
                ));
            }
            Ok(MessageRef::Chat {
                chat_id,
                message_id,
            })
        }
        (None, Some(team_id), Some(channel_id)) => Ok(match reply {
            Some(reply_id) => MessageRef::ChannelReply {
                team_id,
                channel_id,
                message_id,
                reply_id,
            },
            None => MessageRef::Channel {
                team_id,
                channel_id,
                message_id,
            },
        }),
        _ => Err(TeamsError::InvalidInput(
            "Provide either --chat, or both --team and --channel".into(),
        )),
    }
}

fn message_list_row(message: &ChatMessage) -> Vec<String> {
    vec![
        message.id.clone().unwrap_or_default(),
        message
            .from
            .as_ref()
            .and_then(ChatMessageFrom::display_name)
            .unwrap_or_default()
            .to_string(),
        message.subject.clone().unwrap_or_default(),
        message
            .body
            .as_ref()
            .and_then(|body| body.content.as_ref())
            .map(|content| content.chars().take(60).collect())
            .unwrap_or_default(),
        message.created_date_time.clone().unwrap_or_default(),
    ]
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
        subject: None,
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

/// Resolve each `--mention` value (object ID or UPN) through Graph and require
/// a canonical id and display name for each. The result stays in flag order,
/// duplicates included, so `{@N}` in the body can name the N-th flag;
/// [`apply_mentions`] treats flags that resolve to one person as one mention.
async fn resolve_mentions(
    client: &GraphClient,
    mentions: &[String],
) -> Result<Vec<MentionIdentity>> {
    let mut resolved = Vec::with_capacity(mentions.len());
    for value in mentions {
        let user = api::users::get_user(client, value).await?;
        resolved.push(validate_mention_user(&user, value)?);
    }
    Ok(resolved)
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
fn dedup_mentions(identities: &[MentionIdentity]) -> Vec<&MentionIdentity> {
    let mut seen = std::collections::HashSet::new();
    identities
        .iter()
        .filter(|identity| seen.insert(identity.id.as_str()))
        .collect()
}

/// A body split on mention placement tokens. `{@N}` places the N-th
/// `--mention` flag, `{@}` the only one; `{{@N}}` and `{{@}}` are the literal
/// text `{@N}` and `{@}`, and are already unescaped in `Text`.
#[derive(Debug, PartialEq)]
enum BodyPart<'a> {
    Text(String),
    Mention(&'a str),
}

fn split_mention_tokens(body: &str) -> Vec<BodyPart<'_>> {
    /// Length of the `{@digits}` token at the start of `s`, if there is one.
    fn token_len(s: &str) -> Option<usize> {
        let digits = s.strip_prefix("{@")?;
        let n = digits.bytes().take_while(u8::is_ascii_digit).count();
        digits[n..].starts_with('}').then_some(n + 3)
    }

    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = body;
    while let Some(pos) = rest.find('{') {
        text.push_str(&rest[..pos]);
        rest = &rest[pos..];
        if let Some(inner) = rest.strip_prefix('{') {
            if let Some(len) = token_len(inner).filter(|&len| inner[len..].starts_with('}')) {
                text.push_str(&inner[..len]);
                rest = &inner[len + 1..];
                continue;
            }
        }
        if let Some(len) = token_len(rest) {
            parts.push(BodyPart::Text(std::mem::take(&mut text)));
            parts.push(BodyPart::Mention(&rest[..len]));
            rest = &rest[len..];
            continue;
        }
        text.push('{');
        rest = &rest[1..];
    }
    text.push_str(rest);
    parts.push(BodyPart::Text(text));
    parts
}

/// The flag index a token names: `{@N}` is N, `{@}` the only flag.
fn mention_flag(token: &str, flags: usize) -> Result<usize> {
    let digits = &token[2..token.len() - 1];
    if digits.is_empty() {
        return if flags == 1 {
            Ok(0)
        } else {
            Err(TeamsError::InvalidInput(format!(
                "{{@}} is only for a single --mention; with {flags}, write {{@0}} to {{@{}}}",
                flags - 1
            )))
        };
    }
    match digits.parse::<usize>() {
        Ok(index) if index < flags => Ok(index),
        _ => Err(TeamsError::InvalidInput(format!(
            "{token} has no matching --mention: there {} {flags}, numbered from {{@0}}",
            if flags == 1 { "is" } else { "are" }
        ))),
    }
}

fn mention_element(id: usize, identity: &MentionIdentity) -> String {
    format!(
        r#"<at id="{id}">{}</at>"#,
        escape_html_text(&identity.display_name)
    )
}

/// The JSON half of a mention. Graph pairs it with the body's `<at id="N">`
/// element by id, so `id` must match that element; the display name is
/// stored unescaped.
fn mention_entry(id: usize, identity: &MentionIdentity) -> ChatMessageMention {
    ChatMessageMention {
        id: id as i32,
        mention_text: identity.display_name.clone(),
        mentioned: ChatMessageMentioned {
            user: Some(ChatMessageUser {
                id: Some(identity.id.clone()),
                display_name: Some(identity.display_name.clone()),
                user_identity_type: Some("aadUser".to_string()),
            }),
            ..Default::default()
        },
    }
}

/// Synchronize the request's body and `mentions` array so each `<at id="N">`
/// element has a matching entry — both halves are required for Graph to keep
/// the mention. A plain-text body is safely promoted to HTML (escaped, line
/// breaks preserved as `<br>`); an explicit HTML body is kept as supplied.
///
/// `identities` are the `--mention` flags in order. Where the body has
/// `{@N}` tokens, each one becomes an `<at>` element for flag N, numbered in
/// order of appearance, so a person placed twice gets two elements and two
/// entries. Every person must be placed; flags that resolve to the same
/// person count as one. Without tokens, each person is prepended once, in
/// flag order. `{{@N}}` is always the literal `{@N}`.
fn apply_mentions(req: &mut SendMessageRequest, identities: &[MentionIdentity]) -> Result<()> {
    let body = req.body.content.take();
    let parts = split_mention_tokens(body.as_deref().unwrap_or_default());
    let tokens: Vec<&str> = parts
        .iter()
        .filter_map(|part| match part {
            BodyPart::Mention(token) => Some(*token),
            BodyPart::Text(_) => None,
        })
        .collect();

    if identities.is_empty() {
        if let Some(token) = tokens.first() {
            return Err(TeamsError::InvalidInput(format!(
                "{token} places a mention, but no --mention was given; \
                 write {{{token}}} for the literal text"
            )));
        }
        let literal = match parts.into_iter().next() {
            Some(BodyPart::Text(text)) => text,
            _ => String::new(),
        };
        req.body.content = body.is_some().then_some(literal);
        return Ok(());
    }

    let content_type = req.body.content_type.as_deref().unwrap_or_default();
    if !matches!(content_type, "text" | "html") {
        return Err(TeamsError::InvalidInput(format!(
            "--mention requires a text or html body, not '{content_type}'"
        )));
    }
    let promoted = content_type == "text";
    if promoted {
        req.body.content_type = Some("html".to_string());
    }
    let text = |t: &str| {
        if promoted {
            escape_body_text(t)
        } else {
            t.to_string()
        }
    };

    let mut placed: Vec<&MentionIdentity> = Vec::new();
    let mut content = String::new();
    if tokens.is_empty() {
        placed = dedup_mentions(identities);
        let prefix: Vec<String> = placed
            .iter()
            .enumerate()
            .map(|(i, identity)| mention_element(i, identity))
            .collect();
        content = prefix.join(" ");
        let rest = match &parts[..] {
            [BodyPart::Text(t)] => text(t),
            _ => unreachable!("a body without tokens is one text part"),
        };
        if !rest.is_empty() {
            content.push(' ');
            content.push_str(&rest);
        }
    } else {
        for part in &parts {
            match part {
                BodyPart::Text(t) => content.push_str(&text(t)),
                BodyPart::Mention(token) => {
                    let identity = &identities[mention_flag(token, identities.len())?];
                    content.push_str(&mention_element(placed.len(), identity));
                    placed.push(identity);
                }
            }
        }
        for (flag, identity) in identities.iter().enumerate() {
            if !placed.iter().any(|p| p.id == identity.id) {
                return Err(TeamsError::InvalidInput(format!(
                    "--mention {} is not placed in the body: add {{@{flag}}} where it \
                     goes, or remove the flag",
                    identity.display_name
                )));
            }
        }
    }

    req.body.content = Some(content);
    req.mentions = Some(
        placed
            .iter()
            .enumerate()
            .map(|(i, identity)| mention_entry(i, identity))
            .collect(),
    );
    Ok(())
}

/// Build a message body and synchronize any resolved mentions after rejecting
/// raw `<at>` markup. Both new messages and replies use this path so neither can
/// post ambiguous mention IDs or markup that only looks like a notification.
fn build_send_request_with_mentions(
    content: String,
    content_type: &str,
    adaptive_card_path: Option<&str>,
    identities: &[MentionIdentity],
) -> Result<SendMessageRequest> {
    ensure_no_raw_at_markup(content_type, &content)?;
    let mut req = build_send_request(content, content_type, adaptive_card_path)?;
    apply_mentions(&mut req, identities)?;
    Ok(req)
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

    #[test]
    fn human_message_row_preserves_subject_and_body() {
        let mut message: ChatMessage = serde_json::from_value(serde_json::json!({
            "id": "message-id", "subject": "Release α & <plan>",
            "from": {"user": {"displayName": "Example User"}},
            "body": {"content": "Details"}, "createdDateTime": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        assert_eq!(
            message_list_row(&message),
            [
                "message-id",
                "Example User",
                "Release α & <plan>",
                "Details",
                "2026-01-01T00:00:00Z"
            ]
        );
        message.subject = None;
        assert_eq!(message_list_row(&message)[2], "");
    }

    #[test]
    fn human_message_row_names_an_application_sender() {
        let message: ChatMessage = serde_json::from_value(serde_json::json!({
            "id": "message-id",
            "from": {"user": null, "application": {"displayName": "Workflows"}}
        }))
        .unwrap();
        assert_eq!(message_list_row(&message)[1], "Workflows");
    }

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
            subject: None,
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
        let flags = [
            identity("same-oid", "First Name"),
            identity("other-oid", "Other Person"),
            identity("same-oid", "Second Name"),
        ];
        let deduped = dedup_mentions(&flags);
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
            subject: None,
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
            subject: None,
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

    /// `--mention` and `--adaptive-card` both rewrite the body and the content
    /// type, and the send path runs the card first. The card promotes a text
    /// body to escaped HTML and appends its marker; the mention prefix then has
    /// to go in front of that without escaping it a second time.
    #[test]
    fn a_mention_and_a_card_compose_without_re_escaping() {
        let dir = std::env::temp_dir().join("teams-cli-card-mention");
        std::fs::create_dir_all(&dir).unwrap();
        let card = write_card(&dir);

        let req = build_send_request_with_mentions(
            "a < b".to_string(),
            "text",
            Some(&card),
            &[identity("oid-1", "Sophie")],
        )
        .unwrap();

        let body = req.body.content.clone().unwrap();
        let attachment_id = req.attachments.as_ref().unwrap()[0].id.clone().unwrap();

        assert!(body.starts_with(r#"<at id="0">Sophie</at> "#), "{body}");
        // the caller's text is escaped exactly once
        assert!(body.contains("<p>a &lt; b</p>"), "{body}");
        assert!(!body.contains("&amp;lt;"), "body was escaped twice: {body}");
        // and both wire halves survive
        assert!(
            body.ends_with(&format!(
                r#"<attachment id="{attachment_id}"></attachment>"#
            )),
            "{body}"
        );
        assert_eq!(req.body.content_type.as_deref(), Some("html"));
        assert_eq!(req.mentions.as_ref().unwrap().len(), 1);
        assert_eq!(req.mentions.as_ref().unwrap()[0].id, 0);
    }

    /// Sends without mentions keep current behaviour byte for byte: no
    /// transformation, no `mentions` property in the wire payload.
    #[test]
    fn no_mentions_leaves_the_request_untouched() {
        let mut text_req = SendMessageRequest {
            subject: None,
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
    fn message_build_rejects_raw_at_markup_before_applying_mentions() {
        let err = build_send_request_with_mentions(
            r#"<p><at id="0">Sophie</at> please review</p>"#.into(),
            "html",
            None,
            &[identity("oid-1", "Alex")],
        )
        .unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)), "{err:?}");
        assert!(err.to_string().contains("--mention"), "{err}");

        // Uppercase markup is caught too.
        assert!(
            build_send_request_with_mentions("<AT>Sophie</AT>".into(), "HTML", None, &[]).is_err()
        );
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

    fn text_request(body: &str) -> SendMessageRequest {
        let mut req = html_request(body);
        req.body.content_type = Some("text".into());
        req
    }

    fn mention_user_ids(req: &SendMessageRequest) -> Vec<(i32, String)> {
        req.mentions
            .as_ref()
            .unwrap()
            .iter()
            .map(|m| {
                let user = m.mentioned.user.as_ref().unwrap();
                (m.id, user.id.clone().unwrap())
            })
            .collect()
    }

    fn placement_error(body: &str, content_type: &str, identities: &[MentionIdentity]) -> String {
        let err = build_send_request_with_mentions(body.into(), content_type, None, identities)
            .unwrap_err();
        assert!(matches!(err, TeamsError::InvalidInput(_)), "{err:?}");
        assert_eq!(err.exit_code(), 2);
        err.to_string()
    }

    /// `{@0}` places the first mention where it stands in a text body; the
    /// text is escaped and promoted to HTML as it is without tokens.
    #[test]
    fn token_places_the_mention_inside_a_text_body() {
        let mut req = text_request("Hey {@0}, could you review this?\nThanks & bye");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie Daniels")]).unwrap();

        assert_eq!(req.body.content_type.as_deref(), Some("html"));
        assert_eq!(
            req.body.content.as_deref(),
            Some(
                r#"Hey <at id="0">Sophie Daniels</at>, could you review this?<br>Thanks &amp; bye"#
            )
        );
        assert_eq!(mention_user_ids(&req), [(0, "oid-1".to_string())]);
    }

    #[test]
    fn token_places_the_mention_inside_an_html_body() {
        let mut req = html_request("<p>Hey {@0}, <b>look</b> &amp; reply</p>");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<p>Hey <at id="0">Sophie</at>, <b>look</b> &amp; reply</p>"#)
        );
        assert_eq!(mention_user_ids(&req), [(0, "oid-1".to_string())]);
    }

    /// Ids follow the order of appearance in the body, not the flag order.
    #[test]
    fn tokens_take_ids_in_body_order() {
        let mut req = text_request("{@1} then {@0}");
        apply_mentions(
            &mut req,
            &[identity("oid-a", "Ann"), identity("oid-b", "Bob")],
        )
        .unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Bob</at> then <at id="1">Ann</at>"#)
        );
        assert_eq!(
            mention_user_ids(&req),
            [(0, "oid-b".to_string()), (1, "oid-a".to_string())]
        );
    }

    #[test]
    fn bare_token_is_shorthand_for_the_only_mention() {
        let mut req = text_request("Hey {@}!");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"Hey <at id="0">Sophie</at>!"#)
        );

        let err = placement_error(
            "Hey {@}!",
            "text",
            &[identity("oid-a", "Ann"), identity("oid-b", "Bob")],
        );
        assert!(err.contains("{@}"), "{err}");
    }

    /// Graph matches `<at id>` elements to `mentions[]` entries one to one, so
    /// a mention placed twice is two elements with two entries.
    #[test]
    fn repeated_token_gets_a_distinct_id_each_time() {
        let mut req = text_request("{@0}, {@1} and {@0} again");
        apply_mentions(
            &mut req,
            &[identity("oid-a", "Ann"), identity("oid-b", "Bob")],
        )
        .unwrap();

        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Ann</at>, <at id="1">Bob</at> and <at id="2">Ann</at> again"#)
        );
        assert_eq!(
            mention_user_ids(&req),
            [
                (0, "oid-a".to_string()),
                (1, "oid-b".to_string()),
                (2, "oid-a".to_string())
            ]
        );
    }

    #[test]
    fn token_past_the_last_mention_is_invalid_input() {
        let err = placement_error("Hey {@1}", "text", &[identity("oid-1", "Sophie")]);
        assert!(err.contains("{@1}"), "{err}");

        let err = placement_error(
            "Hey {@99999999999999999999999}",
            "text",
            &[identity("oid-1", "Sophie")],
        );
        assert!(err.contains("--mention"), "{err}");
    }

    /// Once tokens place mentions, one left out would silently vanish.
    #[test]
    fn unplaced_mention_is_invalid_input() {
        let err = placement_error(
            "Hey {@0}",
            "text",
            &[identity("oid-a", "Ann"), identity("oid-b", "Bob")],
        );
        assert!(err.contains("{@1}") && err.contains("Bob"), "{err}");
    }

    #[test]
    fn token_without_any_mention_is_invalid_input() {
        let err = placement_error("Hey {@0}", "text", &[]);
        assert!(err.contains("--mention"), "{err}");
        let err = placement_error("<p>Hey {@}</p>", "html", &[]);
        assert!(err.contains("--mention"), "{err}");
    }

    /// `{{@N}}` is the literal text `{@N}`, with or without mentions.
    #[test]
    fn doubled_braces_escape_a_literal_token() {
        let mut req = text_request("{@0}: write {{@0}} or {{@}}");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Sophie</at>: write {@0} or {@}"#)
        );

        // Without tokens the mention is prepended, and the escape still reads
        // as the literal.
        let mut req = text_request("type {{@0}}");
        apply_mentions(&mut req, &[identity("oid-1", "Sophie")]).unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Sophie</at> type {@0}"#)
        );

        let req =
            build_send_request_with_mentions("type {{@0}}".into(), "text", None, &[]).unwrap();
        assert_eq!(req.body.content_type.as_deref(), Some("text"));
        assert_eq!(req.body.content.as_deref(), Some("type {@0}"));
        assert!(req.mentions.is_none());
    }

    /// Tokens are read left to right: a doubled brace is consumed as the
    /// escape before a token can start inside it.
    #[test]
    fn token_scanner_edge_cases() {
        use BodyPart::{Mention, Text};
        let t = |s: &str| Text(s.to_string());
        for (body, parts) in [
            ("{{@0}}", vec![t("{@0}")]),
            ("{{{@0}}}", vec![t("{{@0}}")]),
            ("{@0}}", vec![t(""), Mention("{@0}"), t("}")]),
            ("{{@0}", vec![t("{"), Mention("{@0}"), t("")]),
            ("é{@12}日本", vec![t("é"), Mention("{@12}"), t("日本")]),
            (
                "{@}{@}",
                vec![t(""), Mention("{@}"), t(""), Mention("{@}"), t("")],
            ),
            ("tail {@", vec![t("tail {@")]),
        ] {
            assert_eq!(split_mention_tokens(body), parts, "{body}");
        }
        // A leading zero still names the flag by number.
        assert_eq!(mention_flag("{@00}", 1).unwrap(), 0);
    }

    /// Braces that are not a token are left alone.
    #[test]
    fn other_braces_are_not_tokens() {
        let body = "{@x} {0} {@ 0} {{@x}} @0 {@";
        let req = build_send_request_with_mentions(body.into(), "text", None, &[]).unwrap();
        assert_eq!(req.body.content.as_deref(), Some(body));
    }

    /// The raw-markup check runs on the body as supplied, before tokens are
    /// replaced with the `<at>` elements the CLI builds.
    #[test]
    fn raw_at_markup_is_rejected_alongside_tokens() {
        let err = placement_error(
            r#"<p><at id="0">Sophie</at> {@0}</p>"#,
            "html",
            &[identity("oid-1", "Sophie")],
        );
        assert!(err.contains("raw <at> markup"), "{err}");

        // The <at> the CLI substitutes is not mistaken for raw markup.
        let req = build_send_request_with_mentions(
            "<p>{@0}</p>".into(),
            "html",
            None,
            &[identity("oid-1", "Sophie")],
        )
        .unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<p><at id="0">Sophie</at></p>"#)
        );
    }

    #[test]
    fn placed_display_names_are_escaped_in_html_only() {
        let mut req = text_request("cc {@0}");
        apply_mentions(&mut req, &[identity("oid-1", "R&D <Lead>")]).unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"cc <at id="0">R&amp;D &lt;Lead&gt;</at>"#)
        );
        let mention = &req.mentions.as_ref().unwrap()[0];
        assert_eq!(mention.mention_text, "R&D <Lead>");
        let user = mention.mentioned.user.as_ref().unwrap();
        assert_eq!(user.display_name.as_deref(), Some("R&D <Lead>"));
    }

    /// `{@N}` always names the N-th `--mention` flag. Two flags that resolve
    /// to the same person (a UPN and an object ID, say) are one person:
    /// placing either one counts for both, and without tokens the person is
    /// prepended once, as before.
    #[test]
    fn tokens_index_flags_and_duplicate_people_count_once() {
        let flags = [
            identity("oid-a", "Ann (by UPN)"),
            identity("oid-b", "Bob"),
            identity("oid-a", "Ann (by id)"),
        ];

        let mut req = text_request("{@2} and {@1}");
        apply_mentions(&mut req, &flags).unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Ann (by id)</at> and <at id="1">Bob</at>"#)
        );
        assert_eq!(
            mention_user_ids(&req),
            [(0, "oid-a".to_string()), (1, "oid-b".to_string())]
        );

        // Ann is never placed under either flag.
        let err = placement_error("only {@1}", "text", &flags);
        assert!(err.contains("{@0}"), "{err}");

        let mut req = text_request("hi");
        apply_mentions(&mut req, &flags).unwrap();
        assert_eq!(
            req.body.content.as_deref(),
            Some(r#"<at id="0">Ann (by UPN)</at> <at id="1">Bob</at> hi"#)
        );
        assert_eq!(
            mention_user_ids(&req),
            [(0, "oid-a".to_string()), (1, "oid-b".to_string())]
        );
    }

    /// A card promotes a text body to `<p>`-wrapped HTML before the mentions
    /// are applied; a token inside it still lands in place, escaped once.
    #[test]
    fn token_and_card_compose() {
        let dir = std::env::temp_dir().join("teams-cli-card-token");
        std::fs::create_dir_all(&dir).unwrap();
        let card = write_card(&dir);

        let req = build_send_request_with_mentions(
            "a < b, {@0}".into(),
            "text",
            Some(&card),
            &[identity("oid-1", "Sophie")],
        )
        .unwrap();
        let body = req.body.content.unwrap();
        assert!(
            body.starts_with(r#"<p>a &lt; b, <at id="0">Sophie</at></p><attachment"#),
            "{body}"
        );
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
        let resolved = [
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
        ];
        assert_eq!(
            dedup_mentions(&resolved),
            vec![&identity("oid-a", "Sophie Daniels")]
        );
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
    fn resolve_message_ref_validates_target_combinations() {
        assert!(matches!(
            resolve_message_ref(None, None, Some("19:x".into()), None, "m".into()),
            Ok(MessageRef::Chat { .. })
        ));
        assert!(matches!(
            resolve_message_ref(
                Some("t".into()),
                Some("c".into()),
                None,
                Some("r".into()),
                "m".into()
            ),
            Ok(MessageRef::ChannelReply { .. })
        ));
        assert!(resolve_message_ref(Some("t".into()), None, None, None, "m".into()).is_err());
        assert!(resolve_message_ref(
            None,
            None,
            Some("19:x".into()),
            Some("r".into()),
            "m".into()
        )
        .is_err());
    }

    /// `update --team T --channel C --reply R P` edits reply R in the thread
    /// of parent post P: a PATCH to `.../messages/P/replies/R` with the body.
    #[tokio::test]
    async fn update_with_reply_patches_the_reply() {
        use clap::Parser;
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let cli = crate::cli::Cli::try_parse_from([
            "teams",
            "message",
            "update",
            "--team",
            "T",
            "--channel",
            "C",
            "--reply",
            "R",
            "P",
            "--content-type",
            "html",
            "--body",
            "<p>edited</p>",
        ])
        .unwrap();
        let crate::cli::Commands::Message {
            command:
                MessageCommand::Update {
                    team,
                    channel,
                    chat,
                    reply,
                    message_id,
                    message,
                    body,
                    stdin,
                    content_type,
                    mention: _,
                },
        } = cli.command
        else {
            panic!("expected message update");
        };
        let message_id = resolve_id(message_id, message, "").unwrap();
        let target = resolve_message_ref(team, channel, chat, reply, message_id).unwrap();
        let req =
            build_send_request(resolve_body(body, stdin).unwrap(), &content_type, None).unwrap();
        let url = target.message_url();
        assert_eq!(
            url,
            "https://graph.microsoft.com/v1.0/teams/T/channels/C/messages/P/replies/R"
        );

        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/teams/T/channels/C/messages/P/replies/R"))
            .and(body_json(serde_json::json!({
                "body": { "contentType": "html", "content": "<p>edited</p>" }
            })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        api::messages::update_message_at(
            &read_back::test_client(),
            &url.replace(api::endpoints::GRAPH_V1, &server.uri()),
            &req,
        )
        .await
        .unwrap();
    }

    /// `--reply` picks a reply in a channel thread, so with `--chat` every
    /// command gives the usage error `message delete` gives: exit code 2.
    #[test]
    fn reply_with_chat_is_rejected_like_delete() {
        use clap::Parser;
        for args in [
            &["delete", "P", "--yes"][..],
            &["update", "P", "--body", "x"],
            &["react", "--message-id", "P", "like"],
            &["unreact", "--message-id", "P", "like"],
        ] {
            let err = crate::cli::Cli::try_parse_from(
                ["teams", "message"]
                    .iter()
                    .chain(args)
                    .chain(&["--chat", "19:x", "--reply", "R"]),
            )
            .unwrap_err();
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::ArgumentConflict,
                "{args:?}: {err}"
            );
            assert_eq!(err.exit_code(), 2);
        }
    }

    mod read_back {
        use super::*;
        use crate::auth::token::TokenInfo;
        use crate::config::NetworkConfig;
        use reqwest::Client;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        pub(super) fn test_client() -> GraphClient {
            GraphClient {
                http: Client::new(),
                token: TokenInfo {
                    access_token: "test-token".into(),
                    expires_at: None,
                    token_type: "Bearer".into(),
                    scope: None,
                    refresh_token: None,
                    profile: "default".into(),
                },
                network: NetworkConfig {
                    timeout: 30,
                    max_retries: 0,
                    retry_backoff_base: 2,
                },
            }
        }

        async fn server_returning(status: u16, body: serde_json::Value) -> MockServer {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/me/chats/chat-id/messages/message-id"))
                .respond_with(ResponseTemplate::new(status).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            server
        }

        #[tokio::test]
        async fn failed_reply_read_back_reports_reply_id() {
            for outcome in ["deleted", "restored"] {
                let server = server_returning(
                    404,
                    serde_json::json!({
                        "error": { "code": "NotFound", "message": "Message not found" }
                    }),
                )
                .await;
                let target = MessageRef::ChannelReply {
                    team_id: "team".into(),
                    channel_id: "channel".into(),
                    message_id: "parent-id".into(),
                    reply_id: "reply-id".into(),
                };
                let value = read_back_at(
                    &test_client(),
                    &format!("{}/me/chats/chat-id/messages/message-id", server.uri()),
                    &target,
                    outcome,
                )
                .await;
                assert_eq!(value["id"], "reply-id");
                assert_eq!(value[outcome], true);
                assert!(value["readBackError"].is_string());
            }
        }

        /// After a delete, Graph reports the message with `deletedDateTime`
        /// set and the body blanked; that is what the command prints.
        #[tokio::test]
        async fn reports_deleted_date_time_after_delete() {
            let server = server_returning(
                200,
                serde_json::json!({
                    "id": "message-id",
                    "deletedDateTime": "2026-08-20T20:33:53.378Z",
                    "body": { "contentType": "text", "content": "" }
                }),
            )
            .await;

            let value = read_back_at(
                &test_client(),
                &format!("{}/me/chats/chat-id/messages/message-id", server.uri()),
                &MessageRef::Chat {
                    chat_id: "chat-id".into(),
                    message_id: "message-id".into(),
                },
                "deleted",
            )
            .await;

            assert_eq!(value["deletedDateTime"], "2026-08-20T20:33:53.378Z");
            assert_eq!(value["body"]["content"], "");
            assert!(value.get("deleted").is_none());
        }

        /// After an undelete, Graph omits `deletedDateTime` again and the
        /// original text is back.
        #[tokio::test]
        async fn omits_deleted_date_time_after_restore() {
            let server = server_returning(
                200,
                serde_json::json!({
                    "id": "message-id",
                    "deletedDateTime": null,
                    "body": { "contentType": "text", "content": "hello again" }
                }),
            )
            .await;

            let value = read_back_at(
                &test_client(),
                &format!("{}/me/chats/chat-id/messages/message-id", server.uri()),
                &MessageRef::Chat {
                    chat_id: "chat-id".into(),
                    message_id: "message-id".into(),
                },
                "restored",
            )
            .await;

            assert!(value.get("deletedDateTime").is_none(), "{value}");
            assert_eq!(value["body"]["content"], "hello again");
        }

        /// The mutation has already succeeded when the read-back runs, so a
        /// failed read-back still reports success, naming the outcome and the
        /// read error instead of the message.
        #[tokio::test]
        async fn failed_read_back_reports_the_outcome() {
            let server = server_returning(
                404,
                serde_json::json!({
                    "error": { "code": "NotFound", "message": "Message not found" }
                }),
            )
            .await;

            let value = read_back_at(
                &test_client(),
                &format!("{}/me/chats/chat-id/messages/message-id", server.uri()),
                &MessageRef::Chat {
                    chat_id: "chat-id".into(),
                    message_id: "message-id".into(),
                },
                "deleted",
            )
            .await;

            assert_eq!(value["id"], "message-id");
            assert_eq!(value["deleted"], true);
            assert!(
                value["readBackError"]
                    .as_str()
                    .is_some_and(|e| e.contains("not found")),
                "{value}"
            );
        }
    }

    #[test]
    fn channel_scope_hint_applies_to_channel_targets_only() {
        let channel = MessageRef::Channel {
            team_id: "t".into(),
            channel_id: "c".into(),
            message_id: "m".into(),
        };
        let chat = MessageRef::Chat {
            chat_id: "19:x".into(),
            message_id: "m".into(),
        };

        let denied = || TeamsError::PermissionDenied("Forbidden".into());
        let default_scopes = Some("Chat.ReadWrite ChannelMessage.Send User.Read");

        let hinted = with_channel_scope_hint(denied(), &channel, default_scopes);
        let text = hinted.to_string();
        assert!(text.contains("ChannelMessage.ReadWrite"), "{text}");
        assert!(text.contains("admin consent"), "{text}");
        assert!(text.contains("teams auth consent-url"), "{text}");
        assert_eq!(hinted.exit_code(), 4);

        // No scope list to inspect (a pre-obtained token): still hint.
        let hinted = with_channel_scope_hint(denied(), &channel, None);
        assert!(hinted.to_string().contains("ChannelMessage.ReadWrite"));

        // The token already has the scope, so the 403 has another cause.
        let has_scope = Some("Chat.ReadWrite channelmessage.readwrite User.Read");
        let untouched = with_channel_scope_hint(denied(), &channel, has_scope);
        assert_eq!(untouched.to_string(), "Permission denied: Forbidden");

        let reply = MessageRef::ChannelReply {
            team_id: "t".into(),
            channel_id: "c".into(),
            message_id: "m".into(),
            reply_id: "r".into(),
        };
        let hinted = with_channel_scope_hint(denied(), &reply, default_scopes);
        assert!(hinted.to_string().contains("ChannelMessage.ReadWrite"));

        let untouched = with_channel_scope_hint(denied(), &chat, default_scopes);
        assert_eq!(untouched.to_string(), "Permission denied: Forbidden");

        let other = with_channel_scope_hint(
            TeamsError::NotFound("gone".into()),
            &channel,
            default_scopes,
        );
        assert!(matches!(other, TeamsError::NotFound(_)), "{other:?}");
    }
}
