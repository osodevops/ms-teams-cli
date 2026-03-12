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
    client.get(&endpoints::user_presence(user_id), &[]).await
}

pub async fn get_presence_batch(
    client: &GraphClient,
    ids: Vec<String>,
) -> Result<Vec<Presence>> {
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
