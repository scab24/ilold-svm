use crate::model::statement::{Statement, StatementKind};

use super::types::StatementPath;

/// A single statement in the flattened walk of a function body, carrying
/// its position in program order, nesting depth, and a back-link to its
/// lexical parent (another `FlatStatement` index) when nested inside
/// If/For/While/Block/TryCatch. Top-level statements have `parent = None`.
pub struct FlatStatement<'a> {
    pub path: StatementPath,
    pub statement: &'a Statement,
    pub depth: usize,
    pub parent: Option<usize>,
}

/// Walk a function body in pre-order and emit one `FlatStatement` per
/// encountered statement (including those nested inside control-flow
/// constructs). Paths are single-element `[global_index]` values that
/// match the position of the entry in the returned vector; this keeps
/// lookup by path an O(n) linear search but removes any ambiguity between
/// then/else/body branches.
pub fn flatten_function_body(body: &[Statement]) -> Vec<FlatStatement<'_>> {
    let mut out: Vec<FlatStatement<'_>> = Vec::new();
    for stmt in body {
        walk(stmt, 0, None, &mut out);
    }
    out
}

fn walk<'a>(
    stmt: &'a Statement,
    depth: usize,
    parent: Option<usize>,
    out: &mut Vec<FlatStatement<'a>>,
) {
    let idx = out.len();
    out.push(FlatStatement {
        path: StatementPath(vec![idx]),
        statement: stmt,
        depth,
        parent,
    });

    let child_parent = Some(idx);
    let child_depth = depth + 1;

    match &stmt.kind {
        StatementKind::If { then_body, else_body, .. } => {
            for s in then_body {
                walk(s, child_depth, child_parent, out);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    walk(s, child_depth, child_parent, out);
                }
            }
        }
        StatementKind::For { init, body, .. } => {
            if let Some(init_stmt) = init {
                walk(init_stmt, child_depth, child_parent, out);
            }
            for s in body {
                walk(s, child_depth, child_parent, out);
            }
        }
        StatementKind::While { body, .. } | StatementKind::DoWhile { body, .. } => {
            for s in body {
                walk(s, child_depth, child_parent, out);
            }
        }
        StatementKind::Block { statements } | StatementKind::UncheckedBlock { statements } => {
            for s in statements {
                walk(s, child_depth, child_parent, out);
            }
        }
        StatementKind::TryCatch { clauses, .. } => {
            for clause in clauses {
                for s in &clause.body {
                    walk(s, child_depth, child_parent, out);
                }
            }
        }
        StatementKind::ExpressionStmt { .. }
        | StatementKind::VariableDeclaration { .. }
        | StatementKind::Return { .. }
        | StatementKind::Emit { .. }
        | StatementKind::Revert { .. }
        | StatementKind::Assembly { .. }
        | StatementKind::Placeholder
        | StatementKind::Continue
        | StatementKind::Break => {}
    }
}
