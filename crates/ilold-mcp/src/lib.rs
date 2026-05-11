use std::sync::Arc;

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;

pub mod client;
pub mod error;
pub mod schema;
pub mod server;
pub mod tools;

pub use client::IloldClient;
pub use error::McpClientError;

pub async fn run(server_url: String, contract: String) -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .try_init()
        .ok();

    let client = Arc::new(IloldClient::new(server_url, contract));
    client
        .health_check()
        .await
        .context("Ilold backend health-check failed")?;

    let handler = server::IloldMcpServer::new(client);
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
