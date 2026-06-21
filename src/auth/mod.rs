pub mod auth_code_pkce;
pub mod client_credentials;
pub mod device_code;
pub mod keyring;
pub mod refresh;
pub mod token;

use chrono::{Duration, Utc};

use crate::config::DEFAULT_DELEGATED_SCOPES;
use crate::error::{Result, TeamsError};
use token::TokenInfo;

/// Refresh slightly ahead of the hard expiry so a token does not lapse
/// mid-request.
const EXPIRY_SKEW_SECS: i64 = 60;

/// Resolve an access token using the priority chain:
/// 1. TEAMS_CLI_ACCESS_TOKEN env var (used verbatim, never refreshed)
/// 2. Keyring: if the stored token is (near) expired, silently redeem its
///    refresh token; otherwise return it as-is
/// 3. Error
pub async fn resolve_token(profile: &str) -> Result<TokenInfo> {
    // 1. Direct env var override
    if let Ok(token) = std::env::var("TEAMS_CLI_ACCESS_TOKEN") {
        return Ok(TokenInfo {
            access_token: token,
            expires_at: None,
            token_type: "Bearer".to_string(),
            scope: None,
            refresh_token: None,
            profile: profile.to_string(),
        });
    }

    // 2. Keyring
    match keyring::get_token(profile) {
        Ok(info) => {
            if !needs_refresh(&info) {
                return Ok(info);
            }
            // Token is at/near expiry: attempt a silent refresh-token
            // redemption. If the token is still inside the skew window, keep
            // using it on refresh failure; only hard-expired tokens become
            // TokenExpired.
            match attempt_refresh(profile, &info).await {
                Ok(refreshed) => Ok(refreshed),
                Err(e) => handle_refresh_failure(&info, e),
            }
        }
        Err(_) => Err(TeamsError::AuthError(
            "Not authenticated. Run `teams auth login` first.".into(),
        )),
    }
}

/// Whether the token should be refreshed: already expired, or within the skew
/// window of expiry. Tokens with no expiry information are assumed valid.
fn needs_refresh(info: &TokenInfo) -> bool {
    info.is_expired()
        || matches!(
            info.expires_at,
            Some(expires) if Utc::now() + Duration::seconds(EXPIRY_SKEW_SECS) >= expires
        )
}

fn handle_refresh_failure(info: &TokenInfo, error: TeamsError) -> Result<TokenInfo> {
    if info.is_expired() {
        tracing::debug!("Silent token refresh failed for expired token: {error}");
        Err(TeamsError::TokenExpired)
    } else {
        tracing::debug!("Silent token refresh failed before expiry, using current token: {error}");
        Ok(info.clone())
    }
}

/// Silently redeem the stored refresh token for a fresh access token and persist
/// it. Returns an error (mapped to `TokenExpired` by the caller) when the token
/// cannot be refreshed — e.g. no refresh token, undecodable claims, or the
/// identity platform rejecting the request.
async fn attempt_refresh(profile: &str, info: &TokenInfo) -> Result<TokenInfo> {
    let refresh_token = info
        .refresh_token
        .as_deref()
        .ok_or(TeamsError::TokenExpired)?;

    // The expired JWT still decodes (decoding ignores expiry); recover the
    // tenant + client id originally authenticated with from its claims.
    let claims = info.unverified_claims().ok_or(TeamsError::TokenExpired)?;
    let tenant_id = claims.tid.ok_or(TeamsError::TokenExpired)?;
    let client_id = claims
        .azp
        .or(claims.appid)
        .ok_or(TeamsError::TokenExpired)?;

    let scope = refresh_scope(info.scope.as_deref());

    let response =
        refresh::refresh_access_token(&client_id, &tenant_id, refresh_token, &scope).await?;
    let refreshed = refreshed_token_info(profile, info, response);

    // Best-effort persistence so the next invocation reuses the new token.
    let _ = keyring::store_token(profile, &refreshed);

    Ok(refreshed)
}

fn refreshed_token_info(
    profile: &str,
    previous: &TokenInfo,
    response: token::MsTokenResponse,
) -> TokenInfo {
    let mut refreshed = response.into_token_info(profile);

    if refreshed.refresh_token.is_none() {
        refreshed.refresh_token.clone_from(&previous.refresh_token);
    }

    if refreshed.scope.is_none() {
        refreshed.scope.clone_from(&previous.scope);
    }

    refreshed
}

/// Build the scope string for the refresh request: reuse the originally granted
/// scope (ensuring `offline_access` so future refreshes keep working), or fall
/// back to the default delegated scopes.
fn refresh_scope(existing: Option<&str>) -> String {
    match existing {
        Some(scope) if !scope.trim().is_empty() => {
            if scope.split_whitespace().any(|s| s == "offline_access") {
                scope.to_string()
            } else {
                format!("{scope} offline_access")
            }
        }
        _ => DEFAULT_DELEGATED_SCOPES.to_string(),
    }
}

pub fn require_delegated_token(token: &TokenInfo, operation: &str) -> Result<()> {
    if let Some(claims) = token.unverified_claims() {
        if claims.auth_type() == "app-only" {
            return Err(TeamsError::PermissionDenied(format!(
                "{operation} requires delegated Microsoft Graph auth. App-only/client-credentials tokens cannot send normal live Teams chat or channel messages; use `teams auth login --device-code` or future bot mode."
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_with(
        expires_at: Option<chrono::DateTime<Utc>>,
        refresh_token: Option<&str>,
        scope: Option<&str>,
    ) -> TokenInfo {
        TokenInfo {
            access_token: "not-a-jwt".into(),
            expires_at,
            token_type: "Bearer".into(),
            scope: scope.map(|s| s.to_string()),
            refresh_token: refresh_token.map(|s| s.to_string()),
            profile: "default".into(),
        }
    }

    #[test]
    fn needs_refresh_true_when_expired() {
        let info = token_with(Some(Utc::now() - Duration::hours(1)), None, None);
        assert!(needs_refresh(&info));
    }

    #[test]
    fn needs_refresh_true_within_skew_window() {
        // Expires in 30s, which is inside the 60s skew window.
        let info = token_with(Some(Utc::now() + Duration::seconds(30)), None, None);
        assert!(needs_refresh(&info));
    }

    #[test]
    fn needs_refresh_false_when_well_in_future() {
        let info = token_with(Some(Utc::now() + Duration::hours(1)), None, None);
        assert!(!needs_refresh(&info));
    }

    #[test]
    fn needs_refresh_false_without_expiry() {
        let info = token_with(None, None, None);
        assert!(!needs_refresh(&info));
    }

    #[test]
    fn refresh_scope_falls_back_to_defaults_when_absent() {
        assert_eq!(refresh_scope(None), DEFAULT_DELEGATED_SCOPES);
        assert_eq!(refresh_scope(Some("   ")), DEFAULT_DELEGATED_SCOPES);
    }

    #[test]
    fn refresh_scope_appends_offline_access_when_missing() {
        assert_eq!(
            refresh_scope(Some("User.Read Chat.ReadWrite")),
            "User.Read Chat.ReadWrite offline_access"
        );
    }

    #[test]
    fn refresh_scope_preserves_existing_offline_access() {
        assert_eq!(
            refresh_scope(Some("User.Read offline_access")),
            "User.Read offline_access"
        );
    }

    #[test]
    fn refresh_failure_returns_current_token_inside_skew_window() {
        let info = token_with(
            Some(Utc::now() + Duration::seconds(30)),
            Some("a-refresh-token"),
            Some("User.Read offline_access"),
        );

        let resolved =
            handle_refresh_failure(&info, TeamsError::AuthError("temporary failure".into()))
                .unwrap();

        assert_eq!(resolved.access_token, info.access_token);
        assert_eq!(resolved.refresh_token, info.refresh_token);
    }

    #[test]
    fn refresh_failure_expires_hard_expired_token() {
        let info = token_with(
            Some(Utc::now() - Duration::seconds(1)),
            Some("a-refresh-token"),
            Some("User.Read offline_access"),
        );

        let err =
            handle_refresh_failure(&info, TeamsError::AuthError("rejected".into())).unwrap_err();

        assert!(matches!(err, TeamsError::TokenExpired));
    }

    #[test]
    fn refreshed_token_info_preserves_previous_refresh_token_and_scope_when_missing() {
        let previous = token_with(
            Some(Utc::now() - Duration::hours(1)),
            Some("old-refresh"),
            Some("User.Read offline_access"),
        );
        let response = token::MsTokenResponse {
            access_token: "new-access".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: None,
            refresh_token: None,
        };

        let refreshed = refreshed_token_info("work", &previous, response);

        assert_eq!(refreshed.access_token, "new-access");
        assert_eq!(refreshed.profile, "work");
        assert_eq!(refreshed.scope.as_deref(), Some("User.Read offline_access"));
        assert_eq!(refreshed.refresh_token.as_deref(), Some("old-refresh"));
    }

    #[test]
    fn refreshed_token_info_uses_rotated_refresh_token_and_scope_when_present() {
        let previous = token_with(
            Some(Utc::now() - Duration::hours(1)),
            Some("old-refresh"),
            Some("User.Read offline_access"),
        );
        let response = token::MsTokenResponse {
            access_token: "new-access".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: Some("User.Read Chat.ReadWrite offline_access".into()),
            refresh_token: Some("new-refresh".into()),
        };

        let refreshed = refreshed_token_info("work", &previous, response);

        assert_eq!(
            refreshed.scope.as_deref(),
            Some("User.Read Chat.ReadWrite offline_access")
        );
        assert_eq!(refreshed.refresh_token.as_deref(), Some("new-refresh"));
    }

    #[tokio::test]
    async fn attempt_refresh_without_refresh_token_is_token_expired() {
        // Expired token, no refresh token: must fail fast as TokenExpired with
        // no network call.
        let info = token_with(Some(Utc::now() - Duration::hours(1)), None, None);
        let err = attempt_refresh("default", &info).await.unwrap_err();
        assert!(matches!(err, TeamsError::TokenExpired));
    }

    #[tokio::test]
    async fn attempt_refresh_with_undecodable_claims_is_token_expired() {
        // Has a refresh token, but the access token is not a decodable JWT so we
        // cannot recover tenant/client id: must fail fast as TokenExpired.
        let info = token_with(
            Some(Utc::now() - Duration::hours(1)),
            Some("a-refresh-token"),
            None,
        );
        let err = attempt_refresh("default", &info).await.unwrap_err();
        assert!(matches!(err, TeamsError::TokenExpired));
    }
}
