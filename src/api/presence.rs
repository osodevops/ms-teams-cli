use crate::error::Result;
use crate::models::common::PageResponse;
use crate::models::presence::{
    ClearPresenceRequest, GetPresenceBatchRequest, Presence, SetPresenceRequest,
    SetStatusMessageRequest,
};

use super::client::GraphClient;
use super::endpoints;

pub async fn get_my_presence(client: &GraphClient) -> Result<Presence> {
    client.get(&endpoints::my_presence(), &[]).await
}

pub async fn get_user_presence(client: &GraphClient, user_id: &str) -> Result<Presence> {
    client.get(&endpoints::user_presence(user_id), &[]).await
}

pub async fn get_presence_batch(client: &GraphClient, ids: Vec<String>) -> Result<Vec<Presence>> {
    let req = GetPresenceBatchRequest { ids };
    let resp: PageResponse<Presence> = client.post(&endpoints::presence_batch(), &req).await?;
    Ok(resp.value)
}

pub async fn set_presence(client: &GraphClient, req: &SetPresenceRequest) -> Result<()> {
    set_presence_at(client, &endpoints::set_presence(), req).await
}

async fn set_presence_at(client: &GraphClient, url: &str, req: &SetPresenceRequest) -> Result<()> {
    client.post_no_content(url, req).await
}

pub async fn clear_presence(client: &GraphClient, req: &ClearPresenceRequest) -> Result<()> {
    clear_presence_at(client, &endpoints::clear_presence(), req).await
}

async fn clear_presence_at(
    client: &GraphClient,
    url: &str,
    req: &ClearPresenceRequest,
) -> Result<()> {
    client.post_no_content(url, req).await
}

pub async fn set_status_message(client: &GraphClient, req: &SetStatusMessageRequest) -> Result<()> {
    client
        .post_no_content(&endpoints::set_status_message(), req)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use reqwest::Client;
    use wiremock::matchers::{body_json, method, path};
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
    async fn clear_presence_sends_the_session_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/clearPresence"))
            .and(body_json(serde_json::json!({ "sessionId": "app-id" })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        clear_presence_at(
            &test_client(),
            &format!("{}/me/presence/clearPresence", server.uri()),
            &ClearPresenceRequest {
                session_id: "app-id".to_string(),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn set_presence_sends_the_session_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/setPresence"))
            .and(body_json(serde_json::json!({
                "sessionId": "app-id",
                "availability": "Available",
                "activity": "Available",
                "expirationDuration": "PT1H"
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        set_presence_at(
            &test_client(),
            &format!("{}/me/presence/setPresence", server.uri()),
            &SetPresenceRequest {
                session_id: "app-id".to_string(),
                availability: "Available".to_string(),
                activity: "Available".to_string(),
                expiration_duration: Some("PT1H".to_string()),
            },
        )
        .await
        .unwrap();
    }
}
