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
    /// Project ID for scoping. Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextGetParams {
    /// The key to retrieve.
    pub key: String,
    /// Project ID for scoping. Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextDeleteParams {
    /// The key to delete.
    pub key: String,
    /// Project ID for scoping. Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextListParams {
    /// Project ID for scoping. Omit for global context.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PublishParams {
    /// Topic to publish to (e.g., "releases/my-project" or "mailbox/agent-a").
    pub topic: String,
    /// Message content (max 1,048,576 bytes).
    pub content: String,
    /// Sender identifier. Defaults to "anonymous".
    #[serde(default)]
    pub from_agent: Option<String>,
    /// Reference to a previous message ID.
    #[serde(default)]
    pub reference_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReceiveParams {
    /// Topic to receive from.
    pub topic: String,
    /// Consumer identifier (used for tracking read messages).
    pub consumer: String,
    /// Maximum messages to receive (default: 100, max: 500).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PeekParams {
    /// Topic to peek.
    pub topic: String,
    /// Maximum messages to return (default: 100, max: 500).
    #[serde(default)]
    pub limit: Option<u32>,
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
    // -------------------------------------------------------------------------
    // Context operations
    // -------------------------------------------------------------------------

    #[tool(description = "Set a context value. Omit project_id for global context.")]
    async fn context_set(
        &self,
        Parameters(params): Parameters<ContextSetParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .context_set(params.project_id.as_deref(), &params.key, &params.value)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "ok": true })))
    }

    #[tool(description = "Get a context value. Returns {\"found\": true, \"value\": \"...\"} or {\"found\": false}.")]
    async fn context_get(
        &self,
        Parameters(params): Parameters<ContextGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let value = self
            .db
            .context_get(params.project_id.as_deref(), &params.key)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = match value {
            Some(v) => json!({ "found": true, "value": v }),
            None => json!({ "found": false }),
        };
        Ok(json_response(&response))
    }

    #[tool(description = "Delete a context value. Returns {\"deleted\": true/false}.")]
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

    #[tool(description = "List all context keys. Returns {\"keys\": [...]}.")]
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

    // -------------------------------------------------------------------------
    // Pub-sub operations
    // -------------------------------------------------------------------------

    #[tool(description = "Publish a message to a topic. Returns {\"message_id\": \"...\"}. Topics can be anything: \"releases/project\", \"mailbox/agent\", etc.")]
    async fn publish(
        &self,
        Parameters(params): Parameters<PublishParams>,
    ) -> Result<CallToolResult, McpError> {
        let from_agent = params.from_agent.as_deref().unwrap_or("anonymous");
        let message_id = self
            .db
            .publish(
                &params.topic,
                from_agent,
                &params.content,
                params.reference_id.as_deref(),
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "message_id": message_id })))
    }

    #[tool(description = "Receive unread messages from a topic. Messages are marked as read for this consumer. Returns {\"messages\": [...]}.")]
    async fn receive(
        &self,
        Parameters(params): Parameters<ReceiveParams>,
    ) -> Result<CallToolResult, McpError> {
        let messages = self
            .db
            .receive(&params.topic, &params.consumer, params.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(messages_response(&messages))
    }

    #[tool(description = "Peek at recent messages in a topic without marking as read. Returns {\"messages\": [...]}.")]
    async fn peek(
        &self,
        Parameters(params): Parameters<PeekParams>,
    ) -> Result<CallToolResult, McpError> {
        let messages = self
            .db
            .peek(&params.topic, params.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(messages_response(&messages))
    }

    #[tool(description = "List all topics with messages. Returns {\"topics\": [...]}.")]
    async fn list_topics(&self) -> Result<CallToolResult, McpError> {
        let topics = self
            .db
            .list_topics()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json_response(&json!({ "topics": topics })))
    }
}

#[tool_handler]
impl ServerHandler for MailboxServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Mailbox MCP server for agent communication via pub-sub".to_string()),
        }
    }
}
