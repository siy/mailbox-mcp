use crate::db::{Database, DbResult};
use rusqlite::params;

impl Database {
    pub fn context_set(&self, project_id: Option<&str>, key: &str, value: &str) -> DbResult<()> {
        match project_id {
            Some(pid) => self.with_conn(|conn| {
                conn.execute(
                    r#"INSERT INTO project_context (project_id, key, value, updated_at)
                       VALUES (?1, ?2, ?3, datetime('now'))
                       ON CONFLICT(project_id, key) DO UPDATE SET value = ?3, updated_at = datetime('now')"#,
                    params![pid, key, value],
                )?;
                Ok(())
            }),
            None => self.with_conn(|conn| {
                conn.execute(
                    r#"INSERT INTO global_context (key, value, updated_at)
                       VALUES (?1, ?2, datetime('now'))
                       ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')"#,
                    params![key, value],
                )?;
                Ok(())
            }),
        }
    }

    pub fn context_get(&self, project_id: Option<&str>, key: &str) -> DbResult<Option<String>> {
        match project_id {
            Some(pid) => self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT value FROM project_context WHERE project_id = ?1 AND key = ?2",
                )?;
                let result = stmt.query_row(params![pid, key], |row| row.get(0));
                match result {
                    Ok(value) => Ok(Some(value)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            }),
            None => self.with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT value FROM global_context WHERE key = ?1")?;
                let result = stmt.query_row(params![key], |row| row.get(0));
                match result {
                    Ok(value) => Ok(Some(value)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            }),
        }
    }

    pub fn context_delete(&self, project_id: Option<&str>, key: &str) -> DbResult<bool> {
        match project_id {
            Some(pid) => self.with_conn(|conn| {
                let rows = conn.execute(
                    "DELETE FROM project_context WHERE project_id = ?1 AND key = ?2",
                    params![pid, key],
                )?;
                Ok(rows > 0)
            }),
            None => self.with_conn(|conn| {
                let rows =
                    conn.execute("DELETE FROM global_context WHERE key = ?1", params![key])?;
                Ok(rows > 0)
            }),
        }
    }

    pub fn context_list(&self, project_id: Option<&str>) -> DbResult<Vec<String>> {
        match project_id {
            Some(pid) => self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT key FROM project_context WHERE project_id = ?1 ORDER BY key",
                )?;
                let keys = stmt
                    .query_map(params![pid], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(keys)
            }),
            None => self.with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT key FROM global_context ORDER BY key")?;
                let keys = stmt
                    .query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(keys)
            }),
        }
    }
}
