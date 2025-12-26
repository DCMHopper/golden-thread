use golden_thread_core::{db::apply_migrations, seed::seed_demo};
use golden_thread_core::query::{list_messages, list_threads, search_messages};
use rusqlite::Connection;

#[test]
fn demo_seed_query_roundtrip() {
    let conn = Connection::open_in_memory().expect("memory db");
    apply_migrations(&conn).expect("migrate");
    seed_demo(&conn, 2, 0).expect("seed");

    let threads = list_threads(&conn, 50, 0).expect("threads");
    assert_eq!(threads.len(), 1);

    let messages = list_messages(&conn, "t1", None, None, 50).expect("messages");
    assert_eq!(messages.len(), 2);

    let hits = search_messages(&conn, "Demo", None, 10, 0).expect("search");
    assert_eq!(hits.len(), 1);
}
