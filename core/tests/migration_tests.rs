use golden_thread_core::db::apply_migrations;
use rusqlite::Connection;

#[test]
fn attachments_size_bucket_column_exists() {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    let mut stmt = conn
        .prepare("PRAGMA table_info(attachments);")
        .expect("pragma");
    let mut rows = stmt.query([]).expect("rows");
    let mut found = false;
    while let Some(row) = rows.next().expect("row") {
        let name: String = row.get(1).expect("name");
        if name == "size_bucket" {
            found = true;
            break;
        }
    }
    assert!(found, "size_bucket column missing");
}

#[test]
fn attachments_size_bucket_index_exists() {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type='index' AND name='idx_attachments_size_bucket_message';",
            [],
            |row| row.get(0),
        )
        .expect("index query");
    assert_eq!(count, 1);
}

#[test]
fn message_tags_message_id_tagged_at_index_exists() {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type='index' AND name='idx_message_tags_message_id_tagged_at';",
            [],
            |row| row.get(0),
        )
        .expect("index query");
    assert_eq!(count, 1);
}

#[test]
fn imports_source_hash_unique_index_exists() {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type='index' AND name='idx_imports_source_hash';",
            [],
            |row| row.get(0),
        )
        .expect("index query");
    assert_eq!(count, 1);
}
