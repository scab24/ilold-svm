use crate::model::common::SourceSpan;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Failed to parse {path}: {message}")]
    SyntaxError {
        path: String,
        message: String,
        span: Option<SourceSpan>,
    },

    #[error("Unsupported Solidity version in {path}: requires 0.8.x or higher")]
    UnsupportedVersion { path: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Import resolution failed: {import_path} (from {source_path})")]
    ImportResolution {
        import_path: String,
        source_path: String,
    },

    #[error("Internal parser error: {message}")]
    Internal { message: String },
}
