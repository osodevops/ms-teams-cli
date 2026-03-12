use super::token::TokenInfo;
use crate::error::{Result, TeamsError};

const SERVICE_NAME: &str = "teams-cli";

fn entry_key(profile: &str) -> String {
    format!("{profile}:token")
}

pub fn store_token(profile: &str, token: &TokenInfo) -> Result<()> {
    let key = entry_key(profile);
    let json = serde_json::to_string(token)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to serialize token: {e}")))?;

    let entry = ::keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))?;
    entry.delete_credential().ok();
    entry
        .set_password(&json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store token: {e}")))?;
    Ok(())
}

pub fn get_token(profile: &str) -> Result<TokenInfo> {
    let key = entry_key(profile);
    let entry = ::keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))?;
    let json = entry
        .get_password()
        .map_err(|e| TeamsError::KeyringError(format!("Failed to retrieve token: {e}")))?;
    serde_json::from_str(&json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to parse stored token: {e}")))
}

pub fn delete_token(profile: &str) -> Result<()> {
    let key = entry_key(profile);
    let entry = ::keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))?;
    entry.delete_credential().ok();
    Ok(())
}

pub fn list_profiles() -> Vec<String> {
    // Keyring doesn't support enumeration natively.
    // We maintain a separate index entry.
    let entry = match ::keyring::Entry::new(SERVICE_NAME, "profile-index") {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    match entry.get_password() {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => vec![],
    }
}

pub fn add_profile_to_index(profile: &str) -> Result<()> {
    let mut profiles = list_profiles();
    if !profiles.contains(&profile.to_string()) {
        profiles.push(profile.to_string());
    }
    let json = serde_json::to_string(&profiles)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to serialize index: {e}")))?;
    let entry = ::keyring::Entry::new(SERVICE_NAME, "profile-index")
        .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))?;
    entry.delete_credential().ok();
    entry
        .set_password(&json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store index: {e}")))?;
    Ok(())
}

pub fn remove_profile_from_index(profile: &str) -> Result<()> {
    let mut profiles = list_profiles();
    profiles.retain(|p| p != profile);
    let json = serde_json::to_string(&profiles)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to serialize index: {e}")))?;
    let entry = ::keyring::Entry::new(SERVICE_NAME, "profile-index")
        .map_err(|e| TeamsError::KeyringError(format!("Failed to create keyring entry: {e}")))?;
    entry.delete_credential().ok();
    entry
        .set_password(&json)
        .map_err(|e| TeamsError::KeyringError(format!("Failed to store index: {e}")))?;
    Ok(())
}
