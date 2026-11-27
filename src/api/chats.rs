use crate::error::Result;
use crate::models::chat::{Chat, ChatCreateRequest, ChatUpdateRequest};
use crate::models::common::Page;
use crate::models::member::{AddMemberRequest, ConversationMember};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

/// Ordering `/me/chats` by the newest message requires expanding it: Graph
/// sorts only on `lastMessagePreview/createdDateTime`, descending, and returns
/// the preview only under `$expand`, so one option sets both parameters.
#[derive(Debug, Clone, Copy, Default)]
pub enum ChatOrder {
    #[default]
    Default,
    Activity,
}

impl ChatOrder {
    fn query(self) -> &'static [(&'static str, &'static str)] {
        match self {
            ChatOrder::Default => &[],
            ChatOrder::Activity => &[
                ("$expand", "lastMessagePreview"),
                ("$orderby", "lastMessagePreview/createdDateTime desc"),
            ],
        }
    }
}

pub async fn list_chats(
    client: &GraphClient,
    order: ChatOrder,
    pagination: &PaginationOpts,
) -> Result<Page<Chat>> {
    list_chats_at(client, &endpoints::my_chats(), order, pagination).await
}

async fn list_chats_at(
    client: &GraphClient,
    url: &str,
    order: ChatOrder,
    pagination: &PaginationOpts,
) -> Result<Page<Chat>> {
    client.get_paged(url, order.query(), pagination).await
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
) -> Result<Page<ConversationMember>> {
    list_members_at(client, &endpoints::chat_members(chat_id), pagination).await
}

/// `GET /chats/{id}/members` doesn't support the `$top` OData query option
/// (Graph returns HTTP 400), so page via `@odata.nextLink` only.
pub(crate) async fn list_members_at(
    client: &GraphClient,
    url: &str,
    pagination: &PaginationOpts,
) -> Result<Page<ConversationMember>> {
    client.get_paged_without_top(url, &[], pagination).await
}

pub async fn add_member(
    client: &GraphClient,
    chat_id: &str,
    req: &AddMemberRequest,
) -> Result<ConversationMember> {
    add_member_at(client, &endpoints::chat_members(chat_id), req).await
}

/// Graph documents the answer as `201 Created` with a `Location` header ending in
/// the new membership ID and no body. The member is then read back from the
/// collection URL. The add has already happened, so a failed read-back only warns
/// and returns the ID from the header; an error exit would invite a repeat add.
async fn add_member_at(
    client: &GraphClient,
    url: &str,
    req: &AddMemberRequest,
) -> Result<ConversationMember> {
    let created = client.post_created(url, req).await?;
    if let Some(member) = created.body {
        return Ok(member);
    }
    let Some(member_id) = created.location.as_deref().and_then(membership_id) else {
        tracing::warn!("Member added, but Graph returned neither the member nor its location");
        return Ok(ConversationMember::default());
    };
    let member_url = format!("{url}/{member_id}");
    match client.get(&member_url, &[]).await {
        Ok(member) => Ok(member),
        Err(e) => {
            tracing::warn!("Member added, but reading it back failed: {e}");
            Ok(ConversationMember {
                id: Some(member_id.to_string()),
                ..Default::default()
            })
        }
    }
}

/// The last path segment of a `Location` such as
/// `/chats/19:abc@thread.v2/members/MCMjMjQz...`.
fn membership_id(location: &str) -> Option<&str> {
    let path = location.split(['?', '#']).next()?;
    path.rsplit('/').next().filter(|id| !id.is_empty())
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
    use wiremock::matchers::{
        body_json, body_partial_json, method, path, query_param, query_param_is_missing,
    };
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
    async fn list_chats_by_activity_expands_the_preview_and_orders_by_it() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .and(query_param("$expand", "lastMessagePreview"))
            .and(query_param(
                "$orderby",
                "lastMessagePreview/createdDateTime desc",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{"id": "19:abc@thread.v2", "chatType": "group"}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let chats = list_chats_at(
            &test_client(),
            &format!("{}/me/chats", server.uri()),
            ChatOrder::Activity,
            &PaginationOpts {
                page_size: 50,
                all_pages: false,
            },
        )
        .await
        .unwrap()
        .items;
        assert_eq!(chats.len(), 1);
    }

    #[tokio::test]
    async fn list_chats_default_sends_no_order_and_no_expand() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me/chats"))
            .and(query_param_is_missing("$orderby"))
            .and(query_param_is_missing("$expand"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{"id": "19:abc@thread.v2", "chatType": "group"}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let chats = list_chats_at(
            &test_client(),
            &format!("{}/me/chats", server.uri()),
            ChatOrder::Default,
            &PaginationOpts {
                page_size: 50,
                all_pages: false,
            },
        )
        .await
        .unwrap()
        .items;
        assert_eq!(chats.len(), 1);
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
        .unwrap()
        .items;

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].display_name.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn add_member_posts_visible_history_when_set() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/chat-id/members"))
            .and(body_json(serde_json::json!({
                "@odata.type": "#microsoft.graph.aadUserConversationMember",
                "user@odata.bind": "https://graph.microsoft.com/v1.0/users('user-id')",
                "roles": [],
                "visibleHistoryStartDateTime": "0001-01-01T00:00:00Z"
            })))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("Location", "/chats/chat-id/members/member-id"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/chat-id/members/member-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "member-id",
                "displayName": "Alice",
                "visibleHistoryStartDateTime": "0001-01-01T00:00:00Z"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = AddMemberRequest::new("user-id", vec![])
            .with_visible_history(crate::models::member::ALL_HISTORY);
        let member = add_member_at(
            &test_client(),
            &format!("{}/chats/chat-id/members", server.uri()),
            &req,
        )
        .await
        .unwrap();
        assert_eq!(member.id.as_deref(), Some("member-id"));
        assert_eq!(member.display_name.as_deref(), Some("Alice"));
        assert_eq!(
            member.visible_history_start_date_time.as_deref(),
            Some("0001-01-01T00:00:00Z")
        );
    }

    /// A failed read-back must not turn the completed add into an error exit.
    #[tokio::test]
    async fn add_member_returns_the_location_id_when_read_back_fails() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/chat-id/members"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("Location", "/chats/chat-id/members/member-id"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chats/chat-id/members/member-id"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let member = add_member_at(
            &test_client(),
            &format!("{}/chats/chat-id/members", server.uri()),
            &AddMemberRequest::new("user-id", vec![]),
        )
        .await
        .unwrap();
        assert_eq!(member.id.as_deref(), Some("member-id"));
        assert!(member.display_name.is_none());
    }

    #[test]
    fn membership_id_is_the_last_location_segment() {
        assert_eq!(
            membership_id("/chats/19:a@thread.v2/members/MCMj=="),
            Some("MCMj==")
        );
        assert_eq!(
            membership_id("https://graph.microsoft.com/v1.0/chats/c/members/m1?x=1"),
            Some("m1")
        );
        assert_eq!(membership_id("/chats/c/members/"), None);
    }

    #[tokio::test]
    async fn add_member_omits_visible_history_by_default() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/chat-id/members"))
            .and(body_json(serde_json::json!({
                "@odata.type": "#microsoft.graph.aadUserConversationMember",
                "user@odata.bind": "https://graph.microsoft.com/v1.0/users('user-id')",
                "roles": ["owner"]
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "member-id"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let req = AddMemberRequest::new("user-id", vec!["owner".to_string()]);
        add_member_at(
            &test_client(),
            &format!("{}/chats/chat-id/members", server.uri()),
            &req,
        )
        .await
        .unwrap();
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
