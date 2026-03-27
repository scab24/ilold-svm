use serde::{Deserialize, Serialize};

use super::common::{EnumDef, ErrorDef, EventDef, SourceSpan, StateVar, StructDef};
use super::function::FunctionDef;
use super::modifier::ModifierDef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDef {
    pub name: String,
    pub kind: ContractKind,
    pub functions: Vec<FunctionDef>,
    pub modifiers: Vec<ModifierDef>,
    pub state_vars: Vec<StateVar>,
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub events: Vec<EventDef>,
    pub errors: Vec<ErrorDef>,
    pub inherits: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractKind {
    Contract,
    Interface,
    Library,
    Abstract,
}
