use crate::model::modifier::ModifierDef;
use crate::model::statement::{Statement, StatementKind};

use super::error::CfgError;

/// Statement tagged with the modifier it originates from (if any).
/// `provenance == None` means the statement is part of the function's own body.
#[derive(Debug, Clone)]
pub struct TaggedStatement {
    pub stmt: Statement,
    pub provenance: Option<String>,
}

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
) -> Result<Vec<TaggedStatement>, CfgError> {
    let mut current: Vec<TaggedStatement> = body
        .iter()
        .cloned()
        .map(|s| TaggedStatement { stmt: s, provenance: None })
        .collect();

    for modifier in modifier_defs.iter().rev() {
        if !has_placeholder(&modifier.body) {
            return Err(CfgError::ModifierMissingPlaceholder {
                name: modifier.name.clone(),
            });
        }
        current = replace_placeholder_tagged(&modifier.body, &current, &modifier.name);
    }

    Ok(current)
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

/// Same as `replace_placeholder` but preserves provenance: every statement from
/// the modifier body is tagged with `modifier_name`, while the placeholder
/// substitution keeps the inner function body's existing tags (which may be
/// `None` or an inner modifier from a nested inlining step).
fn replace_placeholder_tagged(
    modifier_body: &[Statement],
    function_body: &[TaggedStatement],
    modifier_name: &str,
) -> Vec<TaggedStatement> {
    let mut result: Vec<TaggedStatement> = Vec::new();

    for stmt in modifier_body {
        match &stmt.kind {
            StatementKind::Placeholder => {
                // The placeholder becomes the inner body verbatim — inner tags
                // are preserved so nested modifiers / function body stay
                // correctly attributed.
                result.extend(function_body.iter().cloned());
            }
            StatementKind::Block { statements } => {
                let inner = replace_placeholder_tagged_block(statements, function_body, modifier_name);
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::Block { statements: inner },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            StatementKind::UncheckedBlock { statements } => {
                let inner = replace_placeholder_tagged_block(statements, function_body, modifier_name);
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::UncheckedBlock { statements: inner },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            StatementKind::If { condition, then_body, else_body } => {
                let then_body = replace_placeholder_tagged_block(then_body, function_body, modifier_name);
                let else_body = else_body
                    .as_ref()
                    .map(|e| replace_placeholder_tagged_block(e, function_body, modifier_name));
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::If {
                            condition: condition.clone(),
                            then_body,
                            else_body,
                        },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            StatementKind::For { init, condition, increment, body } => {
                let body = replace_placeholder_tagged_block(body, function_body, modifier_name);
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::For {
                            init: init.clone(),
                            condition: condition.clone(),
                            increment: increment.clone(),
                            body,
                        },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            StatementKind::While { condition, body } => {
                let body = replace_placeholder_tagged_block(body, function_body, modifier_name);
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::While {
                            condition: condition.clone(),
                            body,
                        },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            StatementKind::DoWhile { body, condition } => {
                let body = replace_placeholder_tagged_block(body, function_body, modifier_name);
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::DoWhile {
                            body,
                            condition: condition.clone(),
                        },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            StatementKind::TryCatch { expression, clauses } => {
                let new_clauses = clauses.iter().map(|c| crate::model::statement::CatchClause {
                    name: c.name.clone(),
                    params: c.params.clone(),
                    body: replace_placeholder_tagged_block(&c.body, function_body, modifier_name),
                }).collect();
                result.push(TaggedStatement {
                    stmt: Statement {
                        kind: StatementKind::TryCatch {
                            expression: expression.clone(),
                            clauses: new_clauses,
                        },
                        span: stmt.span,
                    },
                    provenance: Some(modifier_name.to_string()),
                });
            }
            _ => result.push(TaggedStatement {
                stmt: stmt.clone(),
                provenance: Some(modifier_name.to_string()),
            }),
        }
    }

    result
}

/// Helper: recursive replacement inside compound statements, returns a flat
/// Vec<Statement> (dropping the tagging layer, because compound statements
/// store Statement not TaggedStatement). The inner recursion still correctly
/// re-tags top-level statements via the main `replace_placeholder_tagged`
/// caller when used at the outermost level.
fn replace_placeholder_tagged_block(
    modifier_body: &[Statement],
    function_body: &[TaggedStatement],
    modifier_name: &str,
) -> Vec<Statement> {
    replace_placeholder_tagged(modifier_body, function_body, modifier_name)
        .into_iter()
        .map(|t| t.stmt)
        .collect()
}

/// Replace all Placeholder statements with the given body.
#[allow(dead_code)]
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

        // Expected: require(isOwner) tagged with "onlyOwner", return (no tag)
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0].stmt.kind, StatementKind::ExpressionStmt { .. }));
        assert_eq!(result[0].provenance.as_deref(), Some("onlyOwner"));
        assert!(matches!(result[1].stmt.kind, StatementKind::Return { .. }));
        assert_eq!(result[1].provenance, None);
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
        // Expected: require(a) [tagged A], require(b) [tagged B], return [no tag]
        let result = inline_modifiers(&body, &[&mod_a, &mod_b]).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].provenance.as_deref(), Some("A"));
        assert_eq!(result[1].provenance.as_deref(), Some("B"));
        assert_eq!(result[2].provenance, None);
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
