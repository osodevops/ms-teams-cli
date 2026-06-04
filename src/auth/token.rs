use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const GRAPH_AUDIENCES: &[&str] = &[
    "https://graph.microsoft.com",
    "https://graph.microsoft.com/",
    "00000003-0000-0000-c000-000000000000",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub token_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenClaims {
    #[serde(default)]
    pub aud: Option<serde_json::Value>,
    #[serde(default)]
    pub tid: Option<String>,
    #[serde(default)]
    pub oid: Option<String>,
    #[serde(default)]
    pub appid: Option<String>,
    #[serde(default)]
    pub azp: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub upn: Option<String>,
    #[serde(default)]
    pub scp: Option<String>,
    #[serde(default)]
    pub roles: Option<Vec<String>>,
    #[serde(default)]
    pub exp: Option<i64>,
}

impl TokenClaims {
    pub fn auth_type(&self) -> &'static str {
        if self.scp.as_deref().is_some_and(|s| !s.trim().is_empty()) {
            "delegated"
        } else if self.roles.as_ref().is_some_and(|r| !r.is_empty()) {
            "app-only"
        } else {
            "unknown"
        }
    }

    pub fn audience(&self) -> Option<String> {
        match self.aud.as_ref()? {
            serde_json::Value::String(audience) => Some(audience.clone()),
            other => Some(other.to_string()),
        }
    }

    pub fn is_graph_audience(&self) -> Option<bool> {
        let audience = self.audience()?;
        Some(GRAPH_AUDIENCES.iter().any(|expected| *expected == audience))
    }
}

impl TokenInfo {
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => Utc::now() >= expires,
            None => false, // No expiry info = assume valid
        }
    }

    pub fn bearer_header(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    pub fn unverified_claims(&self) -> Option<TokenClaims> {
        decode_unverified_claims(&self.access_token).ok()
    }
}

pub fn decode_unverified_claims(token: &str) -> Result<TokenClaims, String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| "token is not a JWT".to_string())?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("invalid JWT payload encoding: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JWT payload JSON: {e}"))
}

/// Raw token response from Microsoft Identity Platform
#[derive(Debug, Deserialize)]
pub struct MsTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

impl MsTokenResponse {
    pub fn into_token_info(self, profile: &str) -> TokenInfo {
        let expires_at = Utc::now() + chrono::Duration::seconds(self.expires_in as i64);
        TokenInfo {
            access_token: self.access_token,
            expires_at: Some(expires_at),
            token_type: self.token_type,
            scope: self.scope,
            refresh_token: self.refresh_token,
            profile: profile.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    fn make_token(expires_at: Option<DateTime<Utc>>) -> TokenInfo {
        TokenInfo {
            access_token: "test-token".into(),
            expires_at,
            token_type: "Bearer".into(),
            scope: None,
            refresh_token: None,
            profile: "default".into(),
        }
    }

    #[test]
    fn token_without_expiry_is_not_expired() {
        let token = make_token(None);
        assert!(!token.is_expired());
    }

    #[test]
    fn token_in_future_is_not_expired() {
        let token = make_token(Some(Utc::now() + chrono::Duration::hours(1)));
        assert!(!token.is_expired());
    }

    #[test]
    fn token_in_past_is_expired() {
        let token = make_token(Some(Utc::now() - chrono::Duration::hours(1)));
        assert!(token.is_expired());
    }

    #[test]
    fn bearer_header_format() {
        let token = make_token(None);
        assert_eq!(token.bearer_header(), "Bearer test-token");
    }

    #[test]
    fn ms_token_response_conversion() {
        let resp = MsTokenResponse {
            access_token: "abc".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: Some("User.Read".into()),
            refresh_token: Some("refresh".into()),
        };
        let info = resp.into_token_info("work");
        assert_eq!(info.access_token, "abc");
        assert_eq!(info.profile, "work");
        assert!(info.expires_at.is_some());
        assert!(!info.is_expired());
    }

    #[test]
    fn decodes_delegated_jwt_claims() {
        let payload = serde_json::json!({
            "aud": "https://graph.microsoft.com",
            "tid": "tenant-id",
            "oid": "user-id",
            "scp": "User.Read ChatMessage.Send"
        });
        let token = format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(payload.to_string())
        );

        let claims = decode_unverified_claims(&token).unwrap();

        assert_eq!(claims.tid.as_deref(), Some("tenant-id"));
        assert_eq!(claims.auth_type(), "delegated");
        assert_eq!(
            claims.audience().as_deref(),
            Some("https://graph.microsoft.com")
        );
        assert_eq!(claims.is_graph_audience(), Some(true));
    }

    #[test]
    fn identifies_non_graph_audience() {
        let payload = serde_json::json!({
            "aud": "5e3ce6c0-2b1f-4285-8d4b-75ee78787346"
        });
        let token = format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(payload.to_string())
        );

        let claims = decode_unverified_claims(&token).unwrap();

        assert_eq!(claims.is_graph_audience(), Some(false));
    }

    #[test]
    fn decodes_app_only_jwt_claims() {
        let payload = serde_json::json!({
            "tid": "tenant-id",
            "appid": "client-id",
            "roles": ["Team.ReadBasic.All"]
        });
        let token = format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(payload.to_string())
        );

        let claims = decode_unverified_claims(&token).unwrap();

        assert_eq!(claims.appid.as_deref(), Some("client-id"));
        assert_eq!(claims.auth_type(), "app-only");
    }

    #[test]
    fn rejects_non_jwt_token_claims() {
        let err = decode_unverified_claims("not-a-jwt").unwrap_err();

        assert_eq!(err, "token is not a JWT");
    }
}
