use crate::error::{Result, TeamsError};
use crate::models::common::PageResponse;
use crate::models::presence::{
    ClearPresenceRequest, ClearUserPreferredPresenceRequest, GetPresenceBatchRequest, Presence,
    SetPresenceRequest, SetStatusMessageRequest, SetUserPreferredPresenceRequest,
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
    set_presence_at(client, &endpoints::set_presence(), req).await
}

async fn set_presence_at(client: &GraphClient, url: &str, req: &SetPresenceRequest) -> Result<()> {
    client.post_no_content(url, req).await
}

pub async fn clear_presence(client: &GraphClient, req: &ClearPresenceRequest) -> Result<bool> {
    clear_presence_at(client, &endpoints::clear_presence(), req).await
}

/// Graph answers 404 when the application has no presence session to clear, which is the state
/// `clear` exists to reach; reporting it as a miss would make a second clear, or a retry after
/// an ambiguous response, fail against presence that is already automatic. It is not the same
/// outcome as clearing a live session, though — a session opened under a different application
/// ID answers the same way — so the caller is told which of the two Graph reported. `true` means
/// Graph cleared a session; `false` that it knew of none under this `sessionId`, which is also
/// what a retry sees when the attempt before it succeeded but its response was lost.
async fn clear_presence_at(
    client: &GraphClient,
    url: &str,
    req: &ClearPresenceRequest,
) -> Result<bool> {
    match client.post_no_content(url, req).await {
        Ok(()) => Ok(true),
        Err(TeamsError::NotFound(_)) => Ok(false),
        Err(err) => Err(err),
    }
}

pub async fn set_status_message(client: &GraphClient, req: &SetStatusMessageRequest) -> Result<()> {
    client
        .post_no_content(&endpoints::set_status_message(), req)
        .await
}

pub async fn set_user_preferred_presence(
    client: &GraphClient,
    req: &SetUserPreferredPresenceRequest,
) -> Result<()> {
    set_user_preferred_presence_at(client, &endpoints::set_user_preferred_presence(), req).await
}

async fn set_user_preferred_presence_at(
    client: &GraphClient,
    url: &str,
    req: &SetUserPreferredPresenceRequest,
) -> Result<()> {
    client.post_no_content(url, req).await
}

pub async fn clear_user_preferred_presence(client: &GraphClient) -> Result<()> {
    clear_user_preferred_presence_at(client, &endpoints::clear_user_preferred_presence()).await
}

async fn clear_user_preferred_presence_at(client: &GraphClient, url: &str) -> Result<()> {
    client
        .post_no_content(url, &ClearUserPreferredPresenceRequest {})
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

        let cleared = clear_presence_at(
            &test_client(),
            &format!("{}/me/presence/clearPresence", server.uri()),
            &ClearPresenceRequest {
                session_id: "app-id".to_string(),
            },
        )
        .await
        .unwrap();
        assert!(cleared);
    }

    #[tokio::test]
    async fn clear_presence_treats_a_missing_session_as_already_cleared() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/clearPresence"))
            .respond_with(ResponseTemplate::new(404).set_body_string(
                r#"{"error":{"code":"NotFound","message":"Presence session not found."}}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let cleared = clear_presence_at(
            &test_client(),
            &format!("{}/me/presence/clearPresence", server.uri()),
            &ClearPresenceRequest {
                session_id: "app-id".to_string(),
            },
        )
        .await
        .unwrap();
        assert!(!cleared);
    }

    #[tokio::test]
    async fn clear_presence_reports_no_session_when_a_retry_follows_a_lost_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/clearPresence"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/me/presence/clearPresence"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;

        let mut client = test_client();
        client.network.max_retries = 1;
        let cleared = clear_presence_at(
            &client,
            &format!("{}/me/presence/clearPresence", server.uri()),
            &ClearPresenceRequest {
                session_id: "app-id".to_string(),
            },
        )
        .await
        .unwrap();
        assert!(!cleared);
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

    #[tokio::test]
    async fn set_user_preferred_presence_sends_the_pair_and_expiration() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/setUserPreferredPresence"))
            .and(body_json(serde_json::json!({
                "availability": "Offline",
                "activity": "OffWork",
                "expirationDuration": "P1D"
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        set_user_preferred_presence_at(
            &test_client(),
            &format!("{}/me/presence/setUserPreferredPresence", server.uri()),
            &SetUserPreferredPresenceRequest {
                availability: "Offline".to_string(),
                activity: "OffWork".to_string(),
                expiration_duration: Some("P1D".to_string()),
            },
        )
        .await
        .unwrap();
    }

    /// Graph documents the body as `{}`. `body_json` compares parsed values, so a request that
    /// went out as `null` or with no body would not match.
    #[tokio::test]
    async fn clear_user_preferred_presence_sends_an_empty_object() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/me/presence/clearUserPreferredPresence"))
            .and(body_json(serde_json::json!({})))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        clear_user_preferred_presence_at(
            &test_client(),
            &format!("{}/me/presence/clearUserPreferredPresence", server.uri()),
        )
        .await
        .unwrap();
    }
}
