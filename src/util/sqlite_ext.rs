use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// Open a foreign SQLite DB read-only, reading WAL when present.
pub fn open_foreign_db(path: &Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let wal = path.with_extension("db-wal");
    let wal2 = {
        let mut p = path.to_path_buf();
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            p.set_file_name(format!("{name}-wal"));
        }
        p
    };
    if wal.exists() || wal2.exists() {
        let uri = format!(
            "file:{}?mode=ro",
            path.display().to_string().replace('\\', "/")
        );
        return Connection::open_with_flags(&uri, flags)
            .with_context(|| format!("open sqlite (wal) {}", path.display()));
    }
    let uri = format!(
        "file:{}?mode=ro&immutable=1",
        path.display().to_string().replace('\\', "/")
    );
    Connection::open_with_flags(&uri, flags)
        .with_context(|| format!("open sqlite {}", path.display()))
}

pub fn list_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let sql = format!("PRAGMA table_info(\"{table}\")");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
