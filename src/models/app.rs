use serde::{Deserialize, Serialize};

/// An installed Teams app
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamsAppInstallation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams_app: Option<TeamsApp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams_app_definition: Option<TeamsAppDefinition>,
}

/// A Teams app from the catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamsApp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

/// App definition with version info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamsAppDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Request to install an app
#[derive(Debug, Clone, Serialize)]
pub struct InstallAppRequest {
    #[serde(rename = "teamsApp@odata.bind")]
    pub teams_app_bind: String,
}

impl InstallAppRequest {
    pub fn new(catalog_app_id: &str) -> Self {
        Self {
            teams_app_bind: format!(
                "https://graph.microsoft.com/v1.0/appCatalogs/teamsApps/{}",
                catalog_app_id
            ),
        }
    }
}

/// A tab in a Teams channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamsTab {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<TabConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams_app: Option<TeamsApp>,
}

/// Configuration for a tab
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_url: Option<String>,
}

/// Request to create a tab
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTabRequest {
    pub display_name: String,
    #[serde(rename = "teamsApp@odata.bind")]
    pub teams_app_bind: String,
    pub configuration: TabConfiguration,
}

/// Request to update a tab
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTabRequest {
    pub display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_app_installation_roundtrip() {
        let installation = TeamsAppInstallation {
            id: Some("inst1".into()),
            teams_app: Some(TeamsApp {
                id: Some("app1".into()),
                display_name: Some("Planner".into()),
                distribution_method: Some("store".into()),
                external_id: None,
            }),
            teams_app_definition: Some(TeamsAppDefinition {
                id: Some("def1".into()),
                display_name: Some("Planner".into()),
                version: Some("1.0".into()),
            }),
        };
        let json = serde_json::to_string(&installation).unwrap();
        let parsed: TeamsAppInstallation = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.teams_app.as_ref().unwrap().display_name.as_deref(),
            Some("Planner")
        );
    }

    #[test]
    fn install_app_request_bind_url() {
        let req = InstallAppRequest::new("com.example.app");
        let json = serde_json::to_value(&req).unwrap();
        assert!(json["teamsApp@odata.bind"]
            .as_str()
            .unwrap()
            .contains("com.example.app"));
    }
}
