use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;

mod analyze;
mod colors;
mod context;
mod deps_view;
mod explore;
mod fmt;
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
        #[arg(long)]
        deps: bool,
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
    /// Run the MCP server (stdio) exposing the EVM analysis as tools for LLM agents
    EvmMcp {
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        server_url: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, contract, max_seq_depth, verbose, deps } => {
            if deps {
                analyze::run_deps(&path, contract.as_deref())
            } else {
                analyze::run(&path, contract.as_deref(), max_seq_depth, verbose)
            }
        }
        Commands::Context { path, contract, function, sequence, list } => {
            context::run(&path, contract.as_deref(), function.as_deref(), sequence.as_deref(), list)
        }
        Commands::Serve { path, port, max_seq_depth } => {
            let root = foundry_root(&path)?;
            ilold_web::serve(vec![root], port, max_seq_depth).await
        }
        Commands::Explore { path, port, max_seq_depth, attach } => {
            if attach.is_some() {
                // --attach mode: no local analysis needed, connect to remote server
                explore::run(Vec::new(), port, max_seq_depth, attach).await
            } else {
                let root = foundry_root(&path)?;
                explore::run(vec![root], port, max_seq_depth, attach).await
            }
        }
        Commands::EvmMcp { server_url } => {
            ilold_evm_mcp::run(ilold_evm_mcp::Config { server_url }).await
        }
    }
}

pub fn foundry_root(path: &Path) -> Result<PathBuf> {
    let mut dir = if path.is_dir() {
        path.to_path_buf()
    } else if path.is_file() {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    };

    loop {
        if dir.join("foundry.toml").is_file() {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }

    anyhow::bail!(
        "Not a Foundry project: no foundry.toml found from {}. The Solidity backend requires a compilable Foundry project.",
        path.display()
    );
}
