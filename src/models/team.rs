use serde::{Deserialize, Serialize};

/// Microsoft Graph Team resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_archived: Option<bool>,
}

/// Request body for creating a new team.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamCreateRequest {
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "template@odata.bind")]
    pub template: String,
}

impl TeamCreateRequest {
    pub fn new(display_name: String, description: Option<String>) -> Self {
        Self {
            display_name,
            description,
            template: "https://graph.microsoft.com/v1.0/teamsTemplates('standard')".to_string(),
        }
    }
}

/// Request body for updating a team.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request body for cloning a team.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamCloneRequest {
    pub display_name: String,
    pub parts_to_clone: String,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_roundtrip() {
        let team = Team {
            id: Some("t1".into()),
            display_name: Some("Engineering".into()),
            description: Some("Eng team".into()),
            is_archived: Some(false),
        };
        let json = serde_json::to_string(&team).unwrap();
        let parsed: Team = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name.as_deref(), Some("Engineering"));
    }

    #[test]
    fn team_create_request_serializes() {
        let req = TeamCreateRequest::new("My Team".into(), Some("desc".into()));
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["displayName"], "My Team");
        assert!(json["template@odata.bind"]
            .as_str()
            .unwrap()
            .contains("standard"));
    }
}
