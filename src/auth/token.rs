use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
}
