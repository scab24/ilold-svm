use crate::model::contract::ContractDef;
use crate::model::function::FunctionDef;
use crate::model::project::Project;
use crate::model::statement::{Statement, StatementKind};

use super::types::{StatementOrigin, StatementPath};

/// A single statement in the flattened walk of a function body, carrying
/// its position in program order, nesting depth, a back-link to its
/// lexical parent (another `FlatStatement` index) when nested inside
/// If/For/While/Block/TryCatch, and an `origin` tag distinguishing
/// statements lifted from the function body vs from a wrapping modifier.
pub struct FlatStatement<'a> {
    pub path: StatementPath,
    pub statement: &'a Statement,
    pub depth: usize,
    pub parent: Option<usize>,
    pub origin: StatementOrigin,
}

/// Walk a function and every applied modifier in execution order, emitting
/// one `FlatStatement` per encountered statement.
///
/// Order: for `function f() mod1 mod2 { body }` we walk
///   `mod1.before` → `mod2.before` → `body` → `mod2.after` → `mod1.after`
/// where each modifier body is split at the first top-level `_;`
/// placeholder. Modifiers without an explicit placeholder are treated
/// entirely as `before` (over-inclusive but lossless).
///
/// Modifier resolution walks the inheritance chain via
/// `Project::resolve_modifier`; unresolved modifiers are silently
/// skipped (the slicer should not crash on a partially-parsed project).
///
/// Paths are single-element `[global_index]` values that match the
/// position of the entry in the returned vector — the same scheme as
/// before, just over a larger flat list.
pub fn flatten_function<'a>(
    project: &'a Project,
    contract: &'a ContractDef,
    function: &'a FunctionDef,
) -> Vec<FlatStatement<'a>> {
    let mut out: Vec<FlatStatement<'a>> = Vec::new();

    let body: &'a [Statement] = function.body.as_deref().unwrap_or(&[]);

    // Resolve every applied modifier and split its body at the first
    // top-level Placeholder. We collect the references up-front so we
    // can iterate the "after" parts in reverse order at the end.
    struct ModifierParts<'a> {
        name: &'a str,
        before: &'a [Statement],
        after: &'a [Statement],
    }
    let mut modifier_parts: Vec<ModifierParts<'a>> = Vec::new();
    for m_ref in &function.modifiers {
        let Some(m_def) = project.resolve_modifier(contract, &m_ref.name) else {
            continue;
        };
        let split = m_def
            .body
            .iter()
            .position(|s| matches!(s.kind, StatementKind::Placeholder));
        let (before, after) = match split {
            Some(idx) => (&m_def.body[..idx], &m_def.body[idx + 1..]),
            None => (m_def.body.as_slice(), &[][..]),
        };
        modifier_parts.push(ModifierParts {
            name: m_def.name.as_str(),
            before,
            after,
        });
    }

    // Walk every modifier's "before" half, in declaration order.
    for parts in &modifier_parts {
        let origin = StatementOrigin::Modifier(parts.name.to_string());
        for stmt in parts.before {
            walk(stmt, 0, None, &origin, &mut out);
        }
    }

    // Walk the function body itself.
    for stmt in body {
        walk(stmt, 0, None, &StatementOrigin::FunctionBody, &mut out);
    }

    // Walk every modifier's "after" half in reverse — innermost modifier
    // unwraps first, matching Solidity's call stack semantics.
    for parts in modifier_parts.iter().rev() {
        let origin = StatementOrigin::Modifier(parts.name.to_string());
        for stmt in parts.after {
            walk(stmt, 0, None, &origin, &mut out);
        }
    }

    out
}

fn walk<'a>(
    stmt: &'a Statement,
    depth: usize,
    parent: Option<usize>,
    origin: &StatementOrigin,
    out: &mut Vec<FlatStatement<'a>>,
) {
    let idx = out.len();
    out.push(FlatStatement {
        path: StatementPath(vec![idx]),
        statement: stmt,
        depth,
        parent,
        origin: origin.clone(),
    });

    let child_parent = Some(idx);
    let child_depth = depth + 1;

    match &stmt.kind {
        StatementKind::If { then_body, else_body, .. } => {
            for s in then_body {
                walk(s, child_depth, child_parent, origin, out);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    walk(s, child_depth, child_parent, origin, out);
                }
            }
        }
        StatementKind::For { init, body, .. } => {
            if let Some(init_stmt) = init {
                walk(init_stmt, child_depth, child_parent, origin, out);
            }
            for s in body {
                walk(s, child_depth, child_parent, origin, out);
            }
        }
        StatementKind::While { body, .. } | StatementKind::DoWhile { body, .. } => {
            for s in body {
                walk(s, child_depth, child_parent, origin, out);
            }
        }
        StatementKind::Block { statements } | StatementKind::UncheckedBlock { statements } => {
            for s in statements {
                walk(s, child_depth, child_parent, origin, out);
            }
        }
        StatementKind::TryCatch { clauses, .. } => {
            for clause in clauses {
                for s in &clause.body {
                    walk(s, child_depth, child_parent, origin, out);
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
