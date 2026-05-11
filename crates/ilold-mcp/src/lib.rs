use std::sync::Arc;

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;

pub mod client;
pub mod error;
pub mod narration;
pub mod schema;
pub mod server;
pub mod tools;

pub use client::IloldClient;
pub use error::McpClientError;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub contract: Option<String>,
    pub narration: bool,
}

pub async fn run(cfg: Config) -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .try_init()
        .ok();

    let client = Arc::new(IloldClient::new(cfg.server_url));
    client
        .health_check()
        .await
        .context("Ilold backend health-check failed")?;

    let handler = server::IloldMcpServer::new(client, cfg.contract, cfg.narration);
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
