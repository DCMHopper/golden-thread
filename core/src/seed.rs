use rusqlite::{params, Connection};

use crate::error::CoreError;

pub fn seed_demo(conn: &Connection, primary_count: i64, secondary_threads: i64) -> Result<(), CoreError> {
    conn.execute_batch("BEGIN;")?;
    let result = (|| -> Result<(), CoreError> {
        conn.execute(
            "INSERT OR IGNORE INTO recipients (id, profile_name) VALUES (?1, ?2);",
            params!["r1", "You"],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO recipients (id, profile_name) VALUES (?1, ?2);",
            params!["r2", "Partner"],
        )?;

        let base_ts = 1_700_000_000i64;
        let last_primary_ts = base_ts + primary_count.saturating_sub(1) * 60;
        conn.execute(
            "INSERT OR IGNORE INTO threads (id, name, last_message_at) VALUES (?1, ?2, ?3);",
            params!["t1", "Chat with Partner", last_primary_ts],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO thread_members (thread_id, recipient_id) VALUES (?1, ?2);",
            params!["t1", "r1"],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO thread_members (thread_id, recipient_id) VALUES (?1, ?2);",
            params!["t1", "r2"],
        )?;

        let mut msg_stmt = conn.prepare(
            "INSERT OR IGNORE INTO messages \
             (id, thread_id, sender_id, sent_at, sort_ts, type, body, is_outgoing, quote_message_id, metadata_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
        )?;
        let mut fts_stmt = conn.prepare(
            "INSERT OR IGNORE INTO message_fts (message_id, thread_id, sender_id, body) \
             VALUES (?1, ?2, ?3, ?4);",
        )?;
        let mut reaction_stmt = conn.prepare(
            "INSERT OR IGNORE INTO reactions (message_id, reactor_id, emoji, reacted_at) \
             VALUES (?1, ?2, ?3, ?4);",
        )?;

        for idx in 0..primary_count {
            let id = format!("demo:m{}", idx + 1);
            let ts = base_ts + idx * 60;
            let (sender, outgoing, body) = if idx % 2 == 0 {
                ("r1", 1i64, format!("Demo message {}", idx + 1))
            } else {
                ("r2", 0i64, format!("Reply {}", idx + 1))
            };
            let (quote_id, metadata_json) = if (idx + 1) % 10 == 0 {
                let target = (idx + 1) / 10;
                if target >= 1 {
                    let quote_id = format!("demo:m{}", target);
                    let quoted_body = if target % 2 == 1 {
                        format!("Demo message {}", target)
                    } else {
                        format!("Reply {}", target)
                    };
                    let metadata_json = format!(r#"{{"quote_body":"{}"}}"#, quoted_body);
                    (Some(quote_id), Some(metadata_json))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            msg_stmt.execute(params![id, "t1", sender, ts, ts, "text", body, outgoing, quote_id, metadata_json])?;
            fts_stmt.execute(params![id, "t1", sender, body])?;
            if (idx + 1) % 4 == 0 {
                reaction_stmt.execute(params![id, "r2", "👍", ts + 5])?;
            }
        }

        for idx in 0..secondary_threads {
            let thread_id = format!("t{}", idx + 2);
            let name = format!("Secondary {}", idx + 1);
            let ts = base_ts + idx * 120;
            conn.execute(
                "INSERT OR IGNORE INTO threads (id, name, last_message_at) VALUES (?1, ?2, ?3);",
                params![thread_id, name, ts],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO thread_members (thread_id, recipient_id) VALUES (?1, ?2);",
                params![thread_id, "r1"],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO thread_members (thread_id, recipient_id) VALUES (?1, ?2);",
                params![thread_id, "r2"],
            )?;
            let msg_id = format!("demo:s{}", idx + 1);
            let body = format!("Short thread {}", idx + 1);
            msg_stmt.execute(params![
                msg_id,
                thread_id,
                "r2",
                ts,
                ts,
                "text",
                body,
                0i64,
                Option::<String>::None,
                Option::<String>::None,
            ])?;
            fts_stmt.execute(params![msg_id, thread_id, "r2", body])?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(err)
        }
    }
}
