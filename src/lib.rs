//! # mailbox-mcp
//!
//! A minimalistic MCP server for agent-to-agent communication via pub-sub.
//!
//! ## Features
//!
//! - **Pub-sub messaging**: Topics with per-consumer read tracking
//! - **Shared context**: Global and project-scoped key-value stores
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
