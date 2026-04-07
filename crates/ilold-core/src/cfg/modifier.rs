use crate::model::modifier::ModifierDef;
use crate::model::statement::{Statement, StatementKind};

use super::error::CfgError;

/// Inline modifier chain around a function body.
///
/// Given modifiers [onlyOwner, nonReentrant] and function body:
/// 1. Replace _ in nonReentrant with function body
/// 2. Replace _ in onlyOwner with result of step 1
///
/// Modifiers are processed LAST to FIRST because each one wraps the previous result.
pub fn inline_modifiers(
    body: &[Statement],
    modifier_defs: &[&ModifierDef],
) -> Result<Vec<Statement>, CfgError> {
    let mut current_body = body.to_vec();

    // Process in reverse: last modifier wraps the body first
    for modifier in modifier_defs.iter().rev() {
        if !has_placeholder(&modifier.body) {
            return Err(CfgError::ModifierMissingPlaceholder {
                name: modifier.name.clone(),
            });
        }
        current_body = replace_placeholder(&modifier.body, &current_body);
    }

    Ok(current_body)
}

fn has_placeholder(stmts: &[Statement]) -> bool {
    stmts.iter().any(|s| match &s.kind {
        StatementKind::Placeholder => true,
        StatementKind::Block { statements } => has_placeholder(statements),
        StatementKind::UncheckedBlock { statements } => has_placeholder(statements),
        StatementKind::If { then_body, else_body, .. } => {
            has_placeholder(then_body)
                || else_body.as_ref().is_some_and(|e| has_placeholder(e))
        }
        StatementKind::For { body, .. } => has_placeholder(body),
        StatementKind::While { body, .. } => has_placeholder(body),
        StatementKind::DoWhile { body, .. } => has_placeholder(body),
        StatementKind::TryCatch { clauses, .. } => {
            clauses.iter().any(|c| has_placeholder(&c.body))
        }
        _ => false,
    })
}

/// Replace all Placeholder statements with the given body.
fn replace_placeholder(modifier_body: &[Statement], function_body: &[Statement]) -> Vec<Statement> {
    let mut result = Vec::new();

    for stmt in modifier_body {
        match &stmt.kind {
            StatementKind::Placeholder => {
                result.extend(function_body.iter().cloned());
            }
            StatementKind::Block { statements } => {
                result.push(Statement {
                    kind: StatementKind::Block {
                        statements: replace_placeholder(statements, function_body),
                    },
                    span: stmt.span,
                });
            }
            StatementKind::UncheckedBlock { statements } => {
                result.push(Statement {
                    kind: StatementKind::UncheckedBlock {
                        statements: replace_placeholder(statements, function_body),
                    },
                    span: stmt.span,
                });
            }
            StatementKind::If { condition, then_body, else_body } => {
                result.push(Statement {
                    kind: StatementKind::If {
                        condition: condition.clone(),
                        then_body: replace_placeholder(then_body, function_body),
                        else_body: else_body
                            .as_ref()
                            .map(|e| replace_placeholder(e, function_body)),
                    },
                    span: stmt.span,
                });
            }
            StatementKind::For { init, condition, increment, body } => {
                result.push(Statement {
                    kind: StatementKind::For {
                        init: init.clone(),
                        condition: condition.clone(),
                        increment: increment.clone(),
                        body: replace_placeholder(body, function_body),
                    },
                    span: stmt.span,
                });
            }
            StatementKind::While { condition, body } => {
                result.push(Statement {
                    kind: StatementKind::While {
                        condition: condition.clone(),
                        body: replace_placeholder(body, function_body),
                    },
                    span: stmt.span,
                });
            }
            StatementKind::DoWhile { body, condition } => {
                result.push(Statement {
                    kind: StatementKind::DoWhile {
                        body: replace_placeholder(body, function_body),
                        condition: condition.clone(),
                    },
                    span: stmt.span,
                });
            }
            StatementKind::TryCatch { expression, clauses } => {
                let new_clauses = clauses.iter().map(|c| crate::model::statement::CatchClause {
                    name: c.name.clone(),
                    params: c.params.clone(),
                    body: replace_placeholder(&c.body, function_body),
                }).collect();
                result.push(Statement {
                    kind: StatementKind::TryCatch {
                        expression: expression.clone(),
                        clauses: new_clauses,
                    },
                    span: stmt.span,
                });
            }
            // For any other statement, keep as-is
            _ => result.push(stmt.clone()),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::common::SourceSpan;
    use crate::model::expression::{Expression, ExpressionKind};

    fn span() -> SourceSpan {
        SourceSpan { file_index: 0, start_line: 0, start_col: 0, end_line: 0, end_col: 0 }
    }

    fn make_require(name: &str) -> Statement {
        Statement {
            kind: StatementKind::ExpressionStmt {
                expression: Expression {
                    kind: ExpressionKind::FunctionCall {
                        callee: Box::new(Expression {
                            kind: ExpressionKind::Identifier { name: "require".into() },
                            span: span(),
                        }),
                        arguments: vec![Expression {
                            kind: ExpressionKind::Identifier { name: name.into() },
                            span: span(),
                        }],
                    },
                    span: span(),
                },
            },
            span: span(),
        }
    }

    fn make_placeholder() -> Statement {
        Statement { kind: StatementKind::Placeholder, span: span() }
    }

    fn make_return() -> Statement {
        Statement { kind: StatementKind::Return { value: None }, span: span() }
    }

    #[test]
    fn test_single_modifier_inlining() {
        // modifier onlyOwner { require(isOwner); _; }
        let modifier = ModifierDef {
            name: "onlyOwner".into(),
            params: vec![],
            body: vec![make_require("isOwner"), make_placeholder()],
            span: span(),
        };

        // function body: return;
        let body = vec![make_return()];

        let result = inline_modifiers(&body, &[&modifier]).unwrap();

        // Expected: require(isOwner), return
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0].kind, StatementKind::ExpressionStmt { .. }));
        assert!(matches!(result[1].kind, StatementKind::Return { .. }));
    }

    #[test]
    fn test_chained_modifiers() {
        // modifier A { require(a); _; }
        let mod_a = ModifierDef {
            name: "A".into(),
            params: vec![],
            body: vec![make_require("a"), make_placeholder()],
            span: span(),
        };

        // modifier B { require(b); _; }
        let mod_b = ModifierDef {
            name: "B".into(),
            params: vec![],
            body: vec![make_require("b"), make_placeholder()],
            span: span(),
        };

        let body = vec![make_return()];

        // function foo() A B { return; }
        // Expected: require(a), require(b), return
        let result = inline_modifiers(&body, &[&mod_a, &mod_b]).unwrap();

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_modifier_missing_placeholder() {
        let modifier = ModifierDef {
            name: "broken".into(),
            params: vec![],
            body: vec![make_require("x")], // no placeholder!
            span: span(),
        };

        let body = vec![make_return()];
        let result = inline_modifiers(&body, &[&modifier]);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CfgError::ModifierMissingPlaceholder { .. }
        ));
    }
}
