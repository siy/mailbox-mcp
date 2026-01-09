//! MCP tool handlers for mailbox-mcp.

use crate::db::{Database, Message};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, ErrorData as McpError, Implementation, ProtocolVersion,
        ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

// =============================================================================
// Parameter types
// =============================================================================

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextSetParams {
    /// The key to set (non-empty string).
    pub key: String,
    /// The value to store (max 65,536 bytes).
    pub value: String,
    /// Project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextGetParams {
    /// The key to retrieve.
    pub key: String,
    /// Project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextDeleteParams {
    /// The key to delete.
    pub key: String,
    /// Project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextListParams {
    /// Project ID (e.g., "owner/repo"). Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendMessageParams {
    /// Project ID (e.g., "owner/repo"). Required, cannot be empty.
    pub project_id: String,
    /// Target agent ID to receive the message. Required, cannot be empty.
    pub to_agent: String,
    /// Message content (max 1,048,576 bytes).
    pub content: String,
    /// Sender agent ID. Defaults to "anonymous" if not specified or empty.
    #[serde(default)]
    pub from_agent: Option<String>,
    /// Reference to a previous message ID (for request/response linking).
    #[serde(default)]
    pub reference_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReceiveMessagesParams {
    /// Project ID (e.g., "owner/repo").
    pub project_id: String,
    /// Agent ID to receive messages for.
    pub agent_id: String,
    /// Maximum messages to receive (default: 100, max: 500).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PeekMessagesParams {
    /// Project ID (e.g., "owner/repo").
    pub project_id: String,
    /// Agent ID to peek messages for.
    pub agent_id: String,
    /// Maximum messages to peek (default: 100, max: 500).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteMessageParams {
    /// Message ID to delete (numeric string).
    pub message_id: String,
}

// =============================================================================
// Server implementation
// =============================================================================

/// MCP server for agent-to-agent communication.
#[derive(Clone)]
pub struct MailboxServer {
    db: Arc<Database>,
    tool_router: ToolRouter<Self>,
}

impl MailboxServer {
    /// Creates a new server with the given database.
    #[must_use]
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(db),
            tool_router: Self::tool_router(),
        }
    }
}

fn json_response(value: &serde_json::Value) -> CallToolResult {
    CallToolResult::success(vec![Content::text(value.to_string())])
}

fn messages_response(messages: &[Message]) -> CallToolResult {
    json_response(&json!({ "messages": messages }))
}

#[tool_router]
impl MailboxServer {
    /// Set a context value.
    #[tool(
        description = "Set a context value. Omit project_id for global context. Returns {\"ok\": true}. Errors: EmptyField if key is empty, ContentTooLarge if value > 65536 bytes."
    )]
    async fn context_set(
        &self,
        Parameters(params): Parameters<ContextSetParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .context_set(params.project_id.as_deref(), &params.key, &params.value)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "ok": true })))
    }

    /// Get a context value.
    #[tool(
        description = "Get a context value. Omit project_id for global context. Returns {\"found\": true, \"value\": \"...\"} or {\"found\": false}."
    )]
    async fn context_get(
        &self,
        Parameters(params): Parameters<ContextGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = self
            .db
            .context_get(params.project_id.as_deref(), &params.key)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        #[allow(clippy::option_if_let_else)] // match is clearer here
        let response = match value {
            Some(v) => json!({ "found": true, "value": v }),
            None => json!({ "found": false }),
        };
        Ok(json_response(&response))
    }

    /// Delete a context value.
    #[tool(
        description = "Delete a context value. Omit project_id for global context. Returns {\"deleted\": true} or {\"deleted\": false}."
    )]
    async fn context_delete(
        &self,
        Parameters(params): Parameters<ContextDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let deleted = self
            .db
            .context_delete(params.project_id.as_deref(), &params.key)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "deleted": deleted })))
    }

    /// List all context keys.
    #[tool(
        description = "List all context keys. Omit project_id for global context. Returns {\"keys\": [\"key1\", \"key2\", ...]}."
    )]
    async fn context_list(
        &self,
        Parameters(params): Parameters<ContextListParams>,
    ) -> Result<CallToolResult, McpError> {
        let keys = self
            .db
            .context_list(params.project_id.as_deref())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "keys": keys })))
    }

    /// Send a message to an agent's queue.
    #[tool(
        description = "Send a message to an agent's queue. Returns {\"message_id\": \"...\"}. Errors: EmptyField if project_id/to_agent empty, ContentTooLarge if content > 1048576 bytes."
    )]
    async fn send_message(
        &self,
        Parameters(params): Parameters<SendMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        // Handle empty from_agent as "anonymous", trim whitespace for consistency
        let from_agent = params
            .from_agent
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("anonymous");

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
        Ok(json_response(&json!({ "message_id": message_id })))
    }

    /// Receive and consume messages from an agent's queue.
    #[tool(
        description = "Receive and consume messages from an agent's queue. Messages are deleted after retrieval. Default limit: 100, max: 500 (values above 500 are silently capped). Returns {\"messages\": [...]}."
    )]
    async fn receive_messages(
        &self,
        Parameters(params): Parameters<ReceiveMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let messages = self
            .db
            .receive_messages(&params.project_id, &params.agent_id, params.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(messages_response(&messages))
    }

    /// Peek at messages without consuming them.
    #[tool(
        description = "Peek at messages in an agent's queue without consuming them. Messages remain in queue. Default limit: 100, max: 500 (values above 500 are silently capped). Returns {\"messages\": [...]}."
    )]
    async fn peek_messages(
        &self,
        Parameters(params): Parameters<PeekMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let messages = self
            .db
            .peek_messages(&params.project_id, &params.agent_id, params.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(messages_response(&messages))
    }

    /// Delete a specific message by ID.
    #[tool(
        description = "Delete a specific message by ID. Returns {\"deleted\": true} or {\"deleted\": false}. Errors: InvalidMessageId if ID is not numeric."
    )]
    async fn delete_message(
        &self,
        Parameters(params): Parameters<DeleteMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        let deleted = self
            .db
            .delete_message(&params.message_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "deleted": deleted })))
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
