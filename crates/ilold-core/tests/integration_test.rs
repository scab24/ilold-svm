use std::path::PathBuf;

use ilold_core::cfg::builder::CfgBuilder;
use ilold_core::cfg::types::BlockKind;
use ilold_core::model::function::{FunctionKind, Visibility, Mutability};
use ilold_core::parse::solar_frontend::SolarParser;
use ilold_core::parse::ProjectParser;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_parse_simple_storage() {
    let parser = SolarParser;
    let project = parser.parse(&[fixture_path("simple_storage.sol")]).unwrap();

    // Should have 1 contract
    assert_eq!(project.contracts.len(), 1);
    let contract = &project.contracts[0];
    assert_eq!(contract.name, "SimpleStorage");

    // Should have 2 functions: get and set
    assert_eq!(contract.functions.len(), 2);

    let get_fn = contract.functions.iter().find(|f| f.name == "get").unwrap();
    assert_eq!(get_fn.visibility, Visibility::Public);
    assert_eq!(get_fn.mutability, Mutability::View);
    assert_eq!(get_fn.kind, FunctionKind::Function);
    assert_eq!(get_fn.returns.len(), 1);
    assert_eq!(get_fn.returns[0].type_name, "uint256");

    let set_fn = contract.functions.iter().find(|f| f.name == "set").unwrap();
    assert_eq!(set_fn.visibility, Visibility::Public);
    assert_eq!(set_fn.mutability, Mutability::NonPayable);
    assert_eq!(set_fn.params.len(), 1);
    assert_eq!(set_fn.params[0].name, "newValue");
    assert_eq!(set_fn.params[0].type_name, "uint256");

    // Should have 1 state variable
    assert_eq!(contract.state_vars.len(), 1);
    assert_eq!(contract.state_vars[0].name, "value");

    // Should have 1 event
    assert_eq!(contract.events.len(), 1);
    assert_eq!(contract.events[0].name, "ValueChanged");
}

#[test]
fn test_cfg_simple_get() {
    let parser = SolarParser;
    let project = parser.parse(&[fixture_path("simple_storage.sol")]).unwrap();
    let contract = &project.contracts[0];
    let get_fn = contract.functions.iter().find(|f| f.name == "get").unwrap();

    let cfg = CfgBuilder::build(get_fn, contract).unwrap();

    // get() just returns — should have Entry + Return (2 nodes minimum)
    let node_count = cfg.node_count();
    assert!(node_count >= 2, "Expected at least 2 nodes, got {node_count}");

    // Should have exactly one Entry and one Return
    let entries: Vec<_> = cfg.node_weights().filter(|b| b.kind == BlockKind::Entry).collect();
    let returns: Vec<_> = cfg.node_weights().filter(|b| b.kind == BlockKind::Return).collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(returns.len(), 1);
}

#[test]
fn test_cfg_set_with_require() {
    let parser = SolarParser;
    let project = parser.parse(&[fixture_path("simple_storage.sol")]).unwrap();
    let contract = &project.contracts[0];
    let set_fn = contract.functions.iter().find(|f| f.name == "set").unwrap();

    let cfg = CfgBuilder::build(set_fn, contract).unwrap();

    // set() has require(newValue > 0) which creates a branch:
    // Entry → [require check] → True path (assignment + emit + return)
    //                          → False path (revert)
    let node_count = cfg.node_count();
    assert!(node_count >= 4, "Expected at least 4 nodes, got {node_count}");

    // Must have at least one Revert node (from require failing)
    let reverts: Vec<_> = cfg.node_weights().filter(|b| b.kind == BlockKind::Revert).collect();
    assert!(!reverts.is_empty(), "Expected at least one Revert node");

    // Must have at least one Return node
    let returns: Vec<_> = cfg.node_weights().filter(|b| b.kind == BlockKind::Return).collect();
    assert!(!returns.is_empty(), "Expected at least one Return node");

    // Must have branch edges (from require)
    let edge_count = cfg.edge_count();
    assert!(edge_count >= 3, "Expected at least 3 edges, got {edge_count}");
}

#[test]
fn test_parse_file_not_found() {
    let parser = SolarParser;
    let result = parser.parse(&[PathBuf::from("nonexistent.sol")]);
    assert!(result.is_err());
}
