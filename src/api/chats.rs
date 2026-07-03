use crate::error::Result;
use crate::models::chat::{Chat, ChatCreateRequest, ChatUpdateRequest};
use crate::models::member::{AddMemberRequest, ConversationMember};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_chats(client: &GraphClient, pagination: &PaginationOpts) -> Result<Vec<Chat>> {
    client
        .get_paged(&endpoints::my_chats(), &[], pagination)
        .await
}

pub async fn get_chat(client: &GraphClient, id: &str) -> Result<Chat> {
    client.get(&endpoints::chat(id), &[]).await
}

pub async fn create_chat(client: &GraphClient, req: &ChatCreateRequest) -> Result<Chat> {
    create_chat_at(client, &endpoints::chats(), req).await
}

/// Chat creation must POST to `/chats`; the `/me/chats` endpoint is list-only
/// and answers POST with HTTP 405.
async fn create_chat_at(client: &GraphClient, url: &str, req: &ChatCreateRequest) -> Result<Chat> {
    client.post(url, req).await
}

pub async fn update_chat(client: &GraphClient, id: &str, req: &ChatUpdateRequest) -> Result<Chat> {
    client.patch(&endpoints::chat(id), req).await
}

pub async fn list_members(
    client: &GraphClient,
    chat_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ConversationMember>> {
    list_members_at(client, &endpoints::chat_members(chat_id), pagination).await
}

/// `GET /chats/{id}/members` doesn't support the `$top` OData query option
/// (Graph returns HTTP 400), so page via `@odata.nextLink` only.
async fn list_members_at(
    client: &GraphClient,
    url: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ConversationMember>> {
    client.get_paged_without_top(url, &[], pagination).await
}

pub async fn add_member(
    client: &GraphClient,
    chat_id: &str,
    req: &AddMemberRequest,
) -> Result<ConversationMember> {
    client.post(&endpoints::chat_members(chat_id), req).await
}

pub async fn remove_member(client: &GraphClient, chat_id: &str, member_id: &str) -> Result<()> {
    client
        .delete(&endpoints::chat_member(chat_id, member_id))
        .await
}

pub async fn hide_chat(client: &GraphClient, chat_id: &str, user_id: &str) -> Result<()> {
    let body = crate::models::chat::ChatUserAction {
        user: crate::models::chat::ChatUserRef {
            id: user_id.to_string(),
        },
    };
    client
        .post_no_content(&endpoints::chat_hide(chat_id), &body)
        .await
}

pub async fn unhide_chat(client: &GraphClient, chat_id: &str, user_id: &str) -> Result<()> {
    let body = crate::models::chat::ChatUserAction {
        user: crate::models::chat::ChatUserRef {
            id: user_id.to_string(),
        },
    };
    client
        .post_no_content(&endpoints::chat_unhide(chat_id), &body)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::TokenInfo;
    use crate::config::NetworkConfig;
    use reqwest::Client;
    use wiremock::matchers::{body_partial_json, method, path, query_param_is_missing};
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
    async fn list_members_omits_top_query_parameter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/chats/chat-id/members"))
            .and(query_param_is_missing("$top"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {
                        "id": "member-id",
                        "displayName": "Alice",
                        "roles": ["owner"]
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let members = list_members_at(
            &test_client(),
            &format!("{}/chats/chat-id/members", server.uri()),
            &PaginationOpts {
                page_size: 50,
                all_pages: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].display_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn chat_creation_endpoint_is_not_scoped_to_me() {
        assert_eq!(endpoints::chats(), "https://graph.microsoft.com/v1.0/chats");
    }

    #[tokio::test]
    async fn create_chat_posts_member_roles() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats"))
            .and(body_partial_json(serde_json::json!({
                "chatType": "group",
                "members": [
                    { "roles": ["owner"] },
                    { "roles": ["guest"] }
                ]
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "chat-id",
                "chatType": "group",
                "topic": "Vendor sync"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = ChatCreateRequest {
            chat_type: "group".to_string(),
            topic: Some("Vendor sync".to_string()),
            members: vec![
                AddMemberRequest::new("tenant-user", vec!["owner".to_string()]),
                AddMemberRequest::new("guest-user", vec!["guest".to_string()]),
            ],
        };
        let chat = create_chat_at(&test_client(), &format!("{}/chats", server.uri()), &req)
            .await
            .unwrap();

        assert_eq!(chat.id.as_deref(), Some("chat-id"));
        assert_eq!(chat.topic.as_deref(), Some("Vendor sync"));
    }
}
