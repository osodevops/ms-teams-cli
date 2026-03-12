use crate::error::Result;
use crate::models::tag::{
    AddTagMemberRequest, CreateTagRequest, TeamworkTag, TeamworkTagMember, UpdateTagRequest,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_tags(
    client: &GraphClient,
    team_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<TeamworkTag>> {
    client
        .get_paged(&endpoints::team_tags(team_id), &[], pagination)
        .await
}

pub async fn get_tag(client: &GraphClient, team_id: &str, tag_id: &str) -> Result<TeamworkTag> {
    client.get(&endpoints::team_tag(team_id, tag_id), &[]).await
}

pub async fn create_tag(
    client: &GraphClient,
    team_id: &str,
    req: &CreateTagRequest,
) -> Result<TeamworkTag> {
    client.post(&endpoints::team_tags(team_id), req).await
}

pub async fn update_tag(
    client: &GraphClient,
    team_id: &str,
    tag_id: &str,
    req: &UpdateTagRequest,
) -> Result<TeamworkTag> {
    client
        .patch(&endpoints::team_tag(team_id, tag_id), req)
        .await
}

pub async fn delete_tag(client: &GraphClient, team_id: &str, tag_id: &str) -> Result<()> {
    client.delete(&endpoints::team_tag(team_id, tag_id)).await
}

#[allow(dead_code)]
pub async fn list_tag_members(
    client: &GraphClient,
    team_id: &str,
    tag_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<TeamworkTagMember>> {
    client
        .get_paged(&endpoints::tag_members(team_id, tag_id), &[], pagination)
        .await
}

pub async fn add_tag_member(
    client: &GraphClient,
    team_id: &str,
    tag_id: &str,
    req: &AddTagMemberRequest,
) -> Result<TeamworkTagMember> {
    client
        .post(&endpoints::tag_members(team_id, tag_id), req)
        .await
}

pub async fn remove_tag_member(
    client: &GraphClient,
    team_id: &str,
    tag_id: &str,
    member_id: &str,
) -> Result<()> {
    client
        .delete(&endpoints::tag_member(team_id, tag_id, member_id))
        .await
}
