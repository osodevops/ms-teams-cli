use serde::{Deserialize, Serialize};

/// Request to send an activity notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendActivityNotificationRequest {
    pub topic: ActivityTopic,
    pub activity_type: String,
    pub preview_text: PreviewText,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<ActivityRecipient>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub template_parameters: Vec<KeyValuePair>,
}

/// Topic for the notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTopic {
    pub source: String,
    pub value: String,
}

/// Preview text shown in the notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewText {
    pub content: String,
}

/// Recipient of the notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecipient {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    pub user_id: String,
}

impl ActivityRecipient {
    pub fn user(user_id: String) -> Self {
        Self {
            odata_type: "#microsoft.graph.aadUserNotificationRecipient".to_string(),
            user_id,
        }
    }
}

/// Key-value pair for template parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValuePair {
    pub name: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_activity_notification_request_serializes() {
        let req = SendActivityNotificationRequest {
            topic: ActivityTopic {
                source: "text".into(),
                value: "New update".into(),
            },
            activity_type: "taskCreated".into(),
            preview_text: PreviewText {
                content: "A new task was created".into(),
            },
            recipient: Some(ActivityRecipient::user("user123".into())),
            template_parameters: vec![KeyValuePair {
                name: "taskName".into(),
                value: "Fix bug".into(),
            }],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["activityType"], "taskCreated");
        assert_eq!(
            json["recipient"]["@odata.type"],
            "#microsoft.graph.aadUserNotificationRecipient"
        );
        assert_eq!(json["recipient"]["userId"], "user123");
        assert_eq!(json["topic"]["source"], "text");
    }
}
