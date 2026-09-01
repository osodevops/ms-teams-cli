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
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<ChatMessageFrom>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<ItemBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatMessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<ChatMessageReaction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<Vec<ChatMessageMention>>,
}

/// A reaction on a message. `reaction_type` is the unicode character (or a
/// legacy name such as `like` on older messages); `display_name` is Graph's
/// label for it, for example `Eyes` for 👀.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageReaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reaction_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<ChatMessageFrom>,
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

/// User identity within a message. `user_identity_type` is Graph's
/// `userIdentityType` (for example `aadUser`); it appears on mention
/// identities and is retained so read-backs keep the full identity shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_identity_type: Option<String>,
}

/// A Teams @mention: the numeric `id` must match an `<at id="N">` element in
/// the message body for Teams to render it as a real mention.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageMention {
    pub id: i32,
    pub mention_text: String,
    pub mentioned: ChatMessageMentioned,
}

/// Who a [`ChatMessageMention`] refers to. Only the `user` form is produced
/// or consumed by this CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageMentioned {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<ChatMessageUser>,
}

/// Request body for sending a message.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub body: ItemBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatMessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosted_contents: Option<Vec<HostedContentUpload>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<Vec<ChatMessageMention>>,
}

/// Write-side hosted content: inline image bytes riding a message create
/// call. The body HTML references it as `../hostedContents/{temporaryId}/$value`
/// and Graph rewrites that into a permanent URL on delivery.
#[derive(Debug, Clone, Serialize)]
pub struct HostedContentUpload {
    #[serde(rename = "@microsoft.graph.temporaryId")]
    pub temporary_id: String,
    #[serde(rename = "contentBytes")]
    pub content_bytes: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
}

/// Message attachment (e.g., adaptive card, file reference, code snippet).
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
    pub content_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams_app_id: Option<String>,
}

/// Inline media stored with a message (pasted screenshots, code snippets).
///
/// The list endpoint returns `contentBytes` and `contentType` as null; actual
/// bytes come from the per-item `/$value` endpoint and the real MIME type from
/// that response's Content-Type header.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageHostedContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_bytes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
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
            subject: None,
            from: Some(ChatMessageFrom {
                user: Some(ChatMessageUser {
                    id: Some("u1".into()),
                    display_name: Some("Alice".into()),
                    user_identity_type: None,
                }),
            }),
            body: Some(ItemBody {
                content_type: Some("text".into()),
                content: Some("Hello".into()),
            }),
            attachments: None,
            message_type: Some("message".into()),
            reactions: None,
            mentions: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.body.unwrap().content.as_deref(), Some("Hello"));
    }

    #[test]
    fn send_request_serializes_hosted_contents_with_temporary_id() {
        let req = SendMessageRequest {
            subject: None,
            body: ItemBody {
                content_type: Some("html".into()),
                content: Some(r#"<p><img src="../hostedContents/1/$value"></p>"#.into()),
            },
            attachments: None,
            hosted_contents: Some(vec![HostedContentUpload {
                temporary_id: "1".into(),
                content_bytes: "aVZCT1J3".into(),
                content_type: "image/png".into(),
            }]),
            mentions: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        let hc = &json["hostedContents"][0];
        assert_eq!(hc["@microsoft.graph.temporaryId"], "1");
        assert_eq!(hc["contentBytes"], "aVZCT1J3");
        assert_eq!(hc["contentType"], "image/png");
        assert!(json.get("attachments").is_none());
    }

    /// The wire shape Microsoft's v1.0 `chatMessageMention` contract expects;
    /// any deviation here and Graph strips the mention.
    #[test]
    fn send_request_serializes_the_exact_mention_shape() {
        let req = SendMessageRequest {
            subject: None,
            body: ItemBody {
                content_type: Some("html".into()),
                content: Some(r#"<at id="0">Sophie Daniels</at> Please review"#.into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: Some(vec![ChatMessageMention {
                id: 0,
                mention_text: "Sophie Daniels".into(),
                mentioned: ChatMessageMentioned {
                    user: Some(ChatMessageUser {
                        id: Some("32cbca05-dc05-454f-b0f3-072f331d4c97".into()),
                        display_name: Some("Sophie Daniels".into()),
                        user_identity_type: Some("aadUser".into()),
                    }),
                },
            }]),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "body": {
                    "contentType": "html",
                    "content": "<at id=\"0\">Sophie Daniels</at> Please review"
                },
                "mentions": [
                    {
                        "id": 0,
                        "mentionText": "Sophie Daniels",
                        "mentioned": {
                            "user": {
                                "id": "32cbca05-dc05-454f-b0f3-072f331d4c97",
                                "displayName": "Sophie Daniels",
                                "userIdentityType": "aadUser"
                            }
                        }
                    }
                ]
            })
        );
    }

    /// Mentions Graph returns on reads must survive a parse/print round trip
    /// instead of disappearing from JSON output.
    #[test]
    fn chat_message_roundtrips_returned_mentions() {
        let json = serde_json::json!({
            "id": "1700000000000",
            "body": {
                "contentType": "html",
                "content": "<at id=\"0\">Sophie Daniels</at> Please review"
            },
            "mentions": [
                {
                    "id": 0,
                    "mentionText": "Sophie Daniels",
                    "mentioned": {
                        "user": {
                            "id": "32cbca05-dc05-454f-b0f3-072f331d4c97",
                            "displayName": "Sophie Daniels",
                            "userIdentityType": "aadUser"
                        }
                    }
                }
            ]
        });
        let msg: ChatMessage = serde_json::from_value(json).unwrap();
        let mentions = msg.mentions.as_ref().unwrap();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].id, 0);
        assert_eq!(mentions[0].mention_text, "Sophie Daniels");
        let user = mentions[0].mentioned.user.as_ref().unwrap();
        assert_eq!(
            user.id.as_deref(),
            Some("32cbca05-dc05-454f-b0f3-072f331d4c97")
        );
        assert_eq!(user.display_name.as_deref(), Some("Sophie Daniels"));
        assert_eq!(user.user_identity_type.as_deref(), Some("aadUser"));

        let re = serde_json::to_value(&msg).unwrap();
        assert_eq!(re["mentions"][0]["id"], 0);
        assert_eq!(re["mentions"][0]["mentionText"], "Sophie Daniels");
        assert_eq!(
            re["mentions"][0]["mentioned"]["user"]["userIdentityType"],
            "aadUser"
        );
    }

    /// Graph returns `subject` on channel root messages; it must survive a
    /// parse/print round trip instead of being dropped from JSON output.
    #[test]
    fn chat_message_keeps_returned_subject() {
        let json = serde_json::json!({
            "id": "1700000000000",
            "subject": "Release plan",
            "body": { "contentType": "html", "content": "Team, details inside." }
        });
        let msg: ChatMessage = serde_json::from_value(json).unwrap();
        assert_eq!(msg.subject.as_deref(), Some("Release plan"));

        let re = serde_json::to_value(&msg).unwrap();
        assert_eq!(re["subject"], "Release plan");
    }

    /// Messages without a subject (every chat message, most replies) must not
    /// gain a `"subject": null` field on output.
    #[test]
    fn chat_message_without_subject_omits_the_field() {
        let json = serde_json::json!({
            "id": "1700000000001",
            "body": { "contentType": "text", "content": "hi" }
        });
        let msg: ChatMessage = serde_json::from_value(json).unwrap();
        assert!(msg.subject.is_none());

        let re = serde_json::to_value(&msg).unwrap();
        assert!(re.get("subject").is_none());
    }

    #[test]
    fn send_request_serializes_subject_at_top_level() {
        let req = SendMessageRequest {
            subject: Some("Release plan".into()),
            body: ItemBody {
                content_type: Some("text".into()),
                content: Some("Team, details inside.".into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subject"], "Release plan");
        assert_eq!(json["body"]["content"], "Team, details inside.");
    }

    #[test]
    fn reference_attachment_keeps_content_url() {
        let json = r#"{
            "id": "C0F75B79-7D00-4DC9-918F-5FAEDD1086A4",
            "contentType": "reference",
            "contentUrl": "https://tenant-my.sharepoint.com/personal/user/Documents/NetskopeLogs.zip",
            "content": null,
            "name": "NetskopeLogs.zip",
            "thumbnailUrl": null,
            "teamsAppId": null
        }"#;
        let att: ChatMessageAttachment = serde_json::from_str(json).unwrap();
        assert_eq!(att.content_type.as_deref(), Some("reference"));
        assert_eq!(
            att.content_url.as_deref(),
            Some("https://tenant-my.sharepoint.com/personal/user/Documents/NetskopeLogs.zip")
        );
        assert_eq!(att.name.as_deref(), Some("NetskopeLogs.zip"));
    }

    #[test]
    fn hosted_content_list_entry_has_null_bytes() {
        let json = r#"{"id": "aWQ9eF8wLWZyY2E=", "contentBytes": null, "contentType": null}"#;
        let hc: ChatMessageHostedContent = serde_json::from_str(json).unwrap();
        assert_eq!(hc.id.as_deref(), Some("aWQ9eF8wLWZyY2E="));
        assert!(hc.content_bytes.is_none());
        assert!(hc.content_type.is_none());
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
