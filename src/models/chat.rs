use serde::{Deserialize, Serialize};

/// Microsoft Graph Chat resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_date_time: Option<String>,
}

/// Request body for creating a new chat.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCreateRequest {
    pub chat_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    pub members: Vec<crate::models::member::AddMemberRequest>,
}

/// Request body for updating a chat.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

/// Request body for hide/unhide chat.
#[derive(Debug, Clone, Serialize)]
pub struct ChatUserAction {
    pub user: ChatUserRef,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatUserRef {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_roundtrip() {
        let chat = Chat {
            id: Some("chat1".into()),
            topic: Some("Project X".into()),
            chat_type: Some("group".into()),
            last_updated_date_time: Some("2024-01-01T00:00:00Z".into()),
        };
        let json = serde_json::to_string(&chat).unwrap();
        let parsed: Chat = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.topic.as_deref(), Some("Project X"));
    }
}
