use reqwest::Client;

use super::token::MsTokenResponse;
use crate::error::{Result, TeamsError};

/// Obtain a token using the OAuth2 client credentials flow.
/// This is for application-level (non-delegated) access.
pub async fn authenticate(
    client_id: &str,
    client_secret: &str,
    tenant_id: &str,
) -> Result<MsTokenResponse> {
    let token_url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");

    let params = [
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("scope", "https://graph.microsoft.com/.default"),
    ];

    let http = Client::new();
    let resp = http
        .post(&token_url)
        .form(&params)
        .send()
        .await
        .map_err(TeamsError::NetworkError)?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(TeamsError::AuthError(format!(
            "Client credentials auth failed ({status}): {body}"
        )));
    }

    resp.json::<MsTokenResponse>()
        .await
        .map_err(|e| TeamsError::AuthError(format!("Failed to parse token response: {e}")))
}
