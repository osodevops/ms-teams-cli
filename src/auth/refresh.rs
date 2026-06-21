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
    refresh_access_token_at(&token_url, client_id, refresh_token, scope).await
}

async fn refresh_access_token_at(
    token_url: &str,
    client_id: &str,
    refresh_token: &str,
    scope: &str,
) -> Result<MsTokenResponse> {
    let http = Client::new();

    let resp = http
        .post(token_url)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn refresh_access_token_posts_refresh_grant_and_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-access",
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": "User.Read offline_access",
                "refresh_token": "new-refresh"
            })))
            .mount(&server)
            .await;

        let response = refresh_access_token_at(
            &format!("{}/token", server.uri()),
            "client-id",
            "old-refresh",
            "User.Read offline_access",
        )
        .await
        .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);

        let body = std::str::from_utf8(&requests[0].body).unwrap();
        let form: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(body.as_bytes())
                .into_owned()
                .collect();

        assert_eq!(
            form.get("grant_type").map(String::as_str),
            Some("refresh_token")
        );
        assert_eq!(form.get("client_id").map(String::as_str), Some("client-id"));
        assert_eq!(
            form.get("refresh_token").map(String::as_str),
            Some("old-refresh")
        );
        assert_eq!(
            form.get("scope").map(String::as_str),
            Some("User.Read offline_access")
        );

        assert_eq!(response.access_token, "new-access");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, 3600);
        assert_eq!(response.scope.as_deref(), Some("User.Read offline_access"));
        assert_eq!(response.refresh_token.as_deref(), Some("new-refresh"));
    }

    #[tokio::test]
    async fn refresh_access_token_maps_non_success_to_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;

        let err = refresh_access_token_at(
            &format!("{}/token", server.uri()),
            "client-id",
            "old-refresh",
            "User.Read offline_access",
        )
        .await
        .unwrap_err();

        assert!(matches!(err, TeamsError::AuthError(message) if message.contains("invalid_grant")));
    }
}
