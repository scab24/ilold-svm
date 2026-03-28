use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use ilold_core::callgraph::builder::build_call_graph;
use ilold_core::callgraph::types::CallKind;
use ilold_core::cfg::builder::CfgBuilder;
use ilold_core::model::contract::{ContractDef, ContractKind};
use ilold_core::model::function::Visibility;
use ilold_core::model::project::Project;
use ilold_core::parse::solar_frontend::SolarParser;
use ilold_core::parse::ProjectParser;
use ilold_core::pathtree::config::PruningConfig;
use ilold_core::pathtree::types::PathTree;
use ilold_core::pathtree::walker::build_path_tree;
use ilold_core::sequence::builder::build_sequence_tree;

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

        /// Max sequence depth for function combinations
        #[arg(long, default_value = "3")]
        max_seq_depth: usize,

        /// Print detailed information (CFG blocks, call graph, sequences)
        #[arg(long)]
        verbose: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, contract, max_seq_depth, verbose } => {
            analyze(&path, contract.as_deref(), max_seq_depth, verbose)
        }
    }
}

fn analyze(
    path: &PathBuf,
    contract_filter: Option<&str>,
    max_seq_depth: usize,
    verbose: bool,
) -> Result<()> {
    let paths = collect_sol_files(path)?;
    if paths.is_empty() {
        anyhow::bail!("No .sol files found at {}", path.display());
    }

    let parser = SolarParser;
    let mut project = parser
        .parse(&paths)
        .context(format!("Failed to parse {}", path.display()))?;
    project.rebuild_index();

    println!("Parsed {} file(s), {} contract(s)\n",
        project.source_files.len(), project.contracts.len());

    for contract in &project.contracts {
        if let Some(filter) = contract_filter {
            if contract.name != filter {
                continue;
            }
        }

        print_contract(&project, contract, max_seq_depth, verbose);
    }

    Ok(())
}

fn print_contract(
    project: &Project,
    contract: &ContractDef,
    max_seq_depth: usize,
    verbose: bool,
) {
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

    // Build CFGs and path trees for each function
    let config = PruningConfig::default();
    let mut path_trees: Vec<PathTree> = Vec::new();

    for func in &contract.functions {
        let display_name = if func.name.is_empty() {
            format!("{:?}", func.kind).to_lowercase()
        } else {
            func.name.clone()
        };

        let vis = match func.visibility {
            Visibility::Public => "public",
            Visibility::External => "external",
            Visibility::Internal => "internal",
            Visibility::Private => "private",
        };

        match CfgBuilder::build(func, contract) {
            Ok(cfg) => {
                let pt = build_path_tree(
                    &cfg, &contract.name, &func.name, &contract.state_vars, &config,
                );

                println!("  {} {} — {} blocks, {} edges, {} paths ({} happy, {} revert)",
                    vis, display_name,
                    cfg.node_count(), cfg.edge_count(),
                    pt.stats.total_paths, pt.stats.happy_paths, pt.stats.revert_paths);

                if verbose {
                    // CFG detail
                    for node in cfg.node_indices() {
                        let block = &cfg[node];
                        println!("    [{}] {:?} ({} stmts)",
                            block.id, block.kind, block.statements.len());
                    }
                    for edge in cfg.edge_indices() {
                        let (src, dst) = cfg.edge_endpoints(edge).unwrap();
                        println!("    {} -> {} ({:?})", cfg[src].id, cfg[dst].id, cfg[edge]);
                    }
                    println!();
                }

                path_trees.push(pt);
            }
            Err(e) => {
                println!("  {} {} — CFG error: {e}", vis, display_name);
            }
        }
    }

    // Call graph (verbose only)
    if verbose {
        let cg = build_call_graph(project, contract);
        let edges: Vec<_> = cg.edge_indices().collect();
        if !edges.is_empty() {
            println!("  Call graph:");
            for edge_idx in edges {
                let (src, dst) = cg.edge_endpoints(edge_idx).unwrap();
                let edge = &cg[edge_idx];
                let kind_str = match edge.kind {
                    CallKind::Internal => "internal",
                    CallKind::External => "external",
                    CallKind::Inherited => "inherited",
                };
                println!("    {} → {}.{} ({})",
                    cg[src].function, cg[dst].contract, cg[dst].function, kind_str);
            }
            println!();
        }
    }

    // Sequence tree (skip for interfaces)
    if contract.kind != ContractKind::Interface {
        let st = build_sequence_tree(contract, &path_trees, max_seq_depth);
        if !st.functions.is_empty() {
            let state_changing = st.functions.iter().filter(|f| !f.read_only).count();
            let read_only = st.functions.iter().filter(|f| f.read_only).count();

            println!("  Sequences (depth {}): {} total ({} functions: {} state-changing, {} read-only)",
                max_seq_depth, st.sequences.len(),
                st.functions.len(), state_changing, read_only);

            if verbose {
                for d in 1..=max_seq_depth {
                    let count = st.sequences.iter().filter(|s| s.depth == d).count();
                    println!("    depth {}: {} sequences", d, count);
                }
                println!();
            }
        }
    }

    println!();
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
