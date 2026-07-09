use serde::Serialize;

use crate::models::message::{ChatMessage, ChatMessageHostedContent};

/// What a downloadable (or at least enumerable) piece of message content is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Inline media pasted into the message (screenshots, GIFs).
    HostedContent,
    /// File attached via SharePoint/OneDrive (`contentType: "reference"`).
    FileReference,
    /// Code block; its body is a hosted content behind `codeSnippetUrl`.
    CodeSnippet,
    /// Adaptive/hero/connector card; JSON is inline in the message.
    Card,
    /// Quoted or forwarded message context; inline in the message.
    MessageReference,
    /// Unrecognized attachment content type.
    Other,
}

impl ItemKind {
    pub fn downloadable(self) -> bool {
        matches!(
            self,
            Self::HostedContent | Self::FileReference | Self::CodeSnippet
        )
    }
}

/// One entry in the unified inventory of a message's attachments and inline
/// images, indexed stably so callers can request downloads by number.
#[derive(Debug, Clone, Serialize)]
pub struct AttachmentItem {
    pub index: usize,
    pub kind: ItemKind,
    pub downloadable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosted_content_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_snippet_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<String>,
}

impl AttachmentItem {
    fn new(index: usize, kind: ItemKind) -> Self {
        Self {
            index,
            kind,
            downloadable: kind.downloadable(),
            hosted_content_id: None,
            attachment_id: None,
            name: None,
            content_type: None,
            content_url: None,
            code_snippet_url: None,
            language: None,
            alt: None,
            width: None,
            height: None,
        }
    }
}

/// Build the unified inventory for a message from its body HTML, the
/// hostedContents collection, and the attachments array.
///
/// The body HTML is the source of truth for inline-image order and display
/// metadata; the hostedContents collection is authoritative for existence
/// (its `contentBytes` are always null, so bytes come later via `/$value`).
/// Hosted contents referenced by a code-snippet attachment's `codeSnippetUrl`
/// are folded into that snippet's entry rather than listed twice.
pub fn build_inventory(
    message: &ChatMessage,
    hosted: &[ChatMessageHostedContent],
) -> Vec<AttachmentItem> {
    let body_html = message
        .body
        .as_ref()
        .and_then(|b| b.content.as_deref())
        .unwrap_or_default();
    let body_images = hosted_images_in_body(body_html);

    let snippet_hosted_ids: Vec<String> = message
        .attachments
        .iter()
        .flatten()
        .filter_map(|a| {
            let content = a.content.as_deref()?;
            let url = code_snippet_url(content)?;
            hosted_content_id_from_url(&url)
        })
        .collect();

    let mut items = Vec::new();

    // Inline images first, in body order, joined to the authoritative list.
    for img in &body_images {
        if snippet_hosted_ids.contains(&img.hosted_content_id) {
            continue;
        }
        let mut item = AttachmentItem::new(items.len(), ItemKind::HostedContent);
        item.hosted_content_id = Some(img.hosted_content_id.clone());
        item.alt = img.alt.clone();
        item.width = img.width.clone();
        item.height = img.height.clone();
        items.push(item);
    }

    // Hosted contents not visible in the body (defensive; rare in practice).
    for hc in hosted {
        let Some(id) = hc.id.as_deref() else { continue };
        if snippet_hosted_ids.iter().any(|s| s == id)
            || body_images.iter().any(|i| i.hosted_content_id == id)
        {
            continue;
        }
        let mut item = AttachmentItem::new(items.len(), ItemKind::HostedContent);
        item.hosted_content_id = Some(id.to_string());
        item.content_type = hc.content_type.clone();
        items.push(item);
    }

    for att in message.attachments.iter().flatten() {
        let kind = match att.content_type.as_deref() {
            Some("reference") => ItemKind::FileReference,
            Some("application/vnd.microsoft.card.codesnippet") => ItemKind::CodeSnippet,
            Some(t) if t.starts_with("application/vnd.microsoft.card.") => ItemKind::Card,
            Some("messageReference") | Some("forwardedMessageReference") => {
                ItemKind::MessageReference
            }
            _ => ItemKind::Other,
        };
        let mut item = AttachmentItem::new(items.len(), kind);
        item.attachment_id = att.id.clone();
        item.name = att.name.clone();
        item.content_type = att.content_type.clone();
        item.content_url = att.content_url.clone();
        if kind == ItemKind::CodeSnippet {
            if let Some(content) = att.content.as_deref() {
                item.code_snippet_url = code_snippet_url(content);
                item.language = snippet_language(content);
            }
        }
        items.push(item);
    }

    items
}

struct BodyImage {
    hosted_content_id: String,
    alt: Option<String>,
    width: Option<String>,
    height: Option<String>,
}

/// Extract hosted-content `<img>` references from message body HTML, in
/// document order. Only images whose src is a Graph hostedContents `$value`
/// URL are returned; external or data-URI images are not hosted contents.
fn hosted_images_in_body(html: &str) -> Vec<BodyImage> {
    let mut images = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find("<img") {
        rest = &rest[pos..];
        let end = match rest.find('>') {
            Some(e) => e,
            None => break,
        };
        let tag = &rest[..end];
        rest = &rest[end..];

        let Some(src) = attr_value(tag, "src") else {
            continue;
        };
        let Some(id) = hosted_content_id_from_url(&src) else {
            continue;
        };
        images.push(BodyImage {
            hosted_content_id: id,
            alt: attr_value(tag, "alt"),
            width: attr_value(tag, "width"),
            height: attr_value(tag, "height"),
        });
    }
    images
}

/// Pull the hosted-content ID out of a Graph `.../hostedContents/{id}/$value`
/// URL, percent-decoding it back to the raw base64 form used by the API.
fn hosted_content_id_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("graph.microsoft.com") {
        return None;
    }
    let mut segments = parsed
        .path_segments()?
        .skip_while(|s| *s != "hostedContents");
    segments.next()?;
    let id = segments.next().filter(|s| !s.is_empty())?;
    if segments.next() != Some("$value") || segments.next().is_some() {
        return None;
    }
    Some(
        urlencoding::decode(id)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| id.to_string()),
    )
}

/// Read a double-quoted HTML attribute value from a single tag's text.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn code_snippet_url(content_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(content_json).ok()?;
    value
        .get("codeSnippetUrl")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn snippet_language(content_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(content_json).ok()?;
    value
        .get("language")
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::{ChatMessageAttachment, ItemBody};

    fn message_with(body_html: &str, attachments: Vec<ChatMessageAttachment>) -> ChatMessage {
        ChatMessage {
            id: Some("1783503421261".into()),
            created_date_time: None,
            from: None,
            body: Some(ItemBody {
                content_type: Some("html".into()),
                content: Some(body_html.into()),
            }),
            attachments: Some(attachments),
            message_type: Some("message".into()),
        }
    }

    fn hosted(id: &str) -> ChatMessageHostedContent {
        ChatMessageHostedContent {
            id: Some(id.into()),
            content_bytes: None,
            content_type: None,
        }
    }

    // Captured from a live channel message containing a pasted screenshot.
    const REAL_BODY: &str = r#"<p>Hey, it says on Conductor one I have access to design drive but I still can't get in. Task #64674</p>
<p><img src="https://graph.microsoft.com/v1.0/teams/5c42feaf-58be-42d3-9040-f35698cfdb0f/channels/19:b8fe94da69f446f38066d16443895c91@thread.skype/messages/1783503421261/hostedContents/aWQ9eF8wLWZyY2EtZDE0LTJhMjE5ODZiZTFkOTNhZGI3ZGE2YjA4ZTE3YmViODM3LHR5cGU9MSx1cmw9aHR0cHM6Ly9ldS1hcGkuYXNtLnNreXBlLmNvbS92MS9vYmplY3RzLzAtZnJjYS1kMTQtMmEyMTk4NmJlMWQ5M2FkYjdkYTZiMDhlMTdiZWI4Mzcvdmlld3MvaW1nbw==/$value" width="410" height="204" alt="image" itemid="0-frca-d14-2a21986be1d93adb7da6b08e17beb837"></p>"#;

    const REAL_HOSTED_ID: &str = "aWQ9eF8wLWZyY2EtZDE0LTJhMjE5ODZiZTFkOTNhZGI3ZGE2YjA4ZTE3YmViODM3LHR5cGU9MSx1cmw9aHR0cHM6Ly9ldS1hcGkuYXNtLnNreXBlLmNvbS92MS9vYmplY3RzLzAtZnJjYS1kMTQtMmEyMTk4NmJlMWQ5M2FkYjdkYTZiMDhlMTdiZWI4Mzcvdmlld3MvaW1nbw==";

    #[test]
    fn inline_screenshot_is_inventoried_from_real_payload() {
        let msg = message_with(REAL_BODY, vec![]);
        let items = build_inventory(&msg, &[hosted(REAL_HOSTED_ID)]);
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.kind, ItemKind::HostedContent);
        assert!(item.downloadable);
        assert_eq!(item.hosted_content_id.as_deref(), Some(REAL_HOSTED_ID));
        assert_eq!(item.alt.as_deref(), Some("image"));
        assert_eq!(item.width.as_deref(), Some("410"));
        assert_eq!(item.height.as_deref(), Some("204"));
    }

    #[test]
    fn file_reference_attachment_is_inventoried() {
        let att = ChatMessageAttachment {
            id: Some("C0F75B79".into()),
            content_type: Some("reference".into()),
            content: None,
            content_url: Some(
                "https://tenant-my.sharepoint.com/personal/u/Documents/NetskopeLogs.zip".into(),
            ),
            name: Some("NetskopeLogs.zip".into()),
            thumbnail_url: None,
            teams_app_id: None,
        };
        let msg = message_with("<p>logs attached</p>", vec![att]);
        let items = build_inventory(&msg, &[]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::FileReference);
        assert!(items[0].downloadable);
        assert_eq!(items[0].name.as_deref(), Some("NetskopeLogs.zip"));
        assert!(items[0].content_url.as_deref().unwrap().ends_with(".zip"));
    }

    #[test]
    fn code_snippet_folds_its_hosted_content() {
        let snippet_url = "https://graph.microsoft.com/v1.0/chats/19:abc/messages/m1/hostedContents/c25pcHBldA==/$value";
        let att = ChatMessageAttachment {
            id: Some("snippet1".into()),
            content_type: Some("application/vnd.microsoft.card.codesnippet".into()),
            content: Some(format!(
                r#"{{"language":"Rust","codeSnippetUrl":"{snippet_url}"}}"#
            )),
            content_url: None,
            name: None,
            thumbnail_url: None,
            teams_app_id: None,
        };
        let msg = message_with("<p>code</p>", vec![att]);
        // The snippet's hosted content also appears in the hostedContents list;
        // it must not be double-counted as a standalone inline image.
        let items = build_inventory(&msg, &[hosted("c25pcHBldA==")]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::CodeSnippet);
        assert!(items[0].downloadable);
        assert_eq!(items[0].language.as_deref(), Some("Rust"));
        assert_eq!(items[0].code_snippet_url.as_deref(), Some(snippet_url));
    }

    #[test]
    fn cards_and_message_references_are_listed_but_not_downloadable() {
        let card = ChatMessageAttachment {
            id: Some("card1".into()),
            content_type: Some("application/vnd.microsoft.card.adaptive".into()),
            content: Some("{}".into()),
            content_url: None,
            name: None,
            thumbnail_url: None,
            teams_app_id: None,
        };
        let quoted = ChatMessageAttachment {
            id: Some("ref1".into()),
            content_type: Some("messageReference".into()),
            content: Some("{}".into()),
            content_url: None,
            name: None,
            thumbnail_url: None,
            teams_app_id: None,
        };
        let msg = message_with("<p>hi</p>", vec![card, quoted]);
        let items = build_inventory(&msg, &[]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, ItemKind::Card);
        assert!(!items[0].downloadable);
        assert_eq!(items[1].kind, ItemKind::MessageReference);
        assert!(!items[1].downloadable);
    }

    #[test]
    fn external_and_data_uri_images_are_ignored() {
        let body = r#"<img src="https://media.giphy.com/x.gif"><img src="data:image/png;base64,AAAA"><img src="https://evil.example.com/graph.microsoft.com/v1.0/chats/c/messages/m/hostedContents/x/$value">"#;
        let msg = message_with(body, vec![]);
        assert!(build_inventory(&msg, &[]).is_empty());
    }

    #[test]
    fn unlisted_hosted_content_still_appears() {
        let msg = message_with("<p>no images in body</p>", vec![]);
        let items = build_inventory(&msg, &[hosted("b3JwaGFu")]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::HostedContent);
        assert_eq!(items[0].hosted_content_id.as_deref(), Some("b3JwaGFu"));
    }

    #[test]
    fn empty_message_yields_empty_inventory() {
        let msg = message_with("", vec![]);
        assert!(build_inventory(&msg, &[]).is_empty());
    }

    #[test]
    fn percent_encoded_ids_are_decoded() {
        let url = "https://graph.microsoft.com/v1.0/teams/t/channels/c/messages/m/hostedContents/aWQ9%2Bx%2Fz%3D%3D/$value";
        assert_eq!(
            hosted_content_id_from_url(url).as_deref(),
            Some("aWQ9+x/z==")
        );
    }
}
