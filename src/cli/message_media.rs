use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::error::{Result, TeamsError};
use crate::models::file::DriveRecipient;
use crate::models::member::ConversationMember;
use crate::models::message::{ChatMessageAttachment, HostedContentUpload, SendMessageRequest};

/// Where `--attach` files get uploaded before the message references them.
/// Chats use the sender's own OneDrive; channels use the team's SharePoint
/// library — which is why the two need different Files scopes (see
/// docs/attachments-spec.md).
pub enum AttachDestination<'a> {
    Chat {
        chat_id: &'a str,
    },
    Channel {
        team_id: &'a str,
        channel_id: &'a str,
    },
}

/// Per-image cap: hosted contents ride a single JSON request with a 4MB Graph
/// limit, and base64 inflates payloads by a third. Use decimal 3 MB so the
/// encoded image plus JSON wrapper stays below 4 MiB.
const MAX_INLINE_IMAGE_SIZE: usize = 3_000_000;

/// Aggregate cap across all `--image` files: every image's base64 string is
/// embedded in the same message-create body, so the encoded total — not just
/// each file — must fit Graph's 4MB request limit.
const MAX_INLINE_TOTAL_ENCODED: usize = 4_000_000;

/// Extend a send request with inline images (`--image`, write-side hosted
/// contents) and file attachments (`--attach`, drive upload + reference).
pub async fn apply_media(
    client: &GraphClient,
    req: &mut SendMessageRequest,
    images: &[String],
    attaches: &[String],
    dest: AttachDestination<'_>,
) -> Result<()> {
    if images.is_empty() && attaches.is_empty() {
        return Ok(());
    }

    ensure_html_body(req);
    let mut body = req.body.content.take().unwrap_or_default();
    let mut attachments = req.attachments.take().unwrap_or_default();

    let (images_html, hosted) = inline_images(images)?;
    body.push_str(&images_html);

    // Chat uploads land in the sender's own OneDrive, where nobody else can
    // read them until they are shared — look the chat's members up once so
    // each uploaded file can be shared with them below.
    let chat_recipients = match dest {
        AttachDestination::Chat { chat_id } if !attaches.is_empty() => {
            Some(chat_recipients(client, chat_id).await?)
        }
        _ => None,
    };

    for path in attaches {
        let (bytes, content_type, filename) = read_attachment(path)?;
        let item = match dest {
            AttachDestination::Chat { .. } => {
                let item =
                    api::files::upload_chat_attachment(client, &filename, bytes, &content_type)
                        .await?;
                if let Some(recipients) = &chat_recipients {
                    share_with_chat(client, &item, &filename, recipients).await;
                }
                item
            }
            AttachDestination::Channel {
                team_id,
                channel_id,
            } => {
                api::files::upload_channel_attachment(
                    client,
                    team_id,
                    channel_id,
                    &filename,
                    bytes,
                    &content_type,
                )
                .await?
            }
        };
        let attachment = reference_attachment(&item)?;
        body.push_str(&attachment_tag(
            attachment.id.as_deref().unwrap_or_default(),
        ));
        attachments.push(attachment);
    }

    req.body.content = Some(body);
    if !hosted.is_empty() {
        req.hosted_contents = Some(hosted);
    }
    if !attachments.is_empty() {
        req.attachments = Some(attachments);
    }
    Ok(())
}

/// The people a chat attachment must be shared with: every member of the chat
/// other than the sender. The Teams client grants these permissions itself
/// when a file is attached; a bare drive upload does not, so without this step
/// recipients get "you don't have permission" when they open the file.
async fn chat_recipients(client: &GraphClient, chat_id: &str) -> Result<Vec<DriveRecipient>> {
    let me = api::users::get_me(client).await?;
    let members = api::chats::list_members(
        client,
        chat_id,
        &PaginationOpts {
            page_size: 50,
            all_pages: true,
        },
    )
    .await?;
    Ok(invite_recipients(&members, me.id.as_deref()))
}

/// Members addressed by Entra object ID when the membership carries one (it
/// also covers accounts with no mail attribute), else by email; the sender and
/// members with neither are skipped, and duplicates collapse.
fn invite_recipients(
    members: &[ConversationMember],
    sender_id: Option<&str>,
) -> Vec<DriveRecipient> {
    let mut seen = std::collections::HashSet::new();
    members
        .iter()
        .filter(|m| m.user_id.as_deref() != sender_id || sender_id.is_none())
        .filter_map(|m| match (&m.user_id, &m.email) {
            (Some(id), _) => Some(DriveRecipient {
                object_id: Some(id.clone()),
                email: None,
            }),
            (None, Some(email)) => Some(DriveRecipient {
                object_id: None,
                email: Some(email.clone()),
            }),
            (None, None) => None,
        })
        .filter(|r| seen.insert(r.clone()))
        .collect()
}

/// Share an uploaded chat file with the chat's members. A failure here is
/// reported, not fatal: the upload succeeded and the message can still be
/// sent, but the recipients will not be able to open the file until it is
/// shared from OneDrive by hand.
async fn share_with_chat(
    client: &GraphClient,
    item: &crate::models::file::DriveItem,
    filename: &str,
    recipients: &[DriveRecipient],
) {
    if recipients.is_empty() {
        return;
    }
    let Some(item_id) = item.id.as_deref() else {
        tracing::warn!(
            "Uploaded '{filename}' but the driveItem has no id, so it could not be shared \
             with the chat's members; share it from OneDrive by hand."
        );
        return;
    };
    match api::files::grant_read_access(client, item_id, recipients.to_vec()).await {
        Ok(perms) => tracing::debug!(
            "Shared '{filename}' with {} chat member(s); roles granted: {}",
            recipients.len(),
            perms
                .iter()
                .flat_map(|p| p.roles.iter())
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Err(e) => tracing::warn!(
            "Uploaded '{filename}' but could not share it with the chat's members ({e}); \
             they will get \"you don't have permission\" until it is shared from OneDrive by hand."
        ),
    }
}

/// Build the body-HTML fragment and hosted-content uploads for `--image`
/// files. Temporary IDs are 1-based to match Graph's documented examples.
fn inline_images(images: &[String]) -> Result<(String, Vec<HostedContentUpload>)> {
    let mut html = String::new();
    let mut hosted = Vec::new();
    let mut total_encoded = 0usize;
    for (i, path) in images.iter().enumerate() {
        let temporary_id = (i + 1).to_string();
        let (bytes, content_type) = read_image(path)?;
        let content_bytes = BASE64.encode(&bytes);
        total_encoded += content_bytes.len();
        if total_encoded > MAX_INLINE_TOTAL_ENCODED {
            return Err(TeamsError::InvalidInput(format!(
                "--image files together are {total_encoded} bytes base64-encoded, exceeding \
                 the 4MB limit on the single message create request they all ride in. \
                 Send some of them with --attach instead."
            )));
        }
        html.push_str(&img_html(&temporary_id));
        hosted.push(HostedContentUpload {
            temporary_id,
            content_bytes,
            content_type,
        });
    }
    Ok((html, hosted))
}

/// Media requires an HTML body; escape and wrap a plain-text one.
///
/// Shared with the adaptive-card path, which needs the same promotion: the
/// attachment marker is markup, so a plain-text body has to be escaped rather
/// than concatenated raw.
pub(super) fn ensure_html_body(req: &mut SendMessageRequest) {
    if req.body.content_type.as_deref() == Some("html") {
        return;
    }
    let text = req.body.content.take().unwrap_or_default();
    req.body.content = Some(if text.is_empty() {
        String::new()
    } else {
        format!("<p>{}</p>", escape_html(&text))
    });
    req.body.content_type = Some("html".to_string());
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br>")
}

fn img_html(temporary_id: &str) -> String {
    format!(r#"<p><img src="../hostedContents/{temporary_id}/$value"></p>"#)
}

/// The marker Graph requires in the message body for every attachment it is
/// sent alongside — used for uploaded files and for adaptive cards.
pub(super) fn attachment_tag(attachment_id: &str) -> String {
    format!(r#"<attachment id="{attachment_id}"></attachment>"#)
}

/// The message attachment `id` must be the GUID inside the driveItem's eTag,
/// e.g. `"{5FF69C5F-7CCD-47A2-879C-1A506F961DBC},2"` → the bare GUID.
fn etag_guid(e_tag: &str) -> Option<String> {
    let start = e_tag.find('{')? + 1;
    let end = e_tag.find('}')?;
    let guid = e_tag.get(start..end)?;
    if guid.is_empty() {
        return None;
    }
    Some(guid.to_string())
}

fn reference_attachment(item: &crate::models::file::DriveItem) -> Result<ChatMessageAttachment> {
    let guid = item
        .e_tag
        .as_deref()
        .and_then(etag_guid)
        .ok_or_else(|| TeamsError::ApiError {
            status: 0,
            message: "Upload succeeded but the driveItem has no eTag GUID to reference".into(),
        })?;
    let content_url = item.web_url.clone().ok_or_else(|| TeamsError::ApiError {
        status: 0,
        message: "Upload succeeded but the driveItem has no webUrl".into(),
    })?;
    Ok(ChatMessageAttachment {
        id: Some(guid),
        content_type: Some("reference".to_string()),
        content: None,
        content_url: Some(content_url),
        name: item.name.clone(),
        thumbnail_url: None,
        teams_app_id: None,
    })
}

fn read_image(path: &str) -> Result<(Vec<u8>, String)> {
    let bytes = read_file(path)?;
    if bytes.len() > MAX_INLINE_IMAGE_SIZE {
        return Err(TeamsError::InvalidInput(format!(
            "--image '{path}' is {} bytes; inline images are capped at 3MB because they \
             ride the message create request (Graph's 4MB limit, minus base64 overhead). \
             Send it with --attach instead.",
            bytes.len()
        )));
    }
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    if mime.type_() != mime_guess::mime::IMAGE {
        return Err(TeamsError::InvalidInput(format!(
            "--image '{path}' does not look like an image (guessed type: {mime}); \
             use --attach for non-image files."
        )));
    }
    Ok((bytes, mime.essence_str().to_string()))
}

fn read_attachment(path: &str) -> Result<(Vec<u8>, String, String)> {
    let bytes = read_file(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let filename = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| {
            TeamsError::InvalidInput(format!("--attach '{path}' has no usable filename"))
        })?;
    Ok((bytes, mime.essence_str().to_string(), filename))
}

fn read_file(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|e| TeamsError::InvalidInput(format!("Failed to read '{path}': {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::ItemBody;

    fn text_request(body: &str) -> SendMessageRequest {
        SendMessageRequest {
            body: ItemBody {
                content_type: Some("text".into()),
                content: Some(body.into()),
            },
            attachments: None,
            hosted_contents: None,
            mentions: None,
        }
    }

    fn member(user_id: Option<&str>, email: Option<&str>) -> ConversationMember {
        ConversationMember {
            id: None,
            display_name: None,
            roles: None,
            user_id: user_id.map(str::to_string),
            email: email.map(str::to_string),
        }
    }

    #[test]
    fn invite_recipients_skip_sender_prefer_object_id_and_dedupe() {
        let members = [
            member(Some("me"), Some("me@example.com")),
            member(Some("u1"), Some("u1@example.com")),
            member(Some("u1"), None),
            member(None, Some("mail-only@example.com")),
            member(None, None),
        ];
        let recipients = invite_recipients(&members, Some("me"));
        assert_eq!(
            recipients,
            vec![
                DriveRecipient {
                    object_id: Some("u1".into()),
                    email: None
                },
                DriveRecipient {
                    object_id: None,
                    email: Some("mail-only@example.com".into())
                },
            ]
        );
        // Unknown sender: nobody is skipped on that basis.
        assert_eq!(invite_recipients(&members[..2], None).len(), 2);
    }

    #[test]
    fn etag_guid_extracts_bare_guid() {
        assert_eq!(
            etag_guid("\"{5FF69C5F-7CCD-47A2-879C-1A506F961DBC},2\"").as_deref(),
            Some("5FF69C5F-7CCD-47A2-879C-1A506F961DBC")
        );
        assert_eq!(etag_guid("no-braces"), None);
        assert_eq!(etag_guid("{}"), None);
    }

    #[test]
    fn plain_text_body_is_escaped_and_wrapped_for_media() {
        let mut req = text_request("a <b> & \"c\"\nnext");
        ensure_html_body(&mut req);
        assert_eq!(req.body.content_type.as_deref(), Some("html"));
        assert_eq!(
            req.body.content.as_deref(),
            Some("<p>a &lt;b&gt; &amp; &quot;c&quot;<br>next</p>")
        );
    }

    #[test]
    fn html_body_is_left_alone() {
        let mut req = text_request("<p>already html</p>");
        req.body.content_type = Some("html".into());
        ensure_html_body(&mut req);
        assert_eq!(req.body.content.as_deref(), Some("<p>already html</p>"));
    }

    #[test]
    fn img_and_attachment_html_shapes() {
        assert_eq!(
            img_html("1"),
            r#"<p><img src="../hostedContents/1/$value"></p>"#
        );
        assert_eq!(
            attachment_tag("ABC-123"),
            r#"<attachment id="ABC-123"></attachment>"#
        );
    }

    #[test]
    fn read_image_rejects_non_images_and_oversize() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, b"hello").unwrap();
        let err = read_image(txt.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("--attach"), "got: {err}");

        let big = dir.path().join("big.png");
        std::fs::write(&big, vec![0u8; MAX_INLINE_IMAGE_SIZE + 1]).unwrap();
        let err = read_image(big.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("capped at 3MB"), "got: {err}");
    }

    #[test]
    fn inline_images_builds_html_and_base64_hosted_contents() {
        // Minimal valid PNG header bytes; content doesn't matter for assembly.
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();

        let (html, hosted) = inline_images(&[png.to_string_lossy().into_owned()]).unwrap();
        assert_eq!(html, r#"<p><img src="../hostedContents/1/$value"></p>"#);
        assert_eq!(hosted.len(), 1);
        assert_eq!(hosted[0].temporary_id, "1");
        assert_eq!(hosted[0].content_type, "image/png");
        assert_eq!(
            BASE64.decode(&hosted[0].content_bytes).unwrap(),
            b"\x89PNG\r\n\x1a\n"
        );
    }

    #[test]
    fn inline_images_rejects_aggregate_over_request_limit() {
        // Each file passes the 3MB per-image cap, but their combined base64
        // payload exceeds the 4MB request limit.
        let dir = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for name in ["a.png", "b.png"] {
            let path = dir.path().join(name);
            std::fs::write(&path, vec![0u8; 2 * 1024 * 1024]).unwrap();
            paths.push(path.to_string_lossy().into_owned());
        }
        let err = inline_images(&paths).unwrap_err();
        assert!(err.to_string().contains("together"), "got: {err}");
    }

    #[test]
    fn reference_attachment_requires_etag_and_weburl() {
        let mut item = crate::models::file::DriveItem {
            id: Some("i".into()),
            name: Some("NetskopeLogs.zip".into()),
            size: None,
            web_url: Some("https://x.sharepoint.com/f.zip".into()),
            e_tag: Some("\"{ABC-DEF},1\"".into()),
            created_date_time: None,
            last_modified_date_time: None,
            created_by: None,
            last_modified_by: None,
            file: None,
            folder: None,
            download_url: None,
            parent_reference: None,
        };
        let att = reference_attachment(&item).unwrap();
        assert_eq!(att.id.as_deref(), Some("ABC-DEF"));
        assert_eq!(att.content_type.as_deref(), Some("reference"));
        assert_eq!(att.name.as_deref(), Some("NetskopeLogs.zip"));

        item.e_tag = None;
        assert!(reference_attachment(&item).is_err());
    }
}
