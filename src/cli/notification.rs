use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::notification::{
    ActivityRecipient, ActivityTopic, PreviewText, SendActivityNotificationRequest,
};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum NotifyCommand {
    /// Send activity notification to user(s)
    Send {
        /// Single user ID
        #[arg(long, conflicts_with = "users")]
        user: Option<String>,
        /// Comma-separated user IDs
        #[arg(long, conflicts_with = "user")]
        users: Option<String>,
        /// Notification topic text
        #[arg(long)]
        topic: String,
        /// Activity type (must match app manifest)
        #[arg(long)]
        activity_type: String,
        /// Preview text content
        #[arg(long)]
        preview: String,
    },
    /// Send activity notification to a team
    SendToTeam {
        /// Team ID
        team_id: String,
        /// Notification topic text
        #[arg(long)]
        topic: String,
        /// Activity type
        #[arg(long)]
        activity_type: String,
        /// Preview text content
        #[arg(long)]
        preview: String,
        /// Recipient user ID (optional)
        #[arg(long)]
        recipient_user: Option<String>,
    },
    /// Send activity notification to a chat
    SendToChat {
        /// Chat ID
        chat_id: String,
        /// Notification topic text
        #[arg(long)]
        topic: String,
        /// Activity type
        #[arg(long)]
        activity_type: String,
        /// Preview text content
        #[arg(long)]
        preview: String,
        /// Recipient user ID (optional)
        #[arg(long)]
        recipient_user: Option<String>,
    },
}

fn build_request(
    topic_text: &str,
    activity_type: &str,
    preview: &str,
    recipient: Option<ActivityRecipient>,
) -> SendActivityNotificationRequest {
    SendActivityNotificationRequest {
        topic: ActivityTopic {
            source: "text".to_string(),
            value: topic_text.to_string(),
        },
        activity_type: activity_type.to_string(),
        preview_text: PreviewText {
            content: preview.to_string(),
        },
        recipient,
        template_parameters: vec![],
    }
}

pub async fn run(
    cmd: NotifyCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        NotifyCommand::Send {
            user,
            users,
            topic,
            activity_type,
            preview,
        } => {
            let start = Instant::now();
            let user_ids: Vec<String> = if let Some(uid) = user {
                vec![uid]
            } else if let Some(uids) = users {
                uids.split(',').map(|s| s.trim().to_string()).collect()
            } else {
                return Err(crate::error::TeamsError::InvalidInput(
                    "Either --user or --users must be provided".to_string(),
                ));
            };

            for uid in &user_ids {
                let req = build_request(
                    &topic,
                    &activity_type,
                    &preview,
                    Some(ActivityRecipient::user(uid.clone())),
                );
                api::notifications::send_user_notification(&client, uid, &req).await?;
            }

            let result = serde_json::json!({"status": "sent"});
            output::print_success(format, &result, start);
            Ok(())
        }

        NotifyCommand::SendToTeam {
            team_id,
            topic,
            activity_type,
            preview,
            recipient_user,
        } => {
            let start = Instant::now();
            let recipient = recipient_user.map(ActivityRecipient::user);
            let req = build_request(&topic, &activity_type, &preview, recipient);
            api::notifications::send_team_notification(&client, &team_id, &req).await?;
            let result = serde_json::json!({"status": "sent"});
            output::print_success(format, &result, start);
            Ok(())
        }

        NotifyCommand::SendToChat {
            chat_id,
            topic,
            activity_type,
            preview,
            recipient_user,
        } => {
            let start = Instant::now();
            let recipient = recipient_user.map(ActivityRecipient::user);
            let req = build_request(&topic, &activity_type, &preview, recipient);
            api::notifications::send_chat_notification(&client, &chat_id, &req).await?;
            let result = serde_json::json!({"status": "sent"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
