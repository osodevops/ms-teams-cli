use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient};
use crate::auth;
use crate::auth::token::TokenInfo;
use crate::config::{self, ConfigFile};
use crate::error::{Result, TeamsError};
use crate::models::presence::{
    ClearPresenceRequest, DateTimeTimeZone, SetPresenceRequest, SetStatusMessageBody,
    SetStatusMessageRequest, SetUserPreferredPresenceRequest, StatusMessageContent,
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
        /// Availability, paired with --activity: Available, Busy, Away or DoNotDisturb
        #[arg(long)]
        availability: String,
        /// Activity: Available, InACall, InAConferenceCall, Away or Presenting
        #[arg(long)]
        activity: String,
        /// Expiration as an ISO 8601 duration, PT5M to PT4H (default PT5M)
        #[arg(long, value_parser = parse_expiration)]
        expiration: Option<String>,
    },
    /// Set your preferred presence, which overrides every session's status while one exists
    SetPreferred {
        /// Availability: Available, Busy, DoNotDisturb, BeRightBack, Away or Offline. Graph
        /// pairs each with one activity, which is sent alongside it
        #[arg(long, value_parser = parse_preferred_availability)]
        availability: PreferredPresence,
        /// Positive ISO 8601 duration in whole units, such as PT8H or P1D. Graph's default is
        /// P1D for Busy and DoNotDisturb and P7D for the rest
        #[arg(long, value_parser = parse_preferred_expiration)]
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
    /// Clear your preferred presence, so the sessions' status shows again
    ClearPreferred,
}

/// A preferred presence is set on the user rather than on an application's session, and Graph
/// ranks it above every session while at least one exists. It accepts six availabilities, each
/// paired with exactly one activity, so the activity is derived here rather than asked for: a
/// caller cannot then send a pair Graph rejects, and the derived value is reported back so the
/// request stays visible in full.
#[derive(Debug, Clone)]
pub struct PreferredPresence {
    availability: &'static str,
    activity: &'static str,
}

const PREFERRED_PRESENCE_PAIRS: [(&str, &str); 6] = [
    ("Available", "Available"),
    ("Busy", "Busy"),
    ("DoNotDisturb", "DoNotDisturb"),
    ("BeRightBack", "BeRightBack"),
    ("Away", "Away"),
    ("Offline", "OffWork"),
];

fn parse_preferred_availability(raw: &str) -> std::result::Result<PreferredPresence, String> {
    PREFERRED_PRESENCE_PAIRS
        .iter()
        .find(|(availability, _)| availability.eq_ignore_ascii_case(raw.trim()))
        .map(|&(availability, activity)| PreferredPresence {
            availability,
            activity,
        })
        .ok_or_else(|| {
            let accepted: Vec<&str> = PREFERRED_PRESENCE_PAIRS
                .iter()
                .map(|(availability, _)| *availability)
                .collect();
            format!(
                "`{raw}` is not a preferred availability; Microsoft Graph accepts {}",
                accepted.join(", ")
            )
        })
}

/// Graph documents no bounds for a preferred presence expiration, only the defaults it applies
/// when none is sent, so the check here is that the value is a positive duration in the same
/// whole-unit form the CLI accepts for a session. Unlike a session expiration it may run to days,
/// which is why `parse_expiration` is not reused.
fn parse_preferred_expiration(raw: &str) -> std::result::Result<String, String> {
    match iso8601_duration_seconds(raw) {
        Some(seconds) if seconds > 0 => Ok(raw.to_string()),
        _ => Err(format!(
            "`{raw}` is not an ISO 8601 duration in whole units; write it as PT8H, P1D or P7D"
        )),
    }
}

/// Graph accepts a presence expiration between five minutes and four hours, and applies a
/// five-minute default when none is given, so a session set through this CLI always lapses on its
/// own. What it does not do is say clearly why it rejected a value, and an unattended caller is
/// exactly the one that cannot read a 400 and try again — hence checking the value here, before a
/// write that would otherwise leave the caller to work out whether the presence took.
fn parse_expiration(raw: &str) -> std::result::Result<String, String> {
    let seconds = iso8601_duration_seconds(raw).ok_or_else(|| {
        format!(
            "`{raw}` is not an ISO 8601 duration in whole units; \
             write it as PT30M, PT1H or PT1H30M"
        )
    })?;

    if !(EXPIRATION_MIN_SECONDS..=EXPIRATION_MAX_SECONDS).contains(&seconds) {
        return Err(format!(
            "`{raw}` is {seconds} seconds; Microsoft Graph accepts an expiration \
             from PT5M to PT4H"
        ));
    }

    Ok(raw.to_string())
}

const EXPIRATION_MIN_SECONDS: u64 = 5 * 60;
const EXPIRATION_MAX_SECONDS: u64 = 4 * 60 * 60;

/// Total an ISO 8601 duration written in whole days, hours, minutes and seconds.
///
/// Weeks, months and years are outside Graph's five-minute to four-hour window in every case, so
/// refusing them costs nothing. A fractional component is a different matter: `PT300.5S` is a
/// well-formed duration inside the window, and this parser turns it away. That is deliberate —
/// accepting it would mean carrying a decimal through the range arithmetic to express half a
/// second of expiry — but it does make the check marginally stricter than Graph, so the error
/// says which form to use rather than claiming the value is out of range.
fn iso8601_duration_seconds(raw: &str) -> Option<u64> {
    let rest = raw.strip_prefix('P')?;
    let (date, time) = match rest.split_once('T') {
        Some((_, "")) => return None,
        Some((date, time)) => (date, Some(time)),
        None => (rest, None),
    };

    let mut total = 0u64;
    let mut components = 0usize;
    total = total.checked_add(sum_components(date, &[('D', 86_400)], &mut components)?)?;
    if let Some(time) = time {
        let units = [('H', 3_600), ('M', 60), ('S', 1)];
        total = total.checked_add(sum_components(time, &units, &mut components)?)?;
    }

    (components > 0).then_some(total)
}

/// Accumulate `<digits><unit>` pairs, requiring the units to appear in the order ISO 8601 defines
/// them so that a transposition such as `PT1M1H` is rejected rather than silently reinterpreted.
fn sum_components(section: &str, units: &[(char, u64)], components: &mut usize) -> Option<u64> {
    let mut total = 0u64;
    let mut digits = String::new();
    let mut next_unit = 0usize;

    for ch in section.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        let position = units[next_unit..]
            .iter()
            .position(|(unit, _)| *unit == ch)?;
        if digits.is_empty() {
            return None;
        }
        let (_, multiplier) = units[next_unit + position];
        total = total.checked_add(digits.parse::<u64>().ok()?.checked_mul(multiplier)?)?;
        digits.clear();
        next_unit += position + 1;
        *components += 1;
    }

    digits.is_empty().then_some(total)
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

        PresenceCommand::SetPreferred {
            availability,
            expiration,
        } => {
            auth::require_delegated_token(&client.token, "Setting your preferred Teams presence")?;
            let start = Instant::now();
            let req = SetUserPreferredPresenceRequest {
                availability: availability.availability.to_string(),
                activity: availability.activity.to_string(),
                expiration_duration: expiration,
            };
            api::presence::set_user_preferred_presence(&client, &req).await?;
            let result = serde_json::json!({
                "status": "preferred_presence_set",
                "availability": req.availability,
                "activity": req.activity,
                "expiration_duration": req.expiration_duration,
            });
            output::print_success(format, &result, start);
            Ok(())
        }

        PresenceCommand::ClearPreferred => {
            auth::require_delegated_token(&client.token, "Clearing your preferred Teams presence")?;
            let start = Instant::now();
            api::presence::clear_user_preferred_presence(&client).await?;
            let result = serde_json::json!({ "status": "preferred_presence_cleared" });
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

    #[test]
    fn durations_inside_the_documented_range_are_accepted_unchanged() {
        for raw in ["PT5M", "PT1H", "PT1H30M", "PT4H", "PT300S", "PT3H59M60S"] {
            assert_eq!(parse_expiration(raw).as_deref(), Ok(raw), "{raw}");
        }
    }

    #[test]
    fn durations_outside_the_documented_range_are_rejected_with_the_bounds() {
        for raw in ["PT4M", "PT299S", "PT4H1S", "P1D", "PT0S"] {
            let err = parse_expiration(raw).unwrap_err();
            assert!(err.contains("PT5M to PT4H"), "{raw}: {err}");
        }
    }

    #[test]
    fn values_that_are_not_durations_are_rejected_before_any_request() {
        for raw in [
            "", "1h", "P", "PT", "P1DT", "PTH", "PT1", "PT-1H", "PT1.5H", "1HPT", "PT1X",
        ] {
            let err = parse_expiration(raw).unwrap_err();
            assert!(err.contains("not an ISO 8601 duration"), "{raw}: {err}");
        }
    }

    /// ISO 8601 fixes the order of the components; accepting a transposition would mean guessing
    /// at what the caller meant.
    #[test]
    fn transposed_components_are_rejected() {
        assert!(parse_expiration("PT1M1H").is_err());
        assert!(parse_expiration("PT30S5M").is_err());
    }

    #[test]
    fn the_lower_bound_is_the_five_minutes_graph_documents() {
        assert!(parse_expiration("PT4M59S").is_err());
        assert!(parse_expiration("PT5M").is_ok());
        assert!(parse_expiration("PT300S").is_ok());
    }

    #[test]
    fn a_preferred_availability_derives_the_activity_graph_pairs_with_it() {
        for (availability, activity) in PREFERRED_PRESENCE_PAIRS {
            let pair = parse_preferred_availability(availability).unwrap();
            assert_eq!(pair.availability, availability);
            assert_eq!(pair.activity, activity);
        }
        assert_eq!(
            parse_preferred_availability("Offline").unwrap().activity,
            "OffWork"
        );
    }

    #[test]
    fn a_preferred_availability_is_matched_without_regard_to_case() {
        let pair = parse_preferred_availability(" donotdisturb ").unwrap();
        assert_eq!(pair.availability, "DoNotDisturb");
        assert_eq!(pair.activity, "DoNotDisturb");
    }

    /// `set` accepts activities such as `InACall` that a preferred presence does not, so the
    /// error names the six values rather than leaving the caller to try the other command's.
    #[test]
    fn a_value_that_is_not_a_preferred_availability_is_rejected_with_the_accepted_names() {
        for raw in ["OffWork", "InACall", "Presenting", "", "banana"] {
            let err = parse_preferred_availability(raw).unwrap_err();
            assert!(
                err.contains("Available, Busy, DoNotDisturb, BeRightBack, Away, Offline"),
                "{raw}: {err}"
            );
        }
    }

    #[test]
    fn a_preferred_expiration_may_run_to_days() {
        for raw in ["PT5M", "PT8H", "P1D", "P7D", "P1DT12H"] {
            assert_eq!(parse_preferred_expiration(raw).as_deref(), Ok(raw), "{raw}");
        }
    }

    #[test]
    fn a_preferred_expiration_that_is_not_a_positive_duration_is_rejected() {
        for raw in ["", "8h", "PT0S", "P0D", "P1DT", "PT1.5H", "PT1M1H"] {
            let err = parse_preferred_expiration(raw).unwrap_err();
            assert!(err.contains("not an ISO 8601 duration"), "{raw}: {err}");
        }
    }
}
