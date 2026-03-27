use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::contract::ContractDef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub source_files: Vec<SourceFile>,
    pub contracts: Vec<ContractDef>,
    #[serde(skip)]
    pub contract_index: HashMap<String, usize>,
}

impl Project {
    /// Rebuild the contract_index from the contracts vec.
    /// Call this after deserialization since serde(skip) means the index is empty.
    pub fn rebuild_index(&mut self) {
        self.contract_index = self
            .contracts
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.clone(), i))
            .collect();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}
