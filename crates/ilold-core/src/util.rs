//! Shared utility helpers used across multiple analysis passes.

/// Filter out type casts that look like function calls
/// (e.g. `IERC20(addr)`, `address(0)`, `uint256(x)`).
///
/// Used by the call graph builder and by transitive-effect analysis
/// to avoid treating type casts as real internal calls.
pub fn is_type_cast(name: &str) -> bool {
    let name = name.trim();
    // Solidity elementary types
    if name.starts_with("type(")
        || name.starts_with("address")
        || name.starts_with("uint")
        || name.starts_with("int")
        || name.starts_with("bytes")
        || name.starts_with("bool")
        || name.starts_with("string")
    {
        return true;
    }
    // Interface type casts: starts with I + uppercase (IERC20, IUniswapV2Pair)
    if name.starts_with('I')
        && name.len() > 1
        && name.chars().nth(1).is_some_and(|c| c.is_uppercase())
    {
        return true;
    }
    false
}
