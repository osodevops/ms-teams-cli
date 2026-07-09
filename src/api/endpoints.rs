pub const GRAPH_V1: &str = "https://graph.microsoft.com/v1.0";
pub const GRAPH_BETA: &str = "https://graph.microsoft.com/beta";

// --- Users ---
pub fn me() -> String {
    format!("{GRAPH_V1}/me")
}

pub fn user(id: &str) -> String {
    user_at(GRAPH_V1, id)
}

pub fn user_at(base: &str, id: &str) -> String {
    format!("{base}/users/{id}")
}

pub fn users() -> String {
    format!("{GRAPH_V1}/users")
}

pub fn my_people_at(base: &str) -> String {
    format!("{base}/me/people")
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
    my_chats_at(GRAPH_V1)
}

pub fn my_chats_at(base: &str) -> String {
    format!("{base}/me/chats")
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
    chat_members_at(GRAPH_V1, chat_id)
}

pub fn chat_members_at(base: &str, chat_id: &str) -> String {
    format!("{base}/chats/{chat_id}/members")
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
    let name = urlencoding::encode(filename);
    format!("{GRAPH_V1}/drives/{drive_id}/items/{parent_id}:/{name}:/content")
}

/// Upload into a drive folder, renaming on filename collision instead of
/// replacing — used for message attachments, where clobbering a previously
/// shared file would silently rewrite history.
pub fn drive_upload_to_folder_rename(drive_id: &str, parent_id: &str, filename: &str) -> String {
    let name = urlencoding::encode(filename);
    format!(
        "{GRAPH_V1}/drives/{drive_id}/items/{parent_id}:/{name}:/content?@microsoft.graph.conflictBehavior=rename"
    )
}

pub fn me_drive_root_children() -> String {
    format!("{GRAPH_V1}/me/drive/root/children")
}

/// Upload into the signed-in user's OneDrive `Microsoft Teams Chat Files`
/// folder, where the Teams client itself puts files attached to chats.
pub fn me_chat_files_upload(filename: &str) -> String {
    let name = urlencoding::encode(filename);
    format!(
        "{GRAPH_V1}/me/drive/root:/Microsoft%20Teams%20Chat%20Files/{name}:/content?@microsoft.graph.conflictBehavior=rename"
    )
}

pub fn drive_item_create_link(drive_id: &str, item_id: &str) -> String {
    format!("{GRAPH_V1}/drives/{drive_id}/items/{item_id}/createLink")
}

// --- Hosted contents (inline images, code snippets) ---

pub fn channel_message_hosted_contents(
    team_id: &str,
    channel_id: &str,
    message_id: &str,
) -> String {
    format!("{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/hostedContents")
}

pub fn channel_message_hosted_content_value(
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    hosted_content_id: &str,
) -> String {
    let id = urlencoding::encode(hosted_content_id);
    format!(
        "{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/hostedContents/{id}/$value"
    )
}

pub fn channel_reply_hosted_contents(
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    reply_id: &str,
) -> String {
    format!(
        "{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/replies/{reply_id}/hostedContents"
    )
}

pub fn channel_reply_hosted_content_value(
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    reply_id: &str,
    hosted_content_id: &str,
) -> String {
    let id = urlencoding::encode(hosted_content_id);
    format!(
        "{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/replies/{reply_id}/hostedContents/{id}/$value"
    )
}

pub fn channel_message_reply(
    team_id: &str,
    channel_id: &str,
    message_id: &str,
    reply_id: &str,
) -> String {
    format!(
        "{GRAPH_V1}/teams/{team_id}/channels/{channel_id}/messages/{message_id}/replies/{reply_id}"
    )
}

pub fn chat_message_hosted_contents(chat_id: &str, message_id: &str) -> String {
    format!("{GRAPH_V1}/chats/{chat_id}/messages/{message_id}/hostedContents")
}

pub fn chat_message_hosted_content_value(
    chat_id: &str,
    message_id: &str,
    hosted_content_id: &str,
) -> String {
    let id = urlencoding::encode(hosted_content_id);
    format!("{GRAPH_V1}/chats/{chat_id}/messages/{message_id}/hostedContents/{id}/$value")
}

// --- Shares (resolving attachment contentUrl to a drive item) ---

/// Encode a SharePoint/OneDrive web URL as a Graph sharing token: `u!` plus
/// unpadded base64url of the URL, per the Graph shares API convention.
pub fn sharing_url_token(url: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    format!("u!{}", URL_SAFE_NO_PAD.encode(url))
}

pub fn shares_drive_item_content(sharing_token: &str) -> String {
    format!("{GRAPH_V1}/shares/{sharing_token}/driveItem/content")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharing_url_token_matches_graph_convention() {
        // Expected value computed with: printf '%s' "$URL" | base64 | tr '+/' '-_' | tr -d '='
        let url = "https://tenant-my.sharepoint.com/personal/user/Documents/Microsoft%20Teams%20Chat%20Files/NetskopeLogs.zip";
        let token = sharing_url_token(url);
        assert!(token.starts_with("u!"));
        assert!(!token.contains('='), "token must be unpadded");
        assert!(
            !token.contains('+') && !token.contains('/'),
            "token must be base64url"
        );
        assert_eq!(
            token,
            "u!aHR0cHM6Ly90ZW5hbnQtbXkuc2hhcmVwb2ludC5jb20vcGVyc29uYWwvdXNlci9Eb2N1bWVudHMvTWljcm9zb2Z0JTIwVGVhbXMlMjBDaGF0JTIwRmlsZXMvTmV0c2tvcGVMb2dzLnppcA"
        );
    }

    #[test]
    fn hosted_content_value_encodes_id() {
        // Base64 IDs can contain '+', '/', and '=' which must be percent-encoded in a path.
        let url = channel_message_hosted_content_value("t1", "c1", "m1", "aWQ9+x/z==");
        assert!(url.ends_with("/hostedContents/aWQ9%2Bx%2Fz%3D%3D/$value"));
        assert!(
            url.starts_with("https://graph.microsoft.com/v1.0/teams/t1/channels/c1/messages/m1/")
        );
    }

    #[test]
    fn chat_files_upload_encodes_filename_and_renames_on_conflict() {
        let url = me_chat_files_upload("Net skope Logs.zip");
        assert_eq!(
            url,
            "https://graph.microsoft.com/v1.0/me/drive/root:/Microsoft%20Teams%20Chat%20Files/Net%20skope%20Logs.zip:/content?@microsoft.graph.conflictBehavior=rename"
        );
    }

    #[test]
    fn drive_uploads_encode_filename_path_segment() {
        // '#' and '?' would otherwise be parsed as fragment/query, silently
        // truncating the DriveItem path.
        let url = drive_upload_to_folder_rename("d1", "p1", "report #3?.xlsx");
        assert_eq!(
            url,
            "https://graph.microsoft.com/v1.0/drives/d1/items/p1:/report%20%233%3F.xlsx:/content?@microsoft.graph.conflictBehavior=rename"
        );
        assert_eq!(
            drive_upload_to_folder("d1", "p1", "a b.txt"),
            "https://graph.microsoft.com/v1.0/drives/d1/items/p1:/a%20b.txt:/content"
        );
    }

    #[test]
    fn chat_hosted_contents_urls() {
        assert_eq!(
            chat_message_hosted_contents("19:abc", "m1"),
            "https://graph.microsoft.com/v1.0/chats/19:abc/messages/m1/hostedContents"
        );
        assert!(chat_message_hosted_content_value("19:abc", "m1", "hc1")
            .ends_with("/chats/19:abc/messages/m1/hostedContents/hc1/$value"));
    }
}
