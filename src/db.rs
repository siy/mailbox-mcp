//! Database layer for mailbox-mcp.
//!
//! Provides SQLite-backed storage for context key-value pairs and pub-sub messaging.

use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Maximum allowed size for message content (1MB).
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Maximum allowed size for context values (64KB).
pub const MAX_CONTEXT_VALUE_SIZE: usize = 64 * 1024;

/// Maximum number of messages to retrieve in a single query.
pub const MAX_MESSAGE_LIMIT: u32 = 500;

/// Errors that can occur during database operations.
#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Content too large: {size} bytes exceeds limit of {limit} bytes")]
    ContentTooLarge { size: usize, limit: usize },

    #[error("Required field '{field}' cannot be empty")]
    EmptyField { field: &'static str },
}

pub type DbResult<T> = Result<T, DbError>;

/// A message in a topic.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: String,
    pub topic: String,
    pub from_agent: String,
    pub reference_id: Option<String>,
    pub content: String,
    pub created_at: String,
}

/// Thread-safe database handle.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[allow(clippy::missing_errors_doc)]
impl Database {
    /// Creates a new database connection using the platform-specific default path.
    pub fn new() -> DbResult<Self> {
        let path = Self::default_path()?;
        Self::open(&path)
    }

    /// Opens a database at the specified path.
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
                -- Context table (unchanged)
                CREATE TABLE IF NOT EXISTS context (
                    project_id TEXT,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    PRIMARY KEY (project_id, key)
                );

                -- Messages with topic-based addressing
                CREATE TABLE IF NOT EXISTS messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    topic TEXT NOT NULL,
                    from_agent TEXT NOT NULL,
                    reference_id TEXT,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                );

                CREATE INDEX IF NOT EXISTS idx_messages_topic
                    ON messages(topic, created_at);

                -- Read markers for per-consumer tracking
                CREATE TABLE IF NOT EXISTS read_markers (
                    topic TEXT NOT NULL,
                    message_id INTEGER NOT NULL,
                    consumer TEXT NOT NULL,
                    read_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                    PRIMARY KEY (topic, message_id, consumer)
                );
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
    // Context operations (unchanged)
    // -------------------------------------------------------------------------

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

    pub fn context_delete(&self, project_id: Option<&str>, key: &str) -> DbResult<bool> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM context WHERE project_id IS ?1 AND key = ?2",
                params![project_id, key],
            )?;
            Ok(rows > 0)
        })
    }

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
    // Pub-sub operations
    // -------------------------------------------------------------------------

    /// Publishes a message to a topic. Returns the message ID.
    pub fn publish(
        &self,
        topic: &str,
        from_agent: &str,
        content: &str,
        reference_id: Option<&str>,
    ) -> DbResult<String> {
        let topic = topic.trim();
        if topic.is_empty() {
            return Err(DbError::EmptyField { field: "topic" });
        }
        let from_agent = from_agent.trim();
        let from_agent = if from_agent.is_empty() {
            "anonymous"
        } else {
            from_agent
        };
        if content.len() > MAX_MESSAGE_SIZE {
            return Err(DbError::ContentTooLarge {
                size: content.len(),
                limit: MAX_MESSAGE_SIZE,
            });
        }

        self.with_conn(|conn| {
            conn.execute(
                r"INSERT INTO messages (topic, from_agent, reference_id, content)
                  VALUES (?1, ?2, ?3, ?4)",
                params![topic, from_agent, reference_id, content],
            )?;
            Ok(conn.last_insert_rowid().to_string())
        })
    }

    /// Receives unread messages for a consumer, marking them as read.
    pub fn receive(
        &self,
        topic: &str,
        consumer: &str,
        limit: Option<u32>,
    ) -> DbResult<Vec<Message>> {
        let limit = limit.unwrap_or(100).min(MAX_MESSAGE_LIMIT);
        let topic = topic.trim();
        let consumer = consumer.trim();

        if consumer.is_empty() {
            return Err(DbError::EmptyField { field: "consumer" });
        }

        self.with_conn(|conn| {
            // Get unread messages
            let mut stmt = conn.prepare(
                r"SELECT m.id, m.topic, m.from_agent, m.reference_id, m.content, m.created_at
                  FROM messages m
                  WHERE m.topic = ?1
                    AND NOT EXISTS (
                        SELECT 1 FROM read_markers r
                        WHERE r.topic = m.topic AND r.message_id = m.id AND r.consumer = ?2
                    )
                  ORDER BY m.created_at ASC
                  LIMIT ?3",
            )?;

            let messages: Vec<Message> = stmt
                .query_map(params![topic, consumer, limit], |row| {
                    Ok(Message {
                        id: row.get::<_, i64>(0)?.to_string(),
                        topic: row.get(1)?,
                        from_agent: row.get(2)?,
                        reference_id: row.get(3)?,
                        content: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            // Mark as read
            if !messages.is_empty() {
                let mut insert_stmt = conn.prepare(
                    r"INSERT OR IGNORE INTO read_markers (topic, message_id, consumer)
                      VALUES (?1, ?2, ?3)",
                )?;
                for msg in &messages {
                    insert_stmt.execute(params![
                        topic,
                        msg.id.parse::<i64>().unwrap(),
                        consumer
                    ])?;
                }
            }

            Ok(messages)
        })
    }

    /// Peeks at recent messages without consumer tracking.
    pub fn peek(&self, topic: &str, limit: Option<u32>) -> DbResult<Vec<Message>> {
        let limit = limit.unwrap_or(100).min(MAX_MESSAGE_LIMIT);
        let topic = topic.trim();

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r"SELECT id, topic, from_agent, reference_id, content, created_at
                  FROM messages
                  WHERE topic = ?1
                  ORDER BY created_at DESC
                  LIMIT ?2",
            )?;

            let messages = stmt
                .query_map(params![topic, limit], |row| {
                    Ok(Message {
                        id: row.get::<_, i64>(0)?.to_string(),
                        topic: row.get(1)?,
                        from_agent: row.get(2)?,
                        reference_id: row.get(3)?,
                        content: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(messages)
        })
    }

    /// Lists all topics that have messages.
    pub fn list_topics(&self) -> DbResult<Vec<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(r"SELECT DISTINCT topic FROM messages ORDER BY topic")?;
            let topics = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(topics)
        })
    }

    /// Cleans up messages older than the specified number of hours.
    pub fn cleanup(&self, older_than_hours: u32) -> DbResult<u64> {
        self.with_conn(|conn| {
            // Delete read markers for old messages
            conn.execute(
                r"DELETE FROM read_markers WHERE message_id IN (
                    SELECT id FROM messages
                    WHERE created_at < datetime('now', ?1)
                )",
                params![format!("-{} hours", older_than_hours)],
            )?;

            // Delete old messages
            let deleted = conn.execute(
                r"DELETE FROM messages WHERE created_at < datetime('now', ?1)",
                params![format!("-{} hours", older_than_hours)],
            )?;

            Ok(deleted as u64)
        })
    }
}
