use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Subcommand;

use crate::api::messages::MessageRef;
use crate::api::{self, GraphClient};
use crate::error::{Result, TeamsError};
use crate::models::attachment_inventory::{build_inventory, AttachmentItem, ItemKind};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum AttachmentsCommand {
    /// List attachments and inline images in a message
    List {
        /// Team ID (for channel messages)
        #[arg(long)]
        team: Option<String>,
        /// Channel ID (for channel messages)
        #[arg(long)]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long)]
        chat: Option<String>,
        /// Reply ID (for a reply within a channel thread)
        #[arg(long)]
        reply: Option<String>,
        /// Message ID
        #[arg(required_unless_present = "message", conflicts_with = "message")]
        message_id: Option<String>,
        /// Message ID
        #[arg(
            long = "message",
            alias = "message-id",
            required_unless_present = "message_id",
            conflicts_with = "message_id"
        )]
        message: Option<String>,
    },
    /// Download attachments and inline images from a message
    Download {
        /// Team ID (for channel messages)
        #[arg(long)]
        team: Option<String>,
        /// Channel ID (for channel messages)
        #[arg(long)]
        channel: Option<String>,
        /// Chat ID (for chat messages)
        #[arg(long)]
        chat: Option<String>,
        /// Reply ID (for a reply within a channel thread)
        #[arg(long)]
        reply: Option<String>,
        /// Message ID
        #[arg(required_unless_present = "message", conflicts_with = "message")]
        message_id: Option<String>,
        /// Message ID
        #[arg(
            long = "message",
            alias = "message-id",
            required_unless_present = "message_id",
            conflicts_with = "message_id"
        )]
        message: Option<String>,
        /// Download only the item with this index (from `attachments list`)
        #[arg(long)]
        index: Option<usize>,
        /// Output directory for downloaded files
        #[arg(long, conflicts_with = "path")]
        dir: Option<String>,
        /// Exact output file path for a single item; use '-' for stdout
        #[arg(long, requires = "index")]
        path: Option<String>,
    },
}

pub async fn run(
    cmd: AttachmentsCommand,
    client: &GraphClient,
    format: OutputFormat,
) -> Result<()> {
    match cmd {
        AttachmentsCommand::List {
            team,
            channel,
            chat,
            reply,
            message_id,
            message,
        } => {
            let start = Instant::now();
            let message_id =
                super::message::resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let message_ref = resolve_message_ref(team, channel, chat, reply, message_id.clone())?;
            let items = fetch_inventory(client, &message_ref).await?;
            let result = serde_json::json!({
                "message_id": message_id,
                "items": items,
            });
            output::print_success(format, &result, start);
            Ok(())
        }

        AttachmentsCommand::Download {
            team,
            channel,
            chat,
            reply,
            message_id,
            message,
            index,
            dir,
            path,
        } => {
            let start = Instant::now();
            let message_id =
                super::message::resolve_id(message_id, message, "--message or <MESSAGE_ID>")?;
            let message_ref = resolve_message_ref(team, channel, chat, reply, message_id.clone())?;
            let items = fetch_inventory(client, &message_ref).await?;
            let selected = select_items(&items, index)?;

            if path.as_deref() == Some("-") {
                let item = selected[0];
                let (bytes, _) = fetch_item_bytes(client, &message_ref, item).await?;
                std::io::stdout().write_all(&bytes).map_err(|e| {
                    TeamsError::InvalidInput(format!("Failed to write stdout: {e}"))
                })?;
                return Ok(());
            }

            let dir = PathBuf::from(dir.unwrap_or_else(|| ".".to_string()));
            let mut results = Vec::new();
            let mut first_error: Option<TeamsError> = None;
            for item in &selected {
                match download_item(
                    client,
                    &message_ref,
                    &message_id,
                    item,
                    &dir,
                    path.as_deref(),
                )
                .await
                {
                    Ok(saved) => results.push(saved),
                    Err(e) => {
                        results.push(serde_json::json!({
                            "index": item.index,
                            "kind": item.kind,
                            "error": e.to_string(),
                        }));
                        first_error.get_or_insert(e);
                    }
                }
            }

            if let Some(err) = first_error {
                let saved: Vec<&str> = results
                    .iter()
                    .filter_map(|r| r.get("path").and_then(|p| p.as_str()))
                    .collect();
                let saved_note = if saved.is_empty() {
                    String::new()
                } else {
                    format!(
                        " Successfully downloaded before the failure: {}.",
                        saved.join(", ")
                    )
                };
                return Err(augment_error(err, &saved_note));
            }

            let result = serde_json::json!({
                "message_id": message_id,
                "downloaded": results,
            });
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}

/// Fetch the message and its hosted contents, returning the unified inventory.
pub async fn fetch_inventory(
    client: &GraphClient,
    message_ref: &MessageRef,
) -> Result<Vec<AttachmentItem>> {
    let msg = api::messages::get_message(client, message_ref).await?;
    let hosted = api::messages::list_hosted_contents(client, message_ref).await?;
    Ok(build_inventory(&msg, &hosted))
}

fn resolve_message_ref(
    team: Option<String>,
    channel: Option<String>,
    chat: Option<String>,
    reply: Option<String>,
    message_id: String,
) -> Result<MessageRef> {
    match (chat, team, channel) {
        (Some(chat_id), None, None) => {
            if reply.is_some() {
                return Err(TeamsError::InvalidInput(
                    "--reply only applies to channel messages".into(),
                ));
            }
            Ok(MessageRef::Chat {
                chat_id,
                message_id,
            })
        }
        (None, Some(team_id), Some(channel_id)) => Ok(match reply {
            Some(reply_id) => MessageRef::ChannelReply {
                team_id,
                channel_id,
                message_id,
                reply_id,
            },
            None => MessageRef::Channel {
                team_id,
                channel_id,
                message_id,
            },
        }),
        _ => Err(TeamsError::InvalidInput(
            "Provide either --chat, or both --team and --channel".into(),
        )),
    }
}

fn select_items(items: &[AttachmentItem], index: Option<usize>) -> Result<Vec<&AttachmentItem>> {
    match index {
        Some(i) => {
            let item = items.iter().find(|it| it.index == i).ok_or_else(|| {
                TeamsError::InvalidInput(format!(
                    "No attachment item with index {i}; the message has {} item(s). \
                     Run `teams message attachments list` to see them.",
                    items.len()
                ))
            })?;
            if !item.downloadable {
                return Err(TeamsError::InvalidInput(format!(
                    "Item {i} ({:?}) is not downloadable; its content is inline in `message get` output.",
                    item.kind
                )));
            }
            Ok(vec![item])
        }
        None => {
            let downloadable: Vec<&AttachmentItem> =
                items.iter().filter(|it| it.downloadable).collect();
            if downloadable.is_empty() {
                return Err(TeamsError::NotFound(
                    "Message has no downloadable attachments or inline images".into(),
                ));
            }
            Ok(downloadable)
        }
    }
}

async fn fetch_item_bytes(
    client: &GraphClient,
    message_ref: &MessageRef,
    item: &AttachmentItem,
) -> Result<(Vec<u8>, Option<String>)> {
    match item.kind {
        ItemKind::HostedContent => {
            let id = item.hosted_content_id.as_deref().ok_or_else(|| {
                TeamsError::InvalidInput("Hosted content item is missing its ID".into())
            })?;
            api::messages::get_hosted_content_bytes(client, message_ref, id).await
        }
        ItemKind::CodeSnippet => {
            let url = item.code_snippet_url.as_deref().ok_or_else(|| {
                TeamsError::InvalidInput("Code snippet item is missing codeSnippetUrl".into())
            })?;
            let id = code_snippet_hosted_content_id(url).ok_or_else(|| {
                TeamsError::InvalidInput(format!(
                    "Code snippet codeSnippetUrl is not a Graph hostedContents URL, \
                     refusing to fetch it: {url}"
                ))
            })?;
            api::messages::get_hosted_content_bytes(client, message_ref, &id).await
        }
        ItemKind::FileReference => {
            let url = item.content_url.as_deref().ok_or_else(|| {
                TeamsError::InvalidInput("File attachment is missing contentUrl".into())
            })?;
            api::files::download_shared_item(client, url).await
        }
        _ => Err(TeamsError::InvalidInput(format!(
            "Item {} ({:?}) is not downloadable",
            item.index, item.kind
        ))),
    }
}

/// Extract the hosted-content ID from a `codeSnippetUrl`. The URL is message
/// content an arbitrary sender controls, so it is never fetched directly —
/// doing so would send our Graph bearer token wherever it points. Only the ID
/// is taken, percent-decoded, and the request URL is rebuilt from the message
/// reference we already resolved.
fn code_snippet_hosted_content_id(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let mut segments = path.split('/').skip_while(|s| *s != "hostedContents");
    segments.next()?;
    let id = segments.next().filter(|s| !s.is_empty())?;
    if segments.next() != Some("$value") || segments.next().is_some() {
        return None;
    }
    urlencoding::decode(id).ok().map(|s| s.into_owned())
}

async fn download_item(
    client: &GraphClient,
    message_ref: &MessageRef,
    message_id: &str,
    item: &AttachmentItem,
    dir: &Path,
    explicit_path: Option<&str>,
) -> Result<serde_json::Value> {
    let (bytes, content_type) = fetch_item_bytes(client, message_ref, item).await?;

    let target = match explicit_path {
        Some(p) => PathBuf::from(p),
        None => unique_path(dir.join(default_filename(message_id, item, content_type.as_deref()))),
    };

    write_atomically(&target, &bytes)?;

    Ok(serde_json::json!({
        "index": item.index,
        "kind": item.kind,
        "path": target.to_string_lossy(),
        "size": bytes.len(),
        "content_type": content_type,
    }))
}

fn default_filename(message_id: &str, item: &AttachmentItem, content_type: Option<&str>) -> String {
    match item.kind {
        ItemKind::FileReference => item
            .name
            .as_deref()
            .map(sanitize_filename)
            .unwrap_or_else(|| format!("msg-{message_id}-{}.bin", item.index)),
        ItemKind::CodeSnippet => format!("msg-{message_id}-{}.txt", item.index),
        _ => format!(
            "msg-{message_id}-{}.{}",
            item.index,
            extension_for(content_type)
        ),
    }
}

fn extension_for(content_type: Option<&str>) -> &'static str {
    match content_type {
        Some("image/png") => "png",
        Some("image/jpeg") => "jpg",
        Some("image/gif") => "gif",
        Some("image/webp") => "webp",
        Some("image/bmp") => "bmp",
        Some("image/svg+xml") => "svg",
        Some("text/plain") => "txt",
        Some(ct) => mime_guess::get_mime_extensions_str(ct)
            .and_then(|exts| exts.first())
            .copied()
            .unwrap_or("bin"),
        None => "bin",
    }
}

/// Strip path separators and traversal from an attachment-supplied filename so
/// it cannot escape the output directory.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            other => other,
        })
        .collect();
    let cleaned = cleaned.trim_start_matches(['.', ' ']).trim_end();
    if cleaned.is_empty() {
        "attachment.bin".to_string()
    } else {
        cleaned.to_string()
    }
}

/// Avoid clobbering existing files by suffixing `-1`, `-2`, … before the extension.
fn unique_path(candidate: PathBuf) -> PathBuf {
    if !candidate.exists() {
        return candidate;
    }
    let stem = candidate
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "attachment".to_string());
    let ext = candidate
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = candidate.parent().unwrap_or(Path::new("."));
    (1..)
        .map(|n| parent.join(format!("{stem}-{n}{ext}")))
        .find(|p| !p.exists())
        .expect("unbounded suffix search always terminates")
}

/// Write to `<path>.part` then rename, so a failed download never leaves a
/// truncated file at the final path.
fn write_atomically(target: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TeamsError::InvalidInput(format!(
                    "Failed to create directory '{}': {e}",
                    parent.display()
                ))
            })?;
        }
    }
    let part = target.with_extension(format!(
        "{}part",
        target
            .extension()
            .map(|e| format!("{}.", e.to_string_lossy()))
            .unwrap_or_default()
    ));
    std::fs::write(&part, bytes).map_err(|e| {
        TeamsError::InvalidInput(format!("Failed to write '{}': {e}", part.display()))
    })?;
    std::fs::rename(&part, target).map_err(|e| {
        TeamsError::InvalidInput(format!("Failed to rename to '{}': {e}", target.display()))
    })
}

fn augment_error(err: TeamsError, note: &str) -> TeamsError {
    if note.is_empty() {
        return err;
    }
    match err {
        TeamsError::PermissionDenied(m) => TeamsError::PermissionDenied(format!("{m}{note}")),
        TeamsError::NotFound(m) => TeamsError::NotFound(format!("{m}{note}")),
        TeamsError::InvalidInput(m) => TeamsError::InvalidInput(format!("{m}{note}")),
        TeamsError::ApiError { status, message } => TeamsError::ApiError {
            status,
            message: format!("{message}{note}"),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_traversal_and_separators() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(sanitize_filename("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_filename("  .hidden"), "hidden");
        assert_eq!(sanitize_filename(""), "attachment.bin");
        assert_eq!(sanitize_filename("NetskopeLogs.zip"), "NetskopeLogs.zip");
    }

    #[test]
    fn extension_covers_common_image_types() {
        assert_eq!(extension_for(Some("image/png")), "png");
        assert_eq!(extension_for(Some("image/jpeg")), "jpg");
        assert_eq!(extension_for(None), "bin");
        assert_eq!(extension_for(Some("application/x-unknown-thing")), "bin");
    }

    #[test]
    fn unique_path_suffixes_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("shot.png");
        std::fs::write(&first, b"x").unwrap();
        let next = unique_path(first.clone());
        assert_eq!(next, dir.path().join("shot-1.png"));
        std::fs::write(&next, b"x").unwrap();
        assert_eq!(unique_path(first), dir.path().join("shot-2.png"));
    }

    #[test]
    fn write_atomically_leaves_no_part_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.png");
        write_atomically(&target, b"bytes").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"bytes");
        assert!(!dir.path().join("out.png.part").exists());
    }

    #[test]
    fn code_snippet_id_extracted_from_graph_url_only() {
        assert_eq!(
            code_snippet_hosted_content_id(
                "https://graph.microsoft.com/v1.0/chats/19:x/messages/m1/hostedContents/aWQ9/$value"
            )
            .as_deref(),
            Some("aWQ9")
        );
        // Percent-encoded IDs are decoded so the rebuilt URL doesn't double-encode.
        assert_eq!(
            code_snippet_hosted_content_id(
                "https://graph.microsoft.com/v1.0/chats/c/messages/m/hostedContents/aWQ%3D/$value"
            )
            .as_deref(),
            Some("aWQ=")
        );
        // A URL pointing anywhere else must be rejected, not fetched.
        assert_eq!(
            code_snippet_hosted_content_id("https://evil.example.com/steal-token"),
            None
        );
        assert_eq!(
            code_snippet_hosted_content_id(
                "https://evil.example.com/hostedContents/x/$value/extra"
            ),
            None
        );
        assert_eq!(
            code_snippet_hosted_content_id("https://evil.example.com/hostedContents//$value"),
            None
        );
    }

    #[test]
    fn resolve_message_ref_validates_target_combinations() {
        assert!(matches!(
            resolve_message_ref(None, None, Some("19:x".into()), None, "m".into()),
            Ok(MessageRef::Chat { .. })
        ));
        assert!(matches!(
            resolve_message_ref(
                Some("t".into()),
                Some("c".into()),
                None,
                Some("r".into()),
                "m".into()
            ),
            Ok(MessageRef::ChannelReply { .. })
        ));
        assert!(resolve_message_ref(Some("t".into()), None, None, None, "m".into()).is_err());
        assert!(resolve_message_ref(
            None,
            None,
            Some("19:x".into()),
            Some("r".into()),
            "m".into()
        )
        .is_err());
    }
}
