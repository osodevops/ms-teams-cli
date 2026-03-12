use crate::error::Result;

pub async fn run(port: u16) -> Result<()> {
    crate::listen::run_listener(port).await
}
