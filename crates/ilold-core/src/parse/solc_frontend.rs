use std::path::{Path, PathBuf};

use foundry_compilers::solc::{SolcCompiler, SolcSettings};
use foundry_compilers::{ProjectBuilder, ProjectPathsConfig};
use foundry_compilers_artifacts_solc::ast::{Node, NodeType};
use foundry_compilers_artifacts_solc::Settings;

use crate::model::common::*;
use crate::model::contract::*;
use crate::model::function::*;
use crate::model::modifier::*;
use crate::model::project::*;

use super::error::ParseError;
use super::span::LineIndex;
use super::ProjectParser;

pub struct SolcFrontend;

impl ProjectParser for SolcFrontend {
    fn parse(&self, paths: &[PathBuf]) -> Result<Project, ParseError> {
        let root = project_root(paths)?;
        self.parse_project(&root)
    }
}

impl SolcFrontend {
    pub fn parse_project(&self, root: &Path) -> Result<Project, ParseError> {
        let paths = ProjectPathsConfig::dapptools(root).map_err(|e| ParseError::Internal {
            message: format!("solc paths config: {e}"),
        })?;
        let settings = SolcSettings {
            settings: Settings::default().with_ast(),
            ..Default::default()
        };
        let project = ProjectBuilder::<SolcCompiler>::default()
            .paths(paths)
            .settings(settings)
            .build(SolcCompiler::AutoDetect)
            .map_err(|e| ParseError::Internal {
                message: format!("solc project build: {e}"),
            })?;

        let output = project.compile().map_err(|e| ParseError::Internal {
            message: format!("solc compile: {e}"),
        })?;

        if output.has_compiler_errors() {
            let message = output
                .output()
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ParseError::SyntaxError {
                path: root.display().to_string(),
                message,
                span: None,
            });
        }

        let mut source_files = Vec::new();
        let mut contracts = Vec::new();

        let aggregated = output.into_output();
        for (path, source_file) in aggregated.sources.sources() {
            let Some(ast) = &source_file.ast else { continue };
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let file_index = source_files.len();
            let index = LineIndex::new(file_index, &content);

            for node in &ast.nodes {
                if node.node_type == NodeType::ContractDefinition {
                    if let Some(contract) = map_contract(node, &index) {
                        contracts.push(contract);
                    }
                }
            }

            source_files.push(SourceFile {
                path: path.display().to_string(),
                content,
            });
        }

        let mut project = Project {
            source_files,
            contracts,
            contract_index: Default::default(),
        };
        project.rebuild_index();
        Ok(project)
    }
}

/// Walk up from the first input path until a `foundry.toml` is found; that
/// directory is the project root solc compiles against.
fn project_root(paths: &[PathBuf]) -> Result<PathBuf, ParseError> {
    let first = paths.first().ok_or_else(|| ParseError::Internal {
        message: "no input paths".into(),
    })?;
    let start = if first.is_dir() {
        first.clone()
    } else {
        first.parent().map(Path::to_path_buf).unwrap_or_default()
    };

    let mut dir = start.clone();
    loop {
        if dir.join("foundry.toml").exists() {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => break,
        }
    }
    Ok(start)
}

fn map_contract(node: &Node, index: &LineIndex) -> Option<ContractDef> {
    let name: String = node.attribute("name")?;

    let mut functions = Vec::new();
    let mut modifiers = Vec::new();
    let mut state_vars = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut events = Vec::new();
    let mut errors = Vec::new();

    for child in &node.nodes {
        match child.node_type {
            NodeType::FunctionDefinition => functions.push(map_function(child, index)),
            NodeType::ModifierDefinition => modifiers.push(map_modifier(child, index)),
            NodeType::VariableDeclaration => {
                if child.attribute::<bool>("stateVariable").unwrap_or(false) {
                    state_vars.push(map_state_var(child, index));
                }
            }
            NodeType::StructDefinition => structs.push(map_struct(child, index)),
            NodeType::EnumDefinition => enums.push(map_enum(child, index)),
            NodeType::EventDefinition => events.push(map_event(child, index)),
            NodeType::ErrorDefinition => errors.push(map_error(child, index)),
            _ => {}
        }
    }

    Some(ContractDef {
        name,
        kind: contract_kind(node),
        functions,
        modifiers,
        state_vars,
        structs,
        enums,
        events,
        errors,
        inherits: base_contracts(node),
        span: span_of(node, index),
    })
}

fn map_function(node: &Node, index: &LineIndex) -> FunctionDef {
    FunctionDef {
        name: node.attribute::<String>("name").unwrap_or_default(),
        kind: function_kind(node),
        visibility: visibility(node),
        mutability: mutability(node),
        modifiers: modifier_refs(node),
        params: param_list(node, "parameters"),
        returns: param_list(node, "returnParameters"),
        body: None,
        is_virtual: node.attribute::<bool>("virtual").unwrap_or(false),
        is_override: node.attribute::<serde_json::Value>("overrides").is_some(),
        span: span_of(node, index),
    }
}

fn map_modifier(node: &Node, index: &LineIndex) -> ModifierDef {
    ModifierDef {
        name: node.attribute::<String>("name").unwrap_or_default(),
        params: param_list(node, "parameters"),
        body: Vec::new(),
        span: span_of(node, index),
    }
}

fn map_state_var(node: &Node, index: &LineIndex) -> StateVar {
    StateVar {
        name: node.attribute::<String>("name").unwrap_or_default(),
        type_name: type_string(node),
        visibility: visibility(node),
        is_constant: node.attribute::<bool>("constant").unwrap_or(false),
        is_immutable: node.attribute::<String>("mutability").as_deref() == Some("immutable"),
        initial_value: None,
        span: span_of(node, index),
    }
}

fn map_struct(node: &Node, index: &LineIndex) -> StructDef {
    let fields = node
        .attribute::<Vec<Node>>("members")
        .unwrap_or_default()
        .iter()
        .map(|m| Param {
            name: m.attribute::<String>("name").unwrap_or_default(),
            type_name: type_string(m),
        })
        .collect();
    StructDef {
        name: node.attribute::<String>("name").unwrap_or_default(),
        fields,
        span: span_of(node, index),
    }
}

fn map_enum(node: &Node, index: &LineIndex) -> EnumDef {
    let variants = node
        .attribute::<Vec<Node>>("members")
        .unwrap_or_default()
        .iter()
        .map(|m| m.attribute::<String>("name").unwrap_or_default())
        .collect();
    EnumDef {
        name: node.attribute::<String>("name").unwrap_or_default(),
        variants,
        span: span_of(node, index),
    }
}

fn map_event(node: &Node, index: &LineIndex) -> EventDef {
    EventDef {
        name: node.attribute::<String>("name").unwrap_or_default(),
        params: param_list(node, "parameters"),
        span: span_of(node, index),
    }
}

fn map_error(node: &Node, index: &LineIndex) -> ErrorDef {
    ErrorDef {
        name: node.attribute::<String>("name").unwrap_or_default(),
        params: param_list(node, "parameters"),
        span: span_of(node, index),
    }
}

fn param_list(node: &Node, key: &str) -> Vec<Param> {
    let Some(list) = node.attribute::<Node>(key) else {
        return Vec::new();
    };
    list.attribute::<Vec<Node>>("parameters")
        .unwrap_or_default()
        .iter()
        .map(|p| Param {
            name: p.attribute::<String>("name").unwrap_or_default(),
            type_name: type_string(p),
        })
        .collect()
}

fn modifier_refs(node: &Node) -> Vec<ModifierRef> {
    node.attribute::<Vec<serde_json::Value>>("modifiers")
        .unwrap_or_default()
        .iter()
        .filter_map(|m| {
            m.get("modifierName")
                .and_then(|n| n.get("name"))
                .and_then(|s| s.as_str())
                .map(|name| ModifierRef {
                    name: name.to_string(),
                    arguments: Vec::new(),
                })
        })
        .collect()
}

fn base_contracts(node: &Node) -> Vec<String> {
    node.attribute::<Vec<serde_json::Value>>("baseContracts")
        .unwrap_or_default()
        .iter()
        .filter_map(|b| {
            b.get("baseName")
                .and_then(|n| n.get("name"))
                .and_then(|s| s.as_str())
                .map(str::to_string)
        })
        .collect()
}

fn type_string(node: &Node) -> String {
    node.attribute::<serde_json::Value>("typeDescriptions")
        .and_then(|v| v.get("typeString").and_then(|s| s.as_str().map(str::to_string)))
        .unwrap_or_default()
}

fn contract_kind(node: &Node) -> ContractKind {
    if node.attribute::<bool>("abstract").unwrap_or(false) {
        return ContractKind::Abstract;
    }
    match node.attribute::<String>("contractKind").as_deref() {
        Some("interface") => ContractKind::Interface,
        Some("library") => ContractKind::Library,
        _ => ContractKind::Contract,
    }
}

fn visibility(node: &Node) -> Visibility {
    match node.attribute::<String>("visibility").as_deref() {
        Some("external") => Visibility::External,
        Some("internal") => Visibility::Internal,
        Some("private") => Visibility::Private,
        _ => Visibility::Public,
    }
}

fn mutability(node: &Node) -> Mutability {
    match node.attribute::<String>("stateMutability").as_deref() {
        Some("pure") => Mutability::Pure,
        Some("view") => Mutability::View,
        Some("payable") => Mutability::Payable,
        _ => Mutability::NonPayable,
    }
}

fn function_kind(node: &Node) -> FunctionKind {
    match node.attribute::<String>("kind").as_deref() {
        Some("constructor") => FunctionKind::Constructor,
        Some("fallback") => FunctionKind::Fallback,
        Some("receive") => FunctionKind::Receive,
        _ => FunctionKind::Function,
    }
}

fn span_of(node: &Node, index: &LineIndex) -> SourceSpan {
    let start = node.src.start;
    let end = start + node.src.length.unwrap_or(0);
    index.span(start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/solc/cross")
    }

    #[test]
    fn parses_contracts_functions_inheritance() {
        let project = SolcFrontend.parse_project(&fixture_root()).expect("parse fixture");

        let lending = project
            .contracts
            .iter()
            .find(|c| c.name == "LendingPool")
            .expect("LendingPool present");
        assert!(lending.inherits.contains(&"BasePool".to_string()));
        assert!(lending.inherits.contains(&"IPool".to_string()));
        assert!(lending.functions.iter().any(|f| f.name == "supply"));

        let vault = project
            .contracts
            .iter()
            .find(|c| c.name == "Vault")
            .expect("Vault present");
        assert!(vault.state_vars.iter().any(|sv| sv.name == "pool"));
    }
}
