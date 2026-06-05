use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ilold_solana_core::ingest::{detect, ProjectKind};

mod colors;
mod explore;
mod help;

#[derive(Parser)]
#[command(name = "ilold", version, about = "Solana program execution path analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start interactive web viewer
    Serve {
        path: PathBuf,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    /// Interactive exploration REPL with web canvas
    Explore {
        path: PathBuf,
        #[arg(long, default_value = "0")]
        port: u16,
        /// Attach to a running server instead of starting one locally
        #[arg(long)]
        attach: Option<String>,
    },
    /// Run the MCP server (stdio transport) exposing Solana REPL commands as tools
    Mcp {
        /// Base URL of a running `ilold serve` instance (defaults to http://127.0.0.1:8080)
        #[arg(long, env = "ILOLD_SERVER_URL", default_value = "http://127.0.0.1:8080")]
        server_url: String,
        /// Optional initial active program (LLM can switch later via ilold_use).
        #[arg(long, env = "ILOLD_CONTRACT")]
        contract: Option<String>,
        /// Emit MCP progress notifications describing each tool-call intent.
        #[arg(long, env = "ILOLD_NARRATION")]
        narration: bool,
    },
}

fn ensure_solana(path: &PathBuf) -> Result<ilold_solana_core::ingest::DetectedProject> {
    let detected = detect(path)?;
    match detected.kind {
        ProjectKind::Solana => Ok(detected),
        ProjectKind::Solidity => anyhow::bail!(
            "Only Solana (Anchor) projects are supported in ilold. Solidity support lives in the standalone ilold-evm repo."
        ),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, port } => {
            let detected = ensure_solana(&path)?;
            ilold_web::serve_solana(detected, port).await
        }
        Commands::Explore { path, port, attach } => {
            if attach.is_some() {
                return explore::run(Vec::new(), port, 0, attach).await;
            }
            let detected = ensure_solana(&path)?;
            explore::run_solana(detected, port).await
        }
        Commands::Mcp { server_url, contract, narration } => {
            ilold_mcp::run(ilold_mcp::Config { server_url, contract, narration }).await
        }
    }
}
