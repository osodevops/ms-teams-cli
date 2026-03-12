use crate::error::Result;
use crate::models::member::{AddMemberRequest, ConversationMember};
use crate::models::team::{Team, TeamCloneRequest, TeamCreateRequest, TeamUpdateRequest};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_joined_teams(
    client: &GraphClient,
    pagination: &PaginationOpts,
) -> Result<Vec<Team>> {
    client
        .get_paged(&endpoints::my_joined_teams(), &[], pagination)
        .await
}

pub async fn get_team(client: &GraphClient, id: &str) -> Result<Team> {
    client.get(&endpoints::team(id), &[]).await
}

pub async fn create_team(client: &GraphClient, req: &TeamCreateRequest) -> Result<Option<String>> {
    client.post_for_location(&endpoints::teams(), req).await
}

pub async fn update_team(client: &GraphClient, id: &str, req: &TeamUpdateRequest) -> Result<Team> {
    client.patch(&endpoints::team(id), req).await
}

pub async fn delete_team(client: &GraphClient, id: &str) -> Result<()> {
    client.delete(&endpoints::team(id)).await
}

pub async fn clone_team(
    client: &GraphClient,
    id: &str,
    req: &TeamCloneRequest,
) -> Result<Option<String>> {
    client
        .post_for_location(&endpoints::team_clone(id), req)
        .await
}

pub async fn archive_team(client: &GraphClient, id: &str) -> Result<()> {
    client
        .post_no_content(&endpoints::team_archive(id), &serde_json::json!({}))
        .await
}

pub async fn unarchive_team(client: &GraphClient, id: &str) -> Result<()> {
    client
        .post_no_content(&endpoints::team_unarchive(id), &serde_json::json!({}))
        .await
}

pub async fn list_members(
    client: &GraphClient,
    team_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<ConversationMember>> {
    client
        .get_paged(&endpoints::team_members(team_id), &[], pagination)
        .await
}

pub async fn add_member(
    client: &GraphClient,
    team_id: &str,
    req: &AddMemberRequest,
) -> Result<ConversationMember> {
    client.post(&endpoints::team_members(team_id), req).await
}

pub async fn remove_member(client: &GraphClient, team_id: &str, member_id: &str) -> Result<()> {
    client
        .delete(&endpoints::team_member(team_id, member_id))
        .await
}
