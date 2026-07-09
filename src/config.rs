use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Result, TeamsError};

pub const OSO_PUBLIC_CLIENT_ID: &str = "fba1b5d0-fdd0-4fe2-9729-9ccdc38f9595";
pub const DEFAULT_DELEGATED_TENANT_ID: &str = "organizations";
pub const DEFAULT_DELEGATED_SCOPES: &str = "User.Read Team.ReadBasic.All Channel.ReadBasic.All ChannelMessage.Send Chat.ReadWrite ChatMessage.Send ChatMessage.Read User.ReadBasic.All Presence.Read.All offline_access";
pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8400/callback";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub default: DefaultConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DefaultConfig {
    pub profile: Option<String>,
    pub output: Option<String>,
    pub api_version: Option<String>,
    pub page_size: Option<u64>,
    pub timeout: Option<u64>,
    pub retry: Option<u32>,
    pub color: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_true")]
    pub color: bool,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            color: true,
            page_size: default_page_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_backoff_base")]
    pub retry_backoff_base: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            max_retries: default_max_retries(),
            retry_backoff_base: default_backoff_base(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    pub auth_app: Option<String>,
    pub client_id: Option<String>,
    pub tenant_id: Option<String>,
    pub auth_flow: Option<String>,
    pub scopes: Option<String>,
}

fn default_format() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}
fn default_page_size() -> u64 {
    50
}
fn default_timeout() -> u64 {
    30
}
fn default_max_retries() -> u32 {
    3
}
fn default_backoff_base() -> u64 {
    2
}

pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| TeamsError::ConfigError("Cannot determine config directory".into()))?;
    Ok(dir.join("teams-cli"))
}

pub fn default_config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn load_config(path: Option<&str>) -> Result<ConfigFile> {
    let config_path = match path {
        Some(p) => PathBuf::from(p),
        None => default_config_path()?,
    };

    if !config_path.exists() {
        return Ok(ConfigFile::default());
    }

    let content = fs::read_to_string(&config_path).map_err(|e| {
        TeamsError::ConfigError(format!(
            "Failed to read config at {}: {e}",
            config_path.display()
        ))
    })?;

    toml::from_str(&content)
        .map_err(|e| TeamsError::ConfigError(format!("Invalid config TOML: {e}")))
}

pub fn save_config(config: &ConfigFile, path: Option<&str>) -> Result<()> {
    let config_path = match path {
        Some(p) => PathBuf::from(p),
        None => default_config_path()?,
    };

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| TeamsError::ConfigError(format!("Failed to create config dir: {e}")))?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| TeamsError::ConfigError(format!("Failed to serialize config: {e}")))?;

    fs::write(&config_path, &content).map_err(|e| {
        TeamsError::ConfigError(format!(
            "Failed to write config to {}: {e}",
            config_path.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        fs::set_permissions(&config_path, perms).ok();
    }

    Ok(())
}

pub fn resolve_profile<'a>(cli_profile: &'a str, config: &'a ConfigFile) -> &'a str {
    if cli_profile != "default" {
        return cli_profile;
    }
    config.default.profile.as_deref().unwrap_or("default")
}

pub fn resolve_client_id(
    cli_flag: Option<&str>,
    profile: &str,
    config: &ConfigFile,
) -> Option<String> {
    if let Some(v) = cli_flag {
        return Some(v.to_string());
    }
    if let Ok(v) = std::env::var("TEAMS_CLI_CLIENT_ID") {
        return Some(v);
    }
    config
        .profiles
        .get(profile)
        .and_then(|p| p.client_id.clone())
}

pub fn resolve_delegated_client_id(
    cli_flag: Option<&str>,
    profile: &str,
    config: &ConfigFile,
) -> Result<String> {
    if let Some(client_id) = resolve_client_id(cli_flag, profile, config) {
        return Ok(client_id);
    }

    let auth_app = config
        .profiles
        .get(profile)
        .and_then(|p| p.auth_app.as_deref())
        .unwrap_or("oso");

    match auth_app {
        "oso" => Ok(OSO_PUBLIC_CLIENT_ID.to_string()),
        "byo" => Err(TeamsError::InvalidInput(
            "Client ID is required for auth_app = \"byo\". Use --client-id or set TEAMS_CLI_CLIENT_ID".into(),
        )),
        other => Err(TeamsError::InvalidInput(format!(
            "Unsupported auth_app '{other}'. Use 'oso' or 'byo'."
        ))),
    }
}

pub fn resolve_tenant_id(
    cli_flag: Option<&str>,
    profile: &str,
    config: &ConfigFile,
) -> Option<String> {
    if let Some(v) = cli_flag {
        return Some(v.to_string());
    }
    if let Ok(v) = std::env::var("TEAMS_CLI_TENANT_ID") {
        return Some(v);
    }
    config
        .profiles
        .get(profile)
        .and_then(|p| p.tenant_id.clone())
}

pub fn resolve_delegated_tenant_id(
    cli_flag: Option<&str>,
    profile: &str,
    config: &ConfigFile,
) -> String {
    resolve_tenant_id(cli_flag, profile, config)
        .unwrap_or_else(|| DEFAULT_DELEGATED_TENANT_ID.to_string())
}

/// Append `offline_access` to a delegated scope string when it is missing, so
/// the identity platform always issues a refresh token.
pub fn ensure_offline_access(scopes: &str) -> String {
    let trimmed = scopes.trim();
    if trimmed.split_whitespace().any(|s| s == "offline_access") {
        trimmed.to_string()
    } else {
        format!("{trimmed} offline_access")
    }
}

/// Resolve an explicitly configured delegated scope override: CLI flag or
/// `TEAMS_CLI_SCOPES` env var (delivered via clap), then the profile's
/// `scopes`. Returns `None` when neither is set so callers can pick their own
/// fallback — login falls back to the defaults, refresh to the stored token's
/// scope. Blank values fall through so an empty env var cannot produce an
/// empty OAuth scope request. Overrides always get `offline_access` appended.
pub fn resolve_delegated_scopes_override(
    scopes_arg: Option<&str>,
    profile: &str,
    config: &ConfigFile,
) -> Option<String> {
    let non_blank = |s: &str| {
        let trimmed = s.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };

    scopes_arg
        .and_then(non_blank)
        .or_else(|| {
            config
                .profiles
                .get(profile)
                .and_then(|p| p.scopes.as_deref())
                .and_then(non_blank)
        })
        .map(|scopes| ensure_offline_access(&scopes))
}

/// Resolve the delegated scope string for login: the explicit override when
/// set, otherwise the default delegated scopes.
pub fn resolve_delegated_scopes(
    scopes_arg: Option<&str>,
    profile: &str,
    config: &ConfigFile,
) -> String {
    resolve_delegated_scopes_override(scopes_arg, profile, config)
        .unwrap_or_else(|| DEFAULT_DELEGATED_SCOPES.to_string())
}

pub fn resolve_client_secret(cli_flag: Option<&str>) -> Option<String> {
    if let Some(v) = cli_flag {
        return Some(v.to_string());
    }
    if let Ok(v) = std::env::var("TEAMS_CLI_CLIENT_SECRET") {
        return Some(v);
    }
    None
}

pub fn resolve_output_format<'a>(
    cli_flag: Option<&'a str>,
    config: &'a ConfigFile,
) -> Option<&'a str> {
    cli_flag
        .or(config.default.output.as_deref())
        .or(Some(config.output.format.as_str()))
}

pub fn effective_network_config(
    config: &ConfigFile,
    cli_timeout: Option<u64>,
    cli_retry: Option<u32>,
) -> NetworkConfig {
    let mut network = config.network.clone();

    if let Some(timeout) = config.default.timeout {
        network.timeout = timeout;
    }
    if let Some(retry) = config.default.retry {
        network.max_retries = retry;
    }
    if let Some(timeout) = cli_timeout {
        network.timeout = timeout;
    }
    if let Some(retry) = cli_retry {
        network.max_retries = retry;
    }

    network
}

pub fn effective_page_size(config: &ConfigFile, cli_page_size: Option<u64>) -> u64 {
    cli_page_size
        .or(config.default.page_size)
        .unwrap_or(config.output.page_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sane_values() {
        let config = ConfigFile::default();
        assert_eq!(config.output.format, "auto");
        assert!(config.output.color);
        assert_eq!(config.output.page_size, 50);
        assert_eq!(config.network.timeout, 30);
        assert_eq!(config.network.max_retries, 3);
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn parse_config_toml() {
        let toml_str = r#"
[default]
profile = "work"

[output]
format = "json"
color = false
page_size = 100

[network]
timeout = 60
max_retries = 5

[profiles.work]
client_id = "abc-123"
tenant_id = "tenant-456"
auth_flow = "device-code"
scopes = "User.Read People.Read offline_access"
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default.profile.as_deref(), Some("work"));
        assert_eq!(config.output.format, "json");
        assert!(!config.output.color);
        assert_eq!(config.output.page_size, 100);
        assert_eq!(config.network.timeout, 60);
        assert_eq!(config.network.max_retries, 5);

        let work = config.profiles.get("work").unwrap();
        assert_eq!(work.client_id.as_deref(), Some("abc-123"));
        assert_eq!(work.tenant_id.as_deref(), Some("tenant-456"));
        assert_eq!(work.auth_flow.as_deref(), Some("device-code"));
        assert_eq!(
            work.scopes.as_deref(),
            Some("User.Read People.Read offline_access")
        );
    }

    #[test]
    fn save_and_load_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let path_str = path.to_str().unwrap();

        let mut config = ConfigFile::default();
        config.default.profile = Some("test".into());

        save_config(&config, Some(path_str)).unwrap();
        let loaded = load_config(Some(path_str)).unwrap();
        assert_eq!(loaded.default.profile.as_deref(), Some("test"));
    }

    #[test]
    fn resolve_profile_uses_cli_override() {
        let config = ConfigFile::default();
        assert_eq!(resolve_profile("custom", &config), "custom");
    }

    #[test]
    fn resolve_profile_uses_config_default() {
        let mut config = ConfigFile::default();
        config.default.profile = Some("work".into());
        assert_eq!(resolve_profile("default", &config), "work");
    }

    #[test]
    fn resolve_client_id_priority() {
        let mut config = ConfigFile::default();
        config.profiles.insert(
            "test".into(),
            ProfileConfig {
                auth_app: None,
                client_id: Some("from-config".into()),
                tenant_id: None,
                auth_flow: None,
                scopes: None,
            },
        );

        // CLI flag takes priority
        assert_eq!(
            resolve_client_id(Some("from-cli"), "test", &config),
            Some("from-cli".into())
        );

        // Falls back to config profile
        assert_eq!(
            resolve_client_id(None, "test", &config),
            Some("from-config".into())
        );

        // Returns None if not in config
        assert_eq!(resolve_client_id(None, "nonexistent", &config), None);
    }

    #[test]
    fn delegated_auth_defaults_to_oso_app_and_organizations_tenant() {
        let config = ConfigFile::default();

        assert_eq!(
            resolve_delegated_client_id(None, "default", &config).unwrap(),
            OSO_PUBLIC_CLIENT_ID
        );
        assert_eq!(
            resolve_delegated_tenant_id(None, "default", &config),
            DEFAULT_DELEGATED_TENANT_ID
        );
    }

    #[test]
    fn default_delegated_scopes_avoid_admin_required_channel_read() {
        assert!(DEFAULT_DELEGATED_SCOPES.contains("ChatMessage.Send"));
        assert!(DEFAULT_DELEGATED_SCOPES.contains("ChannelMessage.Send"));
        assert!(!DEFAULT_DELEGATED_SCOPES.contains("ChannelMessage.Read.All"));
    }

    fn config_with_profile_scopes(scopes: Option<&str>) -> ConfigFile {
        let mut config = ConfigFile::default();
        config.profiles.insert(
            "work".into(),
            ProfileConfig {
                scopes: scopes.map(|s| s.to_string()),
                ..ProfileConfig::default()
            },
        );
        config
    }

    #[test]
    fn resolve_delegated_scopes_defaults_when_unset() {
        let config = ConfigFile::default();
        assert_eq!(
            resolve_delegated_scopes(None, "default", &config),
            DEFAULT_DELEGATED_SCOPES
        );
    }

    #[test]
    fn resolve_delegated_scopes_override_none_when_unset() {
        let config = ConfigFile::default();
        assert_eq!(
            resolve_delegated_scopes_override(None, "default", &config),
            None
        );
    }

    #[test]
    fn resolve_delegated_scopes_override_some_for_profile_value() {
        let config = config_with_profile_scopes(Some("User.Read People.Read"));
        assert_eq!(
            resolve_delegated_scopes_override(None, "work", &config).as_deref(),
            Some("User.Read People.Read offline_access")
        );
    }

    #[test]
    fn resolve_delegated_scopes_uses_profile_and_appends_offline_access() {
        let config = config_with_profile_scopes(Some("User.Read People.Read"));
        assert_eq!(
            resolve_delegated_scopes(None, "work", &config),
            "User.Read People.Read offline_access"
        );
    }

    #[test]
    fn resolve_delegated_scopes_preserves_existing_offline_access() {
        let config = config_with_profile_scopes(Some("User.Read offline_access People.Read"));
        assert_eq!(
            resolve_delegated_scopes(None, "work", &config),
            "User.Read offline_access People.Read"
        );
    }

    #[test]
    fn resolve_delegated_scopes_cli_value_beats_profile() {
        let config = config_with_profile_scopes(Some("User.Read People.Read"));
        assert_eq!(
            resolve_delegated_scopes(Some("Chat.ReadWrite"), "work", &config),
            "Chat.ReadWrite offline_access"
        );
    }

    #[test]
    fn resolve_delegated_scopes_blank_values_fall_through() {
        let blank_profile = config_with_profile_scopes(Some("   "));
        assert_eq!(
            resolve_delegated_scopes(Some("  "), "work", &blank_profile),
            DEFAULT_DELEGATED_SCOPES
        );

        let config = config_with_profile_scopes(Some("User.Read"));
        assert_eq!(
            resolve_delegated_scopes(Some(""), "work", &config),
            "User.Read offline_access"
        );
    }

    #[test]
    fn delegated_auth_byo_requires_client_id() {
        let mut config = ConfigFile::default();
        config.profiles.insert(
            "locked".into(),
            ProfileConfig {
                auth_app: Some("byo".into()),
                client_id: None,
                tenant_id: Some("tenant".into()),
                auth_flow: Some("device-code".into()),
                scopes: None,
            },
        );

        assert!(resolve_delegated_client_id(None, "locked", &config).is_err());
    }

    #[test]
    fn resolve_output_format_priority() {
        let mut config = ConfigFile::default();
        config.output.format = "plain".into();
        assert_eq!(resolve_output_format(None, &config), Some("plain"));

        config.default.output = Some("json".into());
        assert_eq!(resolve_output_format(None, &config), Some("json"));
        assert_eq!(resolve_output_format(Some("human"), &config), Some("human"));
    }

    #[test]
    fn effective_network_config_applies_default_and_cli_overrides() {
        let mut config = ConfigFile::default();
        config.network.timeout = 30;
        config.network.max_retries = 3;
        config.default.timeout = Some(45);
        config.default.retry = Some(4);

        let from_default = effective_network_config(&config, None, None);
        assert_eq!(from_default.timeout, 45);
        assert_eq!(from_default.max_retries, 4);

        let from_cli = effective_network_config(&config, Some(10), Some(1));
        assert_eq!(from_cli.timeout, 10);
        assert_eq!(from_cli.max_retries, 1);
    }

    #[test]
    fn effective_page_size_priority() {
        let mut config = ConfigFile::default();
        config.output.page_size = 50;
        assert_eq!(effective_page_size(&config, None), 50);

        config.default.page_size = Some(75);
        assert_eq!(effective_page_size(&config, None), 75);
        assert_eq!(effective_page_size(&config, Some(100)), 100);
    }
}
