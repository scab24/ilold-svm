use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeTrace {
    pub logs: Vec<String>,
    pub compute_units: u64,
    pub inner_instructions: Vec<InnerInstruction>,
    pub account_diffs: Vec<AccountDiff>,
    #[serde(default)]
    pub return_data: Option<Vec<u8>>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnerInstruction {
    pub program: String,
    pub instruction: String,
    pub depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDiff {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub before: Option<Vec<u8>>,
    #[serde(default)]
    pub after: Option<Vec<u8>>,
    pub lamports_delta: i128,
    pub owner_changed: bool,
    #[serde(default)]
    pub decoded_before: Option<DecodedAccount>,
    #[serde(default)]
    pub decoded_after: Option<DecodedAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedAccount {
    pub type_name: String,
    pub value: serde_json::Value,
}
