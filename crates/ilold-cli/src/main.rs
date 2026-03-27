use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use ilold_core::cfg::builder::CfgBuilder;
use ilold_core::cfg::types::BlockKind;
use ilold_core::model::contract::ContractKind;
use ilold_core::model::function::Visibility;
use ilold_core::parse::solar_frontend::SolarParser;
use ilold_core::parse::ProjectParser;

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
        /// Path to .sol file or directory
        path: PathBuf,

        /// Only analyze this contract
        #[arg(long)]
        contract: Option<String>,

        /// Print detailed CFG information
        #[arg(long)]
        verbose: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, contract, verbose } => {
            analyze(&path, contract.as_deref(), verbose)
        }
    }
}

fn analyze(path: &PathBuf, contract_filter: Option<&str>, verbose: bool) -> Result<()> {
    let paths = collect_sol_files(path)?;

    if paths.is_empty() {
        anyhow::bail!("No .sol files found at {}", path.display());
    }

    let parser = SolarParser;
    let project = parser
        .parse(&paths)
        .context(format!("Failed to parse {}", path.display()))?;

    println!("Parsed {} file(s), {} contract(s)\n", project.source_files.len(), project.contracts.len());

    for contract in &project.contracts {
        if let Some(filter) = contract_filter {
            if contract.name != filter {
                continue;
            }
        }

        let kind_str = match contract.kind {
            ContractKind::Contract => "contract",
            ContractKind::Interface => "interface",
            ContractKind::Library => "library",
            ContractKind::Abstract => "abstract",
        };

        println!("{} {} ({} functions, {} state vars)",
            kind_str, contract.name, contract.functions.len(), contract.state_vars.len());

        if !contract.inherits.is_empty() {
            println!("  inherits: {}", contract.inherits.join(", "));
        }

        for func in &contract.functions {
            let vis = match func.visibility {
                Visibility::Public => "public",
                Visibility::External => "external",
                Visibility::Internal => "internal",
                Visibility::Private => "private",
            };

            let display_name = if func.name.is_empty() {
                format!("{:?}", func.kind).to_lowercase()
            } else {
                func.name.clone()
            };

            match CfgBuilder::build(func, contract) {
                Ok(cfg) => {
                    let blocks = cfg.node_count();
                    let edges = cfg.edge_count();
                    let reverts = cfg.node_weights()
                        .filter(|b| b.kind == BlockKind::Revert)
                        .count();

                    println!("  {} {} — {} blocks, {} edges, {} revert paths",
                        vis, display_name, blocks, edges, reverts);

                    if verbose {
                        for node in cfg.node_indices() {
                            let block = &cfg[node];
                            println!("    [{}] {:?} ({} stmts)",
                                block.id, block.kind, block.statements.len());
                        }
                        for edge in cfg.edge_indices() {
                            let (src, dst) = cfg.edge_endpoints(edge).unwrap();
                            let weight = &cfg[edge];
                            println!("    {} -> {} ({:?})",
                                cfg[src].id, cfg[dst].id, weight);
                        }
                        println!();
                    }
                }
                Err(e) => {
                    println!("  {} {} — CFG error: {e}", vis, display_name);
                }
            }
        }
        println!();
    }

    Ok(())
}

fn collect_sol_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
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
