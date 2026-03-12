use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::presence::{
    SetPresenceRequest, SetStatusMessageBody, SetStatusMessageRequest, SetStatusExpiry,
    StatusMessageContent,
};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum PresenceCommand {
    /// Get presence status (yours or another user's)
    Get {
        /// User ID (omit for your own presence)
        #[arg(long)]
        user: Option<String>,
        /// Comma-separated user IDs for batch lookup
        #[arg(long, value_delimiter = ',')]
        users: Option<Vec<String>>,
    },
    /// Set your presence status
    Set {
        /// Availability: Available, Busy, DoNotDisturb, Away, Offline, etc.
        #[arg(long)]
        availability: String,
        /// Activity: Available, InACall, InAMeeting, Presenting, etc.
        #[arg(long)]
        activity: String,
        /// Expiration duration in ISO 8601 format (e.g., PT1H)
        #[arg(long)]
        expiration: Option<String>,
    },
    /// Set your status message
    Status {
        /// Status message text
        #[arg(long)]
        message: String,
        /// Expiry datetime in ISO 8601 format (e.g., 2024-12-31T23:59:59Z)
        #[arg(long)]
        expiry: Option<String>,
    },
    /// Clear your presence (revert to automatic)
    Clear,
}

pub async fn run(
    cmd: PresenceCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        PresenceCommand::Get { user, users } => {
            let start = Instant::now();

            if let Some(ids) = users {
                let presences = api::presence::get_presence_batch(&client, ids).await?;
                if format == OutputFormat::Human {
                    let headers = vec!["ID", "Availability", "Activity"];
                    let rows: Vec<Vec<String>> = presences
                        .iter()
                        .map(|p| {
                            vec![
                                p.id.clone().unwrap_or_default(),
                                p.availability.clone().unwrap_or_default(),
                                p.activity.clone().unwrap_or_default(),
                            ]
                        })
                        .collect();
                    output::table::print_table(headers, rows);
                } else {
                    output::print_success_list(format, &presences, start);
                }
            } else if let Some(user_id) = user {
                let presence = api::presence::get_user_presence(&client, &user_id).await?;
                output::print_success(format, &presence, start);
            } else {
                let presence = api::presence::get_my_presence(&client).await?;
                output::print_success(format, &presence, start);
            }
            Ok(())
        }

        PresenceCommand::Set {
            availability,
            activity,
            expiration,
        } => {
            let start = Instant::now();
            let req = SetPresenceRequest {
                session_id: uuid::Uuid::new_v4().to_string(),
                availability,
                activity,
                expiration_duration: expiration,
            };
            api::presence::set_presence(&client, &req).await?;
            let result = serde_json::json!({"status": "presence_set"});
            output::print_success(format, &result, start);
            Ok(())
        }

        PresenceCommand::Status { message, expiry } => {
            let start = Instant::now();
            let req = SetStatusMessageRequest {
                status_message: SetStatusMessageBody {
                    message: StatusMessageContent {
                        content: Some(message),
                        content_type: Some("text".to_string()),
                    },
                    expiry_date_time: expiry.map(|e| SetStatusExpiry {
                        date_time: e,
                        time_zone: "UTC".to_string(),
                    }),
                },
            };
            api::presence::set_status_message(&client, &req).await?;
            let result = serde_json::json!({"status": "status_message_set"});
            output::print_success(format, &result, start);
            Ok(())
        }

        PresenceCommand::Clear => {
            let start = Instant::now();
            api::presence::clear_presence(&client).await?;
            let result = serde_json::json!({"status": "presence_cleared"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
