use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{path::Path, sync::{Arc, Mutex}};

#[derive(Clone)]
pub struct Database { inner: Arc<Mutex<Connection>> }

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS objects (
              kind TEXT NOT NULL,
              id TEXT NOT NULL,
              json TEXT NOT NULL,
              updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
              PRIMARY KEY(kind,id)
            );
            CREATE TABLE IF NOT EXISTS singleton (
              kind TEXT PRIMARY KEY,
              json TEXT NOT NULL,
              updated_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS workflow_seen (
              workflow_id TEXT NOT NULL,
              path TEXT NOT NULL,
              size INTEGER NOT NULL,
              mtime_ns INTEGER NOT NULL,
              PRIMARY KEY(workflow_id,path,size,mtime_ns)
            );
            CREATE INDEX IF NOT EXISTS objects_kind_updated ON objects(kind,updated_at DESC);
        "#)?;
        Ok(Self { inner: Arc::new(Mutex::new(connection)) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> { self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) }

    pub fn upsert<T: Serialize>(&self, kind: &str, id: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.conn().execute(
            "INSERT INTO objects(kind,id,json,updated_at) VALUES(?1,?2,?3,unixepoch()) ON CONFLICT(kind,id) DO UPDATE SET json=excluded.json,updated_at=excluded.updated_at",
            params![kind, id, json],
        )?;
        Ok(())
    }

    pub fn get<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<Option<T>> {
        let json: Option<String> = self.conn().query_row("SELECT json FROM objects WHERE kind=?1 AND id=?2", params![kind,id], |row| row.get(0)).optional()?;
        json.map(|v| serde_json::from_str(&v).map_err(Into::into)).transpose()
    }

    pub fn list<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT json FROM objects WHERE kind=?1 ORDER BY updated_at DESC,id")?;
        let rows = stmt.query_map([kind], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows { out.push(serde_json::from_str(&row?)?); }
        Ok(out)
    }

    pub fn delete(&self, kind: &str, id: &str) -> Result<bool> { Ok(self.conn().execute("DELETE FROM objects WHERE kind=?1 AND id=?2", params![kind,id])? > 0) }

    pub fn set_singleton<T: Serialize>(&self, kind: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.conn().execute(
            "INSERT INTO singleton(kind,json,updated_at) VALUES(?1,?2,unixepoch()) ON CONFLICT(kind) DO UPDATE SET json=excluded.json,updated_at=excluded.updated_at",
            params![kind,json],
        )?;
        Ok(())
    }

    pub fn get_singleton<T: DeserializeOwned>(&self, kind: &str) -> Result<Option<T>> {
        let json: Option<String> = self.conn().query_row("SELECT json FROM singleton WHERE kind=?1", [kind], |row| row.get(0)).optional()?;
        json.map(|v| serde_json::from_str(&v).map_err(Into::into)).transpose()
    }

    pub fn workflow_seen(&self, workflow_id: &str, path: &str, size: u64, mtime_ns: i128) -> Result<bool> {
        let mtime = i64::try_from(mtime_ns).unwrap_or(i64::MAX);
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM workflow_seen WHERE workflow_id=?1 AND path=?2 AND size=?3 AND mtime_ns=?4",
            params![workflow_id,path,i64::try_from(size).unwrap_or(i64::MAX),mtime], |row| row.get(0)
        )?;
        Ok(count > 0)
    }

    pub fn mark_workflow_seen(&self, workflow_id: &str, path: &str, size: u64, mtime_ns: i128) -> Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO workflow_seen(workflow_id,path,size,mtime_ns) VALUES(?1,?2,?3,?4)",
            params![workflow_id,path,i64::try_from(size).unwrap_or(i64::MAX),i64::try_from(mtime_ns).unwrap_or(i64::MAX)]
        )?;
        Ok(())
    }
}
