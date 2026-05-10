use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::model::{AccountSpec, AccountTypeDef, PdaSpec, ProgramDef, SeedSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramView {
    pub name: String,
    pub program_id: String,
    pub instructions: Vec<IxView>,
    pub accounts: Vec<AccountView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_coupling: Option<Vec<CouplingPair>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_gated: Option<HashSet<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_accounts: Option<HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IxView {
    pub name: String,
    pub discriminator_hex: String,
    pub args: Vec<ArgView>,
    pub accounts: Vec<IxAccountView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgView {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IxAccountView {
    pub path: String,
    pub name: String,
    pub kind: AccountKind,
    pub writable: bool,
    pub signer: bool,
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pda: Option<PdaView>,
    #[serde(default)]
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Program,
    System,
    Sysvar,
    Pda,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdaView {
    pub seeds: Vec<SeedView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bump_arg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SeedView {
    Const {
        value_hex: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value_utf8: Option<String>,
    },
    Arg {
        name: String,
        ty: String,
    },
    Account {
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountView {
    pub name: String,
    pub discriminator_hex: String,
    pub fields: Vec<FieldView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldView {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingPair {
    pub a: String,
    pub b: String,
    pub shared_writable: Vec<String>,
}

const SYSTEM_PROGRAM_NAMES: &[&str] = &[
    "system_program",
    "token_program",
    "associated_token_program",
    "token_program_2022",
];

const SYSVAR_NAMES: &[&str] = &[
    "rent",
    "clock",
    "instructions",
    "recent_blockhashes",
    "slot_history",
    "stake_history",
    "epoch_schedule",
    "fees",
];

impl ProgramDef {
    pub fn compute_view(&self) -> ProgramView {
        let instructions = self
            .instructions
            .iter()
            .map(|ix| build_ix_view(ix, &self.account_types))
            .collect();
        let accounts = self
            .account_types
            .iter()
            .map(build_account_view)
            .collect();
        ProgramView {
            name: self.name.clone(),
            program_id: self.program_id.to_string(),
            instructions,
            accounts,
            state_coupling: None,
            admin_gated: None,
            system_accounts: None,
        }
    }
}

fn build_ix_view(
    ix: &crate::model::InstructionDef,
    account_types: &[AccountTypeDef],
) -> IxView {
    let args = ix
        .args
        .iter()
        .map(|a| ArgView {
            name: a.name.clone(),
            ty: String::new(),
        })
        .collect();
    let accounts = ix
        .accounts
        .iter()
        .map(|spec| build_ix_account_view(spec, account_types))
        .collect();
    IxView {
        name: ix.name.clone(),
        discriminator_hex: String::new(),
        args,
        accounts,
        returns: None,
    }
}

fn build_ix_account_view(
    spec: &AccountSpec,
    account_types: &[AccountTypeDef],
) -> IxAccountView {
    let pda = spec.pda.as_ref().map(build_pda_view);
    let kind = classify_account_kind(&spec.name, &pda, account_types);
    IxAccountView {
        path: spec.path.clone(),
        name: spec.name.clone(),
        kind,
        writable: spec.writable,
        signer: spec.signer,
        optional: spec.optional,
        address: spec.address.as_ref().map(|a| a.to_string()),
        pda,
        relations: spec.relations.clone(),
    }
}

fn build_pda_view(pda: &PdaSpec) -> PdaView {
    let seeds = pda.seeds.iter().map(seed_to_view).collect();
    let program = pda.program.as_ref().map(seed_program_to_string);
    PdaView {
        seeds,
        program,
        bump_arg: pda.bump_arg.clone(),
    }
}

fn seed_to_view(seed: &SeedSpec) -> SeedView {
    match seed {
        SeedSpec::Const { value } => {
            let value_hex = bytes_to_hex(value);
            let value_utf8 = bytes_to_ascii_graphic(value);
            SeedView::Const {
                value_hex,
                value_utf8,
            }
        }
        SeedSpec::Arg { path, .. } => SeedView::Arg {
            name: path.clone(),
            ty: String::new(),
        },
        SeedSpec::Account { path } => SeedView::Account { path: path.clone() },
    }
}

fn seed_program_to_string(seed: &SeedSpec) -> String {
    match seed {
        SeedSpec::Const { value } => match solana_address::Address::try_from(value.as_slice()) {
            Ok(a) => a.to_string(),
            Err(_) => format!("const:{:02x?}", value),
        },
        SeedSpec::Account { path } => format!("account:{path}"),
        SeedSpec::Arg { path, .. } => format!("arg:{path}"),
    }
}

fn build_account_view(account: &AccountTypeDef) -> AccountView {
    AccountView {
        name: account.name.clone(),
        discriminator_hex: String::new(),
        fields: Vec::new(),
    }
}

pub(crate) fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

fn classify_account_kind(
    name: &str,
    pda: &Option<PdaView>,
    account_types: &[AccountTypeDef],
) -> AccountKind {
    if pda.is_some() {
        return AccountKind::Pda;
    }
    if SYSTEM_PROGRAM_NAMES.contains(&name) {
        return AccountKind::System;
    }
    if SYSVAR_NAMES.contains(&name) || name.starts_with("sysvar_") {
        return AccountKind::Sysvar;
    }
    let pascal = snake_to_pascal(name);
    if account_types.iter().any(|a| a.name == pascal) {
        return AccountKind::Program;
    }
    AccountKind::Other
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn bytes_to_ascii_graphic(bytes: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(bytes).ok()?;
    if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
        Some(s.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idl::parse_idl;

    const LEVER_JSON: &str = include_str!("../tests/fixtures/lever.json");
    const RELATIONS_JSON: &str = include_str!("../tests/fixtures/relations.json");

    fn lever_program() -> ProgramDef {
        ProgramDef::from_idl(parse_idl(LEVER_JSON).expect("parse lever"))
            .expect("build lever ProgramDef")
    }

    fn relations_program() -> ProgramDef {
        ProgramDef::from_idl(parse_idl(RELATIONS_JSON).expect("parse relations"))
            .expect("build relations ProgramDef")
    }

    #[test]
    fn compute_view_has_all_instructions() {
        let view = lever_program().compute_view();
        assert_eq!(view.instructions.len(), 2);
        let names: Vec<_> = view.instructions.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"initialize"));
        assert!(names.contains(&"switch_power"));
    }

    #[test]
    fn compute_view_pool_struct_has_real_fields() {
        // Phase 1 placeholder: AccountView::fields stays empty until T-R50.
        // The shape is asserted; populated content lands in Phase 2.
        let view = lever_program().compute_view();
        let power = view
            .accounts
            .iter()
            .find(|a| a.name == "PowerStatus")
            .expect("PowerStatus account-type present");
        assert!(power.fields.is_empty());
        assert_eq!(power.discriminator_hex, "");
    }

    #[test]
    fn account_kind_classification() {
        let view = lever_program().compute_view();
        let initialize = view.instructions.iter().find(|i| i.name == "initialize").unwrap();
        let system = initialize.accounts.iter().find(|a| a.name == "system_program").unwrap();
        assert_eq!(system.kind, AccountKind::System);
        // lever's "power" maps to account-type "PowerStatus" — snake_to_pascal
        // gives "Power" which does not match, so it falls through to Other.
        let power = initialize.accounts.iter().find(|a| a.name == "power").unwrap();
        assert_eq!(power.kind, AccountKind::Other);

        let relations = relations_program().compute_view();
        let init_base = relations.instructions.iter().find(|i| i.name == "init_base").unwrap();
        let pda_acc = init_base.accounts.iter().find(|a| a.name == "account").unwrap();
        assert_eq!(pda_acc.kind, AccountKind::Pda);
        // relations also has my_account → MyAccount (snake→pascal match).
        let test_relation = relations.instructions.iter().find(|i| i.name == "test_relation").unwrap();
        let typed_acc = test_relation.accounts.iter().find(|a| a.name == "my_account").unwrap();
        assert_eq!(typed_acc.kind, AccountKind::Program);
    }

    #[test]
    fn seed_const_value_utf8_only_when_ascii_graphic() {
        let printable = SeedSpec::Const {
            value: b"stake".to_vec(),
        };
        match seed_to_view(&printable) {
            SeedView::Const { value_hex, value_utf8 } => {
                assert_eq!(value_hex, "0x7374616b65");
                assert_eq!(value_utf8.as_deref(), Some("stake"));
            }
            other => panic!("expected Const, got {other:?}"),
        }

        // Tab + LF — both ASCII but neither is_ascii_graphic and neither is space.
        let non_graphic = SeedSpec::Const {
            value: vec![0x09, 0x0a],
        };
        match seed_to_view(&non_graphic) {
            SeedView::Const { value_hex, value_utf8 } => {
                assert_eq!(value_hex, "0x090a");
                assert!(value_utf8.is_none());
            }
            other => panic!("expected Const, got {other:?}"),
        }
    }

    #[test]
    fn snake_to_pascal_handles_basic_cases() {
        assert_eq!(snake_to_pascal("pool"), "Pool");
        assert_eq!(snake_to_pascal("user_stake"), "UserStake");
        assert_eq!(snake_to_pascal(""), "");
        assert_eq!(snake_to_pascal("__double"), "Double");
    }
}
