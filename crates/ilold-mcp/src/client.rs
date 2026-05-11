use ilold_solana_core::exploration::SolanaCommandResult;
use serde_json::Value;

use crate::error::McpClientError;

pub struct IloldClient {
    base_url: String,
    contract: String,
    http: reqwest::Client,
}

impl IloldClient {
    pub fn new(base_url: String, contract: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            contract,
            http: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn contract(&self) -> &str {
        &self.contract
    }

    pub async fn health_check(&self) -> Result<(), McpClientError> {
        let url = format!("{}/api/project/map", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| McpClientError::Unreachable {
                url: url.clone(),
                reason: e.to_string(),
            })?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(McpClientError::HttpError { status, body });
        }
        let v: Value = resp
            .json()
            .await
            .map_err(|e| McpClientError::InvalidResponse(e.to_string()))?;
        let kind = v
            .get("kind")
            .and_then(|x| x.as_str())
            .unwrap_or("(missing)");
        if kind != "solana" {
            return Err(McpClientError::NotSolana {
                url: self.base_url.clone(),
                kind: kind.to_string(),
            });
        }
        Ok(())
    }

    pub async fn send_command(
        &self,
        command: Value,
    ) -> Result<SolanaCommandResult, McpClientError> {
        let url = format!("{}/api/cmd", self.base_url);
        let body = serde_json::json!({
            "contract": self.contract,
            "command": command,
        });
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| McpClientError::Unreachable {
                url: url.clone(),
                reason: e.to_string(),
            })?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(McpClientError::HttpError { status, body });
        }
        resp.json::<SolanaCommandResult>()
            .await
            .map_err(|e| McpClientError::InvalidResponse(e.to_string()))
    }
}
