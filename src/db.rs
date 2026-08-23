use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

pub struct Capture {
    pub id: i64,
    pub source: String,
    pub tool: String,
    pub created_at: String,
    pub byte_len: i64,
    pub snippet: String,
}

pub fn db_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    let dir = PathBuf::from(home).join(".stow");
    std::fs::create_dir_all(&dir).ok();
    dir.join("store.db")
}

pub fn open() -> Result<Connection> {
    let conn = Connection::open(db_path())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE VIRTUAL TABLE IF NOT EXISTS captures USING fts5(
             content,
             source,
             tool,
             created_at UNINDEXED,
             byte_len UNINDEXED
         );",
    )?;
    Ok(conn)
}

pub fn insert(conn: &Connection, content: &str, source: &str, tool: &str) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let byte_len = content.len() as i64;
    conn.execute(
        "INSERT INTO captures (content, source, tool, created_at, byte_len) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![content, source, tool, now, byte_len],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn search(conn: &Connection, query: &str, limit: i64) -> Result<Vec<Capture>> {
    let mut stmt = conn.prepare(
        "SELECT rowid, source, tool, created_at, byte_len,
                snippet(captures, 0, '[', ']', '...', 24) AS snip
         FROM captures
         WHERE captures MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit], |row| {
        Ok(Capture {
            id: row.get(0)?,
            source: row.get(1)?,
            tool: row.get(2)?,
            created_at: row.get(3)?,
            byte_len: row.get(4)?,
            snippet: row.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn show(conn: &Connection, id: i64) -> Result<Option<(String, String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT content, source, tool, created_at FROM captures WHERE rowid = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
    } else {
        Ok(None)
    }
}
