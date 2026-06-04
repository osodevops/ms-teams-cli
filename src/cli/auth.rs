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
    /// Print the admin consent URL for the active auth app
    ConsentUrl {
        /// Azure AD application (client) ID
        #[arg(long, env = "TEAMS_CLI_CLIENT_ID")]
        client_id: Option<String>,

        /// Azure AD tenant ID or domain
        #[arg(long, env = "TEAMS_CLI_TENANT_ID")]
        tenant_id: Option<String>,
    },
    /// Diagnose auth configuration and current token state
    Doctor {
        /// Azure AD application (client) ID
        #[arg(long, env = "TEAMS_CLI_CLIENT_ID")]
        client_id: Option<String>,

        /// Azure AD tenant ID or domain
        #[arg(long, env = "TEAMS_CLI_TENANT_ID")]
        tenant_id: Option<String>,
    },
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

fn token_diagnostics(claims: Option<&auth::token::TokenClaims>) -> Option<serde_json::Value> {
    claims.map(|claims| {
        serde_json::json!({
            "audience": claims.audience(),
            "is_graph_audience": claims.is_graph_audience(),
            "auth_type": claims.auth_type(),
            "tenant_id": claims.tid,
            "app_id": claims.appid.clone().or_else(|| claims.azp.clone()),
            "user": claims.preferred_username.clone().or_else(|| claims.upn.clone()),
        })
    })
}

fn token_warnings(claims: Option<&auth::token::TokenClaims>) -> Vec<String> {
    let mut warnings = Vec::new();

    if let Some(claims) = claims {
        if claims.is_graph_audience() == Some(false) {
            let audience = claims
                .audience()
                .unwrap_or_else(|| "unknown audience".into());
            warnings.push(format!(
                "Token audience is '{audience}', not Microsoft Graph. Graph commands require a Microsoft Graph access token."
            ));
        }
    }

    warnings
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

            let token_response = if client_credentials {
                let client_id = config::resolve_client_id(client_id.as_deref(), profile, config)
                    .ok_or_else(|| {
                        TeamsError::InvalidInput(
                            "Client ID is required for client credentials flow. Use --client-id or set TEAMS_CLI_CLIENT_ID".into(),
                        )
                    })?;
                let tenant_id = config::resolve_tenant_id(tenant_id.as_deref(), profile, config)
                    .ok_or_else(|| {
                        TeamsError::InvalidInput(
                            "Tenant ID is required for client credentials flow. Use --tenant-id or set TEAMS_CLI_TENANT_ID".into(),
                        )
                    })?;
                let client_secret = config::resolve_client_secret(client_secret.as_deref())
                    .ok_or_else(|| {
                        TeamsError::InvalidInput(
                            "Client secret is required for client credentials flow. Use --client-secret or set TEAMS_CLI_CLIENT_SECRET".into(),
                        )
                    })?;

                auth::client_credentials::authenticate(&client_id, &client_secret, &tenant_id)
                    .await?
            } else if device_code {
                let client_id =
                    config::resolve_delegated_client_id(client_id.as_deref(), profile, config)?;
                let tenant_id =
                    config::resolve_delegated_tenant_id(tenant_id.as_deref(), profile, config);
                auth::device_code::authenticate(&client_id, &tenant_id, scopes.as_deref()).await?
            } else {
                // Default: auth code + PKCE
                let client_id =
                    config::resolve_delegated_client_id(client_id.as_deref(), profile, config)?;
                let tenant_id =
                    config::resolve_delegated_tenant_id(tenant_id.as_deref(), profile, config);
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

        AuthCommand::ConsentUrl {
            client_id,
            tenant_id,
        } => {
            let start = Instant::now();
            let client_id =
                config::resolve_delegated_client_id(client_id.as_deref(), profile, config)?;
            let tenant_id =
                config::resolve_delegated_tenant_id(tenant_id.as_deref(), profile, config);
            let url = format!(
                "https://login.microsoftonline.com/{tenant_id}/adminconsent?client_id={client_id}"
            );
            let msg = serde_json::json!({
                "admin_consent_url": url,
                "client_id": client_id,
                "tenant_id": tenant_id,
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        AuthCommand::Doctor {
            client_id,
            tenant_id,
        } => {
            let start = Instant::now();
            let client_id =
                config::resolve_delegated_client_id(client_id.as_deref(), profile, config)?;
            let tenant_id =
                config::resolve_delegated_tenant_id(tenant_id.as_deref(), profile, config);
            let auth_app = if client_id == config::OSO_PUBLIC_CLIENT_ID {
                "oso"
            } else {
                "byo"
            };

            let token = auth::resolve_token(profile).ok();
            let claims = token.as_ref().and_then(|t| t.unverified_claims());
            let warnings = token_warnings(claims.as_ref());
            let msg = serde_json::json!({
                "profile": profile,
                "auth_app": auth_app,
                "client_id": client_id,
                "tenant_id": tenant_id,
                "admin_consent_url": format!("https://login.microsoftonline.com/{tenant_id}/adminconsent?client_id={client_id}"),
                "authenticated": token.is_some(),
                "warnings": warnings,
                "token": token.as_ref().map(|t| serde_json::json!({
                    "expires_at": t.expires_at.map(|e| e.to_rfc3339()),
                    "scope": t.scope,
                    "auth_type": claims.as_ref().map(|c| c.auth_type()).unwrap_or("unknown"),
                    "audience": claims.as_ref().and_then(|c| c.audience()),
                    "is_graph_audience": claims.as_ref().and_then(|c| c.is_graph_audience()),
                    "tenant_id": claims.as_ref().and_then(|c| c.tid.clone()),
                    "app_id": claims.as_ref().and_then(|c| c.appid.clone()).or_else(|| claims.as_ref().and_then(|c| c.azp.clone())),
                    "user": claims.as_ref().and_then(|c| c.preferred_username.clone()).or_else(|| claims.as_ref().and_then(|c| c.upn.clone())),
                })),
            });
            output::print_success(format, &msg, start);
            Ok(())
        }

        AuthCommand::Status => {
            let start = Instant::now();
            match auth::resolve_token(profile) {
                Ok(token) => {
                    let claims = token.unverified_claims();
                    let msg = serde_json::json!({
                        "authenticated": true,
                        "profile": profile,
                        "expires_at": token.expires_at.map(|e| e.to_rfc3339()),
                        "scope": token.scope,
                        "token_diagnostics": token_diagnostics(claims.as_ref()),
                        "warnings": token_warnings(claims.as_ref()),
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
                    let claims = token.unverified_claims();
                    let msg = serde_json::json!({
                        "access_token": token.access_token,
                        "token_type": token.token_type,
                        "expires_at": token.expires_at.map(|e| e.to_rfc3339()),
                        "token_diagnostics": token_diagnostics(claims.as_ref()),
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
