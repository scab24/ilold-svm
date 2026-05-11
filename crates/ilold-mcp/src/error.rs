use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpClientError {
    #[error("cannot reach Ilold server at {url}: {reason}")]
    Unreachable { url: String, reason: String },
    #[error("server at {url} is not Solana (kind={kind})")]
    NotSolana { url: String, kind: String },
    #[error("HTTP {status}: {body}")]
    HttpError { status: u16, body: String },
    #[error("invalid response from server: {0}")]
    InvalidResponse(String),
}
