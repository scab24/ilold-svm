use std::sync::Arc;

use ilold_render::fmt::strip_ansi;
use ilold_render::render_solana_result;
use ilold_solana_core::exploration::SolanaCommandResult;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ProgressNotificationParam, ProgressToken, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::{Peer, RequestContext, RoleServer};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::client::IloldClient;
use crate::narration::intent_for_tool;
use crate::tools;

pub struct IloldMcpServer {
    client: Arc<IloldClient>,
    current_contract: Arc<Mutex<Option<String>>>,
    narration: bool,
}

impl IloldMcpServer {
    pub fn new(
        client: Arc<IloldClient>,
        initial_contract: Option<String>,
        narration: bool,
    ) -> Self {
        Self {
            client,
            current_contract: Arc::new(Mutex::new(initial_contract)),
            narration,
        }
    }
}

impl ServerHandler for IloldMcpServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("ilold-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(format!(
            "Ilold MCP server. Exposes Solana REPL commands as MCP tools \
             backed by the Ilold backend at {}. Call ilold_programs to list \
             available programs and ilold_use <program> to fix the active one \
             before issuing other tool calls.",
            self.client.base_url(),
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tool_name = request.name.to_string();
        let args_value = request.arguments.map(Value::Object);
        if self.narration {
            let token = context.meta.get_progress_token();
            emit_intent_progress(&context.peer, token, &tool_name, args_value.as_ref()).await;
        }
        let res = dispatch(
            &self.client,
            &self.current_contract,
            &tool_name,
            args_value.as_ref(),
        )
        .await;
        Ok(res)
    }
}

async fn emit_intent_progress(
    peer: &Peer<RoleServer>,
    token: Option<ProgressToken>,
    tool_name: &str,
    arguments: Option<&Value>,
) {
    let Some(progress_token) = token else { return };
    let message = intent_for_tool(tool_name, arguments);
    let params = ProgressNotificationParam {
        progress_token,
        progress: 0.0,
        total: Some(1.0),
        message: Some(message),
    };
    if let Err(err) = peer.notify_progress(params).await {
        tracing::debug!(?err, "notify_progress failed");
    }
}

pub async fn dispatch(
    client: &IloldClient,
    current_contract: &Arc<Mutex<Option<String>>>,
    tool_name: &str,
    arguments: Option<&Value>,
) -> CallToolResult {
    if tool_name == "ilold_use" {
        return handle_use(client, current_contract, arguments).await;
    }
    if tool_name == "ilold_programs" {
        let active = current_contract.lock().await.clone();
        return handle_programs(client, active.as_deref()).await;
    }
    let active = match current_contract.lock().await.clone() {
        Some(c) => c,
        None => {
            return error_result(
                "No active contract. Call ilold_use <program> first or list available programs with ilold_programs."
                    .to_string(),
            );
        }
    };
    let command = match tools::build_command(tool_name, arguments) {
        Ok(cmd) => cmd,
        Err(message) => return error_result(format!("Invalid arguments: {message}")),
    };
    match client.send_command(&active, command).await {
        Ok(result) => build_tool_response(&result),
        Err(err) => error_result(err.to_string()),
    }
}

async fn handle_use(
    client: &IloldClient,
    current_contract: &Arc<Mutex<Option<String>>>,
    arguments: Option<&Value>,
) -> CallToolResult {
    let program = match arguments
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("program"))
        .and_then(|v| v.as_str())
    {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return error_result("missing or empty field: program".to_string()),
    };
    let map = match client.project_map().await {
        Ok(v) => v,
        Err(err) => return error_result(err.to_string()),
    };
    let kind = map.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if kind != "solana" {
        return error_result(format!(
            "backend reports kind={kind}; ilold_use only supports Solana workspaces"
        ));
    }
    let known: Vec<String> = map
        .get("programs")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !known.iter().any(|n| n == &program) {
        return error_result(format!(
            "unknown program `{program}`. Available: {}",
            if known.is_empty() {
                "(none)".to_string()
            } else {
                known.join(", ")
            }
        ));
    }
    *current_contract.lock().await = Some(program.clone());
    let text = format!("Active contract set to `{program}`.");
    let mut out = CallToolResult::structured(serde_json::json!({
        "active_contract": program,
    }));
    out.content = vec![Content::text(text)];
    out
}

async fn handle_programs(client: &IloldClient, active_contract: Option<&str>) -> CallToolResult {
    let value = match client.project_map().await {
        Ok(v) => v,
        Err(err) => return error_result(err.to_string()),
    };
    let text = render_programs(&value, active_contract);
    let structured = value.clone();
    let mut out = CallToolResult::structured(serde_json::json!({ "project_map": structured }));
    out.content = vec![Content::text(text)];
    out
}

fn render_programs(map: &Value, active_contract: Option<&str>) -> String {
    let mut out = String::new();
    let label = active_contract.unwrap_or("(none — call ilold_use to set one)");
    out.push_str(&format!("  workspace programs (active: {label})\n"));
    if let Some(programs) = map.get("programs").and_then(|p| p.as_array()) {
        for p in programs {
            let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let pid = p.get("program_id").and_then(|n| n.as_str()).unwrap_or("?");
            let ix_count = p
                .get("instructions")
                .and_then(|i| i.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let marker = if Some(name) == active_contract {
                " ← active"
            } else {
                ""
            };
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
