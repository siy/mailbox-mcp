use crate::db::{Database, DbResult};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub from_agent: String,
    pub reference_id: Option<String>,
    pub content: String,
    pub created_at: String,
}

impl Database {
    pub fn send_message(
        &self,
        project_id: &str,
        to_agent: &str,
        from_agent: &str,
        content: &str,
        reference_id: Option<&str>,
    ) -> DbResult<String> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();

        self.with_conn(|conn| {
            conn.execute(
                r#"INSERT INTO messages (id, project_id, to_agent, from_agent, reference_id, content, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![id, project_id, to_agent, from_agent, reference_id, content, created_at],
            )?;
            Ok(id)
        })
    }

    pub fn receive_messages(
        &self,
        project_id: &str,
        agent_id: &str,
        limit: Option<u32>,
    ) -> DbResult<Vec<Message>> {
        let limit = limit.unwrap_or(100);

        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;

            let messages: Vec<Message> = {
                let mut stmt = tx.prepare(
                    r#"SELECT id, from_agent, reference_id, content, created_at
                       FROM messages
                       WHERE project_id = ?1 AND to_agent = ?2
                       ORDER BY created_at ASC
                       LIMIT ?3"#,
                )?;

                let messages = stmt
                    .query_map(params![project_id, agent_id, limit], |row| {
                        Ok(Message {
                            id: row.get(0)?,
                            from_agent: row.get(1)?,
                            reference_id: row.get(2)?,
                            content: row.get(3)?,
                            created_at: row.get(4)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                messages
            };

            // Delete consumed messages
            let ids: Vec<&str> = messages.iter().map(|m| m.id.as_str()).collect();
            if !ids.is_empty() {
                let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!("DELETE FROM messages WHERE id IN ({})", placeholders);
                let mut stmt = tx.prepare(&sql)?;
                for (i, id) in ids.iter().enumerate() {
                    stmt.raw_bind_parameter(i + 1, *id)?;
                }
                stmt.raw_execute()?;
            }

            tx.commit()?;
            Ok(messages)
        })
    }

    pub fn peek_messages(
        &self,
        project_id: &str,
        agent_id: &str,
        limit: Option<u32>,
    ) -> DbResult<Vec<Message>> {
        let limit = limit.unwrap_or(100);

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, from_agent, reference_id, content, created_at
                   FROM messages
                   WHERE project_id = ?1 AND to_agent = ?2
                   ORDER BY created_at ASC
                   LIMIT ?3"#,
            )?;

            let messages = stmt
                .query_map(params![project_id, agent_id, limit], |row| {
                    Ok(Message {
                        id: row.get(0)?,
                        from_agent: row.get(1)?,
                        reference_id: row.get(2)?,
                        content: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    pub fn delete_message(&self, message_id: &str) -> DbResult<bool> {
        self.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM messages WHERE id = ?1", params![message_id])?;
            Ok(rows > 0)
        })
    }
}
