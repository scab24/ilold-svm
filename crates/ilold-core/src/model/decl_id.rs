use serde::{Deserialize, Serialize};

/// Mirrors solc's AST node `id` (`referencedDeclaration`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeclId(pub isize);
