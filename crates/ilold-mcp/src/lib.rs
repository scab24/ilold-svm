use anyhow::Result;
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;

pub mod server;
pub mod tools;

pub async fn run(server_url: String) -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .init();

    let handler = server::IloldMcpServer::new(server_url);
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
