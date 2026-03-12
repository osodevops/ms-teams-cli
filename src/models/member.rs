use serde::{Deserialize, Serialize};

/// Conversation member — reused across team, channel, and chat member operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMember {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<String>>,
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Request body for adding a member to a team, channel, or chat.
#[derive(Debug, Clone, Serialize)]
pub struct AddMemberRequest {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "user@odata.bind")]
    pub user_odata_bind: String,
    pub roles: Vec<String>,
}

impl AddMemberRequest {
    pub fn new(user_id: &str, roles: Vec<String>) -> Self {
        Self {
            odata_type: "#microsoft.graph.aadUserConversationMember".to_string(),
            user_odata_bind: format!("https://graph.microsoft.com/v1.0/users('{user_id}')"),
            roles,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_member_request_serializes_correctly() {
        let req = AddMemberRequest::new("user-123", vec!["owner".to_string()]);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(
            json["@odata.type"],
            "#microsoft.graph.aadUserConversationMember"
        );
        assert_eq!(
            json["user@odata.bind"],
            "https://graph.microsoft.com/v1.0/users('user-123')"
        );
        assert_eq!(json["roles"][0], "owner");
    }

    #[test]
    fn conversation_member_deserializes() {
        let json = serde_json::json!({
            "id": "m1",
            "displayName": "Alice",
            "roles": ["owner"],
            "userId": "u1",
            "email": "alice@example.com"
        });
        let member: ConversationMember = serde_json::from_value(json).unwrap();
        assert_eq!(member.display_name.as_deref(), Some("Alice"));
        assert_eq!(member.roles.as_ref().unwrap()[0], "owner");
    }
}
