use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use ilold_core::depgraph::{ContractDeps, DepEdge};
use ilold_core::model::contract::ContractKind;

use crate::state::AppState;

#[derive(Serialize)]
pub struct DepGraphResponse {
    pub nodes: Vec<DepGraphNode>,
    pub edges: Vec<DepGraphEdge>,
}

#[derive(Serialize)]
pub struct DepGraphNode {
    pub data: DepNodeData,
}

#[derive(Serialize)]
pub struct DepNodeData {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub folder: String,
    pub layer: usize,
    pub focus: bool,
}

#[derive(Serialize)]
pub struct DepGraphEdge {
    pub data: DepEdgeData,
}

#[derive(Serialize)]
pub struct DepEdgeData {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub kinds: Vec<String>,
    pub call_count: usize,
}

pub async fn get_project_depgraph(State(state): State<Arc<AppState>>) -> Json<DepGraphResponse> {
    let deps = &state.dep_graph;
    let layer_of = layer_map(deps);
    let g = &deps.graph;

    let nodes = g
        .node_indices()
        .map(|idx| {
            let n = &g[idx];
            DepGraphNode {
                data: DepNodeData {
                    id: n.name.clone(),
                    label: n.name.clone(),
                    kind: kind_word(n.kind).into(),
                    folder: n.folder.clone(),
                    layer: *layer_of.get(&n.name).unwrap_or(&0),
                    focus: false,
                },
            }
        })
        .collect();

    let edges = g
        .edge_indices()
        .enumerate()
        .map(|(i, e)| {
            let (s, t) = g.edge_endpoints(e).unwrap();
            edge(format!("e{i}"), &g[s].name, &g[t].name, &g[e])
        })
        .collect();

    Json(DepGraphResponse { nodes, edges })
}

pub async fn get_contract_depgraph(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<DepGraphResponse>, StatusCode> {
    let deps = &state.dep_graph;
    if deps.node(&name).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let layer_of = layer_map(deps);

    let dependencies = deps.dependencies(&name);
    let dependents = deps.dependents(&name);

    let mut names: BTreeSet<String> = BTreeSet::new();
    names.insert(name.clone());
    for d in &dependencies {
        names.insert(d.node.name.clone());
    }
    for d in &dependents {
        names.insert(d.node.name.clone());
    }

    let g = &deps.graph;
    let nodes = names
        .iter()
        .filter_map(|n| deps.node(n).map(|idx| (n, &g[idx])))
        .map(|(n, node)| DepGraphNode {
            data: DepNodeData {
                id: node.name.clone(),
                label: node.name.clone(),
                kind: kind_word(node.kind).into(),
                folder: node.folder.clone(),
                layer: *layer_of.get(n).unwrap_or(&0),
                focus: *n == name,
            },
        })
        .collect();

    let mut edges = Vec::new();
    for (i, d) in dependencies.iter().enumerate() {
        edges.push(edge(format!("o{i}"), &name, &d.node.name, d.edge));
    }
    for (i, d) in dependents.iter().enumerate() {
        edges.push(edge(format!("in{i}"), &d.node.name, &name, d.edge));
    }

    Ok(Json(DepGraphResponse { nodes, edges }))
}

fn edge(id: String, source: &str, target: &str, e: &DepEdge) -> DepGraphEdge {
    let kinds: Vec<String> = e.kinds().iter().map(|k| k.label().to_string()).collect();
    let kind = kinds.first().cloned().unwrap_or_default();
    DepGraphEdge {
        data: DepEdgeData {
            id,
            source: source.to_string(),
            target: target.to_string(),
            kind,
            kinds,
            call_count: e.call_count,
        },
    }
}

fn layer_map(deps: &ContractDeps) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for layer in deps.layers() {
        for group in &layer.groups {
            for &m in &group.members {
                map.insert(deps.graph[m].name.clone(), layer.index);
            }
        }
    }
    map
}

fn kind_word(kind: ContractKind) -> &'static str {
    match kind {
        ContractKind::Contract => "contract",
        ContractKind::Interface => "interface",
        ContractKind::Library => "library",
        ContractKind::Abstract => "abstract",
    }
}
