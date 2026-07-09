use crate::error::Result;
use crate::models::message::{
    ChatMessage, ChatMessageHostedContent, PinMessageRequest, PinnedMessage, ReactionRequest,
    SendMessageRequest,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

/// Location of a message for operations that work across channel messages,
/// channel thread replies, and chat messages.
#[derive(Debug, Clone)]
pub enum MessageRef {
    Channel {
        team_id: String,
        channel_id: String,
        message_id: String,
    },
    ChannelReply {
        team_id: String,
        channel_id: String,
        message_id: String,
        reply_id: String,
    },
    Chat {
        chat_id: String,
        message_id: String,
    },
}

impl MessageRef {
    fn message_url(&self) -> String {
        match self {
            Self::Channel {
                team_id,
                channel_id,
                message_id,
            } => endpoints::channel_message(team_id, channel_id, message_id),
            Self::ChannelReply {
                team_id,
                channel_id,
                message_id,
                reply_id,
            } => endpoints::channel_message_reply(team_id, channel_id, message_id, reply_id),
            Self::Chat {
                chat_id,
                message_id,
            } => endpoints::chat_message(chat_id, message_id),
        }
    }

    fn hosted_contents_url(&self) -> String {
        match self {
            Self::Channel {
                team_id,
                channel_id,
                message_id,
            } => endpoints::channel_message_hosted_contents(team_id, channel_id, message_id),
            Self::ChannelReply {
                team_id,
                channel_id,
                message_id,
                reply_id,
            } => {
                endpoints::channel_reply_hosted_contents(team_id, channel_id, message_id, reply_id)
            }
            Self::Chat {
                chat_id,
                message_id,
            } => endpoints::chat_message_hosted_contents(chat_id, message_id),
        }
    }

    fn hosted_content_value_url(&self, hosted_content_id: &str) -> String {
        match self {
            Self::Channel {
                team_id,
                channel_id,
                message_id,
            } => endpoints::channel_message_hosted_content_value(
                team_id,
                channel_id,
                message_id,
                hosted_content_id,
            ),
            Self::ChannelReply {
                team_id,
                channel_id,
                message_id,
                reply_id,
            } => endpoints::channel_reply_hosted_content_value(
                team_id,
                channel_id,
                message_id,
                reply_id,
                hosted_content_id,
            ),
            Self::Chat {
                chat_id,
                message_id,
            } => {
                endpoints::chat_message_hosted_content_value(chat_id, message_id, hosted_content_id)
            }
        }
    }
}

pub async fn get_message(client: &GraphClient, message: &MessageRef) -> Result<ChatMessage> {
    client.get(&message.message_url(), &[]).await
}

pub async fn list_hosted_contents(
    client: &GraphClient,
    message: &MessageRef,
) -> Result<Vec<ChatMessageHostedContent>> {
    client
        .get_all_pages(&message.hosted_contents_url(), &[])
        .await
}

/// Fetch a hosted content's raw bytes; the MIME type is only available from
/// the response's Content-Type header, returned alongside the bytes.
pub async fn get_hosted_content_bytes(
    client: &GraphClient,
    message: &MessageRef,
    hosted_content_id: &str,
) -> Result<(Vec<u8>, Option<String>)> {
    client
        .get_bytes_with_content_type(&message.hosted_content_value_url(hosted_content_id))
        .await
}

// --- Channel Messages ---

pub async fn list_channel_messages(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ChatMessage>> {
    client
        .get_paged(
            &endpoints::channel_messages(team_id, channel_id),
            &[],
            pagination,
        )
        .await
}

pub async fn get_channel_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<ChatMessage> {
    client
        .get(
            &endpoints::channel_message(team_id, channel_id, message_id),
            &[],
        )
        .await
}

pub async fn send_channel_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    req: &SendMessageRequest,
) -> Result<ChatMessage> {
    client
        .post(&endpoints::channel_messages(team_id, channel_id), req)
        .await
}

pub async fn reply_to_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    req: &SendMessageRequest,
) -> Result<ChatMessage> {
    client
        .post(
            &endpoints::channel_message_replies(team_id, channel_id, message_id),
            req,
        )
        .await
}

pub async fn update_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    req: &SendMessageRequest,
) -> Result<ChatMessage> {
    client
        .patch(
            &endpoints::channel_message(team_id, channel_id, message_id),
            req,
        )
        .await
}

pub async fn delete_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<()> {
    client
        .delete(&endpoints::channel_message(team_id, channel_id, message_id))
        .await
}

// --- Chat Messages ---

pub async fn list_chat_messages(
    client: &GraphClient,
    chat_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ChatMessage>> {
    client
        .get_paged(&endpoints::chat_messages(chat_id), &[], pagination)
        .await
}

pub async fn send_chat_message(
    client: &GraphClient,
    chat_id: &str,
    req: &SendMessageRequest,
) -> Result<ChatMessage> {
    client.post(&endpoints::chat_messages(chat_id), req).await
}

// --- Reactions (beta) ---

pub async fn set_reaction(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    reaction: &str,
) -> Result<()> {
    tracing::warn!("Using beta API endpoint for reactions");
    let req = ReactionRequest {
        reaction_type: reaction.to_string(),
    };
    client
        .post_no_content(
            &endpoints::message_set_reaction(team_id, channel_id, message_id),
            &req,
        )
        .await
}

pub async fn unset_reaction(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    reaction: &str,
) -> Result<()> {
    tracing::warn!("Using beta API endpoint for reactions");
    let req = ReactionRequest {
        reaction_type: reaction.to_string(),
    };
    client
        .post_no_content(
            &endpoints::message_unset_reaction(team_id, channel_id, message_id),
            &req,
        )
        .await
}

// --- Pinned Messages ---

pub async fn pin_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<PinnedMessage> {
    let req = PinMessageRequest {
        message_odata_bind: format!(
            "https://graph.microsoft.com/v1.0/teams('{team_id}')/channels('{channel_id}')/messages('{message_id}')"
        ),
    };
    client
        .post(
            &endpoints::channel_pinned_messages(team_id, channel_id),
            &req,
        )
        .await
}

pub async fn unpin_message(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    pinned_message_id: &str,
) -> Result<()> {
    client
        .delete(&endpoints::channel_pinned_message(
            team_id,
            channel_id,
            pinned_message_id,
        ))
        .await
}
