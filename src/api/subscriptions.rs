use crate::error::Result;
use crate::models::common::Page;
use crate::models::subscription::{
    CreateSubscriptionRequest, RenewSubscriptionRequest, Subscription,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn create_subscription(
    client: &GraphClient,
    req: &CreateSubscriptionRequest,
) -> Result<Subscription> {
    client.post(&endpoints::subscriptions(), req).await
}

pub async fn list_subscriptions(
    client: &GraphClient,
    pagination: &PaginationOpts,
) -> Result<Page<Subscription>> {
    list_subscriptions_at(client, &endpoints::subscriptions(), pagination).await
}

/// Graph answers 400 ("Query option 'Top' is not allowed") to `$top` on
/// `/subscriptions`, so the list is paged by `@odata.nextLink` alone.
async fn list_subscriptions_at(
    client: &GraphClient,
    url: &str,
    pagination: &PaginationOpts,
) -> Result<Page<Subscription>> {
    client.get_paged_without_top(url, &[], pagination).await
}

pub async fn renew_subscription(
    client: &GraphClient,
    id: &str,
    req: &RenewSubscriptionRequest,
) -> Result<Subscription> {
    client.patch(&endpoints::subscription(id), req).await
}

pub async fn delete_subscription(client: &GraphClient, id: &str) -> Result<()> {
    client.delete(&endpoints::subscription(id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use reqwest::Client;
    use wiremock::matchers::{method, path, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client() -> GraphClient {
        GraphClient {
            http: Client::new(),
            token: TokenInfo {
                access_token: "test-token".into(),
                expires_at: None,
                token_type: "Bearer".into(),
                scope: None,
                refresh_token: None,
                profile: "default".into(),
            },
            network: NetworkConfig {
                timeout: 30,
                max_retries: 0,
                retry_backoff_base: 2,
            },
        }
    }

    #[tokio::test]
    async fn list_subscriptions_omits_top_query_parameter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/subscriptions"))
            .and(query_param_is_missing("$top"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{ "id": "sub-1", "resource": "/chats/c/messages" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let subs = list_subscriptions_at(
            &test_client(),
            &format!("{}/subscriptions", server.uri()),
            &PaginationOpts {
                page_size: 50,
                all_pages: false,
            },
        )
        .await
        .unwrap()
        .items;

        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].id.as_deref(), Some("sub-1"));
    }
}
