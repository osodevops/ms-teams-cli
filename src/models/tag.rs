use serde::{Deserialize, Serialize};

/// Microsoft Graph teamwork tag
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamworkTag {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_type: Option<String>,
}

/// A member of a teamwork tag
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamworkTagMember {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Request to create a tag
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagRequest {
    pub display_name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<CreateTagMemberEntry>,
}

/// Entry for creating a tag member
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagMemberEntry {
    pub user_id: String,
}

/// Request to update a tag
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTagRequest {
    pub display_name: String,
}

/// Request to add a member to a tag
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTagMemberRequest {
    pub user_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teamwork_tag_roundtrip() {
        let tag = TeamworkTag {
            id: Some("tag1".into()),
            display_name: Some("Designers".into()),
            description: Some("Design team tag".into()),
            member_count: Some(5),
            tag_type: Some("standard".into()),
        };
        let json = serde_json::to_string(&tag).unwrap();
        let parsed: TeamworkTag = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name.as_deref(), Some("Designers"));
        assert_eq!(parsed.member_count, Some(5));
    }

    #[test]
    fn create_tag_request_with_members() {
        let req = CreateTagRequest {
            display_name: "Engineers".into(),
            members: vec![
                CreateTagMemberEntry { user_id: "u1".into() },
                CreateTagMemberEntry { user_id: "u2".into() },
            ],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["displayName"], "Engineers");
        assert_eq!(json["members"].as_array().unwrap().len(), 2);
        assert_eq!(json["members"][0]["userId"], "u1");
    }
}
