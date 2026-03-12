use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use super::token::MsTokenResponse;
use crate::error::{Result, TeamsError};

const DEFAULT_SCOPES: &str = "User.Read Team.ReadBasic.All Channel.ReadBasic.All ChannelMessage.Send ChannelMessage.Read.All Chat.ReadWrite ChatMessage.Send ChatMessage.Read User.ReadBasic.All Presence.Read.All offline_access";

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(dead_code)]
    expires_in: u64,
    interval: u64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct PollResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

/// Initiate a device code flow and poll until the user completes authentication.
pub async fn authenticate(
    client_id: &str,
    tenant_id: &str,
    scopes: Option<&str>,
) -> Result<MsTokenResponse> {
    let scopes = scopes.unwrap_or(DEFAULT_SCOPES);
    let http = Client::new();

    // Step 1: Request device code
    let device_code_url =
        format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/devicecode");

    let resp = http
        .post(&device_code_url)
        .form(&[("client_id", client_id), ("scope", scopes)])
        .send()
        .await
        .map_err(TeamsError::NetworkError)?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(TeamsError::AuthError(format!(
            "Device code request failed: {body}"
        )));
    }

    let dc: DeviceCodeResponse = resp
        .json()
        .await
        .map_err(|e| TeamsError::AuthError(format!("Failed to parse device code response: {e}")))?;

    // Display instructions to user
    eprintln!();
    eprintln!("{}", dc.message);
    eprintln!();
    eprintln!("  URL:  {}", dc.verification_uri);
    eprintln!("  Code: {}", dc.user_code);
    eprintln!();

    // Step 2: Poll for token
    let token_url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
    let interval = Duration::from_secs(dc.interval.max(5));

    loop {
        tokio::time::sleep(interval).await;

        let resp = http
            .post(&token_url)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", &dc.device_code),
            ])
            .send()
            .await
            .map_err(TeamsError::NetworkError)?;

        let poll: PollResponse = resp
            .json()
            .await
            .map_err(|e| TeamsError::AuthError(format!("Failed to parse poll response: {e}")))?;

        match poll.error.as_deref() {
            Some("authorization_pending") => {
                tracing::debug!("Authorization pending, polling again...");
                continue;
            }
            Some("slow_down") => {
                tracing::debug!("Slow down requested, increasing interval");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Some("authorization_declined") => {
                return Err(TeamsError::AuthError(
                    "Authorization was declined by the user".into(),
                ));
            }
            Some("expired_token") => {
                return Err(TeamsError::AuthError(
                    "Device code expired. Please try again.".into(),
                ));
            }
            Some(other) => {
                return Err(TeamsError::AuthError(format!(
                    "Device code auth failed: {other}"
                )));
            }
            None => {
                // Success
                let access_token = poll.access_token.ok_or_else(|| {
                    TeamsError::AuthError("Missing access_token in response".into())
                })?;
                return Ok(MsTokenResponse {
                    access_token,
                    token_type: poll.token_type.unwrap_or_else(|| "Bearer".into()),
                    expires_in: poll.expires_in.unwrap_or(3600),
                    scope: poll.scope,
                    refresh_token: poll.refresh_token,
                });
            }
        }
    }
}
