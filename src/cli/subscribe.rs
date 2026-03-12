use clap::Subcommand;
use std::time::Instant;

use crate::api::{self, GraphClient, PaginationOpts};
use crate::auth;
use crate::config::ConfigFile;
use crate::error::Result;
use crate::models::subscription::{CreateSubscriptionRequest, RenewSubscriptionRequest};
use crate::output::{self, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum SubscribeCommand {
    /// Create a new subscription
    Create {
        /// Resource path to subscribe to (e.g., /teams/all/messages)
        #[arg(long)]
        resource: String,
        /// Change types to subscribe to (comma-separated: created,updated,deleted)
        #[arg(long)]
        change_type: String,
        /// Webhook URL for notifications
        #[arg(long)]
        webhook_url: String,
        /// Expiration date-time in ISO 8601 format
        #[arg(long)]
        expiration: Option<String>,
        /// Client state string for validation
        #[arg(long)]
        client_state: Option<String>,
    },
    /// List active subscriptions
    List,
    /// Renew a subscription
    Renew {
        /// Subscription ID
        subscription_id: String,
        /// New expiration date-time in ISO 8601 format
        #[arg(long)]
        expiration: Option<String>,
    },
    /// Delete a subscription
    Delete {
        /// Subscription ID
        subscription_id: String,
    },
}

pub async fn run(
    cmd: SubscribeCommand,
    config: &ConfigFile,
    profile: &str,
    format: OutputFormat,
    pagination: &PaginationOpts,
) -> Result<()> {
    let token = auth::resolve_token(profile)?;
    let client = GraphClient::new(token, &config.network)?;

    match cmd {
        SubscribeCommand::Create {
            resource,
            change_type,
            webhook_url,
            expiration,
            client_state,
        } => {
            let start = Instant::now();
            let expiration_date_time = expiration.unwrap_or_else(|| {
                let exp = chrono::Utc::now() + chrono::Duration::hours(1);
                exp.to_rfc3339()
            });
            let req = CreateSubscriptionRequest {
                change_type,
                notification_url: webhook_url,
                resource,
                expiration_date_time,
                client_state,
            };
            let sub = api::subscriptions::create_subscription(&client, &req).await?;
            output::print_success(format, &sub, start);
            Ok(())
        }

        SubscribeCommand::List => {
            let start = Instant::now();
            let subs = api::subscriptions::list_subscriptions(&client, pagination).await?;
            if format == OutputFormat::Human {
                let headers = vec!["ID", "Resource", "Change Type", "Expiration", "Notification URL"];
                let rows: Vec<Vec<String>> = subs
                    .iter()
                    .map(|s| {
                        vec![
                            s.id.clone().unwrap_or_default(),
                            s.resource.clone().unwrap_or_default(),
                            s.change_type.clone().unwrap_or_default(),
                            s.expiration_date_time.clone().unwrap_or_default(),
                            s.notification_url.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table::print_table(headers, rows);
            } else {
                output::print_success_list(format, &subs, start);
            }
            Ok(())
        }

        SubscribeCommand::Renew {
            subscription_id,
            expiration,
        } => {
            let start = Instant::now();
            let expiration_date_time = expiration.unwrap_or_else(|| {
                let exp = chrono::Utc::now() + chrono::Duration::hours(1);
                exp.to_rfc3339()
            });
            let req = RenewSubscriptionRequest {
                expiration_date_time,
            };
            let sub =
                api::subscriptions::renew_subscription(&client, &subscription_id, &req).await?;
            output::print_success(format, &sub, start);
            Ok(())
        }

        SubscribeCommand::Delete { subscription_id } => {
            let start = Instant::now();
            api::subscriptions::delete_subscription(&client, &subscription_id).await?;
            let result = serde_json::json!({"status": "deleted"});
            output::print_success(format, &result, start);
            Ok(())
        }
    }
}
