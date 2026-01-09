//! Database layer for mailbox-mcp.
//!
//! Provides SQLite-backed storage for context key-value pairs and message queues.

use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Maximum allowed size for message content (1MB = 1,048,576 bytes).
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Maximum allowed size for context values (64KB = 65,536 bytes).
pub const MAX_CONTEXT_VALUE_SIZE: usize = 64 * 1024;

/// Maximum number of messages to retrieve in a single query.
pub const MAX_MESSAGE_LIMIT: u32 = 500;

/// Errors that can occur during database operations.
#[derive(Error, Debug)]
pub enum DbError {
    /// Database operation failed.
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// IO error during database operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Content exceeds maximum allowed size.
    #[error("Content too large: {size} bytes exceeds limit of {limit} bytes")]
    ContentTooLarge { size: usize, limit: usize },

    /// Required field is empty.
    #[error("Required field '{field}' cannot be empty")]
    EmptyField { field: &'static str },

    /// Invalid message ID format.
    #[error("Invalid message ID: '{id}' (must be a numeric ID)")]
    InvalidMessageId { id: String },
}

/// Result type for database operations.
pub type DbResult<T> = Result<T, DbError>;

/// A message in an agent's queue.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: String,
    /// Agent that sent this message.
    pub from_agent: String,
    /// Optional reference to a previous message (for request/response linking).
    pub reference_id: Option<String>,
    /// Message content.
    pub content: String,
    /// Timestamp when the message was created (ISO 8601 format: `2025-01-08T12:00:00Z`).
    pub created_at: String,
}

/// Thread-safe database handle.
///
/// All operations are serialized through an internal mutex. This is appropriate
/// for local-only use with low concurrency.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[allow(clippy::missing_errors_doc)]
impl Database {
    /// Creates a new database connection using the platform-specific default path.
    ///
    /// The database file is stored in the user's application data directory:
    /// - Linux: `~/.local/share/mailbox-mcp/mailbox.db`
    /// - macOS: `~/Library/Application Support/mailbox-mcp/mailbox.db`
    /// - Windows: `%APPDATA%\mailbox-mcp\mailbox.db`
    pub fn new() -> DbResult<Self> {
        let path = Self::default_path()?;
        Self::open(&path)
    }

    /// Opens a database at the specified path.
    ///
    /// Creates the parent directory if it doesn't exist.
    /// Runs migrations to ensure the schema is up to date.
    pub fn open(path: &Path) -> DbResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn default_path() -> DbResult<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "No home directory found")
            })?;

        #[cfg(target_os = "macos")]
        let path = PathBuf::from(home).join("Library/Application Support/mailbox-mcp/mailbox.db");

        #[cfg(target_os = "windows")]
        let path = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&home))
            .join("mailbox-mcp/mailbox.db");

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let path = PathBuf::from(home).join(".local/share/mailbox-mcp/mailbox.db");

        Ok(path)
    }

    fn migrate(&self) -> DbResult<()> {
        self.with_conn(|conn| {
            conn.execute_batch(
                r"
                -- Unified context table (project_id NULL = global)
                CREATE TABLE IF NOT EXISTS context (
                    project_id TEXT,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    PRIMARY KEY (project_id, key)
                );

                -- Message queue
                CREATE TABLE IF NOT EXISTS messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    project_id TEXT NOT NULL,
                    to_agent TEXT NOT NULL,
                    from_agent TEXT NOT NULL,
                    reference_id TEXT,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                );

                CREATE INDEX IF NOT EXISTS idx_messages_queue
                    ON messages(project_id, to_agent, created_at);
                ",
            )?;
            Ok(())
        })
    }

    fn with_conn<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Connection) -> SqliteResult<T>,
    {
        let conn = self
            .conn
            .lock()
            .expect("Database mutex poisoned - this indicates a bug");
        f(&conn).map_err(DbError::from)
    }

    // -------------------------------------------------------------------------
    // Context operations
    // -------------------------------------------------------------------------

    /// Sets a context value.
    ///
    /// If `project_id` is `None`, sets a global context value.
    /// If `project_id` is `Some`, sets a project-scoped context value.
    ///
    /// # Errors
    /// - `EmptyField` if key is empty
    /// - `ContentTooLarge` if value exceeds 65,536 bytes
    pub fn context_set(&self, project_id: Option<&str>, key: &str, value: &str) -> DbResult<()> {
        let key = key.trim();
        if key.is_empty() {
            return Err(DbError::EmptyField { field: "key" });
        }
        if value.len() > MAX_CONTEXT_VALUE_SIZE {
            return Err(DbError::ContentTooLarge {
                size: value.len(),
                limit: MAX_CONTEXT_VALUE_SIZE,
            });
        }

        self.with_conn(|conn| {
            conn.execute(
                r"INSERT INTO context (project_id, key, value)
                  VALUES (?1, ?2, ?3)
                  ON CONFLICT(project_id, key) DO UPDATE SET value = ?3",
                params![project_id, key, value],
            )?;
            Ok(())
        })
    }

    /// Gets a context value.
    ///
    /// Returns `Ok(Some(value))` if the key exists, `Ok(None)` if it doesn't.
    pub fn context_get(&self, project_id: Option<&str>, key: &str) -> DbResult<Option<String>> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT value FROM context WHERE project_id IS ?1 AND key = ?2")?;
            let result = stmt.query_row(params![project_id, key], |row| row.get(0));
            match result {
                Ok(value) => Ok(Some(value)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Deletes a context value.
    ///
    /// Returns `true` if a value was deleted, `false` if the key didn't exist.
    pub fn context_delete(&self, project_id: Option<&str>, key: &str) -> DbResult<bool> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM context WHERE project_id IS ?1 AND key = ?2",
                params![project_id, key],
            )?;
            Ok(rows > 0)
        })
    }

    /// Lists all context keys.
    ///
    /// If `project_id` is `None`, lists global context keys.
    /// If `project_id` is `Some`, lists project-scoped context keys.
    pub fn context_list(&self, project_id: Option<&str>) -> DbResult<Vec<String>> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT key FROM context WHERE project_id IS ?1 ORDER BY key")?;
            let keys = stmt
                .query_map(params![project_id], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(keys)
        })
    }

    // -------------------------------------------------------------------------
    // Message operations
    // -------------------------------------------------------------------------

    /// Sends a message to an agent's queue.
    ///
    /// Returns the message ID.
    ///
    /// # Errors
    /// - `EmptyField` if `project_id` or `to_agent` is empty (Note: `from_agent` is validated
    ///   at the API layer, which defaults empty values to "anonymous")
    /// - `ContentTooLarge` if content exceeds 1,048,576 bytes
    pub fn send_message(
        &self,
        project_id: &str,
        to_agent: &str,
        from_agent: &str,
        content: &str,
        reference_id: Option<&str>,
    ) -> DbResult<String> {
        if project_id.trim().is_empty() {
            return Err(DbError::EmptyField {
                field: "project_id",
            });
        }
        if to_agent.trim().is_empty() {
            return Err(DbError::EmptyField { field: "to_agent" });
        }
        if from_agent.trim().is_empty() {
            return Err(DbError::EmptyField {
                field: "from_agent",
            });
        }
        if content.len() > MAX_MESSAGE_SIZE {
            return Err(DbError::ContentTooLarge {
                size: content.len(),
                limit: MAX_MESSAGE_SIZE,
            });
        }

        self.with_conn(|conn| {
            conn.execute(
                r"INSERT INTO messages (project_id, to_agent, from_agent, reference_id, content)
                  VALUES (?1, ?2, ?3, ?4, ?5)",
                params![project_id, to_agent, from_agent, reference_id, content],
            )?;
            Ok(conn.last_insert_rowid().to_string())
        })
    }

    /// Retrieves and consumes messages from an agent's queue.
    ///
    /// Messages are returned in chronological order and deleted from the queue.
    /// Use [`peek_messages`](Self::peek_messages) to view without consuming.
    ///
    /// Limit is capped at [`MAX_MESSAGE_LIMIT`] (500).
    pub fn receive_messages(
        &self,
        project_id: &str,
        agent_id: &str,
        limit: Option<u32>,
    ) -> DbResult<Vec<Message>> {
        let limit = limit.unwrap_or(100).min(MAX_MESSAGE_LIMIT);

        self.with_conn(|conn| {
            let messages = Self::query_messages(conn, project_id, agent_id, limit)?;

            // Delete consumed messages in a single statement
            if !messages.is_empty() {
                let ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
                let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!("DELETE FROM messages WHERE id IN ({placeholders})");
                let mut stmt = conn.prepare(&sql)?;
                for (i, id) in ids.iter().enumerate() {
                    stmt.raw_bind_parameter(i + 1, id)?;
                }
                stmt.raw_execute()?;
            }

            Ok(messages)
        })
    }

    /// Peeks at messages in an agent's queue without consuming them.
    ///
    /// Messages are returned in chronological order but remain in the queue.
    ///
    /// Limit is capped at [`MAX_MESSAGE_LIMIT`] (500).
    pub fn peek_messages(
        &self,
        project_id: &str,
        agent_id: &str,
        limit: Option<u32>,
    ) -> DbResult<Vec<Message>> {
        let limit = limit.unwrap_or(100).min(MAX_MESSAGE_LIMIT);

        self.with_conn(|conn| Self::query_messages(conn, project_id, agent_id, limit))
    }

    fn query_messages(
        conn: &Connection,
        project_id: &str,
        agent_id: &str,
        limit: u32,
    ) -> SqliteResult<Vec<Message>> {
        let mut stmt = conn.prepare(
            r"SELECT id, from_agent, reference_id, content, created_at
              FROM messages
              WHERE project_id = ?1 AND to_agent = ?2
              ORDER BY created_at ASC
              LIMIT ?3",
        )?;

        let messages = stmt
            .query_map(params![project_id, agent_id, limit], |row| {
                Ok(Message {
                    id: row.get::<_, i64>(0)?.to_string(),
                    from_agent: row.get(1)?,
                    reference_id: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(messages)
    }

    /// Deletes a specific message by ID.
    ///
    /// Returns `true` if the message was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    /// - `InvalidMessageId` if the message ID is not a valid numeric ID
    pub fn delete_message(&self, message_id: &str) -> DbResult<bool> {
        let id: i64 = message_id.parse().map_err(|_| DbError::InvalidMessageId {
            id: message_id.to_string(),
        })?;
        self.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM messages WHERE id = ?1", params![id])?;
            Ok(rows > 0)
        })
    }
}
