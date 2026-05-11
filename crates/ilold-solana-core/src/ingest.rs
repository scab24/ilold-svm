use std::path::{Path, PathBuf};

use crate::error::SolanaError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Solana,
    Solidity,
}

#[derive(Debug, Clone)]
pub struct DetectedProject {
    pub kind: ProjectKind,
    pub root: PathBuf,
    pub idl_paths: Vec<PathBuf>,
    pub so_paths: Vec<PathBuf>,
}

pub fn detect(path: &Path) -> Result<DetectedProject, SolanaError> {
    let anchor_root = find_anchor_root(path);
    let solidity_marker = has_solidity_marker(path);

    match (anchor_root, solidity_marker) {
        (Some(_), true) => Err(SolanaError::MixedProject {
            path: path.to_path_buf(),
        }),
        (Some(root), false) => Ok(DetectedProject {
            kind: ProjectKind::Solana,
            idl_paths: find_idls(&root),
            so_paths: find_so(&root),
            root,
        }),
        (None, true) => Ok(DetectedProject {
            kind: ProjectKind::Solidity,
            root: path.to_path_buf(),
            idl_paths: vec![],
            so_paths: vec![],
        }),
        (None, false) => Err(SolanaError::UnknownProjectType {
            path: path.to_path_buf(),
        }),
    }
}

fn find_anchor_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent()?.to_path_buf()
    } else {
        path.to_path_buf()
    };
    loop {
        if current.join("Anchor.toml").is_file() {
            return Some(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

fn has_solidity_marker(path: &Path) -> bool {
    let dir = if path.is_file() {
        match path.parent() {
            Some(p) => p,
            None => return false,
        }
    } else {
        path
    };
    if dir.join("foundry.toml").is_file() || dir.join("hardhat.config.ts").is_file()
        || dir.join("hardhat.config.js").is_file()
    {
        return true;
    }
    has_sol_anywhere(dir, 6)
}

fn has_sol_anywhere(dir: &Path, depth_remaining: usize) -> bool {
    if depth_remaining == 0 {
        return false;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|e| e == "sol") {
            return true;
        }
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || matches!(name, "node_modules" | "target" | "out" | "cache" | "lib") {
                continue;
            }
            if has_sol_anywhere(&p, depth_remaining - 1) {
                return true;
            }
        }
    }
    false
}

pub fn find_idls(anchor_root: &Path) -> Vec<PathBuf> {
    let preferred = anchor_root.join("target").join("idl");
    let fallback = anchor_root.join("idls");
    let mut found = collect_jsons(&preferred);
    if found.is_empty() {
        found = collect_jsons(&fallback);
    }
    found.sort();
    found
}

pub fn find_so(anchor_root: &Path) -> Vec<PathBuf> {
    let preferred = anchor_root.join("target").join("deploy");
    let fallback = anchor_root.join("bin");
    let mut found = collect_so(&preferred);
    if found.is_empty() {
        found = collect_so(&fallback);
    }
    found.sort();
    found
}

fn collect_so(dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "so"))
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn collect_jsons(dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect(),
        Err(_) => Vec::new(),
    }
}
