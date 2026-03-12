use crate::error::Result;
use crate::models::chat::{Chat, ChatCreateRequest, ChatUpdateRequest};
use crate::models::member::{AddMemberRequest, ConversationMember};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_chats(
    client: &GraphClient,
    pagination: &PaginationOpts,
) -> Result<Vec<Chat>> {
    client
        .get_paged(&endpoints::my_chats(), &[], pagination)
        .await
}

pub async fn get_chat(client: &GraphClient, id: &str) -> Result<Chat> {
    client.get(&endpoints::chat(id), &[]).await
}

pub async fn create_chat(client: &GraphClient, req: &ChatCreateRequest) -> Result<Chat> {
    client.post(&endpoints::my_chats(), req).await
}

pub async fn update_chat(client: &GraphClient, id: &str, req: &ChatUpdateRequest) -> Result<Chat> {
    client.patch(&endpoints::chat(id), req).await
}

pub async fn list_members(
    client: &GraphClient,
    chat_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ConversationMember>> {
    client
        .get_paged(&endpoints::chat_members(chat_id), &[], pagination)
        .await
}

pub async fn add_member(
    client: &GraphClient,
    chat_id: &str,
    req: &AddMemberRequest,
) -> Result<ConversationMember> {
    client.post(&endpoints::chat_members(chat_id), req).await
}

pub async fn remove_member(
    client: &GraphClient,
    chat_id: &str,
    member_id: &str,
) -> Result<()> {
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
