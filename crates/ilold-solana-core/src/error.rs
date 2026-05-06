use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SolanaError {
    #[error("failed to read IDL file '{path}': {source}")]
    IdlReadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse or convert IDL JSON: {message}")]
    IdlParseFailed { message: String },

    #[error("IDL spec '{0}' is not a recognized version")]
    UnsupportedIdlSpec(String),

    #[error("IDL type '{0}' uses generics, which are not supported")]
    UnsupportedGeneric(String),

    #[error("IDL address '{0}' is not a valid base58 pubkey")]
    InvalidProgramId(String),

    #[error("expected 8-byte discriminator for '{name}', got {len}")]
    InvalidDiscriminatorLength { name: String, len: usize },

    #[error("PDA seed references arg '{path}' which is not declared on the instruction")]
    SeedArgUnresolved { path: String },

    #[error("project at '{path}' contains both Anchor.toml and Solidity sources")]
    MixedProject { path: PathBuf },

    #[error("project at '{path}' is neither Anchor nor Solidity (no Anchor.toml, foundry.toml or *.sol found)")]
    UnknownProjectType { path: PathBuf },

    #[error("Borsh decode failed: {0}")]
    DecodeFailed(String),

    #[error("unknown account discriminator: {hex}")]
    UnknownDiscriminator { hex: String },

    #[error("IDL references unknown type '{0}'")]
    UnknownType(String),

    #[error("VM boot failed: {0}")]
    VmBootFailed(String),

    #[error("VM operation failed: {0}")]
    VmOperationFailed(String),

    #[error("PDA seed type mismatch at '{path}': expected {expected}, got {got}")]
    SeedTypeMismatch {
        path: String,
        expected: String,
        got: String,
    },

    #[error("PDA program override is bound to an arg seed '{path}', which is not supported")]
    PdaProgramArgUnsupported { path: String },
}
