pub const GRAPH_V1: &str = "https://graph.microsoft.com/v1.0";
pub const GRAPH_BETA: &str = "https://graph.microsoft.com/beta";

// --- Users ---
pub fn me() -> String {
    format!("{GRAPH_V1}/me")
}

pub fn user(id: &str) -> String {
    format!("{GRAPH_V1}/users/{id}")
}

pub fn users() -> String {
    format!("{GRAPH_V1}/users")
}

// --- Teams ---
pub fn my_joined_teams() -> String {
    format!("{GRAPH_V1}/me/joinedTeams")
}

pub fn teams() -> String {
    format!("{GRAPH_V1}/teams")
}

pub fn team(id: &str) -> String {
    format!("{GRAPH_V1}/teams/{id}")
}

pub fn team_archive(id: &str) -> String {
    format!("{GRAPH_V1}/teams/{id}/archive")
}

pub fn team_unarchive(id: &str) -> String {
    format!("{GRAPH_V1}/teams/{id}/unarchive")
}

pub fn team_clone(id: &str) -> String {
    format!("{GRAPH_V1}/teams/{id}/clone")
}

pub fn team_members(id: &str) -> String {
    format!("{GRAPH_V1}/teams/{id}/members")
}

pub fn team_member(team_id: &str, member_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/members/{member_id}")
}

// --- Channels ---
pub fn channels(team_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels")
}

pub fn channel(team_id: &str, channel_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}")
}

pub fn channel_members(team_id: &str, channel_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/members")
}

pub fn channel_member(team_id: &str, channel_id: &str, member_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/members/{member_id}")
}

// --- Channel Messages ---
pub fn channel_messages(team_id: &str, channel_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages")
}

pub fn channel_message(team_id: &str, channel_id: &str, message_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}")
}

pub fn channel_message_replies(team_id: &str, channel_id: &str, message_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/replies")
}

pub fn message_set_reaction(team_id: &str, channel_id: &str, message_id: &str) -> String {
    format!("{GRAPH_BETA}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/setReaction")
}

pub fn message_unset_reaction(team_id: &str, channel_id: &str, message_id: &str) -> String {
    format!(
        "{GRAPH_BETA}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/unsetReaction"
    )
}

// --- Chat Messages ---
pub fn chat_messages(chat_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/messages")
}

#[allow(dead_code)]
pub fn chat_message(chat_id: &str, message_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/messages/{message_id}")
}

// --- Pinned Messages ---
pub fn channel_pinned_messages(team_id: &str, channel_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/pinnedMessages")
}

pub fn channel_pinned_message(team_id: &str, channel_id: &str, pinned_message_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/pinnedMessages/{pinned_message_id}")
}

// --- Chats ---
pub fn my_chats() -> String {
    format!("{GRAPH_V1}/me/chats")
}

/// Chat creation lives at `/chats`; `/me/chats` is list-only and returns
/// HTTP 405 for POST.
pub fn chats() -> String {
    format!("{GRAPH_V1}/chats")
}

pub fn chat(id: &str) -> String {
    format!("{GRAPH_V1}/chats/{id}")
}

pub fn chat_members(chat_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/members")
}

pub fn chat_member(chat_id: &str, member_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/members/{member_id}")
}

pub fn chat_hide(chat_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/hideForUser")
}

pub fn chat_unhide(chat_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/unhideForUser")
}

// --- Presence ---
pub fn my_presence() -> String {
    format!("{GRAPH_V1}/me/presence")
}

pub fn user_presence(user_id: &str) -> String {
    format!("{GRAPH_V1}/users/{user_id}/presence")
}

pub fn presence_batch() -> String {
    format!("{GRAPH_V1}/communications/getPresencesByUserId")
}

pub fn set_presence() -> String {
    format!("{GRAPH_V1}/me/presence/setPresence")
}

pub fn clear_presence() -> String {
    format!("{GRAPH_V1}/me/presence/clearPresence")
}

pub fn set_status_message() -> String {
    format!("{GRAPH_V1}/me/presence/setStatusMessage")
}

// --- Search ---
pub fn search_query() -> String {
    format!("{GRAPH_V1}/search/query")
}

// --- Tags ---
pub fn team_tags(team_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/tags")
}

pub fn team_tag(team_id: &str, tag_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/tags/{tag_id}")
}

pub fn tag_members(team_id: &str, tag_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/tags/{tag_id}/members")
}

pub fn tag_member(team_id: &str, tag_id: &str, member_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/tags/{tag_id}/members/{member_id}")
}

// --- Meetings ---
pub fn my_online_meetings() -> String {
    format!("{GRAPH_V1}/me/onlineMeetings")
}

pub fn online_meeting(meeting_id: &str) -> String {
    format!("{GRAPH_V1}/me/onlineMeetings/{meeting_id}")
}

pub fn meeting_attendance_reports(meeting_id: &str) -> String {
    format!("{GRAPH_V1}/me/onlineMeetings/{meeting_id}/attendanceReports")
}

#[allow(dead_code)]
pub fn meeting_attendance_report(meeting_id: &str, report_id: &str) -> String {
    format!("{GRAPH_V1}/me/onlineMeetings/{meeting_id}/attendanceReports/{report_id}")
}

pub fn attendance_records(meeting_id: &str, report_id: &str) -> String {
    format!(
        "{GRAPH_V1}/me/onlineMeetings/{meeting_id}/attendanceReports/{report_id}/attendanceRecords"
    )
}

// --- Notifications ---
pub fn team_send_activity(team_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/sendActivityNotification")
}

pub fn user_send_activity(user_id: &str) -> String {
    format!("{GRAPH_V1}/users/{user_id}/teamwork/sendActivityNotification")
}

pub fn chat_send_activity(chat_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/sendActivityNotification")
}

// --- Apps ---
pub fn team_installed_apps(team_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/installedApps")
}

pub fn team_installed_app(team_id: &str, installation_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/installedApps/{installation_id}")
}

// --- Tabs ---
pub fn channel_tabs(team_id: &str, channel_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/tabs")
}

pub fn channel_tab(team_id: &str, channel_id: &str, tab_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/tabs/{tab_id}")
}

// --- Subscriptions ---
pub fn subscriptions() -> String {
    format!("{GRAPH_V1}/subscriptions")
}

pub fn subscription(id: &str) -> String {
    format!("{GRAPH_V1}/subscriptions/{id}")
}

// --- Files ---
pub fn channel_files_folder(team_id: &str, channel_id: &str) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/filesFolder")
}

pub fn drive_item(drive_id: &str, item_id: &str) -> String {
    format!("{GRAPH_V1}/drives/{drive_id}/items/{item_id}")
}

pub fn drive_item_children(drive_id: &str, item_id: &str) -> String {
    format!("{GRAPH_V1}/drives/{drive_id}/items/{item_id}/children")
}

pub fn drive_item_content(drive_id: &str, item_id: &str) -> String {
    format!("{GRAPH_V1}/drives/{drive_id}/items/{item_id}/content")
}

pub fn drive_upload_to_folder(drive_id: &str, parent_id: &str, filename: &str) -> String {
    format!("{GRAPH_V1}/drives/{drive_id}/items/{parent_id}:/{filename}:/content")
}

pub fn drive_item_create_link(drive_id: &str, item_id: &str) -> String {
    format!("{GRAPH_V1}/drives/{drive_id}/items/{item_id}/createLink")
}
