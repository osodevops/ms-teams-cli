use serde::{Deserialize, Serialize};

/// Conversation member — reused across team, channel, and chat member operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// How far back the member can see the chat's history, when Graph returns it.
    /// It can be set only for chat members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_history_start_date_time: Option<String>,
}

/// Request body for adding a member to a team, channel, or chat.
#[derive(Debug, Clone, Serialize)]
pub struct AddMemberRequest {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "user@odata.bind")]
    pub user_odata_bind: String,
    pub roles: Vec<String>,
    /// How far back the chat's history is visible to the new member. Graph
    /// accepts it only for chat members; the team and channel paths never set
    /// it. Left out, the member sees no history from before they were added.
    #[serde(
        rename = "visibleHistoryStartDateTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub visible_history_start_date_time: Option<String>,
}

/// Graph's `visibleHistoryStartDateTime` value for "all history".
pub const ALL_HISTORY: &str = "0001-01-01T00:00:00Z";

impl AddMemberRequest {
    pub fn new(user_id: &str, roles: Vec<String>) -> Self {
        Self {
            odata_type: "#microsoft.graph.aadUserConversationMember".to_string(),
            user_odata_bind: format!("https://graph.microsoft.com/v1.0/users('{user_id}')"),
            roles,
            visible_history_start_date_time: None,
        }
    }

    /// Shares the chat's history from `start` (an RFC 3339 timestamp, or
    /// [`ALL_HISTORY`]) with the new member. Only for chat members.
    pub fn with_visible_history(mut self, start: impl Into<String>) -> Self {
        self.visible_history_start_date_time = Some(start.into());
        self
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
        assert!(json.get("visibleHistoryStartDateTime").is_none());
    }

    #[test]
    fn add_member_request_shares_all_history() {
        let req = AddMemberRequest::new("user-123", vec![]).with_visible_history(ALL_HISTORY);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["visibleHistoryStartDateTime"], "0001-01-01T00:00:00Z");
    }

    #[test]
    fn add_member_request_shares_history_from_a_timestamp() {
        let req = AddMemberRequest::new("user-123", vec![])
            .with_visible_history("2026-11-01T09:30:00+01:00");
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(
            json["visibleHistoryStartDateTime"],
            "2026-11-01T09:30:00+01:00"
        );
    }

    #[test]
    fn conversation_member_deserializes() {
        let json = serde_json::json!({
            "id": "m1",
            "displayName": "Alice",
            "roles": ["owner"],
            "userId": "u1",
            "tenantId": "tenant-1",
            "email": "alice@example.com"
        });
        let member: ConversationMember = serde_json::from_value(json).unwrap();
        assert_eq!(member.tenant_id.as_deref(), Some("tenant-1"));
        assert_eq!(member.display_name.as_deref(), Some("Alice"));
        assert_eq!(member.roles.as_ref().unwrap()[0], "owner");
    }
}
