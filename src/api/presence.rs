use crate::error::Result;
use crate::models::common::PageResponse;
use crate::models::presence::{
    GetPresenceBatchRequest, Presence, SetPresenceRequest, SetStatusMessageRequest,
};

use super::client::GraphClient;
use super::endpoints;

pub async fn get_my_presence(client: &GraphClient) -> Result<Presence> {
    client.get(&endpoints::my_presence(), &[]).await
}

pub async fn get_user_presence(client: &GraphClient, user_id: &str) -> Result<Presence> {
    get_user_presence_at(client, &endpoints::user_presence(user_id)).await
}

async fn get_user_presence_at(client: &GraphClient, url: &str) -> Result<Presence> {
    client.get(url, &[]).await
}

pub async fn get_presence_batch(client: &GraphClient, ids: Vec<String>) -> Result<Vec<Presence>> {
    let req = GetPresenceBatchRequest { ids };
    let resp: PageResponse<Presence> = client.post(&endpoints::presence_batch(), &req).await?;
    Ok(resp.value)
}

pub async fn set_presence(client: &GraphClient, req: &SetPresenceRequest) -> Result<()> {
    client
        .post_no_content(&endpoints::set_presence(), req)
        .await
}

pub async fn clear_presence(client: &GraphClient) -> Result<()> {
    client
        .post_no_content(&endpoints::clear_presence(), &serde_json::json!({}))
        .await
}

pub async fn set_status_message(client: &GraphClient, req: &SetStatusMessageRequest) -> Result<()> {
    client
        .post_no_content(&endpoints::set_status_message(), req)
        .await
}

#[cfg(test)]
mod expiry_tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use reqwest::Client;
    use wiremock::matchers::{method, path};
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

    /// `/users/{id}/presence` returns the same expiry object as `/me/presence` and failed the same
    /// way, but it is a distinct call site: a refactor that gave it a response type of its own
    /// would reintroduce the defect with a fixture test on the shared struct still passing. The
    /// body is what Graph sends for a user whose status message carries an expiry, the
    /// `@odata.context` wrapper and `publishedDateTime` included.
    #[tokio::test]
    async fn a_user_lookup_parses_a_status_message_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/user-2/presence"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('user-2')/presence/$entity",
                    "id": "user-2",
                    "availability": "Away",
                    "activity": "Away",
                    "statusMessage": {
                        "message": { "content": "Out until Monday", "contentType": "text" },
                        "publishedDateTime": "2026-08-27T09:14:22.9411568Z",
                        "expiryDateTime": {
                            "dateTime": "2026-09-01T08:00:00.0000000",
                            "timeZone": "UTC"
                        }
                    }
                }"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let presence = get_user_presence_at(
            &test_client(),
            &format!("{}/users/user-2/presence", server.uri()),
        )
        .await
        .unwrap();

        assert_eq!(presence.availability.as_deref(), Some("Away"));
        let expiry = presence
            .status_message
            .expect("statusMessage")
            .expiry_date_time
            .expect("expiryDateTime");
        assert_eq!(
            expiry.date_time.as_deref(),
            Some("2026-09-01T08:00:00.0000000")
        );
        assert_eq!(expiry.time_zone.as_deref(), Some("UTC"));
    }
}
