use std::collections::HashSet;

use crate::model::expression::{Expression, ExpressionKind};
use crate::model::statement::{Statement, StatementKind};

/// Walk an expression tree and collect every `Identifier` leaf name.
/// Used to build the "uses" set of a statement.
pub fn walk_expr_identifiers(expr: &Expression, out: &mut HashSet<String>) {
    match &expr.kind {
        ExpressionKind::Identifier { name, .. } => {
            out.insert(name.clone());
        }
        ExpressionKind::MemberAccess { object, .. } => {
            walk_expr_identifiers(object, out);
        }
        ExpressionKind::IndexAccess { base, index } => {
            walk_expr_identifiers(base, out);
            if let Some(idx) = index {
                walk_expr_identifiers(idx, out);
            }
        }
        ExpressionKind::BinaryOp { left, right, .. } => {
            walk_expr_identifiers(left, out);
            walk_expr_identifiers(right, out);
        }
        ExpressionKind::UnaryOp { operand, .. } => {
            walk_expr_identifiers(operand, out);
        }
        ExpressionKind::FunctionCall { callee, arguments } => {
            // The callee may itself mention identifiers (e.g. `someFn`, or
            // `obj.method` — we care about `obj`). Skip the callee if it's
            // just a plain Identifier that names the function being called,
            // since that's not a data read.
            match &callee.kind {
                ExpressionKind::Identifier { .. } => {
                    // Pure function name — not a data use.
                }
                _ => walk_expr_identifiers(callee, out),
            }
            for arg in arguments {
                walk_expr_identifiers(arg, out);
            }
        }
        ExpressionKind::Assignment { target, value, .. } => {
            // Used when an assignment is part of a bigger expression. The
            // target's side effects (e.g. `a[i] = x` reads `i`) need to be
            // captured; the value is a normal use.
            walk_assignment_target_uses(target, out);
            walk_expr_identifiers(value, out);
        }
        ExpressionKind::Ternary { condition, true_expr, false_expr } => {
            walk_expr_identifiers(condition, out);
            walk_expr_identifiers(true_expr, out);
            walk_expr_identifiers(false_expr, out);
        }
        ExpressionKind::TypeCast { expression, .. } => {
            walk_expr_identifiers(expression, out);
        }
        ExpressionKind::New { arguments, .. } => {
            for arg in arguments {
                walk_expr_identifiers(arg, out);
            }
        }
        ExpressionKind::Tuple { elements } => {
            for e in elements.iter().flatten() {
                walk_expr_identifiers(e, out);
            }
        }
        ExpressionKind::IndexRange { base, start, end } => {
            walk_expr_identifiers(base, out);
            if let Some(s) = start {
                walk_expr_identifiers(s, out);
            }
            if let Some(e) = end {
                walk_expr_identifiers(e, out);
            }
        }
        ExpressionKind::Literal { .. } | ExpressionKind::TypeMeta { .. } => {}
    }
}

/// Collect the USE identifiers inside an assignment target. `x = …` has
/// no uses on the target, but `arr[i] = …` uses `i`, and `obj.field = …`
/// uses `obj`. The base identifier itself (`x`, `arr`, `obj`) is the DEF
/// and is NOT added here.
fn walk_assignment_target_uses(target: &Expression, out: &mut HashSet<String>) {
    match &target.kind {
        ExpressionKind::Identifier { .. } => {
            // The target identifier is the def, not a use.
        }
        ExpressionKind::IndexAccess { base, index } => {
            walk_assignment_target_uses(base, out);
            if let Some(idx) = index {
                walk_expr_identifiers(idx, out);
            }
        }
        ExpressionKind::MemberAccess { object, .. } => {
            walk_assignment_target_uses(object, out);
        }
        _ => {
            // Anything else on the LHS is unusual (tuple? deref?) — treat
            // sub-expressions as uses to stay safe.
            walk_expr_identifiers(target, out);
        }
    }
}

/// Collect the DEF identifier of an assignment target. Returns the base
/// variable name being written (e.g. `balances` for `balances[to] = x`).
pub fn extract_assignment_def(target: &Expression) -> Option<String> {
    match &target.kind {
        ExpressionKind::Identifier { name, .. } => Some(name.clone()),
        ExpressionKind::IndexAccess { base, .. } => extract_assignment_def(base),
        ExpressionKind::MemberAccess { object, .. } => extract_assignment_def(object),
        _ => None,
    }
}

/// Use/def sets for a single `Statement`. Does NOT recurse into nested
/// statements — callers handle block walking separately so they can
/// associate sub-statements with their own paths.
///
/// For statements that only introduce control flow (If/For/While), the
/// returned uses are the condition's uses; defs is empty.
pub fn extract_statement_uses_defs(stmt: &Statement) -> (HashSet<String>, HashSet<String>) {
    let mut uses: HashSet<String> = HashSet::new();
    let mut defs: HashSet<String> = HashSet::new();

    match &stmt.kind {
        StatementKind::ExpressionStmt { expression } => {
            match &expression.kind {
                ExpressionKind::Assignment { target, value, .. } => {
                    if let Some(def) = extract_assignment_def(target) {
                        defs.insert(def);
                    }
                    walk_assignment_target_uses(target, &mut uses);
                    walk_expr_identifiers(value, &mut uses);
                }
                _ => walk_expr_identifiers(expression, &mut uses),
            }
        }
        StatementKind::VariableDeclaration { name, initial_value, .. } => {
            defs.insert(name.clone());
            if let Some(val) = initial_value {
                walk_expr_identifiers(val, &mut uses);
            }
        }
        StatementKind::If { condition, .. } => {
            walk_expr_identifiers(condition, &mut uses);
        }
        StatementKind::For { init: _, condition, increment, .. } => {
            // `init` is a Statement of its own and will be handled as a
            // separate flat entry; same for the body. Increment is an
            // Expression evaluated each iteration — its uses count for
            // the loop header.
            if let Some(c) = condition {
                walk_expr_identifiers(c, &mut uses);
            }
            if let Some(inc) = increment {
                walk_expr_identifiers(inc, &mut uses);
            }
        }
        StatementKind::While { condition, .. } | StatementKind::DoWhile { condition, .. } => {
            walk_expr_identifiers(condition, &mut uses);
        }
        StatementKind::Return { value } => {
            if let Some(v) = value {
                walk_expr_identifiers(v, &mut uses);
            }
        }
        StatementKind::Emit { arguments, .. } => {
            for arg in arguments {
                walk_expr_identifiers(arg, &mut uses);
            }
        }
        StatementKind::Revert { arguments, .. } => {
            for arg in arguments {
                walk_expr_identifiers(arg, &mut uses);
            }
        }
        StatementKind::TryCatch { expression, .. } => {
            walk_expr_identifiers(expression, &mut uses);
        }
        StatementKind::Block { .. }
        | StatementKind::UncheckedBlock { .. }
        | StatementKind::Assembly { .. }
        | StatementKind::Placeholder
        | StatementKind::Continue
        | StatementKind::Break => {}
    }

    (uses, defs)
}

/// Render a statement as a short human-readable string for slice output.
/// This is NOT a full Solidity pretty-printer — just enough to identify
/// the statement in a slice result.
pub fn statement_text(stmt: &Statement) -> String {
    match &stmt.kind {
        StatementKind::ExpressionStmt { expression } => expr_to_text(expression),
        StatementKind::VariableDeclaration { name, type_name, initial_value, .. } => {
            match initial_value {
                Some(v) => format!("{} {} = {}", type_name, name, expr_to_text(v)),
                None => format!("{} {}", type_name, name),
            }
        }
        StatementKind::If { condition, .. } => format!("if ({}) {{ … }}", expr_to_text(condition)),
        StatementKind::For { .. } => "for (…) { … }".to_string(),
        StatementKind::While { condition, .. } => format!("while ({}) {{ … }}", expr_to_text(condition)),
        StatementKind::DoWhile { condition, .. } => format!("do {{ … }} while ({})", expr_to_text(condition)),
        StatementKind::Return { value } => match value {
            Some(v) => format!("return {}", expr_to_text(v)),
            None => "return".to_string(),
        },
        StatementKind::Emit { event_name, arguments } => {
            let args = arguments.iter().map(expr_to_text).collect::<Vec<_>>().join(", ");
            format!("emit {}({})", event_name, args)
        }
        StatementKind::Revert { error_name, arguments } => {
            let args = arguments.iter().map(expr_to_text).collect::<Vec<_>>().join(", ");
            match error_name {
                Some(n) => format!("revert {}({})", n, args),
                None => format!("revert({})", args),
            }
        }
        StatementKind::TryCatch { expression, .. } => {
            format!("try {} {{ … }} catch {{ … }}", expr_to_text(expression))
        }
        StatementKind::Block { .. } => "{ … }".to_string(),
        StatementKind::UncheckedBlock { .. } => "unchecked { … }".to_string(),
        StatementKind::Assembly { .. } => "assembly { … }".to_string(),
        StatementKind::Placeholder => "_".to_string(),
        StatementKind::Continue => "continue".to_string(),
        StatementKind::Break => "break".to_string(),
    }
}

/// Minimal expression-to-text for slice entries. Keeps the output
/// compact — full fidelity is not the goal.
fn expr_to_text(expr: &Expression) -> String {
    match &expr.kind {
        ExpressionKind::Identifier { name, .. } => name.clone(),
        ExpressionKind::Literal { value, .. } => value.clone(),
        ExpressionKind::MemberAccess { object, member, .. } => {
            format!("{}.{}", expr_to_text(object), member)
        }
        ExpressionKind::IndexAccess { base, index } => {
            let idx = index.as_ref().map(|e| expr_to_text(e)).unwrap_or_default();
            format!("{}[{}]", expr_to_text(base), idx)
        }
        ExpressionKind::BinaryOp { left, operator, right } => {
            format!("{} {} {}", expr_to_text(left), operator.as_str(), expr_to_text(right))
        }
        ExpressionKind::UnaryOp { operator, operand } => {
            let (sym, postfix) = operator.format_parts();
            if postfix {
                format!("{}{}", expr_to_text(operand), sym)
            } else {
                format!("{}{}", sym, expr_to_text(operand))
            }
        }
        ExpressionKind::FunctionCall { callee, arguments } => {
            let args = arguments.iter().map(expr_to_text).collect::<Vec<_>>().join(", ");
            format!("{}({})", expr_to_text(callee), args)
        }
        ExpressionKind::Assignment { target, operator, value } => {
            format!("{} {} {}", expr_to_text(target), operator.as_str(), expr_to_text(value))
        }
        ExpressionKind::Ternary { condition, true_expr, false_expr } => {
            format!("{} ? {} : {}", expr_to_text(condition), expr_to_text(true_expr), expr_to_text(false_expr))
        }
        ExpressionKind::TypeCast { type_name, expression } => {
            format!("{}({})", type_name, expr_to_text(expression))
        }
        ExpressionKind::TypeMeta { type_name } => format!("type({})", type_name),
        ExpressionKind::New { type_name, arguments } => {
            let args = arguments.iter().map(expr_to_text).collect::<Vec<_>>().join(", ");
            format!("new {}({})", type_name, args)
        }
        ExpressionKind::Tuple { elements } => {
            let parts = elements
                .iter()
                .map(|e| e.as_ref().map(expr_to_text).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", parts)
        }
        ExpressionKind::IndexRange { base, start, end } => {
            let s = start.as_ref().map(|e| expr_to_text(e)).unwrap_or_default();
            let e = end.as_ref().map(|e| expr_to_text(e)).unwrap_or_default();
            format!("{}[{}:{}]", expr_to_text(base), s, e)
        }
    }
}
