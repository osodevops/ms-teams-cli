use clap::Subcommand;
use std::path::PathBuf;
use std::time::Instant;

use crate::config::{self, ConfigFile};
use crate::error::{Result, TeamsError};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Initialise config file with defaults
    Init,
    /// Show current configuration
    Show,
    /// Get a config value
    Get {
        /// Config key (e.g., "default.profile", "network.timeout")
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key (e.g., "default.profile", "network.timeout")
        key: String,
        /// Config value
        value: String,
    },
    /// Print config file location
    Path,
    /// List all profiles
    Profiles,
}

pub async fn run(
    cmd: ConfigCommand,
    config: &ConfigFile,
    config_path: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    match cmd {
        ConfigCommand::Init => {
            let start = Instant::now();
            let path = match config_path {
                Some(path) => PathBuf::from(path),
                None => config::default_config_path()?,
            };
            if path.exists() {
                return Err(TeamsError::InvalidInput(format!(
                    "Config already exists at {}",
                    path.display()
                )));
            }
            config::save_config(&ConfigFile::default(), config_path)?;
            let msg = serde_json::json!({
                "message": "Config initialised",
                "path": path.display().to_string(),
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        ConfigCommand::Show => {
            let start = Instant::now();
            let config_json = serde_json::to_value(config)
                .map_err(|e| TeamsError::ConfigError(format!("Serialization error: {e}")))?;
            output::print_success(format, &config_json, start);
            Ok(())
        }

        ConfigCommand::Get { key } => {
            let start = Instant::now();
            let config_json = serde_json::to_value(config)
                .map_err(|e| TeamsError::ConfigError(format!("Serialization error: {e}")))?;

            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &config_json;
            for part in &parts {
                current = current.get(part).unwrap_or(&serde_json::Value::Null);
            }

            output::print_success(format, current, start);
            Ok(())
        }

        ConfigCommand::Set { key, value } => {
            let start = Instant::now();
            let mut config_json = serde_json::to_value(config)
                .map_err(|e| TeamsError::ConfigError(format!("Serialization error: {e}")))?;

            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &mut config_json;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 {
                    current[part] = parse_config_value(&value);
                } else {
                    if !current.get(part).is_some_and(|v| v.is_object()) {
                        current[part] = serde_json::Value::Object(serde_json::Map::new());
                    }
                    current = &mut current[part];
                }
            }

            let updated: ConfigFile = serde_json::from_value(config_json)
                .map_err(|e| TeamsError::ConfigError(format!("Invalid config: {e}")))?;
            config::save_config(&updated, config_path)?;

            let msg = serde_json::json!({
                "message": format!("Set {key} = {value}"),
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        ConfigCommand::Path => {
            let start = Instant::now();
            let path = config::default_config_path()?;
            let msg = serde_json::json!({
                "path": path.display().to_string(),
                "exists": path.exists(),
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        ConfigCommand::Profiles => {
            let start = Instant::now();
            let profiles: Vec<String> = config.profiles.keys().cloned().collect();
            let msg = serde_json::json!({
                "profiles": profiles,
                "default": config.default.profile,
            });
            output::print_success(format, &msg, start);
            Ok(())
        }
    }
}

fn parse_config_value(value: &str) -> serde_json::Value {
    if value.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if value.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if value.eq_ignore_ascii_case("null") {
        return serde_json::Value::Null;
    }
    if let Ok(v) = value.parse::<i64>() {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = value.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return serde_json::Value::Number(n);
        }
    }
    serde_json::Value::String(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_value_preserves_scalar_types() {
        assert_eq!(parse_config_value("true"), serde_json::json!(true));
        assert_eq!(parse_config_value("false"), serde_json::json!(false));
        assert_eq!(parse_config_value("60"), serde_json::json!(60));
        assert_eq!(
            parse_config_value("client-id"),
            serde_json::json!("client-id")
        );
    }
}
