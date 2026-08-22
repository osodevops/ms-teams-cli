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
    pub expiry_date_time: Option<SetStatusExpiry>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    fn presence_parses_an_object_valued_status_message_expiry() {
        let json = r#"{
            "id": "user-1",
            "availability": "Available",
            "activity": "Available",
            "statusMessage": {
                "message": { "content": "Back on Monday", "contentType": "text" },
                "expiryDateTime": {
                    "dateTime": "9999-12-31T00:00:00.0000000",
                    "timeZone": "UTC"
                }
            }
        }"#;
        let p: Presence = serde_json::from_str(json).unwrap();
        let expiry = p
            .status_message
            .expect("statusMessage")
            .expiry_date_time
            .expect("expiryDateTime");
        assert_eq!(expiry.date_time, "9999-12-31T00:00:00.0000000");
        assert_eq!(expiry.time_zone, "UTC");
    }

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
