use serde::{Deserialize, Serialize};

use super::common::IdentitySet;

/// Microsoft Graph drive item (file or folder)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<IdentitySet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by: Option<IdentitySet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<FolderInfo>,
    #[serde(
        rename = "@microsoft.graph.downloadUrl",
        skip_serializing_if = "Option::is_none"
    )]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_reference: Option<ItemReference>,
}

/// File-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Folder-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,
}

/// Reference to a parent item/drive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The files folder for a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesFolder {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_reference: Option<ItemReference>,
}

/// Request to create a sharing link
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareLinkRequest {
    #[serde(rename = "type")]
    pub link_type: String,
    pub scope: String,
}

/// Response from creating a sharing link
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareLinkResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<SharingLink>,
}

/// A sharing link
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharingLink {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_item_with_download_url() {
        let json = r#"{
            "id": "item1",
            "name": "report.pdf",
            "size": 1024,
            "@microsoft.graph.downloadUrl": "https://download.example.com/file",
            "file": { "mimeType": "application/pdf" }
        }"#;
        let item: DriveItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.name.as_deref(), Some("report.pdf"));
        assert_eq!(item.size, Some(1024));
        assert_eq!(
            item.download_url.as_deref(),
            Some("https://download.example.com/file")
        );
        assert_eq!(
            item.file.as_ref().unwrap().mime_type.as_deref(),
            Some("application/pdf")
        );
    }

    #[test]
    fn files_folder_with_parent_reference() {
        let json = r#"{
            "id": "folder1",
            "name": "General",
            "parentReference": {
                "driveId": "drive123",
                "id": "root-id"
            }
        }"#;
        let folder: FilesFolder = serde_json::from_str(json).unwrap();
        assert_eq!(folder.id.as_deref(), Some("folder1"));
        let parent = folder.parent_reference.unwrap();
        assert_eq!(parent.drive_id.as_deref(), Some("drive123"));
    }
}
