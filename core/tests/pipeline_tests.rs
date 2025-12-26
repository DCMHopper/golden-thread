use std::fs;
use std::path::Path;

use golden_thread_core::importer::import_from_signal_db_for_tests;
use golden_thread_core::query::{list_messages, list_thread_media, list_threads, search_messages};
use golden_thread_core::open_archive;
use rusqlite::Connection;
use tempfile::tempdir;

fn create_signal_db(path: &Path) {
    let conn = Connection::open(path).expect("db");
    conn.execute_batch(
        r#"
        CREATE TABLE recipient (_id INTEGER PRIMARY KEY, e164 TEXT, system_joined_name TEXT, profile_given_name TEXT, group_id INTEGER);
        CREATE TABLE groups (group_id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE thread (_id INTEGER PRIMARY KEY, recipient_id INTEGER, date INTEGER, message_count INTEGER);
        CREATE TABLE sms (_id INTEGER PRIMARY KEY, thread_id INTEGER, body TEXT, date INTEGER, date_sent INTEGER, type INTEGER, recipient_id INTEGER);
        CREATE TABLE mms (_id INTEGER PRIMARY KEY, thread_id INTEGER, body TEXT, date_received INTEGER, date_sent INTEGER, type INTEGER, recipient_id INTEGER);
        CREATE TABLE part (_id INTEGER PRIMARY KEY, message_id INTEGER, unique_id INTEGER, content_type TEXT, data_size INTEGER, file_name TEXT);
        "#,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO recipient (_id, e164, system_joined_name, profile_given_name, group_id) VALUES (1, '+15550001111', 'Alice', 'Alice', NULL);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread (_id, recipient_id, date, message_count) VALUES (1, 1, 3, 2);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sms (_id, thread_id, body, date, date_sent, type, recipient_id) VALUES (10, 1, 'hello world', 1, 1, 1, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mms (_id, thread_id, body, date_received, date_sent, type, recipient_id) VALUES (1, 1, 'photo caption', 2, 2, 1, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO part (_id, message_id, unique_id, content_type, data_size, file_name) VALUES (5, 1, 1, 'image/jpeg', 4, 'pic.jpg');",
        [],
    )
    .unwrap();
}

#[test]
fn pipeline_import_populates_queries() {
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    create_signal_db(&signal_db);

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames");
    fs::write(export_dir.join("Attachment_5_1.bin"), b"test").expect("attachment");

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import");

    let archive = open_archive(&archive_path).expect("open");
    let threads = list_threads(&archive.conn, 10, 0).expect("threads");
    assert_eq!(threads.len(), 1);

    let messages = list_messages(&archive.conn, "1", None, None, 10).expect("messages");
    assert_eq!(messages.len(), 2);

    let media = list_thread_media(&archive.conn, "1", None, None, None, "date_desc", 10, 0).expect("media");
    assert_eq!(media.len(), 1);

    let hits = search_messages(&archive.conn, "hello", Some("1"), 10, 0).expect("search");
    assert_eq!(hits.len(), 1);
}
