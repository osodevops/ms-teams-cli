use serde::{Deserialize, Serialize};

use super::common::IdentitySet;

/// Microsoft Graph online meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMeeting {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_web_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_teleconference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants: Option<MeetingParticipants>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lobby_bypass_settings: Option<LobbyBypassSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_presenters: Option<String>,
}

/// Meeting participants container
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingParticipants {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<MeetingParticipantInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<MeetingParticipantInfo>,
}

/// Information about a meeting participant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingParticipantInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentitySet>,
}

/// Lobby bypass settings for a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyBypassSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dial_in_bypass_enabled: Option<bool>,
}

/// Request to create a meeting
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMeetingRequest {
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants: Option<MeetingParticipants>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lobby_bypass_settings: Option<LobbyBypassSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_presenters: Option<String>,
}

/// Request to update a meeting
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMeetingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_presenters: Option<String>,
}

/// Meeting attendance report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingAttendanceReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_participant_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting_start_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting_end_date_time: Option<String>,
}

/// Individual attendance record
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_attendance_in_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentitySet>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_meeting_roundtrip() {
        let meeting = OnlineMeeting {
            id: Some("m1".into()),
            subject: Some("Standup".into()),
            start_date_time: Some("2024-01-01T09:00:00Z".into()),
            end_date_time: Some("2024-01-01T09:30:00Z".into()),
            join_web_url: Some("https://teams.microsoft.com/join/123".into()),
            video_teleconference_id: None,
            participants: Some(MeetingParticipants {
                organizer: Some(MeetingParticipantInfo {
                    upn: Some("user@example.com".into()),
                    role: Some("presenter".into()),
                    identity: Some(IdentitySet {
                        user: Some(crate::models::common::Identity {
                            id: Some("u1".into()),
                            display_name: Some("Alice".into()),
                        }),
                    }),
                }),
                attendees: vec![],
            }),
            lobby_bypass_settings: None,
            allowed_presenters: Some("everyone".into()),
        };
        let json = serde_json::to_string(&meeting).unwrap();
        let parsed: OnlineMeeting = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subject.as_deref(), Some("Standup"));
        assert!(parsed.participants.is_some());
    }

    #[test]
    fn create_meeting_request_serializes() {
        let req = CreateMeetingRequest {
            subject: "Review".into(),
            start_date_time: Some("2024-06-01T10:00:00Z".into()),
            end_date_time: Some("2024-06-01T11:00:00Z".into()),
            participants: None,
            lobby_bypass_settings: None,
            allowed_presenters: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subject"], "Review");
        assert_eq!(json["startDateTime"], "2024-06-01T10:00:00Z");
    }
}
