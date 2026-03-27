use crate::model::common::SourceSpan;

#[derive(Debug, thiserror::Error)]
pub enum CfgError {
    #[error("Modifier '{name}' not found in contract '{contract}'")]
    ModifierNotFound { name: String, contract: String },

    #[error("Modifier '{name}' has no _ placeholder")]
    ModifierMissingPlaceholder { name: String },

    #[error("Unsupported statement at {span:?}: {kind}")]
    UnsupportedStatement {
        kind: String,
        span: Option<SourceSpan>,
    },
}
