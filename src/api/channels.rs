use crate::error::Result;
use crate::models::channel::{Channel, ChannelCreateRequest, ChannelUpdateRequest};
use crate::models::member::{AddMemberRequest, ConversationMember};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_channels(
    client: &GraphClient,
    team_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<Channel>> {
    list_channels_at(client, &endpoints::channels(team_id), pagination).await
}

async fn list_channels_at(
    client: &GraphClient,
    url: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<Channel>> {
    client.get_paged_without_top(url, &[], pagination).await
}

pub async fn get_channel(client: &GraphClient, team_id: &str, channel_id: &str) -> Result<Channel> {
    client
        .get(&endpoints::channel(team_id, channel_id), &[])
        .await
}

pub async fn create_channel(
    client: &GraphClient,
    team_id: &str,
    req: &ChannelCreateRequest,
) -> Result<Channel> {
    client.post(&endpoints::channels(team_id), req).await
}

pub async fn update_channel(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    req: &ChannelUpdateRequest,
) -> Result<Channel> {
    client
        .patch(&endpoints::channel(team_id, channel_id), req)
        .await
}

pub async fn delete_channel(client: &GraphClient, team_id: &str, channel_id: &str) -> Result<()> {
    client
        .delete(&endpoints::channel(team_id, channel_id))
        .await
}

pub async fn list_members(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ConversationMember>> {
    client
        .get_paged(
            &endpoints::channel_members(team_id, channel_id),
            &[],
            pagination,
        )
        .await
}

pub async fn add_member(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    req: &AddMemberRequest,
) -> Result<ConversationMember> {
    client
        .post(&endpoints::channel_members(team_id, channel_id), req)
        .await
}

pub async fn remove_member(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    member_id: &str,
) -> Result<()> {
    client
        .delete(&endpoints::channel_member(team_id, channel_id, member_id))
        .await
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
    async fn list_channels_omits_top_query_parameter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/teams/team-id/channels"))
            .and(query_param_is_missing("$top"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "channel-id",
                        "displayName": "General",
                        "membershipType": "standard"
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let channels = list_channels_at(
            &test_client(),
            &format!("{}/teams/team-id/channels", server.uri()),
            &PaginationOpts {
                page_size: 50,
                all_pages: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].display_name.as_deref(), Some("General"));
    }
}
