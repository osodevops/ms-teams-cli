use std::net::IpAddr;

use crate::error::Result;

pub async fn run(bind: IpAddr, port: u16, client_state: Option<String>) -> Result<()> {
    let client_state = client_state.filter(|s| !s.is_empty());
    crate::listen::run_listener(bind, port, client_state).await
}
