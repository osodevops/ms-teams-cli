use crate::error::Result;
use crate::models::subscription::{
    CreateSubscriptionRequest, RenewSubscriptionRequest, Subscription,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn create_subscription(
    client: &GraphClient,
    req: &CreateSubscriptionRequest,
) -> Result<Subscription> {
    client.post(&endpoints::subscriptions(), req).await
}

pub async fn list_subscriptions(
    client: &GraphClient,
    pagination: &PaginationOpts,
) -> Result<Vec<Subscription>> {
    client
        .get_paged(&endpoints::subscriptions(), &[], pagination)
        .await
}

pub async fn renew_subscription(
    client: &GraphClient,
    id: &str,
    req: &RenewSubscriptionRequest,
) -> Result<Subscription> {
    client.patch(&endpoints::subscription(id), req).await
}

pub async fn delete_subscription(client: &GraphClient, id: &str) -> Result<()> {
    client.delete(&endpoints::subscription(id)).await
}
