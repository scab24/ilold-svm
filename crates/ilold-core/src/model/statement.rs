use serde::{Deserialize, Serialize};

use super::common::SourceSpan;
use super::expression::Expression;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatementKind {
    If {
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    For {
        init: Option<Box<Statement>>,
        condition: Option<Expression>,
        increment: Option<Expression>,
        body: Vec<Statement>,
    },
    While {
        condition: Expression,
        body: Vec<Statement>,
    },
    DoWhile {
        body: Vec<Statement>,
        condition: Expression,
    },
    Block {
        statements: Vec<Statement>,
    },
    UncheckedBlock {
        statements: Vec<Statement>,
    },
    Return {
        value: Option<Expression>,
    },
    Emit {
        event_name: String,
        arguments: Vec<Expression>,
    },
    Revert {
        error_name: Option<String>,
        arguments: Vec<Expression>,
    },
    ExpressionStmt {
        expression: Expression,
    },
    VariableDeclaration {
        name: String,
        type_name: String,
        initial_value: Option<Expression>,
    },
    Assembly {
        span: SourceSpan,
    },
    Break,
    Continue,
    /// The _ placeholder in modifiers
    Placeholder,
}
