use crate::db::Database;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ErrorData as McpError, *},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Parameter structs

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextSetParams {
    /// The key to set
    pub key: String,
    /// The value to set
    pub value: String,
    /// Optional project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextGetParams {
    /// The key to get
    pub key: String,
    /// Optional project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextDeleteParams {
    /// The key to delete
    pub key: String,
    /// Optional project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextListParams {
    /// Optional project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendMessageParams {
    /// Project ID (e.g., "owner/repo")
    pub project_id: String,
    /// Target agent ID
    pub to_agent: String,
    /// Message content
    pub content: String,
    /// Sender agent ID (optional)
    #[serde(default)]
    pub from_agent: Option<String>,
    /// Reference to a previous message ID (for request/response linking)
    #[serde(default)]
    pub reference_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReceiveMessagesParams {
    /// Project ID (e.g., "owner/repo")
    pub project_id: String,
    /// Agent ID to receive messages for
    pub agent_id: String,
    /// Maximum number of messages to receive (default: 100)
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PeekMessagesParams {
    /// Project ID (e.g., "owner/repo")
    pub project_id: String,
    /// Agent ID to peek messages for
    pub agent_id: String,
    /// Maximum number of messages to peek (default: 100)
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteMessageParams {
    /// Message ID to delete
    pub message_id: String,
}

// Response types

#[derive(Debug, Serialize)]
struct SendMessageResponse {
    message_id: String,
}

#[derive(Debug, Serialize)]
struct ContextListResponse {
    keys: Vec<String>,
}

// Server implementation

#[derive(Clone)]
pub struct MailboxServer {
    db: Arc<Database>,
    tool_router: ToolRouter<Self>,
}

impl MailboxServer {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(db),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl MailboxServer {
    #[tool(description = "Set a context value. Omit project_id for global context.")]
    async fn context_set(
        &self,
        Parameters(params): Parameters<ContextSetParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .context_set(params.project_id.as_deref(), &params.key, &params.value)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text("OK")]))
    }

    #[tool(description = "Get a context value. Omit project_id for global context.")]
    async fn context_get(
        &self,
        Parameters(params): Parameters<ContextGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = self
            .db
            .context_get(params.project_id.as_deref(), &params.key)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        match value {
            Some(v) => Ok(CallToolResult::success(vec![Content::text(v)])),
            None => Ok(CallToolResult::success(vec![Content::text("")])),
        }
    }

    #[tool(description = "Delete a context value. Omit project_id for global context.")]
    async fn context_delete(
        &self,
        Parameters(params): Parameters<ContextDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let deleted = self
            .db
            .context_delete(params.project_id.as_deref(), &params.key)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let msg = if deleted { "deleted" } else { "not_found" };
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "List all context keys. Omit project_id for global context.")]
    async fn context_list(
        &self,
        Parameters(params): Parameters<ContextListParams>,
    ) -> Result<CallToolResult, McpError> {
        let keys = self
            .db
            .context_list(params.project_id.as_deref())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let response = ContextListResponse { keys };
        let json = serde_json::to_string(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Send a message to an agent's queue. Returns the message ID.")]
    async fn send_message(
        &self,
        Parameters(params): Parameters<SendMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        let from_agent = params.from_agent.as_deref().unwrap_or("anonymous");
        let message_id = self
            .db
            .send_message(
                &params.project_id,
                &params.to_agent,
                from_agent,
                &params.content,
                params.reference_id.as_deref(),
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let response = SendMessageResponse { message_id };
        let json = serde_json::to_string(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Receive and consume messages from an agent's queue.")]
    async fn receive_messages(
        &self,
        Parameters(params): Parameters<ReceiveMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let messages = self
            .db
            .receive_messages(&params.project_id, &params.agent_id, params.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let json = serde_json::to_string(&messages)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Peek at messages in an agent's queue without consuming them.")]
    async fn peek_messages(
        &self,
        Parameters(params): Parameters<PeekMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let messages = self
            .db
            .peek_messages(&params.project_id, &params.agent_id, params.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let json = serde_json::to_string(&messages)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Delete a specific message by ID.")]
    async fn delete_message(
        &self,
        Parameters(params): Parameters<DeleteMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        let deleted = self
            .db
            .delete_message(&params.message_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let msg = if deleted { "deleted" } else { "not_found" };
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }
}

#[tool_handler]
impl ServerHandler for MailboxServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Mailbox MCP server for agent-to-agent communication".to_string()),
        }
    }
}
