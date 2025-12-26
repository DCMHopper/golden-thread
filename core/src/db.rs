use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use crate::error::CoreError;
use crate::migrations::MIGRATIONS;

pub struct ArchiveDb {
    pub path: PathBuf,
    pub conn: Connection,
}

pub fn open_archive(path: impl AsRef<Path>) -> Result<ArchiveDb, CoreError> {
    let path = path.as_ref().to_path_buf();
    let conn = Connection::open(&path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL; \
         PRAGMA synchronous = NORMAL; \
         PRAGMA foreign_keys = ON; \
         PRAGMA journal_size_limit = 67108864; \
         PRAGMA temp_store = MEMORY;",
    )?;
    apply_migrations(&conn)?;
    conn.execute(
        "UPDATE imports \
         SET status = 'failed', \
             stats_json = COALESCE(stats_json, '{\"error\":\"import interrupted\"}') \
         WHERE status = 'running';",
        [],
    )?;
    Ok(ArchiveDb { path, conn })
}

pub fn apply_migrations(conn: &Connection) -> Result<(), CoreError> {
    let current_version: i64 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    let mut version = current_version as usize;
    for (idx, sql) in MIGRATIONS.iter().enumerate() {
        let next_version = idx + 1;
        if next_version <= version {
            continue;
        }
        conn.execute_batch(sql)?;
        conn.execute_batch(&format!("PRAGMA user_version = {};", next_version))?;
        version = next_version;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_create_schema() {
        let conn = Connection::open_in_memory().expect("memory db");
        apply_migrations(&conn).expect("migrate");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages';",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1);
    }
}
