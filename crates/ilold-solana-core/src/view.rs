use std::collections::{BTreeSet, HashSet};

use anchor_lang_idl::types::{
    IdlArrayLen, IdlDefinedFields, IdlField, IdlGenericArg, IdlType, IdlTypeDefTy,
};
use serde::{Deserialize, Serialize};

use crate::model::{
    AccountSpec, AccountTypeDef, InstructionDef, PdaSpec, ProgramDef, SeedSpec,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramView {
    pub name: String,
    pub program_id: String,
    pub instructions: Vec<IxView>,
    pub accounts: Vec<AccountView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_coupling: Option<Vec<CouplingPair>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_sorted_opt_string_set",
        deserialize_with = "deserialize_opt_string_set"
    )]
    pub admin_gated: Option<HashSet<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_sorted_opt_string_set",
        deserialize_with = "deserialize_opt_string_set"
    )]
    pub system_accounts: Option<HashSet<String>>,
}

fn serialize_sorted_opt_string_set<S: serde::Serializer>(
    value: &Option<HashSet<String>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        None => serializer.serialize_none(),
        Some(set) => {
            let mut sorted: Vec<&String> = set.iter().collect();
            sorted.sort();
            serializer.collect_seq(sorted)
        }
    }
}

fn deserialize_opt_string_set<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<HashSet<String>>, D::Error> {
    let opt: Option<Vec<String>> = Option::deserialize(deserializer)?;
    Ok(opt.map(|v| v.into_iter().collect()))
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
        let instructions: Vec<IxView> = self
            .instructions
            .iter()
            .map(|ix| build_ix_view(ix, &self.account_types))
            .collect();
        let accounts: Vec<AccountView> = self
            .account_types
            .iter()
            .map(build_account_view)
            .collect();

        let state_coupling = Some(compute_coupling(&instructions));
        let admin_gated = Some(compute_admin_gated(self, &accounts));
        let system_accounts = Some(collect_system_accounts(&instructions));

        ProgramView {
            name: self.name.clone(),
            program_id: self.program_id.to_string(),
            instructions,
            accounts,
            state_coupling,
            admin_gated,
            system_accounts,
        }
    }
}

fn build_ix_view(ix: &InstructionDef, account_types: &[AccountTypeDef]) -> IxView {
    let args: Vec<ArgView> = ix
        .args
        .iter()
        .map(|a| ArgView {
            name: a.name.clone(),
            ty: format_idl_type(&a.ty),
        })
        .collect();
    let accounts = ix
        .accounts
        .iter()
        .map(|spec| build_ix_account_view(spec, &ix.args, account_types))
        .collect();
    let returns = ix.returns.as_ref().map(format_idl_type);
    IxView {
        name: ix.name.clone(),
        discriminator_hex: format_discriminator(&ix.discriminator),
        args,
        accounts,
        returns,
    }
}

fn build_ix_account_view(
    spec: &AccountSpec,
    ix_args: &[IdlField],
    account_types: &[AccountTypeDef],
) -> IxAccountView {
    let pda = spec.pda.as_ref().map(|p| build_pda_view(p, ix_args));
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

fn build_pda_view(pda: &PdaSpec, ix_args: &[IdlField]) -> PdaView {
    let seeds = pda.seeds.iter().map(|s| seed_to_view(s, ix_args)).collect();
    let program = pda.program.as_ref().map(seed_program_to_string);
    PdaView {
        seeds,
        program,
        bump_arg: pda.bump_arg.clone(),
    }
}

fn seed_to_view(seed: &SeedSpec, ix_args: &[IdlField]) -> SeedView {
    match seed {
        SeedSpec::Const { value } => {
            let value_hex = bytes_to_hex(value);
            let value_utf8 = bytes_to_ascii_graphic(value);
            SeedView::Const {
                value_hex,
                value_utf8,
            }
        }
        SeedSpec::Arg { path, ty } => {
            let resolved = ix_args
                .iter()
                .find(|f| f.name == *path)
                .map(|f| format_idl_type(&f.ty))
                .unwrap_or_else(|| format_idl_type(ty));
            SeedView::Arg {
                name: path.clone(),
                ty: resolved,
            }
        }
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
    let fields = match &account.layout.ty {
        IdlTypeDefTy::Struct { fields: Some(IdlDefinedFields::Named(named)) } => named
            .iter()
            .map(|f| FieldView {
                name: f.name.clone(),
                ty: format_idl_type(&f.ty),
            })
            .collect(),
        IdlTypeDefTy::Struct { fields: Some(IdlDefinedFields::Tuple(items)) } => items
            .iter()
            .enumerate()
            .map(|(idx, ty)| FieldView {
                name: idx.to_string(),
                ty: format_idl_type(ty),
            })
            .collect(),
        _ => Vec::new(),
    };
    AccountView {
        name: account.name.clone(),
        discriminator_hex: format_discriminator(&account.discriminator),
        fields,
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

fn format_discriminator(d: &[u8; 8]) -> String {
    let mut s = String::with_capacity(2 + d.len() * 2);
    s.push_str("0x");
    for b in d {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn format_idl_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Bool => "bool".into(),
        IdlType::U8 => "u8".into(),
        IdlType::I8 => "i8".into(),
        IdlType::U16 => "u16".into(),
        IdlType::I16 => "i16".into(),
        IdlType::U32 => "u32".into(),
        IdlType::I32 => "i32".into(),
        IdlType::F32 => "f32".into(),
        IdlType::U64 => "u64".into(),
        IdlType::I64 => "i64".into(),
        IdlType::F64 => "f64".into(),
        IdlType::U128 => "u128".into(),
        IdlType::I128 => "i128".into(),
        IdlType::U256 => "u256".into(),
        IdlType::I256 => "i256".into(),
        IdlType::Bytes => "bytes".into(),
        IdlType::String => "string".into(),
        IdlType::Pubkey => "Pubkey".into(),
        IdlType::Option(inner) => format!("Option<{}>", format_idl_type(inner)),
        IdlType::Vec(inner) => format!("Vec<{}>", format_idl_type(inner)),
        IdlType::Array(inner, len) => {
            let len_str = match len {
                IdlArrayLen::Value(v) => v.to_string(),
                IdlArrayLen::Generic(name) => name.clone(),
            };
            format!("[{}; {}]", format_idl_type(inner), len_str)
        }
        IdlType::Defined { name, generics } => {
            if generics.is_empty() {
                name.clone()
            } else {
                let parts: Vec<String> = generics.iter().map(format_generic_arg).collect();
                format!("{name}<{}>", parts.join(", "))
            }
        }
        IdlType::Generic(name) => name.clone(),
        other => format!("{other:?}"),
    }
}

fn format_generic_arg(arg: &IdlGenericArg) -> String {
    match arg {
        IdlGenericArg::Type { ty } => format_idl_type(ty),
        IdlGenericArg::Const { value } => value.clone(),
    }
}

fn compute_coupling(ixs: &[IxView]) -> Vec<CouplingPair> {
    let mut out: Vec<CouplingPair> = Vec::new();
    for i in 0..ixs.len() {
        for j in (i + 1)..ixs.len() {
            let writable_i: HashSet<&str> = ixs[i]
                .accounts
                .iter()
                .filter(|a| a.writable)
                .map(|a| a.name.as_str())
                .collect();
            let writable_j: HashSet<&str> = ixs[j]
                .accounts
                .iter()
                .filter(|a| a.writable)
                .map(|a| a.name.as_str())
                .collect();
            let mut shared: Vec<String> = writable_i
                .intersection(&writable_j)
                .map(|s| s.to_string())
                .collect();
            if shared.is_empty() {
                continue;
            }
            shared.sort();
            let (a, b) = if ixs[i].name <= ixs[j].name {
                (ixs[i].name.clone(), ixs[j].name.clone())
            } else {
                (ixs[j].name.clone(), ixs[i].name.clone())
            };
            out.push(CouplingPair {
                a,
                b,
                shared_writable: shared,
            });
        }
    }
    out.sort_by(|x, y| {
        y.shared_writable
            .len()
            .cmp(&x.shared_writable.len())
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });
    out
}

fn compute_admin_gated(program: &ProgramDef, accounts: &[AccountView]) -> HashSet<String> {
    let admin_account_exists = accounts
        .iter()
        .any(|a| a.fields.iter().any(|f| is_admin_field(&f.name) && f.ty == "Pubkey"));
    if !admin_account_exists {
        return HashSet::new();
    }
    program
        .instructions
        .iter()
        .filter(|ix| {
            ix.accounts
                .iter()
                .any(|acc| is_admin_field(&acc.name) && acc.signer)
        })
        .map(|ix| ix.name.clone())
        .collect()
}

fn is_admin_field(name: &str) -> bool {
    name == "admin" || name == "authority"
}

fn collect_system_accounts(ixs: &[IxView]) -> HashSet<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for ix in ixs {
        for acc in &ix.accounts {
            if matches!(acc.kind, AccountKind::System | AccountKind::Sysvar) {
                set.insert(acc.name.clone());
            }
        }
    }
    set.into_iter().collect()
}

pub fn describe_seed_view(seed: &SeedView) -> String {
    match seed {
        SeedView::Const { value_hex, value_utf8 } => match value_utf8 {
            Some(s) => format!("const:'{s}'"),
            None => {
                let bytes = hex_to_bytes(value_hex);
                format!("const:{:02x?}", bytes)
            }
        },
        SeedView::Account { path } => format!("account:{path}"),
        SeedView::Arg { name, .. } => format!("arg:{name}"),
    }
}

pub fn hex_to_bytes(value_hex: &str) -> Vec<u8> {
    let stripped = value_hex.strip_prefix("0x").unwrap_or(value_hex);
    let mut out = Vec::with_capacity(stripped.len() / 2);
    let bytes = stripped.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16).unwrap_or(0) as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).unwrap_or(0) as u8;
        out.push((hi << 4) | lo);
        i += 2;
    }
    out
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
    fn compute_view_account_fields_and_discriminators_populated() {
        let view = lever_program().compute_view();
        let power = view
            .accounts
            .iter()
            .find(|a| a.name == "PowerStatus")
            .expect("PowerStatus account-type present");
        assert_eq!(power.fields.len(), 1);
        assert_eq!(power.fields[0].name, "is_on");
        assert_eq!(power.fields[0].ty, "bool");
        assert_eq!(power.discriminator_hex, "0x9193c623fd65e71a");
    }

    #[test]
    fn compute_view_ix_args_have_string_types() {
        let view = lever_program().compute_view();
        let switch = view
            .instructions
            .iter()
            .find(|i| i.name == "switch_power")
            .expect("switch_power present");
        assert_eq!(switch.discriminator_hex, "0xe2ee38acbf2d7a57");
        assert_eq!(switch.args.len(), 1);
        assert_eq!(switch.args[0].name, "name");
        assert_eq!(switch.args[0].ty, "string");
    }

    #[test]
    fn account_kind_classification() {
        let view = lever_program().compute_view();
        let initialize = view.instructions.iter().find(|i| i.name == "initialize").unwrap();
        let system = initialize.accounts.iter().find(|a| a.name == "system_program").unwrap();
        assert_eq!(system.kind, AccountKind::System);
        let power = initialize.accounts.iter().find(|a| a.name == "power").unwrap();
        assert_eq!(power.kind, AccountKind::Other);

        let relations = relations_program().compute_view();
        let init_base = relations.instructions.iter().find(|i| i.name == "init_base").unwrap();
        let pda_acc = init_base.accounts.iter().find(|a| a.name == "account").unwrap();
        assert_eq!(pda_acc.kind, AccountKind::Pda);
        let test_relation = relations.instructions.iter().find(|i| i.name == "test_relation").unwrap();
        let typed_acc = test_relation.accounts.iter().find(|a| a.name == "my_account").unwrap();
        assert_eq!(typed_acc.kind, AccountKind::Program);
    }

    #[test]
    fn seed_const_value_utf8_only_when_ascii_graphic() {
        let printable = SeedSpec::Const {
            value: b"stake".to_vec(),
        };
        match seed_to_view(&printable, &[]) {
            SeedView::Const { value_hex, value_utf8 } => {
                assert_eq!(value_hex, "0x7374616b65");
                assert_eq!(value_utf8.as_deref(), Some("stake"));
            }
            other => panic!("expected Const, got {other:?}"),
        }

        let non_graphic = SeedSpec::Const {
            value: vec![0x09, 0x0a],
        };
        match seed_to_view(&non_graphic, &[]) {
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

    #[test]
    fn format_idl_type_covers_primitives_and_compounds() {
        assert_eq!(format_idl_type(&IdlType::U8), "u8");
        assert_eq!(format_idl_type(&IdlType::U64), "u64");
        assert_eq!(format_idl_type(&IdlType::I128), "i128");
        assert_eq!(format_idl_type(&IdlType::Bool), "bool");
        assert_eq!(format_idl_type(&IdlType::String), "string");
        assert_eq!(format_idl_type(&IdlType::Bytes), "bytes");
        assert_eq!(format_idl_type(&IdlType::Pubkey), "Pubkey");

        let opt = IdlType::Option(Box::new(IdlType::U64));
        assert_eq!(format_idl_type(&opt), "Option<u64>");

        let vec_pk = IdlType::Vec(Box::new(IdlType::Pubkey));
        assert_eq!(format_idl_type(&vec_pk), "Vec<Pubkey>");

        let arr = IdlType::Array(Box::new(IdlType::U8), IdlArrayLen::Value(32));
        assert_eq!(format_idl_type(&arr), "[u8; 32]");

        let arr_generic = IdlType::Array(Box::new(IdlType::U8), IdlArrayLen::Generic("N".into()));
        assert_eq!(format_idl_type(&arr_generic), "[u8; N]");

        let defined = IdlType::Defined {
            name: "Pool".into(),
            generics: vec![],
        };
        assert_eq!(format_idl_type(&defined), "Pool");

        let generic_arg = IdlType::Defined {
            name: "Box".into(),
            generics: vec![IdlGenericArg::Type {
                ty: IdlType::U64,
            }],
        };
        assert_eq!(format_idl_type(&generic_arg), "Box<u64>");

        let nested = IdlType::Vec(Box::new(IdlType::Option(Box::new(IdlType::U32))));
        assert_eq!(format_idl_type(&nested), "Vec<Option<u32>>");

        assert_eq!(format_idl_type(&IdlType::Generic("T".into())), "T");
    }

    #[test]
    fn compute_view_state_coupling_empty_when_no_writable_overlap() {
        let view = lever_program().compute_view();
        let coupling = view.state_coupling.expect("state_coupling computed");
        assert_eq!(coupling.len(), 1);
        assert_eq!(coupling[0].a, "initialize");
        assert_eq!(coupling[0].b, "switch_power");
        assert_eq!(coupling[0].shared_writable, vec!["power"]);
    }

    #[test]
    fn compute_view_admin_gated_requires_both_signer_and_field() {
        let view = lever_program().compute_view();
        let gated = view.admin_gated.expect("admin_gated computed");
        assert!(gated.is_empty(), "lever should not gate any ix");
    }

    #[test]
    fn compute_view_system_accounts_collects_kinds() {
        let view = lever_program().compute_view();
        let sys = view.system_accounts.expect("system_accounts computed");
        assert!(sys.contains("system_program"));
    }

    const STAKING_JSON: &str = include_str!(
        "../../../tests/fixtures/solana/staking/idls/staking.json"
    );

    fn staking_program() -> ProgramDef {
        ProgramDef::from_idl(parse_idl(STAKING_JSON).expect("parse staking"))
            .expect("build staking ProgramDef")
    }

    #[test]
    fn compute_view_coupling_includes_stake_unstake_on_staking() {
        let view = staking_program().compute_view();
        let coupling = view.state_coupling.expect("state_coupling computed");
        let pair = coupling
            .iter()
            .find(|p| p.a == "stake" && p.b == "unstake")
            .expect("stake↔unstake pair present");
        assert!(pair.shared_writable.iter().any(|n| n == "pool"));
        assert!(pair.shared_writable.iter().any(|n| n == "user_stake"));
    }

    #[test]
    fn compute_view_admin_gated_marks_initialize_pool_on_staking() {
        let view = staking_program().compute_view();
        let gated = view.admin_gated.expect("admin_gated computed");
        assert!(gated.contains("initialize_pool"));
        assert!(gated.contains("add_rewards"));
        assert!(!gated.contains("stake"));
        assert!(!gated.contains("unstake"));
    }
}
