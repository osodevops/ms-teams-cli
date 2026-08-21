use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient};
use crate::auth;
use crate::auth::token::TokenInfo;
use crate::config::ConfigFile;
use crate::error::{Result, TeamsError};
use crate::models::presence::{
    ClearPresenceRequest, DateTimeTimeZone, SetPresenceRequest, SetStatusMessageBody,
    SetStatusMessageRequest, StatusMessageContent,
};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum PresenceCommand {
    /// Get presence status (yours or another user's)
    Get {
        /// User ID (omit for your own presence)
        #[arg(long = "user", alias = "user-id")]
        user: Option<String>,
        /// Comma-separated user IDs for batch lookup
        #[arg(long = "users", alias = "user-ids", value_delimiter = ',')]
        users: Option<Vec<String>>,
    },
    /// Get presence for multiple users
    GetBatch {
        /// Comma-separated user IDs for batch lookup
        #[arg(
            long = "user-ids",
            alias = "users",
            value_delimiter = ',',
            required = true
        )]
        user_ids: Vec<String>,
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
    let token = auth::resolve_token(profile).await?;
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

        PresenceCommand::GetBatch { user_ids } => {
            let start = Instant::now();
            let presences = api::presence::get_presence_batch(&client, user_ids).await?;
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
            Ok(())
        }

        PresenceCommand::Set {
            availability,
            activity,
            expiration,
        } => {
            let start = Instant::now();
            let req = SetPresenceRequest {
                session_id: presence_session_id(&client.token)?,
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
                    expiry_date_time: expiry.map(|e| DateTimeTimeZone {
                        date_time: Some(e),
                        time_zone: Some("UTC".to_string()),
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
            let req = ClearPresenceRequest {
                session_id: presence_session_id(&client.token)?,
            };
            api::presence::clear_presence(&client, &req).await?;
            let result = serde_json::json!({"status": "presence_cleared"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}

/// Graph identifies a presence session by the application that owns it and expects that
/// application's ID as `sessionId`. Reading it from the token's own claims keeps `set` and
/// `clear` on one session regardless of what the profile's configured client ID says now,
/// and works for a token supplied through `TEAMS_CLI_ACCESS_TOKEN`.
fn presence_session_id(token: &TokenInfo) -> Result<String> {
    token
        .unverified_claims()
        .and_then(|claims| {
            [claims.azp, claims.appid]
                .into_iter()
                .flatten()
                .find(|id| !id.trim().is_empty())
        })
        .ok_or_else(|| {
            TeamsError::AuthError(
                "Access token carries no application ID claim (azp or appid), which Graph \
                 requires as the presence sessionId. Sign in again with `teams auth login`, or \
                 check the token in TEAMS_CLI_ACCESS_TOKEN if that is set."
                    .to_string(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    fn token_with_claims(claims: serde_json::Value) -> TokenInfo {
        TokenInfo {
            access_token: format!(
                "header.{}.signature",
                URL_SAFE_NO_PAD.encode(claims.to_string())
            ),
            expires_at: None,
            token_type: "Bearer".into(),
            scope: None,
            refresh_token: None,
            profile: "default".into(),
        }
    }

    #[test]
    fn session_id_prefers_the_authorized_party_claim() {
        let token = token_with_claims(serde_json::json!({
            "azp": "authorized-party",
            "appid": "application-id"
        }));
        assert_eq!(presence_session_id(&token).unwrap(), "authorized-party");
    }

    #[test]
    fn session_id_falls_back_to_the_application_id_claim() {
        let token = token_with_claims(serde_json::json!({ "appid": "application-id" }));
        assert_eq!(presence_session_id(&token).unwrap(), "application-id");
    }

    #[test]
    fn session_id_skips_an_empty_authorized_party_claim() {
        let token = token_with_claims(serde_json::json!({
            "azp": "",
            "appid": "application-id"
        }));
        assert_eq!(presence_session_id(&token).unwrap(), "application-id");
    }

    #[test]
    fn session_id_rejects_blank_application_claims() {
        let token = token_with_claims(serde_json::json!({ "azp": "", "appid": "   " }));
        assert!(matches!(
            presence_session_id(&token),
            Err(TeamsError::AuthError(_))
        ));
    }

    #[test]
    fn session_id_rejects_a_token_without_an_application_claim() {
        let token = token_with_claims(serde_json::json!({ "tid": "tenant-id" }));
        assert!(matches!(
            presence_session_id(&token),
            Err(TeamsError::AuthError(_))
        ));
    }

    #[test]
    fn session_id_rejects_an_undecodable_token() {
        let mut token = token_with_claims(serde_json::json!({ "appid": "application-id" }));
        token.access_token = "not-a-jwt".to_string();
        assert!(matches!(
            presence_session_id(&token),
            Err(TeamsError::AuthError(_))
        ));
    }
}
