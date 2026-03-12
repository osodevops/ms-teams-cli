use crate::error::Result;
use crate::models::notification::SendActivityNotificationRequest;

use super::client::GraphClient;
use super::endpoints;

pub async fn send_team_notification(
    client: &GraphClient,
    team_id: &str,
    req: &SendActivityNotificationRequest,
) -> Result<()> {
    client
        .post_no_content(&endpoints::team_send_activity(team_id), req)
        .await
}

pub async fn send_user_notification(
    client: &GraphClient,
    user_id: &str,
    req: &SendActivityNotificationRequest,
) -> Result<()> {
    client
        .post_no_content(&endpoints::user_send_activity(user_id), req)
        .await
}

pub async fn send_chat_notification(
    client: &GraphClient,
    chat_id: &str,
    req: &SendActivityNotificationRequest,
) -> Result<()> {
    client
        .post_no_content(&endpoints::chat_send_activity(chat_id), req)
        .await
}
