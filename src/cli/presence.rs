use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient};
use crate::auth;
use crate::auth::token::TokenInfo;
use crate::config::{self, ConfigFile};
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
            auth::require_delegated_token(&client.token, "Setting your Teams presence")?;
            let start = Instant::now();
            let req = SetPresenceRequest {
                session_id: presence_session_id(
                    &client.token,
                    config::resolve_client_id(None, profile, config),
                )?,
                availability,
                activity,
                expiration_duration: expiration,
            };
            let session_id = req.session_id.clone();
            api::presence::set_presence(&client, &req).await?;
            let result = serde_json::json!({
                "status": "presence_set",
                "session_id": session_id,
            });
            output::print_success(format, &result, start);
            Ok(())
        }

        PresenceCommand::Status { message, expiry } => {
            auth::require_delegated_token(&client.token, "Setting your Teams status message")?;
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
            auth::require_delegated_token(&client.token, "Clearing your Teams presence")?;
            let start = Instant::now();
            let req = ClearPresenceRequest {
                session_id: presence_session_id(
                    &client.token,
                    config::resolve_client_id(None, profile, config),
                )?,
            };
            let session_id = req.session_id.clone();
            let cleared = api::presence::clear_presence(&client, &req).await?;
            let result = serde_json::json!({
                "status": if cleared {
                    "presence_cleared"
                } else {
                    "no_presence_session"
                },
                "session_id": session_id,
            });
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}

/// Graph identifies a presence session by the application that owns it and expects that
/// application's ID as `sessionId`. A configured client ID names that application directly and
/// wins, because Microsoft asks callers to treat access tokens as opaque and a Graph token is
/// not guaranteed to be a readable JSON Web Token. Logins through the built-in application
/// configure no client ID, so the token's own `azp` or `appid` claim stands in. `set` and
/// `clear` agree either way, because both resolve the value the same way.
fn presence_session_id(token: &TokenInfo, configured_client_id: Option<String>) -> Result<String> {
    let configured = configured_client_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());
    if let Some(client_id) = configured {
        return Ok(client_id);
    }

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
                "No application ID is available for the presence sessionId, which Graph \
                 requires: the access token carries no readable azp or appid claim, and no \
                 client ID is configured. Set TEAMS_CLI_CLIENT_ID or the profile's client_id \
                 to the application the token was issued to, or sign in again with `teams auth \
                 login`."
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
    fn session_id_prefers_a_configured_client_id_over_the_token_claims() {
        let token = token_with_claims(serde_json::json!({ "azp": "authorized-party" }));
        assert_eq!(
            presence_session_id(&token, Some("configured-client".to_string())).unwrap(),
            "configured-client"
        );
    }

    #[test]
    fn session_id_falls_back_to_the_token_when_the_configured_client_id_is_blank() {
        let token = token_with_claims(serde_json::json!({ "azp": "authorized-party" }));
        assert_eq!(
            presence_session_id(&token, Some("   ".to_string())).unwrap(),
            "authorized-party"
        );
    }

    #[test]
    fn session_id_accepts_an_opaque_token_when_a_client_id_is_configured() {
        let mut token = token_with_claims(serde_json::json!({ "azp": "authorized-party" }));
        token.access_token = "opaque-token".to_string();
        assert_eq!(
            presence_session_id(&token, Some("configured-client".to_string())).unwrap(),
            "configured-client"
        );
    }

    #[test]
    fn session_id_prefers_the_authorized_party_claim() {
        let token = token_with_claims(serde_json::json!({
            "azp": "authorized-party",
            "appid": "application-id"
        }));
        assert_eq!(
            presence_session_id(&token, None).unwrap(),
            "authorized-party"
        );
    }

    #[test]
    fn session_id_falls_back_to_the_application_id_claim() {
        let token = token_with_claims(serde_json::json!({ "appid": "application-id" }));
        assert_eq!(presence_session_id(&token, None).unwrap(), "application-id");
    }

    #[test]
    fn session_id_skips_an_empty_authorized_party_claim() {
        let token = token_with_claims(serde_json::json!({
            "azp": "",
            "appid": "application-id"
        }));
        assert_eq!(presence_session_id(&token, None).unwrap(), "application-id");
    }

    #[test]
    fn session_id_rejects_blank_application_claims() {
        let token = token_with_claims(serde_json::json!({ "azp": "", "appid": "   " }));
        assert!(matches!(
            presence_session_id(&token, None),
            Err(TeamsError::AuthError(_))
        ));
    }

    #[test]
    fn session_id_rejects_a_token_without_an_application_claim() {
        let token = token_with_claims(serde_json::json!({ "tid": "tenant-id" }));
        assert!(matches!(
            presence_session_id(&token, None),
            Err(TeamsError::AuthError(_))
        ));
    }

    #[test]
    fn session_id_rejects_an_undecodable_token() {
        let mut token = token_with_claims(serde_json::json!({ "appid": "application-id" }));
        token.access_token = "not-a-jwt".to_string();
        assert!(matches!(
            presence_session_id(&token, None),
            Err(TeamsError::AuthError(_))
        ));
    }
}
