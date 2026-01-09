//! # mailbox-mcp
//!
//! A minimalistic MCP (Model Context Protocol) server for agent-to-agent communication.
//!
//! ## Features
//!
//! - **Message Queues**: Per-agent message queues with request/response linking
//! - **Shared Context**: Global and project-scoped key-value stores
//! - **Persistence**: Messages and context survive server restarts
//!
//! ## Usage
//!
//! ```no_run
//! use mailbox_mcp::{Database, MailboxServer};
//!
//! let db = Database::new().expect("Failed to open database");
//! let server = MailboxServer::new(db);
//! // Use server with MCP transport...
//! ```

pub mod db;
pub mod tools;

pub use db::{Database, Message};
pub use tools::MailboxServer;
