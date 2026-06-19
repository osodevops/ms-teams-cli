use reqwest::Client;

use super::token::MsTokenResponse;
use crate::error::{Result, TeamsError};

/// Redeem a stored refresh token for a fresh access token using the OAuth2
/// `refresh_token` grant against the Microsoft identity platform.
///
/// Mirrors the error handling of [`super::device_code::authenticate`]: network
/// failures surface as [`TeamsError::NetworkError`], non-2xx responses and parse
/// failures as [`TeamsError::AuthError`].
pub async fn refresh_access_token(
    client_id: &str,
    tenant_id: &str,
    refresh_token: &str,
    scope: &str,
) -> Result<MsTokenResponse> {
    let token_url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
    let http = Client::new();

    let resp = http
        .post(&token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("scope", scope),
        ])
        .send()
        .await
        .map_err(TeamsError::NetworkError)?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(TeamsError::AuthError(format!(
            "Token refresh failed: {body}"
        )));
    }

    resp.json::<MsTokenResponse>()
        .await
        .map_err(|e| TeamsError::AuthError(format!("Failed to parse refresh response: {e}")))
}
