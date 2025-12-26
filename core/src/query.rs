use rusqlite::{params, Connection, OptionalExtension};

use crate::error::CoreError;
use crate::models::{ArchiveStats, MediaRow, MessageRow, ReactionSummary, ScrapbookMessage, SearchHit, Tag, ThreadMediaRow, ThreadSummary, MessageTags};

pub fn list_threads(conn: &Connection, limit: i64, offset: i64) -> Result<Vec<ThreadSummary>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.last_message_at, \
         (SELECT COUNT(1) FROM messages m WHERE m.thread_id = t.id) AS message_count \
         FROM threads t \
         ORDER BY t.last_message_at DESC NULLS LAST, t.id ASC \
         LIMIT ?1 OFFSET ?2;",
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(ThreadSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            last_message_at: row.get(2)?,
            message_count: row.get(3)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn list_messages(
    conn: &Connection,
    thread_id: &str,
    before_ts: Option<i64>,
    before_id: Option<&str>,
    limit: i64,
) -> Result<Vec<MessageRow>, CoreError> {
    let (sql, params_vec): (String, Vec<rusqlite::types::Value>) = match (before_ts, before_id) {
        (Some(ts), Some(id)) => (
            "SELECT id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, \
                    quote_message_id, metadata_json \
             FROM messages \
             WHERE thread_id = ?1 AND (sort_ts < ?2 \
               OR (sort_ts = ?2 AND id < ?3)) \
             ORDER BY sort_ts DESC, id DESC \
             LIMIT ?4;"
                .to_string(),
            vec![
                thread_id.to_string().into(),
                ts.into(),
                id.to_string().into(),
                limit.into(),
            ],
        ),
        (Some(ts), None) => (
            "SELECT id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, \
                    quote_message_id, metadata_json \
             FROM messages \
             WHERE thread_id = ?1 AND sort_ts < ?2 \
             ORDER BY sort_ts DESC, id DESC \
             LIMIT ?3;"
                .to_string(),
            vec![thread_id.to_string().into(), ts.into(), limit.into()],
        ),
        (None, _) => (
            "SELECT id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, \
                    quote_message_id, metadata_json \
             FROM messages \
             WHERE thread_id = ?1 \
             ORDER BY sort_ts DESC, id DESC \
             LIMIT ?2;"
                .to_string(),
            vec![thread_id.to_string().into(), limit.into()],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
        Ok(MessageRow {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            sender_id: row.get(2)?,
            sent_at: row.get(3)?,
            received_at: row.get(4)?,
            message_type: row.get(5)?,
            body: row.get(6)?,
            is_outgoing: row.get::<_, i64>(7)? != 0,
            is_view_once: row.get::<_, i64>(8)? != 0,
            quote_message_id: row.get(9)?,
            metadata_json: row.get(10)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn list_messages_after(
    conn: &Connection,
    thread_id: &str,
    after_ts: i64,
    after_id: Option<&str>,
    limit: i64,
) -> Result<Vec<MessageRow>, CoreError> {
    let (sql, params_vec): (String, Vec<rusqlite::types::Value>) = match after_id {
        Some(id) => (
            "SELECT id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, \
                    quote_message_id, metadata_json \
             FROM messages \
             WHERE thread_id = ?1 AND (sort_ts > ?2 \
               OR (sort_ts = ?2 AND id > ?3)) \
             ORDER BY sort_ts ASC, id ASC \
             LIMIT ?4;"
                .to_string(),
            vec![
                thread_id.to_string().into(),
                after_ts.into(),
                id.to_string().into(),
                limit.into(),
            ],
        ),
        None => (
            "SELECT id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, \
                    quote_message_id, metadata_json \
             FROM messages \
             WHERE thread_id = ?1 AND sort_ts > ?2 \
             ORDER BY sort_ts ASC, id ASC \
             LIMIT ?3;"
                .to_string(),
            vec![thread_id.to_string().into(), after_ts.into(), limit.into()],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
        Ok(MessageRow {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            sender_id: row.get(2)?,
            sent_at: row.get(3)?,
            received_at: row.get(4)?,
            message_type: row.get(5)?,
            body: row.get(6)?,
            is_outgoing: row.get::<_, i64>(7)? != 0,
            is_view_once: row.get::<_, i64>(8)? != 0,
            quote_message_id: row.get(9)?,
            metadata_json: row.get(10)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn get_message(conn: &Connection, message_id: &str) -> Result<MessageRow, CoreError> {
    conn.query_row(
        "SELECT id, thread_id, sender_id, sent_at, received_at, type, body, is_outgoing, is_view_once, \
                quote_message_id, metadata_json \
         FROM messages \
         WHERE id = ?1;",
        params![message_id],
        |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                sender_id: row.get(2)?,
                sent_at: row.get(3)?,
                received_at: row.get(4)?,
                message_type: row.get(5)?,
                body: row.get(6)?,
                is_outgoing: row.get::<_, i64>(7)? != 0,
                is_view_once: row.get::<_, i64>(8)? != 0,
                quote_message_id: row.get(9)?,
                metadata_json: row.get(10)?,
            })
        },
    )
    .map_err(CoreError::from)
}

pub fn list_messages_around(
    conn: &Connection,
    message_id: &str,
    before: i64,
    after: i64,
) -> Result<Vec<MessageRow>, CoreError> {
    let center = get_message(conn, message_id)?;
    let center_ts = center.sent_at.or(center.received_at).unwrap_or(0);
    let mut older = list_messages(conn, &center.thread_id, Some(center_ts), Some(&center.id), before)?;
    older.reverse();
    let newer = list_messages_after(conn, &center.thread_id, center_ts, Some(&center.id), after)?;
    let mut result = Vec::with_capacity(older.len() + 1 + newer.len());
    result.extend(older);
    result.push(center);
    result.extend(newer);
    Ok(result)
}

pub fn search_messages(
    conn: &Connection,
    query: &str,
    thread_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SearchHit>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.thread_id, m.sender_id, m.sent_at, m.received_at, m.type, m.body, \
                m.is_outgoing, m.is_view_once, m.quote_message_id, m.metadata_json, \
                bm25(message_fts) AS rank \
         FROM message_fts \
         JOIN messages m ON m.id = message_fts.message_id \
         WHERE message_fts MATCH ?1 AND (?2 IS NULL OR m.thread_id = ?2) \
         ORDER BY rank \
         LIMIT ?3 OFFSET ?4;",
    )?;
    let rows = stmt.query_map(params![query, thread_id, limit, offset], |row| {
        let message = MessageRow {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            sender_id: row.get(2)?,
            sent_at: row.get(3)?,
            received_at: row.get(4)?,
            message_type: row.get(5)?,
            body: row.get(6)?,
            is_outgoing: row.get::<_, i64>(7)? != 0,
            is_view_once: row.get::<_, i64>(8)? != 0,
            quote_message_id: row.get(9)?,
            metadata_json: row.get(10)?,
        };
        Ok(SearchHit {
            message,
            rank: row.get(11)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn list_reactions_for_messages(
    conn: &Connection,
    message_ids: &[String],
) -> Result<Vec<ReactionSummary>, CoreError> {
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut placeholders = String::new();
    for (idx, _) in message_ids.iter().enumerate() {
        if idx > 0 {
            placeholders.push(',');
        }
        placeholders.push('?');
    }
    let sql = format!(
        "SELECT message_id, emoji, COUNT(1) \
         FROM reactions \
         WHERE message_id IN ({}) \
         GROUP BY message_id, emoji;",
        placeholders
    );
    let params_vec: Vec<rusqlite::types::Value> =
        message_ids.iter().cloned().map(|v| v.into()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
        Ok(ReactionSummary {
            message_id: row.get(0)?,
            emoji: row.get(1)?,
            count: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn list_media(
    conn: &Connection,
    thread_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<MediaRow>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.message_id, a.sha256, a.mime, a.size_bytes, a.original_filename, \
                a.kind, a.width, a.height, a.duration_ms \
         FROM attachments a \
         JOIN messages m ON m.id = a.message_id \
         WHERE (?1 IS NULL OR m.thread_id = ?1) \
         ORDER BY m.sent_at DESC NULLS LAST, a.id ASC \
         LIMIT ?2 OFFSET ?3;",
    )?;
    let rows = stmt.query_map(params![thread_id, limit, offset], |row| {
        Ok(MediaRow {
            id: row.get(0)?,
            message_id: row.get(1)?,
            sha256: row.get(2)?,
            mime: row.get(3)?,
            size_bytes: row.get(4)?,
            original_filename: row.get(5)?,
            kind: row.get(6)?,
            width: row.get(7)?,
            height: row.get(8)?,
            duration_ms: row.get(9)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn list_thread_media(
    conn: &Connection,
    thread_id: &str,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    size_bucket: Option<i64>,
    sort: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<ThreadMediaRow>, CoreError> {
    let mut where_clauses = vec!["m.thread_id = ?1".to_string()];
    let mut params: Vec<rusqlite::types::Value> = vec![thread_id.to_string().into()];
    let mut next_idx = 2;

    if let Some(from_ts) = from_ts {
        where_clauses.push(format!("COALESCE(m.sent_at, m.received_at, 0) >= ?{}", next_idx));
        params.push(from_ts.into());
        next_idx += 1;
    }
    if let Some(to_ts) = to_ts {
        where_clauses.push(format!("COALESCE(m.sent_at, m.received_at, 0) <= ?{}", next_idx));
        params.push(to_ts.into());
        next_idx += 1;
    }
    if let Some(size_bucket) = size_bucket {
        where_clauses.push(format!("a.size_bucket = ?{}", next_idx));
        params.push(size_bucket.into());
        next_idx += 1;
    }

    let order_by = match sort {
        "size_asc" => "IFNULL(a.size_bytes, 0) ASC, COALESCE(m.sent_at, m.received_at, 0) DESC",
        "size_desc" => "IFNULL(a.size_bytes, 0) DESC, COALESCE(m.sent_at, m.received_at, 0) DESC",
        "date_asc" => "COALESCE(m.sent_at, m.received_at, 0) ASC, a.id ASC",
        _ => "COALESCE(m.sent_at, m.received_at, 0) DESC, a.id ASC",
    };

    let sql = format!(
        "SELECT a.id, a.message_id, m.thread_id, a.sha256, a.mime, a.size_bytes, a.original_filename, \
                a.kind, a.width, a.height, a.duration_ms, m.sent_at, m.received_at \
         FROM attachments a \
         JOIN messages m ON m.id = a.message_id \
         WHERE {} \
         ORDER BY {} \
         LIMIT ?{} OFFSET ?{};",
        where_clauses.join(" AND "),
        order_by,
        next_idx,
        next_idx + 1
    );
    params.push(limit.into());
    params.push(offset.into());

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(ThreadMediaRow {
            id: row.get(0)?,
            message_id: row.get(1)?,
            thread_id: row.get(2)?,
            sha256: row.get(3)?,
            mime: row.get(4)?,
            size_bytes: row.get(5)?,
            original_filename: row.get(6)?,
            kind: row.get(7)?,
            width: row.get(8)?,
            height: row.get(9)?,
            duration_ms: row.get(10)?,
            sent_at: row.get(11)?,
            received_at: row.get(12)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn list_attachments_for_message(
    conn: &Connection,
    message_id: &str,
) -> Result<Vec<MediaRow>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, message_id, sha256, mime, size_bytes, original_filename, \
                kind, width, height, duration_ms \
         FROM attachments \
         WHERE message_id = ?1 \
         ORDER BY id ASC;",
    )?;
    let rows = stmt.query_map(params![message_id], |row| {
        Ok(MediaRow {
            id: row.get(0)?,
            message_id: row.get(1)?,
            sha256: row.get(2)?,
            mime: row.get(3)?,
            size_bytes: row.get(4)?,
            original_filename: row.get(5)?,
            kind: row.get(6)?,
            width: row.get(7)?,
            height: row.get(8)?,
            duration_ms: row.get(9)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn thread_exists(conn: &Connection, thread_id: &str) -> Result<bool, CoreError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM threads WHERE id = ?1 LIMIT 1;",
            params![thread_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}

pub fn archive_stats(conn: &Connection) -> Result<ArchiveStats, CoreError> {
    let threads: i64 = conn.query_row("SELECT COUNT(1) FROM threads;", [], |row| row.get(0))?;
    let messages: i64 = conn.query_row("SELECT COUNT(1) FROM messages;", [], |row| row.get(0))?;
    let recipients: i64 = conn.query_row("SELECT COUNT(1) FROM recipients;", [], |row| row.get(0))?;
    let attachments: i64 = conn.query_row("SELECT COUNT(1) FROM attachments;", [], |row| row.get(0))?;
    Ok(ArchiveStats {
        threads,
        messages,
        recipients,
        attachments,
    })
}

// ===== Tag Management Functions =====

pub fn list_tags(conn: &Connection) -> Result<Vec<Tag>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at, display_order \
         FROM tags \
         ORDER BY display_order ASC, created_at ASC;"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
            display_order: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn create_tag(conn: &Connection, name: &str, color: &str) -> Result<Tag, CoreError> {
    // Generate a simple ID (timestamp-based)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let id = format!("tag:{}", now);

    // Get next display order
    let max_order: Option<i64> = conn
        .query_row("SELECT MAX(display_order) FROM tags;", [], |row| row.get(0))
        .optional()?
        .flatten();
    let display_order = max_order.unwrap_or(-1) + 1;

    conn.execute(
        "INSERT INTO tags (id, name, color, created_at, display_order) VALUES (?1, ?2, ?3, ?4, ?5);",
        params![&id, name, color, now, display_order],
    )?;

    Ok(Tag {
        id,
        name: name.to_string(),
        color: color.to_string(),
        created_at: now,
        display_order,
    })
}

pub fn update_tag(conn: &Connection, id: &str, name: &str, color: &str) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE tags SET name = ?1, color = ?2 WHERE id = ?3;",
        params![name, color, id],
    )?;
    Ok(())
}

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), CoreError> {
    // CASCADE DELETE will handle message_tags cleanup
    conn.execute("DELETE FROM tags WHERE id = ?1;", params![id])?;
    Ok(())
}

pub fn get_message_tags(conn: &Connection, message_id: &str) -> Result<Vec<Tag>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.created_at, t.display_order \
         FROM tags t \
         JOIN message_tags mt ON mt.tag_id = t.id \
         WHERE mt.message_id = ?1 \
         ORDER BY t.display_order ASC, t.created_at ASC;"
    )?;
    let rows = stmt.query_map(params![message_id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
            display_order: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn get_message_tags_bulk(conn: &Connection, message_ids: &[String]) -> Result<Vec<MessageTags>, CoreError> {
    if message_ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders = std::iter::repeat("?")
        .take(message_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT mt.message_id, t.id, t.name, t.color, t.created_at, t.display_order \
         FROM message_tags mt \
         JOIN tags t ON mt.tag_id = t.id \
         WHERE mt.message_id IN ({}) \
         ORDER BY mt.message_id ASC, t.display_order ASC, t.created_at ASC;",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(message_ids.iter()), |row| {
        Ok((
            row.get::<_, String>(0)?,
            Tag {
                id: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
                created_at: row.get(4)?,
                display_order: row.get(5)?,
            },
        ))
    })?;

    let mut map: std::collections::HashMap<String, Vec<Tag>> = std::collections::HashMap::new();
    for row in rows {
        let (message_id, tag) = row?;
        map.entry(message_id).or_default().push(tag);
    }

    let mut result = Vec::with_capacity(message_ids.len());
    for message_id in message_ids {
        let tags = map.remove(message_id).unwrap_or_default();
        result.push(MessageTags {
            message_id: message_id.clone(),
            tags,
        });
    }
    Ok(result)
}

pub fn set_message_tags(conn: &Connection, message_id: &str, tag_ids: &[String]) -> Result<(), CoreError> {
    // Delete existing tags for this message
    conn.execute("DELETE FROM message_tags WHERE message_id = ?1;", params![message_id])?;

    // Insert new tags
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    for tag_id in tag_ids {
        conn.execute(
            "INSERT INTO message_tags (message_id, tag_id, tagged_at) VALUES (?1, ?2, ?3);",
            params![message_id, tag_id, now],
        )?;
    }

    Ok(())
}

// ===== Scrapbook Functions =====

/// Checks if two messages are adjacent (consecutive) in their thread's timeline.
///
/// Returns `true` if there are no other messages between the two messages in the same thread.
/// This is used for discontinuity detection in the scrapbook view.
///
/// # Algorithm
///
/// Given two message IDs (earlier and later based on sent_at/received_at timestamps):
/// 1. Verify both messages exist and are in the same thread
/// 2. Check if any message m3 exists in the same thread where:
///    - m3's timestamp > earlier message's timestamp
///    - m3's timestamp < later message's timestamp
/// 3. If no such message exists (count == 0), the messages are adjacent
///
/// # Timestamp Handling
///
/// - Uses `COALESCE(sent_at, received_at, 0)` to handle messages with only one timestamp
/// - When timestamps are equal, uses message ID for deterministic ordering
///
/// # Example
///
/// ```text
/// Thread timeline: [msg1 (t=1), msg2 (t=5), msg3 (t=10)]
/// are_messages_adjacent(msg1, msg2)? -> true (no messages between)
/// are_messages_adjacent(msg1, msg3)? -> false (msg2 exists between them)
/// ```
fn are_messages_adjacent(conn: &Connection, earlier_id: &str, later_id: &str) -> Result<bool, CoreError> {
    // Count messages in the same thread that fall between the two given messages
    let count: i64 = conn.query_row(
        "SELECT COUNT(1) FROM messages m1, messages m2
         WHERE m1.id = ?1 AND m2.id = ?2 AND m1.thread_id = m2.thread_id
         AND EXISTS (
           SELECT 1 FROM messages m3 WHERE m3.thread_id = m1.thread_id
           AND (COALESCE(m3.sent_at, m3.received_at, 0) > COALESCE(m1.sent_at, m1.received_at, 0)
                OR (COALESCE(m3.sent_at, m3.received_at, 0) = COALESCE(m1.sent_at, m1.received_at, 0)
                    AND m3.id > m1.id))
           AND (COALESCE(m3.sent_at, m3.received_at, 0) < COALESCE(m2.sent_at, m2.received_at, 0)
                OR (COALESCE(m3.sent_at, m3.received_at, 0) = COALESCE(m2.sent_at, m2.received_at, 0)
                    AND m3.id < m2.id))
         )",
        params![earlier_id, later_id],
        |row| row.get(0),
    )?;
    Ok(count == 0)
}

/// Lists messages tagged with a specific tag, ordered by when they were tagged (newest first).
///
/// # Scrapbook View
///
/// This function powers the Scrapbook tab, which shows a cross-thread view of all messages
/// tagged with a specific tag. Messages are ordered by `tagged_at DESC` (when the tag was
/// applied), not by message timestamp.
///
/// # Pagination
///
/// Uses cursor-based pagination with `tagged_at` timestamp and message ID:
/// - `before_ts`: Only return messages tagged before this timestamp
/// - `before_id`: When timestamp matches, only return messages with ID < this ID
/// - `limit`: Maximum number of messages to return
///
/// # Discontinuity Detection
///
/// Each message includes an `is_discontinuous` flag that indicates whether there are
/// untagged messages between it and the previous message in the same thread. This helps
/// users understand the original conversation context.
///
/// # Thread Names
///
/// Includes the thread name for each message to help identify which conversation it's from
/// in the cross-thread view.
///
/// # Example
///
/// ```rust,ignore
/// // Get first page of messages for a tag
/// let messages = list_scrapbook_messages(&conn, "tag:123", None, None, 50)?;
///
/// // Get next page
/// let last_msg = messages.last().unwrap();
/// let tagged_at = /* get from message_tags */;
/// let next_page = list_scrapbook_messages(&conn, "tag:123", Some(tagged_at), Some(&last_msg.message.id), 50)?;
/// ```
pub fn list_scrapbook_messages(
    conn: &Connection,
    tag_id: &str,
    before_ts: Option<i64>,
    before_id: Option<&str>,
    limit: i64,
) -> Result<Vec<ScrapbookMessage>, CoreError> {
    // Build query to get tagged messages with thread names
    let (sql, params_vec): (String, Vec<rusqlite::types::Value>) = match (before_ts, before_id) {
        (Some(ts), Some(id)) => (
            "SELECT m.id, m.thread_id, m.sender_id, m.sent_at, m.received_at, m.type, m.body, \
                    m.is_outgoing, m.is_view_once, m.quote_message_id, m.metadata_json, t.name \
             FROM messages m \
             JOIN message_tags mt ON mt.message_id = m.id \
             JOIN threads t ON t.id = m.thread_id \
             WHERE mt.tag_id = ?1 AND (mt.tagged_at < ?2 \
               OR (mt.tagged_at = ?2 AND m.id < ?3)) \
             ORDER BY mt.tagged_at DESC, m.id DESC \
             LIMIT ?4;"
                .to_string(),
            vec![
                tag_id.to_string().into(),
                ts.into(),
                id.to_string().into(),
                limit.into(),
            ],
        ),
        (Some(ts), None) => (
            "SELECT m.id, m.thread_id, m.sender_id, m.sent_at, m.received_at, m.type, m.body, \
                    m.is_outgoing, m.is_view_once, m.quote_message_id, m.metadata_json, t.name \
             FROM messages m \
             JOIN message_tags mt ON mt.message_id = m.id \
             JOIN threads t ON t.id = m.thread_id \
             WHERE mt.tag_id = ?1 AND mt.tagged_at < ?2 \
             ORDER BY mt.tagged_at DESC, m.id DESC \
             LIMIT ?3;"
                .to_string(),
            vec![tag_id.to_string().into(), ts.into(), limit.into()],
        ),
        (None, _) => (
            "SELECT m.id, m.thread_id, m.sender_id, m.sent_at, m.received_at, m.type, m.body, \
                    m.is_outgoing, m.is_view_once, m.quote_message_id, m.metadata_json, t.name \
             FROM messages m \
             JOIN message_tags mt ON mt.message_id = m.id \
             JOIN threads t ON t.id = m.thread_id \
             WHERE mt.tag_id = ?1 \
             ORDER BY mt.tagged_at DESC, m.id DESC \
             LIMIT ?2;"
                .to_string(),
            vec![tag_id.to_string().into(), limit.into()],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let messages_with_threads: Vec<(MessageRow, Option<String>)> = stmt
        .query_map(rusqlite::params_from_iter(params_vec), |row| {
            let message = MessageRow {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                sender_id: row.get(2)?,
                sent_at: row.get(3)?,
                received_at: row.get(4)?,
                message_type: row.get(5)?,
                body: row.get(6)?,
                is_outgoing: row.get::<_, i64>(7)? != 0,
                is_view_once: row.get::<_, i64>(8)? != 0,
                quote_message_id: row.get(9)?,
                metadata_json: row.get(10)?,
            };
            let thread_name: Option<String> = row.get(11)?;
            Ok((message, thread_name))
        })?
        .filter_map(Result::ok)
        .collect();

    // Helper to get message sort timestamp (same logic as frontend/backend ordering)
    let msg_ts = |msg: &MessageRow| msg.sent_at.or(msg.received_at).unwrap_or(0);

    // Discontinuity Detection:
    //
    // The scrapbook results are ordered by `tagged_at DESC` (when the tag was applied),
    // but discontinuity must be detected based on the message timeline (sent_at/received_at).
    //
    // For each pair of consecutive messages in the result set that belong to the same thread,
    // we check if there are any messages between them in the original conversation timeline.
    //
    // Example:
    // - Thread has messages: [m1(t=1), m2(t=5), m3(t=10), m4(t=15)]
    // - User tags m1 and m3 (skipping m2)
    // - Scrapbook shows: [m3, m1] (ordered by tagged_at)
    // - When processing m1 (index 1), we compare it to m3 (index 0)
    // - We find m2 exists between them -> m1 is marked discontinuous
    // - The "⋯" indicator will show above m1 in the UI
    let mut result = Vec::new();
    for (i, (message, thread_name)) in messages_with_threads.iter().enumerate() {
        let is_discontinuous = if i > 0 {
            let prev_message = &messages_with_threads[i - 1].0;
            // Only check discontinuity if same thread (cross-thread gaps don't need indicators)
            if prev_message.thread_id == message.thread_id {
                // Determine which message is earlier in the MESSAGE timeline
                // (not tagged_at, which is how results are ordered)
                let prev_ts = msg_ts(prev_message);
                let curr_ts = msg_ts(message);
                let (earlier_id, later_id) = if curr_ts < prev_ts {
                    (&message.id, &prev_message.id)
                } else {
                    (&prev_message.id, &message.id)
                };
                // Check if there are messages between them in the original thread
                !are_messages_adjacent(conn, earlier_id, later_id)?
            } else {
                false // Different thread, no discontinuity indicator
            }
        } else {
            false // First message in results, never discontinuous
        };

        result.push(ScrapbookMessage {
            message: message.clone(),
            thread_name: thread_name.clone(),
            is_discontinuous,
        });
    }

    Ok(result)
}
