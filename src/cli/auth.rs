use clap::Subcommand;
use std::time::Instant;

use crate::auth;
use crate::config::{self, ConfigFile};
use crate::error::{Result, TeamsError};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Authenticate with Microsoft Teams
    Login {
        /// Use client credentials flow (non-interactive)
        #[arg(long)]
        client_credentials: bool,

        /// Use device code flow
        #[arg(long)]
        device_code: bool,

        /// Azure AD application (client) ID
        #[arg(long, env = "TEAMS_CLI_CLIENT_ID")]
        client_id: Option<String>,

        /// Azure AD client secret
        #[arg(long, env = "TEAMS_CLI_CLIENT_SECRET")]
        client_secret: Option<String>,

        /// Azure AD tenant ID
        #[arg(long, env = "TEAMS_CLI_TENANT_ID")]
        tenant_id: Option<String>,

        /// OAuth scopes (space-separated, for delegated flows)
        #[arg(long)]
        scopes: Option<String>,
    },
    /// Check current auth status
    Status,
    /// List all authenticated profiles
    List,
    /// Switch active profile
    Switch {
        /// Profile name to switch to
        name: String,
    },
    /// Clear stored credentials
    Logout {
        /// Logout a specific profile
        #[arg(long)]
        profile: Option<String>,
        /// Logout all profiles
        #[arg(long)]
        all: bool,
    },
    /// Export current access token
    Token {
        /// Token output format: bearer, json
        #[arg(long, default_value = "bearer")]
        format: String,
    },
}

pub async fn run(
    cmd: AuthCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
) -> Result<()> {
    match cmd {
        AuthCommand::Login {
            client_credentials,
            device_code,
            client_id,
            client_secret,
            tenant_id,
            scopes,
        } => {
            let start = Instant::now();

            let client_id = config::resolve_client_id(client_id.as_deref(), profile, config)
                .ok_or_else(|| {
                    TeamsError::InvalidInput(
                        "Client ID is required. Use --client-id or set TEAMS_CLI_CLIENT_ID".into(),
                    )
                })?;

            let tenant_id = config::resolve_tenant_id(tenant_id.as_deref(), profile, config)
                .ok_or_else(|| {
                    TeamsError::InvalidInput(
                        "Tenant ID is required. Use --tenant-id or set TEAMS_CLI_TENANT_ID".into(),
                    )
                })?;

            let token_response = if client_credentials {
                let client_secret = config::resolve_client_secret(client_secret.as_deref())
                    .ok_or_else(|| {
                        TeamsError::InvalidInput(
                            "Client secret is required for client credentials flow. Use --client-secret or set TEAMS_CLI_CLIENT_SECRET".into(),
                        )
                    })?;

                auth::client_credentials::authenticate(&client_id, &client_secret, &tenant_id)
                    .await?
            } else if device_code {
                auth::device_code::authenticate(&client_id, &tenant_id, scopes.as_deref()).await?
            } else {
                // Default: auth code + PKCE
                auth::auth_code_pkce::authenticate(&client_id, &tenant_id, scopes.as_deref())
                    .await?
            };

            let token_info = token_response.into_token_info(profile);

            // Store in keyring
            auth::keyring::store_token(profile, &token_info)?;
            auth::keyring::add_profile_to_index(profile)?;

            let msg = serde_json::json!({
                "message": "Authenticated successfully",
                "profile": profile,
                "expires_at": token_info.expires_at.map(|e| e.to_rfc3339()),
                "scope": token_info.scope,
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        AuthCommand::Status => {
            let start = Instant::now();
            match auth::resolve_token(profile) {
                Ok(token) => {
                    let msg = serde_json::json!({
                        "authenticated": true,
                        "profile": profile,
                        "expires_at": token.expires_at.map(|e| e.to_rfc3339()),
                        "scope": token.scope,
                    });
                    output::print_success(format, &msg, start);
                    Ok(())
                }
                Err(_) => {
                    let msg = serde_json::json!({
                        "authenticated": false,
                        "profile": profile,
                    });
                    output::print_success(format, &msg, start);
                    std::process::exit(1);
                }
            }
        }

        AuthCommand::List => {
            let start = Instant::now();
            let profiles = auth::keyring::list_profiles();
            let msg = serde_json::json!({
                "profiles": profiles,
                "active": profile,
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        AuthCommand::Switch { name } => {
            let start = Instant::now();
            // Verify the profile has a token
            auth::resolve_token(&name)?;

            // Update config to set default profile
            let mut updated_config = config.clone();
            updated_config.default.profile = Some(name.clone());
            if let Err(e) = config::save_config(&updated_config, None) {
                tracing::warn!("Could not save profile switch to config: {e}");
            }

            let msg = serde_json::json!({
                "message": format!("Switched to profile '{name}'"),
                "profile": name,
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        AuthCommand::Logout {
            profile: target,
            all,
        } => {
            let start = Instant::now();
            if all {
                let profiles = auth::keyring::list_profiles();
                for p in &profiles {
                    auth::keyring::delete_token(p)?;
                    auth::keyring::remove_profile_from_index(p)?;
                }
                let msg = serde_json::json!({
                    "message": format!("Logged out from {} profile(s)", profiles.len()),
                });
                output::print_success(format, &msg, start);
            } else {
                let target = target.as_deref().unwrap_or(profile);
                auth::keyring::delete_token(target)?;
                auth::keyring::remove_profile_from_index(target)?;
                let msg = serde_json::json!({
                    "message": format!("Logged out from profile '{target}'"),
                });
                output::print_success(format, &msg, start);
            }
            Ok(())
        }

        AuthCommand::Token {
            format: token_format,
        } => {
            let token = auth::resolve_token(profile)?;
            match token_format.as_str() {
                "json" => {
                    let msg = serde_json::json!({
                        "access_token": token.access_token,
                        "token_type": token.token_type,
                        "expires_at": token.expires_at.map(|e| e.to_rfc3339()),
                    });
                    let start = Instant::now();
                    output::print_success(format, &msg, start);
                }
                _ => {
                    // bearer (default) — just print the token
                    println!("{}", token.access_token);
                }
            }
            Ok(())
        }
    }
}
