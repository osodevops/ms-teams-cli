pub mod auth_code_pkce;
pub mod client_credentials;
pub mod device_code;
pub mod keyring;
pub mod token;

use crate::error::{Result, TeamsError};
use token::TokenInfo;

/// Resolve an access token using the priority chain:
/// 1. TEAMS_CLI_ACCESS_TOKEN env var
/// 2. Keyring (stored token, check expiry)
/// 3. Error
pub fn resolve_token(profile: &str) -> Result<TokenInfo> {
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
            if info.is_expired() {
                Err(TeamsError::TokenExpired)
            } else {
                Ok(info)
            }
        }
        Err(_) => Err(TeamsError::AuthError(
            "Not authenticated. Run `teams auth login` first.".into(),
        )),
    }
}
