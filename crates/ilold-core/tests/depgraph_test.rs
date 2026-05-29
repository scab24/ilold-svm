use std::path::PathBuf;

use ilold_core::depgraph::ContractDeps;
use ilold_core::parse::solc_frontend::SolcFrontend;

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
fn cross_contract_calls_become_dep_edges() {
    let project = SolcFrontend
        .parse_project(&fixture_path("solc/cross"))
        .expect("parse cross fixture");
    let deps = ContractDeps::build(&project);

    let vault = deps.dependencies("Vault");
    let edge_to = |name: &str| vault.iter().find(|d| d.node.name == name);

    // pool.supply() and IPool(addr).supply() resolve to IPool — a real
    // calls edge, not a placeholder. The IPool state var also gives `holds`.
    let ipool = edge_to("IPool").expect("Vault must depend on IPool");
    assert!(ipool.edge.calls, "pool.supply() must produce a calls edge");
    assert!(ipool.edge.holds, "the `IPool pool` state var must produce a holds edge");

    // amount.safeAdd(...) via using-for resolves to the SafeMath library.
    let safe = edge_to("SafeMath").expect("Vault must depend on SafeMath");
    assert!(safe.edge.calls, "safeAdd must produce a calls edge to SafeMath");

    // Inheritance is captured structurally.
    let lp = deps.dependencies("LendingPool");
    assert!(
        lp.iter().any(|d| d.node.name == "BasePool" && d.edge.inherits),
        "LendingPool is BasePool must be an inherits edge"
    );
}

#[test]
fn bases_are_ordered_before_dependents() {
    let project = SolcFrontend
        .parse_project(&fixture_path("solc/cross"))
        .expect("parse cross fixture");
    let deps = ContractDeps::build(&project);
    let layers = deps.layers();

    let layer_of = |name: &str| -> usize {
        for layer in &layers {
            for group in &layer.groups {
                if group.members.iter().any(|&m| deps.graph[m].name == name) {
                    return layer.index;
                }
            }
        }
        panic!("{name} not found in any layer");
    };

    // IPool depends on nobody → read first; Vault depends on it → later.
    assert!(
        layer_of("IPool") < layer_of("Vault"),
        "IPool must come before Vault in reading order"
    );
}
