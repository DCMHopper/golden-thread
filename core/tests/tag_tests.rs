use golden_thread_core::db::apply_migrations;
use golden_thread_core::query::{
    create_tag, delete_tag, get_message_tags, list_scrapbook_messages, list_tags,
    set_message_tags, update_tag,
};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    conn
}

fn seed_test_data(conn: &Connection) {
    // Create thread
    conn.execute(
        "INSERT INTO threads (id, name, last_message_at) VALUES (?1, ?2, ?3);",
        rusqlite::params!["t1", "Test Thread", 10_i64],
    )
    .unwrap();

    // Create messages with timestamps spaced out
    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m1", "t1", "r1", 1_i64, 1_i64, "text", "First message", "d1"],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m2", "t1", "r1", 5_i64, 5_i64, "text", "Middle message", "d2"],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m3", "t1", "r1", 10_i64, 10_i64, "text", "Last message", "d3"],
    )
    .unwrap();

    // Create a message in between m1 and m2 (not tagged) to test discontinuity
    conn.execute(
        "INSERT INTO messages (id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8);",
        rusqlite::params!["m_between", "t1", "r1", 3_i64, 3_i64, "text", "Between message", "d_between"],
    )
    .unwrap();
}

#[test]
fn create_tag_generates_id() {
    let conn = setup_db();
    let tag = create_tag(&conn, "Important", "#ff0000").expect("create tag");

    assert!(!tag.id.is_empty());
    assert_eq!(tag.name, "Important");
    assert_eq!(tag.color, "#ff0000");
    assert!(tag.created_at > 0);
    assert_eq!(tag.display_order, 0);
}

#[test]
fn create_tag_increments_display_order() {
    let conn = setup_db();
    let tag1 = create_tag(&conn, "First", "#ff0000").expect("create tag1");
    std::thread::sleep(std::time::Duration::from_millis(2)); // Ensure unique timestamp
    let tag2 = create_tag(&conn, "Second", "#00ff00").expect("create tag2");

    assert_eq!(tag1.display_order, 0);
    assert_eq!(tag2.display_order, 1);
}

#[test]
fn create_tag_enforces_unique_name() {
    let conn = setup_db();
    create_tag(&conn, "Duplicate", "#ff0000").expect("create first tag");
    let result = create_tag(&conn, "Duplicate", "#00ff00");

    assert!(result.is_err(), "Should fail on duplicate tag name");
}

#[test]
fn list_tags_returns_ordered_by_display_order() {
    let conn = setup_db();
    create_tag(&conn, "Third", "#0000ff").expect("tag 3");
    std::thread::sleep(std::time::Duration::from_millis(2));
    create_tag(&conn, "First", "#ff0000").expect("tag 1");
    std::thread::sleep(std::time::Duration::from_millis(2));
    create_tag(&conn, "Second", "#00ff00").expect("tag 2");

    let tags = list_tags(&conn).expect("list tags");
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].name, "Third");
    assert_eq!(tags[1].name, "First");
    assert_eq!(tags[2].name, "Second");
}

#[test]
fn update_tag_changes_name_and_color() {
    let conn = setup_db();
    let tag = create_tag(&conn, "Old Name", "#ff0000").expect("create");

    update_tag(&conn, &tag.id, "New Name", "#00ff00").expect("update");

    let tags = list_tags(&conn).expect("list");
    assert_eq!(tags[0].name, "New Name");
    assert_eq!(tags[0].color, "#00ff00");
}

#[test]
fn delete_tag_removes_tag() {
    let conn = setup_db();
    let tag = create_tag(&conn, "To Delete", "#ff0000").expect("create");

    delete_tag(&conn, &tag.id).expect("delete");

    let tags = list_tags(&conn).expect("list");
    assert_eq!(tags.len(), 0);
}

#[test]
fn delete_tag_cascades_to_message_tags() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag = create_tag(&conn, "Test Tag", "#ff0000").expect("create");
    set_message_tags(&conn, "m1", &[tag.id.clone()]).expect("set tags");

    // Verify tag was set
    let tags_before = get_message_tags(&conn, "m1").expect("get tags");
    assert_eq!(tags_before.len(), 1);

    // Delete tag
    delete_tag(&conn, &tag.id).expect("delete");

    // Verify message_tags were cascaded
    let tags_after = get_message_tags(&conn, "m1").expect("get tags");
    assert_eq!(tags_after.len(), 0);
}

#[test]
fn set_message_tags_replaces_existing() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag1 = create_tag(&conn, "Tag 1", "#ff0000").expect("tag 1");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let tag2 = create_tag(&conn, "Tag 2", "#00ff00").expect("tag 2");

    // Set initial tags
    set_message_tags(&conn, "m1", &[tag1.id.clone()]).expect("set tags");
    let tags = get_message_tags(&conn, "m1").expect("get tags");
    assert_eq!(tags.len(), 1);

    // Replace with different tag
    set_message_tags(&conn, "m1", &[tag2.id.clone()]).expect("replace tags");
    let tags = get_message_tags(&conn, "m1").expect("get tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, tag2.id);
}

#[test]
fn set_message_tags_supports_multiple_tags() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag1 = create_tag(&conn, "Tag 1", "#ff0000").expect("tag 1");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let tag2 = create_tag(&conn, "Tag 2", "#00ff00").expect("tag 2");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let tag3 = create_tag(&conn, "Tag 3", "#0000ff").expect("tag 3");

    set_message_tags(&conn, "m1", &[tag1.id.clone(), tag2.id.clone(), tag3.id.clone()])
        .expect("set tags");

    let tags = get_message_tags(&conn, "m1").expect("get tags");
    assert_eq!(tags.len(), 3);
}

#[test]
fn get_message_tags_returns_ordered_by_display_order() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag1 = create_tag(&conn, "Tag 1", "#ff0000").expect("tag 1");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let tag2 = create_tag(&conn, "Tag 2", "#00ff00").expect("tag 2");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let tag3 = create_tag(&conn, "Tag 3", "#0000ff").expect("tag 3");

    set_message_tags(&conn, "m1", &[tag3.id.clone(), tag1.id.clone(), tag2.id.clone()])
        .expect("set tags");

    let tags = get_message_tags(&conn, "m1").expect("get tags");
    assert_eq!(tags[0].name, "Tag 1");
    assert_eq!(tags[1].name, "Tag 2");
    assert_eq!(tags[2].name, "Tag 3");
}

#[test]
fn list_scrapbook_messages_returns_tagged_messages() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag = create_tag(&conn, "Important", "#ff0000").expect("create tag");

    // Tag messages m1 and m3
    set_message_tags(&conn, "m1", &[tag.id.clone()]).expect("tag m1");
    set_message_tags(&conn, "m3", &[tag.id.clone()]).expect("tag m3");

    let scrapbook = list_scrapbook_messages(&conn, &tag.id, None, None, 10)
        .expect("list scrapbook");

    assert_eq!(scrapbook.len(), 2);
    assert_eq!(scrapbook[0].message.id, "m3"); // Newest first
    assert_eq!(scrapbook[1].message.id, "m1");
}

#[test]
fn list_scrapbook_messages_includes_thread_name() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag = create_tag(&conn, "Test", "#ff0000").expect("create tag");
    set_message_tags(&conn, "m1", &[tag.id.clone()]).expect("tag m1");

    let scrapbook = list_scrapbook_messages(&conn, &tag.id, None, None, 10)
        .expect("list scrapbook");

    assert_eq!(scrapbook[0].thread_name, Some("Test Thread".to_string()));
}

#[test]
fn list_scrapbook_messages_detects_discontinuity() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag = create_tag(&conn, "Test", "#ff0000").expect("create tag");

    // Tag m1 and m2, but there's m_between (timestamp 3) that's not tagged
    set_message_tags(&conn, "m1", &[tag.id.clone()]).expect("tag m1");
    std::thread::sleep(std::time::Duration::from_millis(2));
    set_message_tags(&conn, "m2", &[tag.id.clone()]).expect("tag m2");

    let scrapbook = list_scrapbook_messages(&conn, &tag.id, None, None, 10)
        .expect("list scrapbook");

    // Results ordered by tagged_at DESC, so m2 comes first
    // Discontinuity is detected when processing each message against the previous one
    // scrapbook[1] (m1) is compared to scrapbook[0] (m2) and found to have m_between in between
    assert_eq!(scrapbook.len(), 2);
    assert_eq!(scrapbook[0].message.id, "m2"); // Newest first by tagged_at
    assert_eq!(scrapbook[0].is_discontinuous, false, "First message in results never discontinuous");
    assert_eq!(scrapbook[1].message.id, "m1");
    assert_eq!(scrapbook[1].is_discontinuous, true, "m1 should be discontinuous from m2 (m_between exists in between)");
}

#[test]
fn list_scrapbook_messages_no_discontinuity_for_adjacent() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag = create_tag(&conn, "Test", "#ff0000").expect("create tag");

    // Tag m2 and m3 which are adjacent (timestamps 5 and 10, nothing in between)
    set_message_tags(&conn, "m2", &[tag.id.clone()]).expect("tag m2");
    set_message_tags(&conn, "m3", &[tag.id.clone()]).expect("tag m3");

    let scrapbook = list_scrapbook_messages(&conn, &tag.id, None, None, 10)
        .expect("list scrapbook");

    assert_eq!(scrapbook.len(), 2);
    assert_eq!(scrapbook[0].message.id, "m3");
    assert_eq!(scrapbook[0].is_discontinuous, false, "m3 is adjacent to m2");
    assert_eq!(scrapbook[1].message.id, "m2");
}

#[test]
fn list_scrapbook_messages_paginates_with_tagged_at() {
    let conn = setup_db();
    seed_test_data(&conn);

    let tag = create_tag(&conn, "Test", "#ff0000").expect("create tag");
    set_message_tags(&conn, "m1", &[tag.id.clone()]).expect("tag m1");
    set_message_tags(&conn, "m2", &[tag.id.clone()]).expect("tag m2");
    set_message_tags(&conn, "m3", &[tag.id.clone()]).expect("tag m3");

    // Get first page (limit 2)
    let page1 = list_scrapbook_messages(&conn, &tag.id, None, None, 2)
        .expect("page 1");
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].message.id, "m3"); // Newest by tagged_at
    assert_eq!(page1[1].message.id, "m2");

    // Get next page using last message's tagged_at
    let tagged_at = conn
        .query_row(
            "SELECT tagged_at FROM message_tags WHERE message_id = ?1 AND tag_id = ?2",
            rusqlite::params!["m2", &tag.id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();

    let page2 = list_scrapbook_messages(&conn, &tag.id, Some(tagged_at), Some("m2"), 2)
        .expect("page 2");
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].message.id, "m1");
}
