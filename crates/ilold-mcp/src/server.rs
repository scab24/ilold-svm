use std::sync::Arc;

use ilold_render::fmt::strip_ansi;
use ilold_render::render_solana_result;
use ilold_solana_core::exploration::SolanaCommandResult;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::Value;

use crate::client::IloldClient;
use crate::tools;

pub struct IloldMcpServer {
    client: Arc<IloldClient>,
}

impl IloldMcpServer {
    pub fn new(client: Arc<IloldClient>) -> Self {
        Self { client }
    }
}

impl ServerHandler for IloldMcpServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("ilold-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(format!(
            "Ilold MCP server. Exposes Solana REPL commands as MCP tools \
             backed by the Ilold backend at {}. Target program: {}.",
            self.client.base_url(),
            self.client.contract(),
        ));
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let tools = tools::build_tool_registry();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tool_name = request.name.to_string();
        let args_value = request.arguments.map(Value::Object);
        let res = dispatch(&self.client, &tool_name, args_value.as_ref()).await;
        Ok(res)
    }
}

async fn dispatch(
    client: &IloldClient,
    tool_name: &str,
    arguments: Option<&Value>,
) -> CallToolResult {
    if tool_name == "ilold_programs" {
        return handle_programs(client).await;
    }
    let command = match tools::build_command(tool_name, arguments) {
        Ok(cmd) => cmd,
        Err(message) => return error_result(format!("Invalid arguments: {message}")),
    };
    match client.send_command(command).await {
        Ok(result) => build_tool_response(&result),
        Err(err) => error_result(err.to_string()),
    }
}

async fn handle_programs(client: &IloldClient) -> CallToolResult {
    let url = format!("{}/api/project/map", client.base_url());
    let resp = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => return error_result(format!("cannot reach {url}: {e}")),
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return error_result(format!("HTTP {status}: {body}"));
    }
    let value: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return error_result(format!("invalid response: {e}")),
    };
    let text = render_programs(&value, client.contract());
    let structured = value.clone();
    let mut out = CallToolResult::structured(serde_json::json!({ "project_map": structured }));
    out.content = vec![Content::text(text)];
    out
}

fn render_programs(map: &Value, active_contract: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  workspace programs (active: {active_contract})\n"
    ));
    if let Some(programs) = map.get("programs").and_then(|p| p.as_array()) {
        for p in programs {
            let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let pid = p.get("program_id").and_then(|n| n.as_str()).unwrap_or("?");
            let ix_count = p
                .get("instructions")
                .and_then(|i| i.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let marker = if name == active_contract { " ← active" } else { "" };
            out.push_str(&format!(
                "  · {name} (program_id={pid}, instructions={ix_count}){marker}\n"
            ));
        }
    }
    out
}

fn build_tool_response(result: &SolanaCommandResult) -> CallToolResult {
    let structured = serde_json::to_value(result).unwrap_or(Value::Null);
    let pretty = strip_ansi(&render_solana_result(result));
    let is_error = matches!(result, SolanaCommandResult::Error { .. });
    let mut out = CallToolResult::structured(structured);
    out.content = vec![Content::text(pretty)];
    out.is_error = Some(is_error);
    out
}

fn error_result(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}
