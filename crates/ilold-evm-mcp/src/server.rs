use std::sync::Arc;

use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::Value;

use crate::client::IloldClient;
use crate::tools;

pub struct EvmMcpServer {
    client: Arc<IloldClient>,
}

impl EvmMcpServer {
    pub fn new(client: Arc<IloldClient>) -> Self {
        Self { client }
    }
}

impl ServerHandler for EvmMcpServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("ilold-evm-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(format!(
            "Ilold EVM analysis server backed by {}. Start with ilold_project_overview \
             and ilold_dependency_graph to orient, then drill into a contract.",
            self.client.base_url(),
        ));
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult::with_all_items(tools::build_tool_registry()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let name = request.name.to_string();
        let args = request.arguments.map(Value::Object);
        Ok(dispatch(&self.client, &name, args.as_ref()).await)
    }
}

pub async fn dispatch(
    client: &IloldClient,
    tool_name: &str,
    arguments: Option<&Value>,
) -> CallToolResult {
    let result = match tool_name {
        "ilold_project_overview" => client.get("/api/project").await,
        "ilold_project_map" => client.get("/api/project/map").await,
        "ilold_dependency_graph" => client.get("/api/project/depgraph").await,
        "ilold_contract_dependencies" => match arg_str(arguments, "contract") {
            Some(c) => client.get(&format!("/api/contract/{c}/depgraph")).await,
            None => return error_result("missing required field: contract".to_string()),
        },
        other => return error_result(format!("unknown tool: {other}")),
    };
    match result {
        Ok(value) => ok_result(value),
        Err(err) => error_result(err.to_string()),
    }
}

fn arg_str(arguments: Option<&Value>, key: &str) -> Option<String> {
    arguments
        .and_then(|v| v.as_object())
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn ok_result(value: Value) -> CallToolResult {
    let text = serde_json::to_string_pretty(&value).unwrap_or_default();
    let mut out = CallToolResult::structured(value);
    out.content = vec![Content::text(text)];
    out
}

fn error_result(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}
