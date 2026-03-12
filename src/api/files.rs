use crate::error::{Result, TeamsError};
use crate::models::file::{
    DriveItem, FilesFolder, ShareLinkRequest, ShareLinkResponse,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

const MAX_UPLOAD_SIZE: usize = 4 * 1024 * 1024; // 4MB

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
            "File size ({} bytes) exceeds 4MB upload limit. Use upload sessions for larger files.",
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
