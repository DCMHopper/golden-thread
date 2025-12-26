use std::time::Instant;

use crate::error::CoreError;

pub(super) fn build_message_fts<F>(tx: &rusqlite::Transaction, progress: &F) -> Result<(), CoreError>
where
    F: Fn(&str),
{
    progress("Building search index...");
    let build_start = Instant::now();
    tx.execute("DELETE FROM message_fts;", [])?;

    let total: i64 = tx
        .query_row(
            "SELECT COUNT(1) FROM messages WHERE body IS NOT NULL AND length(trim(body)) > 0;",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let max_rowid: i64 = tx
        .query_row("SELECT COALESCE(MAX(rowid), 0) FROM messages;", [], |row| row.get(0))
        .unwrap_or(0);

    let mut inserted: i64 = 0;
    let batch_size: i64 = 50_000;
    let mut start: i64 = 0;
    while start < max_rowid {
        let end = start + batch_size;
        tx.execute(
            "INSERT INTO message_fts (message_id, thread_id, sender_id, body)
             SELECT id, thread_id, sender_id, body
             FROM messages
             WHERE rowid > ?1 AND rowid <= ?2
               AND body IS NOT NULL AND length(trim(body)) > 0;",
            rusqlite::params![start, end],
        )?;
        inserted += tx.changes() as i64;
        if total > 0 {
            let msg = format!("Building search index... {}/{}", inserted, total);
            progress(&msg);
        }
        start = end;
    }

    let build_secs = build_start.elapsed().as_secs_f32();
    progress(&format!("Search index built in {:.1}s", build_secs));

    let optimize_start = Instant::now();
    tx.execute("INSERT INTO message_fts(message_fts) VALUES('optimize');", [])?;
    let optimize_secs = optimize_start.elapsed().as_secs_f32();
    progress(&format!("Search index optimized in {:.1}s", optimize_secs));

    Ok(())
}
