use std::path::{Path, PathBuf};

use anchor_lang_idl::convert::convert_idl;
use anchor_lang_idl::types::Idl;

use crate::error::SolanaError;

pub fn parse_idl(json: &str) -> Result<Idl, SolanaError> {
    convert_idl(json.as_bytes()).map_err(|e| SolanaError::IdlParseFailed {
        message: e.to_string(),
    })
}

pub fn parse_idl_dir(dir: &Path) -> Result<Vec<(PathBuf, Idl)>, SolanaError> {
    let mut idls = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| SolanaError::IdlReadFailed {
        path: dir.to_path_buf(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| SolanaError::IdlReadFailed {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let json = std::fs::read_to_string(&path).map_err(|e| SolanaError::IdlReadFailed {
                path: path.clone(),
                source: e,
            })?;
            let idl = parse_idl(&json)?;
            idls.push((path, idl));
        }
    }
    idls.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(idls)
}
