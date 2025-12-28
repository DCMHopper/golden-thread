use std::fs;
use std::path::Path;

use golden_thread_core::importer::import_from_signal_db_for_tests;
use golden_thread_core::{crypto, open_archive, CoreError};
use rusqlite::Connection;
use tempfile::tempdir;

fn set_test_key() {
    crypto::set_test_key_from_passphrase("golden-thread-tests");
}


fn create_signal_db(path: &Path) -> Result<(), CoreError> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        CREATE TABLE recipient (
          _id INTEGER PRIMARY KEY,
          e164 TEXT,
          system_joined_name TEXT,
          profile_given_name TEXT,
          group_id INTEGER
        );
        CREATE TABLE groups (
          group_id INTEGER PRIMARY KEY,
          title TEXT
        );
        CREATE TABLE thread (
          _id INTEGER PRIMARY KEY,
          recipient_id INTEGER,
          date INTEGER,
          message_count INTEGER
        );
        CREATE TABLE sms (
          _id INTEGER PRIMARY KEY,
          thread_id INTEGER,
          body TEXT,
          date INTEGER,
          date_sent INTEGER,
          type INTEGER,
          recipient_id INTEGER,
          quote_id INTEGER,
          quote_author INTEGER,
          quote_body TEXT
        );
        CREATE TABLE mms (
          _id INTEGER PRIMARY KEY,
          thread_id INTEGER,
          body TEXT,
          date_received INTEGER,
          date_sent INTEGER,
          type INTEGER,
          recipient_id INTEGER,
          quote_id INTEGER,
          quote_author INTEGER,
          quote_body TEXT
        );
        CREATE TABLE part (
          _id INTEGER PRIMARY KEY,
          message_id INTEGER,
          unique_id INTEGER,
          content_type TEXT,
          data_size INTEGER,
          file_name TEXT
        );
        CREATE TABLE reaction (
          message_id INTEGER,
          emoji TEXT,
          author_id INTEGER,
          date INTEGER
        );
        "#,
    )?;

    conn.execute(
        "INSERT INTO recipient (_id, e164, system_joined_name, profile_given_name, group_id) VALUES (1, '+15550001111', 'Alice', 'Alice', NULL);",
        [],
    )?;
    conn.execute(
        "INSERT INTO thread (_id, recipient_id, date, message_count) VALUES (1, 1, 3, 2);",
        [],
    )?;
    conn.execute(
        "INSERT INTO sms (_id, thread_id, body, date, date_sent, type, recipient_id, quote_id, quote_author, quote_body) \
         VALUES (10, 1, 'sms body', 1, 1, 1, 1, 10, 1, 'quoted');",
        [],
    )?;
    conn.execute(
        "INSERT INTO mms (_id, thread_id, body, date_received, date_sent, type, recipient_id, quote_id, quote_author, quote_body) \
         VALUES (1, 1, 'mms body', 2, 2, 1, 1, NULL, NULL, NULL);",
        [],
    )?;
    conn.execute(
        "INSERT INTO part (_id, message_id, unique_id, content_type, data_size, file_name) \
         VALUES (5, 1, 1, 'image/jpeg', 4, 'pic.jpg');",
        [],
    )?;
    conn.execute(
        "INSERT INTO reaction (message_id, emoji, author_id, date) VALUES (1, '👍', 1, 2);",
        [],
    )?;
    Ok(())
}

#[test]
fn importer_ingests_messages_attachments_reactions() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    create_signal_db(&signal_db).expect("signal db");

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    let attachment_path = export_dir.join("Attachment_5_1.bin");
    fs::write(&attachment_path, b"test").expect("attachment file");

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import");

    let archive = open_archive(&archive_path).expect("open archive");
    let msg_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM messages;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(msg_count, 2);
    let attachment_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM attachments;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attachment_count, 1);
    let reaction_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM reactions;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(reaction_count, 1);
    let metadata: Option<String> = archive
        .conn
        .query_row(
            "SELECT metadata_json FROM messages WHERE id = 'sms:10';",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(metadata.is_some());
}

#[test]
fn importer_handles_missing_attachment_files() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    create_signal_db(&signal_db).expect("signal db");

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    // no attachment file written on purpose

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import");

    let archive = open_archive(&archive_path).expect("open archive");
    let attachment_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM attachments;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attachment_count, 0);
}

#[test]
fn importer_dedupes_attachment_files_by_hash() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    let conn = Connection::open(&signal_db).expect("db");
    conn.execute_batch(
        r#"
        CREATE TABLE recipient (_id INTEGER PRIMARY KEY, e164 TEXT, system_joined_name TEXT, profile_given_name TEXT, group_id INTEGER);
        CREATE TABLE groups (group_id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE thread (_id INTEGER PRIMARY KEY, recipient_id INTEGER, date INTEGER, message_count INTEGER);
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
        "INSERT INTO thread (_id, recipient_id, date, message_count) VALUES (1, 1, 2, 2);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mms (_id, thread_id, body, date_received, date_sent, type, recipient_id) VALUES (1, 1, 'first', 1, 1, 1, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mms (_id, thread_id, body, date_received, date_sent, type, recipient_id) VALUES (2, 1, 'second', 2, 2, 1, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO part (_id, message_id, unique_id, content_type, data_size, file_name) VALUES (5, 1, 1, 'image/jpeg', 4, 'a.jpg');",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO part (_id, message_id, unique_id, content_type, data_size, file_name) VALUES (6, 2, 1, 'image/jpeg', 4, 'b.jpg');",
        [],
    )
    .unwrap();

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    fs::write(export_dir.join("Attachment_5_1.bin"), b"same").expect("attachment a");
    fs::write(export_dir.join("Attachment_6_1.bin"), b"same").expect("attachment b");

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import");

    let archive = open_archive(&archive_path).expect("open archive");
    let attachment_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM attachments;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attachment_count, 2);
    let unique_sha: i64 = archive
        .conn
        .query_row("SELECT COUNT(DISTINCT sha256) FROM attachments;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(unique_sha, 1);

    let attachments_dir = archive_path.parent().unwrap().join("attachments");
    let entries: Vec<_> = fs::read_dir(&attachments_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn importer_handles_missing_attachment_metadata() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    let conn = Connection::open(&signal_db).expect("db");
    conn.execute_batch(
        r#"
        CREATE TABLE recipient (_id INTEGER PRIMARY KEY, e164 TEXT, system_joined_name TEXT, profile_given_name TEXT, group_id INTEGER);
        CREATE TABLE groups (group_id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE thread (_id INTEGER PRIMARY KEY, recipient_id INTEGER, date INTEGER, message_count INTEGER);
        CREATE TABLE mms (_id INTEGER PRIMARY KEY, thread_id INTEGER, body TEXT, date_received INTEGER, date_sent INTEGER, type INTEGER, recipient_id INTEGER);
        CREATE TABLE part (_id INTEGER PRIMARY KEY, message_id INTEGER, unique_id INTEGER, data_size INTEGER, file_name TEXT);
        "#,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO recipient (_id, e164, system_joined_name, profile_given_name, group_id) VALUES (1, '+15550001111', 'Alice', 'Alice', NULL);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread (_id, recipient_id, date, message_count) VALUES (1, 1, 2, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mms (_id, thread_id, body, date_received, date_sent, type, recipient_id) VALUES (1, 1, 'photo', 1, 1, 1, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO part (_id, message_id, unique_id, data_size, file_name) VALUES (5, 1, 1, NULL, 'a.jpg');",
        [],
    )
    .unwrap();

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    fs::write(export_dir.join("Attachment_5_1.bin"), b"test").expect("attachment");

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import");

    let archive = open_archive(&archive_path).expect("open archive");
    let size: Option<i64> = archive
        .conn
        .query_row("SELECT size_bytes FROM attachments LIMIT 1;", [], |row| row.get(0))
        .unwrap();
    assert!(size.is_some());
}

#[test]
fn importer_idempotent_same_archive() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    create_signal_db(&signal_db).expect("signal db");

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    fs::write(export_dir.join("Attachment_5_1.bin"), b"test").expect("attachment");

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import 1");
    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import 2");

    let archive = open_archive(&archive_path).expect("open archive");
    let msg_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM messages;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(msg_count, 2);
    let attachment_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM attachments;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attachment_count, 1);
}

#[test]
fn importer_incremental_adds_new_messages_only() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db_a = tmp.path().join("signal_a.sqlite");
    create_signal_db(&signal_db_a).expect("signal db a");

    let signal_db_b = tmp.path().join("signal_b.sqlite");
    create_signal_db(&signal_db_b).expect("signal db b");
    let conn_b = Connection::open(&signal_db_b).expect("db b");
    conn_b.execute(
        "INSERT INTO sms (_id, thread_id, body, date, date_sent, type, recipient_id, quote_id, quote_author, quote_body) \
         VALUES (11, 1, 'new message', 4, 4, 1, 1, NULL, NULL, NULL);",
        [],
    )
    .unwrap();

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    fs::write(export_dir.join("Attachment_5_1.bin"), b"test").expect("attachment");

    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db_a, &archive_path, &export_dir).expect("import a");
    import_from_signal_db_for_tests(&signal_db_b, &archive_path, &export_dir).expect("import b");

    let archive = open_archive(&archive_path).expect("open archive");
    let msg_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM messages;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(msg_count, 3);
}
#[test]
fn importer_handles_missing_optional_columns() {
    set_test_key();
    let tmp = tempdir().expect("temp");
    let signal_db = tmp.path().join("signal.sqlite");
    let conn = Connection::open(&signal_db).expect("db");
    conn.execute_batch(
        r#"
        CREATE TABLE recipient (_id INTEGER PRIMARY KEY, e164 TEXT, system_joined_name TEXT, profile_given_name TEXT, group_id INTEGER);
        CREATE TABLE groups (group_id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE thread (_id INTEGER PRIMARY KEY, recipient_id INTEGER, date INTEGER);
        CREATE TABLE sms (_id INTEGER PRIMARY KEY, thread_id INTEGER, body TEXT, date INTEGER, date_sent INTEGER, type INTEGER, recipient_id INTEGER);
        CREATE TABLE mms (_id INTEGER PRIMARY KEY, thread_id INTEGER, body TEXT, date_received INTEGER, date_sent INTEGER, type INTEGER, recipient_id INTEGER);
        "#,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO recipient (_id, e164) VALUES (1, '+15550001111');",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread (_id, recipient_id, date) VALUES (1, 1, 2);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sms (_id, thread_id, body, date, date_sent, type, recipient_id) VALUES (1, 1, 'hi', 1, 1, 1, 1);",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mms (_id, thread_id, body, date_received, date_sent, type, recipient_id) VALUES (2, 1, 'hey', 2, 2, 1, 1);",
        [],
    )
    .unwrap();

    let export_dir = tmp.path().join("frames");
    fs::create_dir_all(&export_dir).expect("frames dir");
    let archive_dir = tmp.path().join("archive");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive_path = archive_dir.join("archive.sqlite");

    import_from_signal_db_for_tests(&signal_db, &archive_path, &export_dir).expect("import");

    let archive = open_archive(&archive_path).expect("open archive");
    let msg_count: i64 = archive
        .conn
        .query_row("SELECT COUNT(1) FROM messages;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(msg_count, 2);
}
