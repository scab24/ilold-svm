use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};

use crate::tools;

pub struct IloldMcpServer {
    server_url: String,
}

impl IloldMcpServer {
    pub fn new(server_url: String) -> Self {
        Self { server_url }
    }

    pub fn server_url(&self) -> &str {
        &self.server_url
    }
}

impl ServerHandler for IloldMcpServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("ilold-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Ilold MCP server. Exposes Solana REPL commands as MCP tools. \
             Tool handlers are stubs in T-R55a; functional dispatch lands in T-R55b."
                .to_string(),
        );
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
        let msg = format!(
            "tool `{}` not yet implemented (T-R55b). server_url={}",
            request.name, self.server_url
        );
        Ok(CallToolResult::error(vec![Content::text(msg)]))
    }
}
