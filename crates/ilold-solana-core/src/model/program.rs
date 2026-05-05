use anchor_lang_idl::types::{
    Idl, IdlField, IdlInstruction, IdlInstructionAccount, IdlInstructionAccountItem, IdlPda,
    IdlSeed, IdlType, IdlTypeDef,
};
use serde::{Deserialize, Serialize};
use solana_address::Address;

use crate::error::SolanaError;

use super::{
    AccountSpec, AccountTypeDef, InstructionDef, PdaSpec, SeedSpec,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramDef {
    pub name: String,
    pub program_id: Address,
    pub instructions: Vec<InstructionDef>,
    pub account_types: Vec<AccountTypeDef>,
    pub types: Vec<IdlTypeDef>,
}

impl ProgramDef {
    pub fn from_idl(idl: Idl) -> Result<Self, SolanaError> {
        let program_id = idl
            .address
            .parse::<Address>()
            .map_err(|_| SolanaError::InvalidProgramId(idl.address.clone()))?;

        let instructions = idl
            .instructions
            .iter()
            .map(build_instruction)
            .collect::<Result<Vec<_>, _>>()?;

        let account_types = idl
            .accounts
            .iter()
            .map(|a| build_account_type(a, &idl.types))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            name: idl.metadata.name.clone(),
            program_id,
            instructions,
            account_types,
            types: idl.types,
        })
    }
}

fn build_instruction(ix: &IdlInstruction) -> Result<InstructionDef, SolanaError> {
    let discriminator: [u8; 8] = ix.discriminator.as_slice().try_into().map_err(|_| {
        SolanaError::InvalidDiscriminatorLength {
            name: ix.name.clone(),
            len: ix.discriminator.len(),
        }
    })?;

    let mut accounts = Vec::new();
    for item in &ix.accounts {
        flatten_account_item(item, "", &ix.args, &mut accounts)?;
    }

    let bump_arg = detect_bump_arg(&ix.args);
    if let Some(arg) = bump_arg.as_deref() {
        for spec in accounts.iter_mut() {
            if let Some(pda) = spec.pda.as_mut() {
                if pda.bump_arg.is_none() {
                    pda.bump_arg = Some(arg.to_string());
                }
            }
        }
    }

    Ok(InstructionDef {
        name: ix.name.clone(),
        discriminator,
        args: ix.args.clone(),
        accounts,
        returns: ix.returns.clone(),
    })
}

fn flatten_account_item(
    item: &IdlInstructionAccountItem,
    prefix: &str,
    ix_args: &[IdlField],
    out: &mut Vec<AccountSpec>,
) -> Result<(), SolanaError> {
    match item {
        IdlInstructionAccountItem::Single(single) => {
            out.push(build_account_spec(single, prefix, ix_args)?);
            Ok(())
        }
        IdlInstructionAccountItem::Composite(group) => {
            let new_prefix = join_path(prefix, &group.name);
            for sub in &group.accounts {
                flatten_account_item(sub, &new_prefix, ix_args, out)?;
            }
            Ok(())
        }
    }
}

fn build_account_spec(
    src: &IdlInstructionAccount,
    prefix: &str,
    ix_args: &[IdlField],
) -> Result<AccountSpec, SolanaError> {
    let address = match &src.address {
        Some(s) => Some(
            s.parse::<Address>()
                .map_err(|_| SolanaError::InvalidProgramId(s.clone()))?,
        ),
        None => None,
    };

    let pda = match &src.pda {
        Some(spec) => Some(map_pda(spec, ix_args)?),
        None => None,
    };

    Ok(AccountSpec {
        path: join_path(prefix, &src.name),
        name: src.name.clone(),
        writable: src.writable,
        signer: src.signer,
        optional: src.optional,
        address,
        pda,
        relations: src.relations.clone(),
    })
}

fn map_pda(spec: &IdlPda, ix_args: &[IdlField]) -> Result<PdaSpec, SolanaError> {
    let seeds = spec
        .seeds
        .iter()
        .map(|s| map_seed(s, ix_args))
        .collect::<Result<Vec<_>, _>>()?;
    let program = match &spec.program {
        Some(s) => Some(map_seed(s, ix_args)?),
        None => None,
    };
    Ok(PdaSpec {
        seeds,
        program,
        bump_arg: None,
    })
}

fn map_seed(seed: &IdlSeed, ix_args: &[IdlField]) -> Result<SeedSpec, SolanaError> {
    Ok(match seed {
        IdlSeed::Const(c) => SeedSpec::Const { value: c.value.clone() },
        IdlSeed::Arg(a) => {
            let ty = ix_args
                .iter()
                .find(|f| f.name == a.path)
                .map(|f| f.ty.clone())
                .ok_or_else(|| SolanaError::SeedArgUnresolved {
                    path: a.path.clone(),
                })?;
            SeedSpec::Arg {
                path: a.path.clone(),
                ty,
            }
        }
        IdlSeed::Account(a) => SeedSpec::Account { path: a.path.clone() },
    })
}

fn detect_bump_arg(args: &[IdlField]) -> Option<String> {
    args.iter()
        .find(|f| (f.name == "bump" || f.name.ends_with("_bump")) && matches!(f.ty, IdlType::U8))
        .map(|f| f.name.clone())
}

fn build_account_type(
    src: &anchor_lang_idl::types::IdlAccount,
    types: &[IdlTypeDef],
) -> Result<AccountTypeDef, SolanaError> {
    let discriminator: [u8; 8] = src.discriminator.as_slice().try_into().map_err(|_| {
        SolanaError::InvalidDiscriminatorLength {
            name: src.name.clone(),
            len: src.discriminator.len(),
        }
    })?;
    let layout = types
        .iter()
        .find(|t| t.name == src.name)
        .cloned()
        .unwrap_or_else(|| placeholder_typedef(&src.name));
    Ok(AccountTypeDef {
        name: src.name.clone(),
        discriminator,
        layout,
    })
}

fn placeholder_typedef(name: &str) -> IdlTypeDef {
    use anchor_lang_idl::types::{IdlSerialization, IdlTypeDefTy};
    IdlTypeDef {
        name: name.to_string(),
        docs: vec![],
        serialization: IdlSerialization::Borsh,
        repr: None,
        generics: vec![],
        ty: IdlTypeDefTy::Struct { fields: None },
    }
}

fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}
