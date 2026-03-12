use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Result, TeamsError};

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
    pub client_id: Option<String>,
    pub tenant_id: Option<String>,
    pub auth_flow: Option<String>,
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

pub fn resolve_client_secret(cli_flag: Option<&str>) -> Option<String> {
    if let Some(v) = cli_flag {
        return Some(v.to_string());
    }
    if let Ok(v) = std::env::var("TEAMS_CLI_CLIENT_SECRET") {
        return Some(v);
    }
    None
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
                client_id: Some("from-config".into()),
                tenant_id: None,
                auth_flow: None,
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
}
