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

    #[error("IDL type '{0}' uses generics, which are not supported in MVP")]
    UnsupportedGeneric(String),

    #[error("IDL address '{0}' is not a valid base58 pubkey")]
    InvalidProgramId(String),
}
