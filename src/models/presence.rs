use serde::{Deserialize, Serialize};

/// Microsoft Graph Presence resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<PresenceStatusMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceStatusMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<StatusMessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusMessageContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

/// Request body for POST /me/presence/setPresence
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPresenceRequest {
    pub session_id: String,
    pub availability: String,
    pub activity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_duration: Option<String>,
}

/// Request body for POST /me/presence/setStatusMessage
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStatusMessageRequest {
    pub status_message: SetStatusMessageBody,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStatusMessageBody {
    pub message: StatusMessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date_time: Option<SetStatusExpiry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStatusExpiry {
    pub date_time: String,
    pub time_zone: String,
}

/// Request body for batch presence lookup
#[derive(Debug, Clone, Serialize)]
pub struct GetPresenceBatchRequest {
    pub ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_serde_round_trip() {
        let json = r#"{
            "id": "user-1",
            "availability": "Available",
            "activity": "Available",
            "statusMessage": null
        }"#;
        let p: Presence = serde_json::from_str(json).unwrap();
        assert_eq!(p.availability.as_deref(), Some("Available"));
        let serialized = serde_json::to_string(&p).unwrap();
        assert!(serialized.contains("Available"));
    }
}
