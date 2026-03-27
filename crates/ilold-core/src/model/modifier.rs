use serde::{Deserialize, Serialize};

use super::common::{Param, SourceSpan};
use super::expression::Expression;
use super::statement::Statement;

/// Modifier definition (the declaration with its body)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierDef {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Statement>,
    pub span: SourceSpan,
}

/// Modifier reference on a function (the usage, e.g. `onlyOwner` in function header)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierRef {
    pub name: String,
    pub arguments: Vec<Expression>,
}
