use crate::error::Result;
use crate::models::app::{
    CreateTabRequest, InstallAppRequest, TeamsAppInstallation, TeamsTab, UpdateTabRequest,
};

use super::client::{GraphClient, PaginationOpts};
use super::endpoints;

pub async fn list_team_apps(
    client: &GraphClient,
    team_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<TeamsAppInstallation>> {
    client
        .get_paged(
            &endpoints::team_installed_apps(team_id),
            &[("$expand", "teamsApp")],
            pagination,
        )
        .await
}

pub async fn install_team_app(
    client: &GraphClient,
    team_id: &str,
    req: &InstallAppRequest,
) -> Result<()> {
    client
        .post_no_content(&endpoints::team_installed_apps(team_id), req)
        .await
}

pub async fn uninstall_team_app(
    client: &GraphClient,
    team_id: &str,
    installation_id: &str,
) -> Result<()> {
    client
        .delete(&endpoints::team_installed_app(team_id, installation_id))
        .await
}

pub async fn list_tabs(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    pagination: &PaginationOpts,
) -> Result<Vec<TeamsTab>> {
    client
        .get_paged(
            &endpoints::channel_tabs(team_id, channel_id),
            &[("$expand", "teamsApp")],
            pagination,
        )
        .await
}

pub async fn create_tab(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    req: &CreateTabRequest,
) -> Result<TeamsTab> {
    client
        .post(&endpoints::channel_tabs(team_id, channel_id), req)
        .await
}

#[allow(dead_code)]
pub async fn update_tab(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    tab_id: &str,
    req: &UpdateTabRequest,
) -> Result<TeamsTab> {
    client
        .patch(
            &endpoints::channel_tab(team_id, channel_id, tab_id),
            req,
        )
        .await
}

pub async fn delete_tab(
    client: &GraphClient,
    team_id: &str,
    channel_id: &str,
    tab_id: &str,
) -> Result<()> {
    client
        .delete(&endpoints::channel_tab(team_id, channel_id, tab_id))
        .await
}
