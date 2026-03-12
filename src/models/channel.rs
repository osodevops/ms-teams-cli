use serde::{Deserialize, Serialize};

/// Microsoft Graph Channel resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Request body for creating a new channel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelCreateRequest {
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_type: Option<String>,
}

/// Request body for updating a channel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_roundtrip() {
        let ch = Channel {
            id: Some("c1".into()),
            display_name: Some("General".into()),
            description: None,
            membership_type: Some("standard".into()),
            email: Some("general@example.com".into()),
        };
        let json = serde_json::to_string(&ch).unwrap();
        let parsed: Channel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name.as_deref(), Some("General"));
    }
}
