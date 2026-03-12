use serde::{Deserialize, Serialize};

/// Microsoft Graph ChatMessage resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<ChatMessageFrom>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<ItemBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatMessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
}

/// Message body with content type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Sender identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageFrom {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<ChatMessageUser>,
}

/// User identity within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Request body for sending a message.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub body: ItemBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatMessageAttachment>>,
}

/// Message attachment (e.g., adaptive card).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageAttachment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Request body for setting/unsetting a reaction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionRequest {
    pub reaction_type: String,
}

/// Pinned message info returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ChatMessage>,
}

/// Request body for pinning a message.
#[derive(Debug, Clone, Serialize)]
pub struct PinMessageRequest {
    #[serde(rename = "message@odata.bind")]
    pub message_odata_bind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_roundtrip() {
        let msg = ChatMessage {
            id: Some("msg1".into()),
            created_date_time: Some("2024-01-01T00:00:00Z".into()),
            from: Some(ChatMessageFrom {
                user: Some(ChatMessageUser {
                    id: Some("u1".into()),
                    display_name: Some("Alice".into()),
                }),
            }),
            body: Some(ItemBody {
                content_type: Some("text".into()),
                content: Some("Hello".into()),
            }),
            attachments: None,
            message_type: Some("message".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.body.unwrap().content.as_deref(), Some("Hello"));
    }

    #[test]
    fn reaction_request_serializes() {
        let req = ReactionRequest {
            reaction_type: "like".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["reactionType"], "like");
    }
}
