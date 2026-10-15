use serde::{Deserialize, Serialize};

/// Microsoft Graph Subscription resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_notification_url: Option<String>,
}

/// Request body for creating a new subscription.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionRequest {
    pub change_type: String,
    pub notification_url: String,
    pub resource: String,
    pub expiration_date_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_notification_url: Option<String>,
}

/// Request body for renewing a subscription.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenewSubscriptionRequest {
    pub expiration_date_time: String,
}

/// Collection of change notifications from a webhook callback.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeNotificationCollection {
    pub value: Vec<ChangeNotification>,
}

/// A single change notification from Microsoft Graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeNotification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_event: Option<String>,
    /// Fields not modelled above, kept so output carries everything Graph sent.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl ChangeNotification {
    /// Output form: every field Graph sent except `clientState`, plus `kind`
    /// (`lifecycle` when `lifecycleEvent` is present, else `change`).
    pub fn to_output(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        if let Some(map) = value.as_object_mut() {
            map.remove("clientState");
            let kind = if self.lifecycle_event.is_some() {
                "lifecycle"
            } else {
                "change"
            };
            map.insert("kind".into(), kind.into());
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_roundtrip() {
        let sub = Subscription {
            id: Some("sub-1".into()),
            resource: Some("/teams/all/messages".into()),
            change_type: Some("created".into()),
            notification_url: Some("https://example.com/webhook".into()),
            expiration_date_time: Some("2026-03-15T00:00:00Z".into()),
            client_state: Some("my-secret".into()),
            lifecycle_notification_url: None,
        };
        let json = serde_json::to_string(&sub).unwrap();
        let parsed: Subscription = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id.as_deref(), Some("sub-1"));
        assert_eq!(parsed.resource.as_deref(), Some("/teams/all/messages"));
    }

    #[test]
    fn create_subscription_request_serializes() {
        let req = CreateSubscriptionRequest {
            change_type: "created".into(),
            notification_url: "https://example.com/webhook".into(),
            resource: "/teams/all/messages".into(),
            expiration_date_time: "2026-03-15T00:00:00Z".into(),
            client_state: Some("secret".into()),
            lifecycle_notification_url: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["changeType"], "created");
        assert_eq!(json["notificationUrl"], "https://example.com/webhook");
        assert_eq!(json["clientState"], "secret");
        assert!(json.get("lifecycleNotificationUrl").is_none());
    }

    #[test]
    fn create_subscription_request_sends_lifecycle_url() {
        let req = CreateSubscriptionRequest {
            change_type: "created".into(),
            notification_url: "https://example.com/webhook".into(),
            resource: "/teams/all/messages".into(),
            expiration_date_time: "2026-03-15T00:00:00Z".into(),
            client_state: None,
            lifecycle_notification_url: Some("https://example.com/lifecycle".into()),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(
            json["lifecycleNotificationUrl"],
            "https://example.com/lifecycle"
        );
    }

    #[test]
    fn change_notification_deserializes() {
        let json_str = r#"{
            "subscriptionId": "sub-1",
            "changeType": "created",
            "resource": "/teams/abc/channels/def/messages/123",
            "tenantId": "tenant-1",
            "clientState": "my-secret"
        }"#;
        let notif: ChangeNotification = serde_json::from_str(json_str).unwrap();
        assert_eq!(notif.subscription_id.as_deref(), Some("sub-1"));
        assert_eq!(notif.change_type.as_deref(), Some("created"));
        assert_eq!(notif.tenant_id.as_deref(), Some("tenant-1"));
    }

    #[test]
    fn change_notification_output_keeps_unknown_fields_and_drops_client_state() {
        let notif: ChangeNotification = serde_json::from_str(
            r#"{
                "subscriptionId": "sub-1",
                "changeType": "created",
                "clientState": "my-secret",
                "subscriptionExpirationDateTime": "2026-03-15T00:00:00Z",
                "encryptedContent": {"data": "abc"}
            }"#,
        )
        .unwrap();
        let out = notif.to_output();
        assert_eq!(out["kind"], "change");
        assert_eq!(out["subscriptionId"], "sub-1");
        assert_eq!(
            out["subscriptionExpirationDateTime"],
            "2026-03-15T00:00:00Z"
        );
        assert_eq!(out["encryptedContent"]["data"], "abc");
        assert!(out.get("clientState").is_none());
    }

    #[test]
    fn lifecycle_notification_output_is_kind_lifecycle() {
        let notif: ChangeNotification = serde_json::from_str(
            r#"{"subscriptionId": "sub-1", "lifecycleEvent": "reauthorizationRequired"}"#,
        )
        .unwrap();
        let out = notif.to_output();
        assert_eq!(out["kind"], "lifecycle");
        assert_eq!(out["lifecycleEvent"], "reauthorizationRequired");
    }
}
