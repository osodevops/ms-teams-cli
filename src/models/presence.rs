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
    pub expiry_date_time: Option<DateTimeTimeZone>,
    /// Read-only, and Graph sends it only for `GET /me/presence`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_date_time: Option<String>,
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

/// Request body for POST /me/presence/clearPresence
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearPresenceRequest {
    pub session_id: String,
}

/// Request body for POST /me/presence/setUserPreferredPresence. A preferred presence belongs to
/// the user rather than to an application's session, so there is no `sessionId`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetUserPreferredPresenceRequest {
    pub availability: String,
    pub activity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_duration: Option<String>,
}

/// Request body for POST /me/presence/clearUserPreferredPresence, which Graph documents as an
/// empty JSON object. A braced struct serializes to `{}`; a unit struct would go out as `null`.
#[derive(Debug, Clone, Serialize)]
pub struct ClearUserPreferredPresenceRequest {}

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
    pub expiry_date_time: Option<DateTimeTimeZone>,
}

/// Microsoft Graph `dateTimeTimeZone`, sent on `setStatusMessage` and returned
/// by `GET /me/presence`. Both components are optional on the read path: a
/// response that omits one is worth surfacing as `null` rather than failing the
/// whole command, which is the defect this type was introduced to fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
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
        assert_eq!(
            expiry.date_time.as_deref(),
            Some("9999-12-31T00:00:00.0000000")
        );
        assert_eq!(expiry.time_zone.as_deref(), Some("UTC"));
    }

    /// Graph documents `publishedDateTime` on `presenceStatusMessage`, and sends it
    /// for `GET /me/presence`. It used to be dropped on the floor by serde.
    #[test]
    fn presence_keeps_the_status_message_published_date_time() {
        let json = r#"{
            "id": "user-1",
            "availability": "Away",
            "activity": "Away",
            "statusMessage": {
                "message": { "content": "Back on Monday", "contentType": "text" },
                "publishedDateTime": "2026-08-27T09:14:22.9411568Z"
            }
        }"#;
        let p: Presence = serde_json::from_str(json).unwrap();
        let status = p.status_message.expect("statusMessage");
        assert_eq!(
            status.published_date_time.as_deref(),
            Some("2026-08-27T09:14:22.9411568Z")
        );
        assert!(status.expiry_date_time.is_none());
        // and it survives back out to the JSON envelope the agent contract promises
        let out = serde_json::to_value(&status).unwrap();
        assert_eq!(out["publishedDateTime"], "2026-08-27T09:14:22.9411568Z");
    }

    /// A response that carries only part of the object still yields a record rather
    /// than failing the whole command, which is the failure mode #69 was.
    #[test]
    fn a_partial_expiry_object_does_not_fail_the_read() {
        let json = r#"{
            "statusMessage": { "expiryDateTime": { "dateTime": "2026-09-01T08:00:00.0000000" } }
        }"#;
        let p: Presence = serde_json::from_str(json).unwrap();
        let expiry = p
            .status_message
            .expect("statusMessage")
            .expiry_date_time
            .expect("expiryDateTime");
        assert_eq!(
            expiry.date_time.as_deref(),
            Some("2026-09-01T08:00:00.0000000")
        );
        assert!(expiry.time_zone.is_none());
    }

    /// The write path is the reason both components exist; optional fields on the
    /// shared type must not let `setStatusMessage` go out missing one.
    #[test]
    fn set_status_message_sends_both_expiry_components() {
        let req = SetStatusMessageRequest {
            status_message: SetStatusMessageBody {
                message: StatusMessageContent {
                    content: Some("Back on Monday".to_string()),
                    content_type: Some("text".to_string()),
                },
                expiry_date_time: Some(DateTimeTimeZone {
                    date_time: Some("2026-09-01T08:00:00".to_string()),
                    time_zone: Some("UTC".to_string()),
                }),
            },
        };
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            serde_json::json!({
                "statusMessage": {
                    "message": { "content": "Back on Monday", "contentType": "text" },
                    "expiryDateTime": {
                        "dateTime": "2026-09-01T08:00:00",
                        "timeZone": "UTC"
                    }
                }
            })
        );
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

    #[test]
    fn clear_presence_request_sends_camel_case_session_id() {
        let req = ClearPresenceRequest {
            session_id: "app-id".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            serde_json::json!({"sessionId": "app-id"})
        );
    }

    /// Graph applies its own default when no expiration is sent, so the key must be absent rather
    /// than `null`.
    #[test]
    fn set_user_preferred_presence_request_omits_an_absent_expiration() {
        let req = SetUserPreferredPresenceRequest {
            availability: "Available".to_string(),
            activity: "Available".to_string(),
            expiration_duration: None,
        };
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            serde_json::json!({"availability": "Available", "activity": "Available"})
        );
    }

    #[test]
    fn clear_user_preferred_presence_request_is_an_empty_object() {
        assert_eq!(
            serde_json::to_value(ClearUserPreferredPresenceRequest {}).unwrap(),
            serde_json::json!({})
        );
    }
}
