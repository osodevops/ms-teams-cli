use crate::error::Result;
use crate::models::user::User;

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn get_me(client: &GraphClient) -> Result<User> {
    client.get(&endpoints::me(), &[]).await
}

pub async fn get_user(client: &GraphClient, id: &str) -> Result<User> {
    client.get(&endpoints::user(id), &[]).await
}

pub async fn list_users(
    client: &GraphClient,
    filter: Option<&str>,
    pagination: &PaginationOpts,
) -> Result<Vec<User>> {
    let mut query: Vec<(&str, &str)> = vec![];
    if let Some(f) = filter {
        query.push(("$filter", f));
    }
    client
        .get_paged(&endpoints::users(), &query, pagination)
        .await
}
