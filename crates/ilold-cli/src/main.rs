use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod analyze;
mod colors;
mod context;
mod explore;
mod fmt;

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
            let paths = collect_sol_files(&path)?;
            if paths.is_empty() {
                anyhow::bail!("No .sol files found at {}", path.display());
            }
            ilold_web::serve(paths, port, max_seq_depth).await
        }
        Commands::Explore { path, port, max_seq_depth } => {
            let paths = collect_sol_files(&path)?;
            if paths.is_empty() {
                anyhow::bail!("No .sol files found at {}", path.display());
            }
            explore::run(paths, port, max_seq_depth).await
        }
    }
}

pub fn collect_sol_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }
    if path.is_dir() {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().is_some_and(|ext| ext == "sol") {
                files.push(p);
            }
        }
        files.sort();
        return Ok(files);
    }
    anyhow::bail!("Path does not exist: {}", path.display());
}
