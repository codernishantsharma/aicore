use std::path::PathBuf;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub session_id: String,
    pub provider: String,
    pub conversation_id: Option<String>,
    pub parent_message_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub metadata: Option<String>,
}

pub struct Storage {
    db_path: PathBuf,
}

impl Storage {
    pub fn new(db_path: PathBuf) -> Result<Self, StorageError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        let storage = Self { db_path };
        storage.init_tables(&conn)?;
        Ok(storage)
    }

    pub fn in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            db_path: PathBuf::from(":memory:"),
        };
        storage.init_tables(&conn)?;
        Ok(storage)
    }

    pub fn get_connection(&self) -> Result<Connection, StorageError> {
        if self.db_path.to_str() == Some(":memory:") {
            Ok(Connection::open_in_memory()?)
        } else {
            Ok(Connection::open(&self.db_path)?)
        }
    }

    fn init_tables(&self, conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                conversation_id TEXT,
                parent_message_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                metadata TEXT
            );

            CREATE TABLE IF NOT EXISTS permissions (
                client_id TEXT NOT NULL,
                method TEXT NOT NULL,
                allowed INTEGER NOT NULL,
                PRIMARY KEY (client_id, method)
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn create_session(
        &self,
        conn: &Connection,
        session_id: &str,
        provider: &str,
    ) -> Result<SessionRecord, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        conn.execute(
            "INSERT INTO sessions (session_id, provider, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id) DO UPDATE SET updated_at = ?4",
            params![session_id, provider, now, now],
        )?;

        Ok(SessionRecord {
            session_id: session_id.to_string(),
            provider: provider.to_string(),
            conversation_id: None,
            parent_message_id: None,
            created_at: now,
            updated_at: now,
            metadata: None,
        })
    }

    pub fn get_session(
        &self,
        conn: &Connection,
        session_id: &str,
    ) -> Result<Option<SessionRecord>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT session_id, provider, conversation_id, parent_message_id, created_at, updated_at, metadata
             FROM sessions WHERE session_id = ?1",
        )?;

        let mut rows = stmt.query(params![session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SessionRecord {
                session_id: row.get(0)?,
                provider: row.get(1)?,
                conversation_id: row.get(2)?,
                parent_message_id: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                metadata: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_session(
        &self,
        conn: &Connection,
        session_id: &str,
        conversation_id: Option<&str>,
        parent_message_id: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        conn.execute(
            "UPDATE sessions SET conversation_id = ?1, parent_message_id = ?2, updated_at = ?3 WHERE session_id = ?4",
            params![conversation_id, parent_message_id, now, session_id],
        )?;
        Ok(())
    }

    pub fn set_permission(
        &self,
        conn: &Connection,
        client_id: &str,
        method: &str,
        allowed: bool,
    ) -> Result<(), StorageError> {
        let allowed_int = if allowed { 1 } else { 0 };
        conn.execute(
            "INSERT INTO permissions (client_id, method, allowed) VALUES (?1, ?2, ?3)
             ON CONFLICT(client_id, method) DO UPDATE SET allowed = ?3",
            params![client_id, method, allowed_int],
        )?;
        Ok(())
    }

    pub fn get_permission(
        &self,
        conn: &Connection,
        client_id: &str,
        method: &str,
    ) -> Result<Option<bool>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT allowed FROM permissions WHERE client_id = ?1 AND method = ?2",
        )?;
        let mut rows = stmt.query(params![client_id, method])?;
        if let Some(row) = rows.next()? {
            let val: i32 = row.get(0)?;
            Ok(Some(val == 1))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_sessions_and_permissions() {
        let conn = Connection::open_in_memory().unwrap();
        let storage = Storage::in_memory().unwrap();
        storage.init_tables(&conn).unwrap();

        let sess = storage.create_session(&conn, "sess_1", "chatgpt").unwrap();
        assert_eq!(sess.session_id, "sess_1");

        storage
            .update_session(&conn, "sess_1", Some("conv_123"), Some("msg_456"))
            .unwrap();

        let loaded = storage.get_session(&conn, "sess_1").unwrap().unwrap();
        assert_eq!(loaded.conversation_id.as_deref(), Some("conv_123"));
        assert_eq!(loaded.parent_message_id.as_deref(), Some("msg_456"));

        storage.set_permission(&conn, "fluxnotes", "chat.send", true).unwrap();
        assert_eq!(
            storage.get_permission(&conn, "fluxnotes", "chat.send").unwrap(),
            Some(true)
        );
    }
}
