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

/// Edit a message already sent to a channel, a channel reply, or a chat.
///
/// A delegated caller may update every property of a message except
/// `policyViolation`, so one's own message body can be rewritten in place. An
/// application-only token is confined to `policyViolation`, which is why the
/// command layer requires a delegated token first.
///
/// For delegated callers Graph answers this PATCH with `204 No Content` on
/// every target rather than the updated message, so nothing is returned; call
/// [`get_message`] afterwards to show the new text.
pub async fn update_message(
    client: &GraphClient,
    message: &MessageRef,
    req: &SendMessageRequest,
) -> Result<()> {
    update_message_at(client, &message.message_url(), req).await
}

async fn update_message_at(
    client: &GraphClient,
    url: &str,
    req: &SendMessageRequest,
) -> Result<()> {
    client.patch_no_content(url, req).await
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
    send_chat_message_at(client, &endpoints::chat_messages(chat_id), req).await
}

async fn send_chat_message_at(
    client: &GraphClient,
    url: &str,
    req: &SendMessageRequest,
) -> Result<ChatMessage> {
    client.post(url, req).await
}

// --- Reactions ---
//
// Microsoft Graph documents setReaction/unsetReaction on v1.0 for channel
// messages, channel replies, and chat messages. The request body must carry
// the reaction as a unicode character; legacy names such as `like` are only
// ever returned on reads, and are rejected on writes with HTTP 400.

pub async fn set_reaction(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    reaction: &str,
) -> Result<()> {
    set_reaction_at(
        client,
        &endpoints::message_set_reaction(team_id, channel_id, message_id),
        reaction,
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
    set_reaction_at(
        client,
        &endpoints::message_unset_reaction(team_id, channel_id, message_id),
        reaction,
    )
    .await
}

pub async fn set_chat_reaction(
    client: &GraphClient,
    chat_id: &str,
    message_id: &str,
    reaction: &str,
) -> Result<()> {
    set_reaction_at(
        client,
        &endpoints::chat_message_set_reaction(chat_id, message_id),
        reaction,
    )
    .await
}

pub async fn unset_chat_reaction(
    client: &GraphClient,
    chat_id: &str,
    message_id: &str,
    reaction: &str,
) -> Result<()> {
    set_reaction_at(
        client,
        &endpoints::chat_message_unset_reaction(chat_id, message_id),
        reaction,
    )
    .await
}

/// POST `{"reactionType": reaction}` to a setReaction/unsetReaction action URL.
/// Graph answers both with `204 No Content`.
async fn set_reaction_at(client: &GraphClient, url: &str, reaction: &str) -> Result<()> {
    let req = ReactionRequest {
        reaction_type: reaction.to_string(),
    };
    client.post_no_content(url, &req).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use crate::error::TeamsError;
    use crate::models::message::ItemBody;
    use reqwest::Client;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client() -> GraphClient {
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

    #[test]
    fn reaction_endpoints_target_v1() {
        assert_eq!(
            endpoints::chat_message_set_reaction("19:abc@thread.v2", "1700000000000"),
            "https://graph.microsoft.com/v1.0/chats/19:abc@thread.v2/messages/1700000000000/setReaction"
        );
        assert_eq!(
            endpoints::chat_message_unset_reaction("19:abc@thread.v2", "1700000000000"),
            "https://graph.microsoft.com/v1.0/chats/19:abc@thread.v2/messages/1700000000000/unsetReaction"
        );
        assert_eq!(
            endpoints::message_set_reaction("team-id", "channel-id", "1700000000000"),
            "https://graph.microsoft.com/v1.0/teams/team-id/channels/channel-id/messages/1700000000000/setReaction"
        );
        assert_eq!(
            endpoints::message_unset_reaction("team-id", "channel-id", "1700000000000"),
            "https://graph.microsoft.com/v1.0/teams/team-id/channels/channel-id/messages/1700000000000/unsetReaction"
        );
    }

    /// Graph expects the reaction as a unicode character in `reactionType`
    /// and answers a successful setReaction with 204 and an empty body.
    #[tokio::test]
    async fn set_chat_reaction_posts_unicode_and_accepts_no_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/chat-id/messages/message-id/setReaction"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(serde_json::json!({ "reactionType": "👀" })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        set_reaction_at(
            &test_client(),
            &format!(
                "{}/chats/chat-id/messages/message-id/setReaction",
                server.uri()
            ),
            "👀",
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn unset_chat_reaction_posts_unicode_and_accepts_no_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/chat-id/messages/message-id/unsetReaction"))
            .and(body_json(serde_json::json!({ "reactionType": "👍" })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        set_reaction_at(
            &test_client(),
            &format!(
                "{}/chats/chat-id/messages/message-id/unsetReaction",
                server.uri()
            ),
            "👍",
        )
        .await
        .unwrap();
    }

    /// A legacy name that slips through to Graph is rejected with 400; the
    /// client must surface that as an API error rather than a parse failure.
    #[tokio::test]
    async fn set_reaction_surfaces_bad_request_for_unsupported_value() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": {
                    "code": "BadRequest",
                    "message": "Unicode 'like' in the payload is not supported"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let err = set_reaction_at(
            &test_client(),
            &format!(
                "{}/chats/chat-id/messages/message-id/setReaction",
                server.uri()
            ),
            "like",
        )
        .await
        .unwrap_err();

        assert!(
            matches!(&err, TeamsError::ApiError { status: 400, message } if message.contains("not supported")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn set_reaction_reports_permission_denied() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": { "code": "Forbidden", "message": "Insufficient privileges" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let err = set_reaction_at(
            &test_client(),
            &format!(
                "{}/chats/chat-id/messages/message-id/setReaction",
                server.uri()
            ),
            "👀",
        )
        .await
        .unwrap_err();

        assert!(matches!(err, TeamsError::PermissionDenied(_)), "{err:?}");
    }

    fn edit_request() -> SendMessageRequest {
        SendMessageRequest {
            body: ItemBody {
                content_type: Some("text".to_string()),
                content: Some("corrected text".to_string()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        }
    }

    /// A chat send carrying mentions must post the body and the `mentions`
    /// array in one request, with the ids Graph needs to keep them.
    #[tokio::test]
    async fn chat_send_posts_synchronized_mentions() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/19:abc@thread.v2/messages"))
            .and(body_json(serde_json::json!({
                "body": {
                    "contentType": "html",
                    "content": "<at id=\"0\">Sophie Daniels</at> Please review"
                },
                "mentions": [{
                    "id": 0,
                    "mentionText": "Sophie Daniels",
                    "mentioned": {
                        "user": {
                            "id": "32cbca05-dc05-454f-b0f3-072f331d4c97",
                            "displayName": "Sophie Daniels",
                            "userIdentityType": "aadUser"
                        }
                    }
                }]
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "1700000000000",
                "body": {
                    "contentType": "html",
                    "content": "<at id=\"0\">Sophie Daniels</at> Please review"
                },
                "mentions": [{
                    "id": 0,
                    "mentionText": "Sophie Daniels",
                    "mentioned": {
                        "user": {
                            "id": "32cbca05-dc05-454f-b0f3-072f331d4c97",
                            "displayName": "Sophie Daniels",
                            "userIdentityType": "aadUser"
                        }
                    }
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = SendMessageRequest {
            body: ItemBody {
                content_type: Some("html".into()),
                content: Some("<at id=\"0\">Sophie Daniels</at> Please review".into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: Some(vec![crate::models::message::ChatMessageMention {
                id: 0,
                mention_text: "Sophie Daniels".into(),
                mentioned: crate::models::message::ChatMessageMentioned {
                    user: Some(crate::models::message::ChatMessageUser {
                        id: Some("32cbca05-dc05-454f-b0f3-072f331d4c97".into()),
                        display_name: Some("Sophie Daniels".into()),
                        user_identity_type: Some("aadUser".into()),
                    }),
                },
            }]),
        };

        let msg = send_chat_message_at(
            &test_client(),
            &format!("{}/chats/19:abc@thread.v2/messages", server.uri()),
            &req,
        )
        .await
        .unwrap();
        let mentions = msg.mentions.unwrap();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].mention_text, "Sophie Daniels");
    }

    #[test]
    fn chat_message_endpoint_targets_the_chats_collection() {
        assert_eq!(
            endpoints::chat_message("19:abc@thread.v2", "1700000000000"),
            "https://graph.microsoft.com/v1.0/chats/19:abc@thread.v2/messages/1700000000000"
        );
    }

    #[test]
    fn message_ref_update_urls_target_the_message_resource() {
        let chat = MessageRef::Chat {
            chat_id: "19:abc@thread.v2".into(),
            message_id: "1700000000000".into(),
        };
        assert_eq!(
            chat.message_url(),
            "https://graph.microsoft.com/v1.0/chats/19:abc@thread.v2/messages/1700000000000"
        );
        let channel = MessageRef::Channel {
            team_id: "team-id".into(),
            channel_id: "channel-id".into(),
            message_id: "1700000000000".into(),
        };
        assert_eq!(
            channel.message_url(),
            "https://graph.microsoft.com/v1.0/teams/team-id/channels/channel-id/messages/1700000000000"
        );
    }

    /// Graph answers a successful delegated edit with 204 and an empty body,
    /// for chat messages and channel messages alike.
    #[tokio::test]
    async fn update_message_sends_the_body_and_accepts_no_content() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/chats/chat-id/messages/message-id"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(serde_json::json!({
                "body": { "contentType": "text", "content": "corrected text" }
            })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        update_message_at(
            &test_client(),
            &format!("{}/chats/chat-id/messages/message-id", server.uri()),
            &edit_request(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn update_channel_message_accepts_no_content() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path(
                "/teams/team-id/channels/channel-id/messages/message-id",
            ))
            .and(body_json(serde_json::json!({
                "body": { "contentType": "text", "content": "corrected text" }
            })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        update_message_at(
            &test_client(),
            &format!(
                "{}/teams/team-id/channels/channel-id/messages/message-id",
                server.uri()
            ),
            &edit_request(),
        )
        .await
        .unwrap();
    }

    /// Editing someone else's message, or one in a chat the caller cannot
    /// write to, must surface as a permission error rather than a parse error.
    #[tokio::test]
    async fn update_message_reports_permission_denied() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": { "code": "Forbidden", "message": "Insufficient privileges" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let err = update_message_at(
            &test_client(),
            &format!("{}/chats/chat-id/messages/message-id", server.uri()),
            &edit_request(),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, TeamsError::PermissionDenied(_)), "{err:?}");
    }

    #[tokio::test]
    async fn update_message_reports_missing_message() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": { "code": "NotFound", "message": "Message not found" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let err = update_message_at(
            &test_client(),
            &format!("{}/chats/chat-id/messages/message-id", server.uri()),
            &edit_request(),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, TeamsError::NotFound(_)), "{err:?}");
    }
}
