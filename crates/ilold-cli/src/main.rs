use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ilold_solana_core::ingest::{detect, ProjectKind};

mod analyze;
mod colors;
mod context;
mod explore;
mod fmt;
mod help;
mod interactive;

#[derive(Parser)]
#[command(name = "ilold", version, about = "Smart contract execution path analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Analyze Solidity contracts
    Analyze {
        path: PathBuf,
        #[arg(long)]
        contract: Option<String>,
        #[arg(long, default_value = "3")]
        max_seq_depth: usize,
        #[arg(long)]
        verbose: bool,
    },
    /// Generate context for a function or sequence
    Context {
        path: PathBuf,
        #[arg(long)]
        contract: Option<String>,
        #[arg(long)]
        function: Option<String>,
        #[arg(long)]
        sequence: Option<String>,
        #[arg(long)]
        list: bool,
    },
    /// Start interactive web viewer
    Serve {
        path: PathBuf,
        #[arg(long, default_value = "8080")]
        port: u16,
        #[arg(long, default_value = "3")]
        max_seq_depth: usize,
    },
    /// Interactive exploration REPL with web canvas
    Explore {
        path: PathBuf,
        #[arg(long, default_value = "0")]
        port: u16,
        #[arg(long, default_value = "3")]
        max_seq_depth: usize,
        /// Attach to a running server instead of starting one locally
        #[arg(long)]
        attach: Option<String>,
    },
    /// Run the MCP server (stdio transport) exposing Solana REPL commands as tools
    Mcp {
        /// Base URL of a running `ilold serve` instance (defaults to http://127.0.0.1:8080)
        #[arg(long, env = "ILOLD_SERVER_URL", default_value = "http://127.0.0.1:8080")]
        server_url: String,
        /// Target program name. All tool calls route to this program via the
        /// `contract` field of `/api/cmd`. Required so the MCP client never
        /// has to think about workspace topology.
        #[arg(long, env = "ILOLD_CONTRACT")]
        contract: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, contract, max_seq_depth, verbose } => {
            analyze::run(&path, contract.as_deref(), max_seq_depth, verbose)
        }
        Commands::Context { path, contract, function, sequence, list } => {
            context::run(&path, contract.as_deref(), function.as_deref(), sequence.as_deref(), list)
        }
        Commands::Serve { path, port, max_seq_depth } => {
            let detected = detect(&path)?;
            match detected.kind {
                ProjectKind::Solidity => serve_solidity(&path, port, max_seq_depth).await,
                ProjectKind::Solana => ilold_web::serve_solana(detected, port).await,
            }
        }
        Commands::Explore { path, port, max_seq_depth, attach } => {
            if attach.is_some() {
                return explore::run(Vec::new(), port, max_seq_depth, attach).await;
            }
            let detected = detect(&path)?;
            match detected.kind {
                ProjectKind::Solidity => explore_solidity(&path, port, max_seq_depth, attach).await,
                ProjectKind::Solana => explore::run_solana(detected, port).await,
            }
        }
        Commands::Mcp { server_url, contract } => ilold_mcp::run(server_url, contract).await,
    }
}

async fn serve_solidity(path: &PathBuf, port: u16, max_seq_depth: usize) -> Result<()> {
    let paths = collect_sol_files(path)?;
    if paths.is_empty() {
        anyhow::bail!("No .sol files found at {}", path.display());
    }
    ilold_web::serve(paths, port, max_seq_depth).await
}

async fn explore_solidity(path: &PathBuf, port: u16, max_seq_depth: usize, attach: Option<String>) -> Result<()> {
    let paths = collect_sol_files(path)?;
    if paths.is_empty() {
        anyhow::bail!("No .sol files found at {}", path.display());
    }
    explore::run(paths, port, max_seq_depth, attach).await
}

pub fn collect_sol_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }
    if path.is_dir() {
        let mut files = Vec::new();
        walk_sol_files(path, &mut files)?;
        files.sort();
        return Ok(files);
    }
    anyhow::bail!("Path does not exist: {}", path.display());
}

fn walk_sol_files(dir: &PathBuf, out: &mut Vec<PathBuf>) -> Result<()> {
    const SKIP: &[&str] = &["out", "cache", "node_modules", "lib", "target", ".git", ".svelte-kit"];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || SKIP.contains(&name) {
                continue;
            }
            walk_sol_files(&p, out)?;
        } else if p.extension().is_some_and(|ext| ext == "sol") {
            out.push(p);
        }
    }
    Ok(())
}
