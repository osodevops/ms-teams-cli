use crate::error::Result;
use crate::models::common::PageResponse;
use crate::models::search::{SearchRequest, SearchResponse};
use crate::models::team::Team;

use super::client::GraphClient;
use super::endpoints;

pub async fn search_messages(
    client: &GraphClient,
    query: &str,
    top: Option<u64>,
) -> Result<SearchResponse> {
    let req = SearchRequest::new("chatMessage", query, top.unwrap_or(25));
    client.post(&endpoints::search_query(), &req).await
}

pub async fn search_users(
    client: &GraphClient,
    query: &str,
    top: Option<u64>,
) -> Result<SearchResponse> {
    let req = SearchRequest::new("person", query, top.unwrap_or(25));
    client.post(&endpoints::search_query(), &req).await
}

pub async fn search_teams(client: &GraphClient, query: &str) -> Result<Vec<Team>> {
    // Graph doesn't support "group" in /search/query; use $search on joinedTeams instead
    let resp: PageResponse<Team> = client
        .get(
            &endpoints::my_joined_teams(),
            &[
                ("$search", &format!("\"displayName:{query}\"")),
                ("$select", "id,displayName,description"),
            ],
        )
        .await?;
    Ok(resp.value)
}
