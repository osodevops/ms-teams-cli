use crate::error::{Result, TeamsError};
use crate::models::file::{DriveItem, FilesFolder, ShareLinkRequest, ShareLinkResponse};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

const MAX_UPLOAD_SIZE: usize = 250 * 1024 * 1024; // DriveItem simple upload limit.

pub async fn get_files_folder(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
) -> Result<FilesFolder> {
    client
        .get(&endpoints::channel_files_folder(team_id, channel_id), &[])
        .await
}

pub async fn list_files(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<DriveItem>> {
    let folder = get_files_folder(client, team_id, channel_id).await?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;
    let folder_id = folder.id.as_deref().ok_or_else(|| TeamsError::ApiError {
        status: 500,
        message: "filesFolder missing id".to_string(),
    })?;

    client
        .get_paged(
            &endpoints::drive_item_children(drive_id, folder_id),
            &[],
            pagination,
        )
        .await
}

pub async fn get_file(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    file_id: &str,
) -> Result<DriveItem> {
    let folder = get_files_folder(client, team_id, channel_id).await?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;

    client
        .get(&endpoints::drive_item(drive_id, file_id), &[])
        .await
}

pub async fn download_file(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    file_id: &str,
) -> Result<Vec<u8>> {
    let folder = get_files_folder(client, team_id, channel_id).await?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;

    client
        .get_bytes(&endpoints::drive_item_content(drive_id, file_id))
        .await
}

/// Download the bytes behind a shared web URL, following Graph's redirect to
/// the pre-authenticated download URL.
pub async fn download_shared_item(
    client: &GraphClient,
    content_url: &str,
) -> Result<(Vec<u8>, Option<String>)> {
    let token = endpoints::sharing_url_token(content_url);
    client
        .get_bytes_with_content_type(&endpoints::shares_drive_item_content(&token))
        .await
        .map_err(shares_scope_hint)
}

fn shares_scope_hint(err: TeamsError) -> TeamsError {
    match err {
        TeamsError::PermissionDenied(msg) => TeamsError::PermissionDenied(format!(
            "{msg}\nHint: downloading file attachments requires the Files.Read.All delegated \
             scope. Add it to your profile's scopes and run `teams auth refresh`."
        )),
        other => other,
    }
}

pub async fn upload_file(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    filename: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<DriveItem> {
    if bytes.len() > MAX_UPLOAD_SIZE {
        return Err(TeamsError::InvalidInput(format!(
            "File size ({} bytes) exceeds the 250MB simple upload limit. Use upload sessions for larger files.",
            bytes.len()
        )));
    }

    let folder = get_files_folder(client, team_id, channel_id).await?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;
    let folder_id = folder.id.as_deref().ok_or_else(|| TeamsError::ApiError {
        status: 500,
        message: "filesFolder missing id".to_string(),
    })?;

    client
        .put_bytes(
            &endpoints::drive_upload_to_folder(drive_id, folder_id, filename),
            bytes,
            content_type,
        )
        .await
}

/// Upload a message attachment into the signed-in user's OneDrive
/// `Microsoft Teams Chat Files` folder (where the Teams client puts files
/// attached to chats), renaming on collision.
pub async fn upload_chat_attachment(
    client: &GraphClient,
    filename: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<DriveItem> {
    check_upload_size(&bytes)?;
    let url = endpoints::me_chat_files_upload(filename);
    match client.put_bytes(&url, bytes.clone(), content_type).await {
        // A 404 can mean the `Microsoft Teams Chat Files` folder simply
        // doesn't exist yet (the user never attached a chat file); create it
        // and retry once. If the drive itself is inaccessible the create
        // fails too and the hint below explains the scope angle.
        Err(TeamsError::NotFound(_)) => {
            ensure_chat_files_folder(client)
                .await
                .map_err(|e| attach_scope_hint(e, AttachTarget::Chat))?;
            client
                .put_bytes(&url, bytes, content_type)
                .await
                .map_err(|e| attach_scope_hint(e, AttachTarget::Chat))
        }
        other => other.map_err(|e| attach_scope_hint(e, AttachTarget::Chat)),
    }
}

async fn ensure_chat_files_folder(client: &GraphClient) -> Result<()> {
    let body = serde_json::json!({
        "name": "Microsoft Teams Chat Files",
        "folder": {},
        "@microsoft.graph.conflictBehavior": "fail",
    });
    match client
        .post::<DriveItem, _>(&endpoints::me_drive_root_children(), &body)
        .await
    {
        Ok(_) => Ok(()),
        // 409 nameAlreadyExists means the folder is there; anything else is real.
        Err(TeamsError::ApiError { status: 409, .. }) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Upload a message attachment into the channel's folder in the team's
/// SharePoint library, renaming on collision (unlike `upload_file`, which
/// replaces — attachments must never rewrite a previously shared file).
pub async fn upload_channel_attachment(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    filename: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<DriveItem> {
    check_upload_size(&bytes)?;
    let folder = get_files_folder(client, team_id, channel_id)
        .await
        .map_err(|e| attach_scope_hint(e, AttachTarget::Channel))?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;
    let folder_id = folder.id.as_deref().ok_or_else(|| TeamsError::ApiError {
        status: 500,
        message: "filesFolder missing id".to_string(),
    })?;

    client
        .put_bytes(
            &endpoints::drive_upload_to_folder_rename(drive_id, folder_id, filename),
            bytes,
            content_type,
        )
        .await
        .map_err(|e| attach_scope_hint(e, AttachTarget::Channel))
}

enum AttachTarget {
    Chat,
    Channel,
}

/// Turn a bare 403/404 from the attachment-upload step into an abbreviated
/// version of the scope explainer in docs/attachments-spec.md: say where the
/// file was headed, which scope that storage write needs, and how to grant it.
///
/// Graph masks drives the token cannot see as 404 `itemNotFound` rather than
/// 403 (verified live: `GET /me/drive` without any Files scope is a 404), so
/// both statuses get the teaching treatment.
fn attach_scope_hint(err: TeamsError, target: AttachTarget) -> TeamsError {
    let (storage, scope) = match target {
        AttachTarget::Chat => (
            "your own OneDrive ('Microsoft Teams Chat Files')",
            "Files.ReadWrite",
        ),
        AttachTarget::Channel => (
            "the team's SharePoint library (the channel's Files tab)",
            "Files.ReadWrite.All (admin consent may be required)",
        ),
    };
    let how = "Add the scope to your profile's `scopes` and run `teams auth refresh`. \
               See docs/auth.md and docs/attachments-spec.md for the full explanation.";
    match err {
        TeamsError::PermissionDenied(msg) => TeamsError::PermissionDenied(format!(
            "{msg}\nHint: attaching a file uploads it to {storage} before linking it in \
             the message; that storage write needs the {scope} delegated scope, which \
             your token doesn't have. Inline images (--image) travel inside the message \
             and don't need it. {how}"
        )),
        TeamsError::NotFound(msg) => TeamsError::NotFound(format!(
            "{msg}\nHint: attaching a file uploads it to {storage} before linking it in \
             the message, and Graph reported that storage as not found. That usually \
             means the token lacks the {scope} delegated scope (Graph masks drives the \
             token cannot see as 404, not 403){}. Inline images (--image) travel inside \
             the message and need no Files scope. {how}",
            match target {
                AttachTarget::Chat =>
                    ", or your OneDrive has never been provisioned — \
                                       open onedrive.com once to initialize it",
                AttachTarget::Channel => "",
            }
        )),
        other => other,
    }
}

fn check_upload_size(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_UPLOAD_SIZE {
        return Err(TeamsError::InvalidInput(format!(
            "File size ({} bytes) exceeds the 250MB simple-upload limit for message \
             attachments.",
            bytes.len()
        )));
    }
    Ok(())
}

pub async fn delete_file(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    file_id: &str,
) -> Result<()> {
    let folder = get_files_folder(client, team_id, channel_id).await?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;

    client
        .delete(&endpoints::drive_item(drive_id, file_id))
        .await
}

pub async fn create_share_link(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    file_id: &str,
    link_type: &str,
    scope: &str,
) -> Result<ShareLinkResponse> {
    let folder = get_files_folder(client, team_id, channel_id).await?;
    let drive_id = folder
        .parent_reference
        .as_ref()
        .and_then(|r| r.drive_id.as_deref())
        .ok_or_else(|| TeamsError::ApiError {
            status: 500,
            message: "filesFolder missing driveId".to_string(),
        })?;

    let req = ShareLinkRequest {
        link_type: link_type.to_string(),
        scope: scope.to_string(),
    };
    client
        .post(&endpoints::drive_item_create_link(drive_id, file_id), &req)
        .await
}
