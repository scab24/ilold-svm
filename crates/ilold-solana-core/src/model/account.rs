use anchor_lang_idl::types::IdlTypeDef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTypeDef {
    pub name: String,
    #[serde(with = "super::instruction::discriminator_serde")]
    pub discriminator: [u8; 8],
    pub layout: IdlTypeDef,
}
