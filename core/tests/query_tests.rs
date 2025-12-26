use golden_thread_core::db::apply_migrations;
use golden_thread_core::query::{
    list_messages, list_messages_after, list_messages_around, list_threads, search_messages,
};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    conn
}

fn seed_messages(conn: &Connection) {
    conn.execute(
        "INSERT INTO threads (id, name, last_message_at) VALUES (?1, ?2, ?3);",
        rusqlite::params!["t1", "Thread 1", 3_i64],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO recipients (id, phone_e164, profile_name, contact_name) VALUES (?1, ?2, ?3, ?4);",
        rusqlite::params!["r1", "+15550001111", "Alice", "Alice"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m1", "t1", "r1", 1_i64, 1_i64, "text", "hello world", "d1"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m2", "t1", "r1", 2_i64, 2_i64, "text", "another note", "d2"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m3", "t1", "r1", 3_i64, 3_i64, "text", "search me", "d3"],
    )
    .unwrap();
}

#[test]
fn list_threads_returns_message_count() {
    let conn = setup_db();
    seed_messages(&conn);
    let threads = list_threads(&conn, 10, 0).expect("list threads");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].message_count, 3);
}

#[test]
fn list_messages_paginates_desc() {
    let conn = setup_db();
    seed_messages(&conn);
    let first = list_messages(&conn, "t1", None, None, 2).expect("list");
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].id, "m3");
    let next = list_messages(&conn, "t1", Some(2), Some("m2"), 2).expect("next");
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id, "m1");
}

#[test]
fn list_messages_after_asc() {
    let conn = setup_db();
    seed_messages(&conn);
    let after = list_messages_after(&conn, "t1", 1, Some("m1"), 2).expect("after");
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].id, "m2");
    assert_eq!(after[1].id, "m3");
}

#[test]
fn list_messages_around_includes_center() {
    let conn = setup_db();
    seed_messages(&conn);
    let around = list_messages_around(&conn, "m2", 1, 1).expect("around");
    assert_eq!(around.len(), 3);
    assert_eq!(around[1].id, "m2");
}

#[test]
fn search_messages_filters_thread() {
    let conn = setup_db();
    seed_messages(&conn);
    // build FTS
    conn.execute("INSERT INTO message_fts (message_id, thread_id, sender_id, body) VALUES (?1, ?2, ?3, ?4);",
        rusqlite::params!["m1", "t1", "r1", "hello world"]).unwrap();
    conn.execute("INSERT INTO message_fts (message_id, thread_id, sender_id, body) VALUES (?1, ?2, ?3, ?4);",
        rusqlite::params!["m2", "t1", "r1", "another note"]).unwrap();
    conn.execute("INSERT INTO message_fts (message_id, thread_id, sender_id, body) VALUES (?1, ?2, ?3, ?4);",
        rusqlite::params!["m3", "t1", "r1", "search me"]).unwrap();

    let hits = search_messages(&conn, "search", Some("t1"), 10, 0).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message.id, "m3");
}
