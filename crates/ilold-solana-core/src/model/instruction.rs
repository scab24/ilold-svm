use anchor_lang_idl::types::{IdlField, IdlType};
use serde::{Deserialize, Serialize};
use solana_address::Address;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionDef {
    pub name: String,
    #[serde(with = "discriminator_serde")]
    pub discriminator: [u8; 8],
    pub args: Vec<IdlField>,
    pub accounts: Vec<AccountSpec>,
    #[serde(default)]
    pub returns: Option<IdlType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSpec {
    pub path: String,
    pub name: String,
    pub writable: bool,
    pub signer: bool,
    pub optional: bool,
    #[serde(default)]
    pub address: Option<Address>,
    #[serde(default)]
    pub pda: Option<PdaSpec>,
    #[serde(default)]
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdaSpec {
    pub seeds: Vec<SeedSpec>,
    #[serde(default)]
    pub program: Option<SeedSpec>,
    #[serde(default)]
    pub bump_arg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SeedSpec {
    Const { value: Vec<u8> },
    Arg { path: String, ty: IdlType },
    Account { path: String },
}

pub(crate) mod discriminator_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 8], s: S) -> Result<S::Ok, S::Error> {
        bytes.as_slice().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 8], D::Error> {
        let v = Vec::<u8>::deserialize(d)?;
        v.try_into().map_err(|v: Vec<u8>| {
            serde::de::Error::custom(format!("expected 8-byte discriminator, got {}", v.len()))
        })
    }
}
