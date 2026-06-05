//! Shared utility helpers used across multiple analysis passes.

pub fn is_type_cast(name: &str) -> bool {
    let name = name.trim();
    // `Type(expr)` is a cast; a bare non-elementary name (internalTransfer) is a call.
    if let Some((head, _)) = name.split_once('(') {
        return is_elementary_type(head) || is_user_type_name(head);
    }
    is_elementary_type(name)
}

fn is_elementary_type(name: &str) -> bool {
    matches!(name, "address" | "bool" | "string")
        || is_sized_type(name, "uint")
        || is_sized_type(name, "int")
        || is_sized_type(name, "bytes")
}

fn is_sized_type(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.bytes().all(|b| b.is_ascii_digit()))
}

fn is_user_type_name(head: &str) -> bool {
    head.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && head.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Extract the base variable name from an assignment target expression.
///
/// Strips any indexing (`balances[user]` → `balances`) and field access
/// (`config.fee` → `config`). Used by mutation harvesters and path
/// analyzers to match target expressions against state variable names.
pub fn target_base_name(target: &str) -> &str {
    let base = target.split('[').next().unwrap_or(target);
    base.split('.').next().unwrap_or(base)
}
