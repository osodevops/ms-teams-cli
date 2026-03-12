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
    client
        .get_paged(&endpoints::channels(team_id), &[], pagination)
        .await
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
