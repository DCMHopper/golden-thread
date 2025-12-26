use std::fs;
use std::path::Path;
use crate::error::CoreError;
use crate::ffi::signalbackup;
use crate::{db::open_archive};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use chrono::Utc;
use libc::statvfs;

#[path = "importer/attachments.rs"]
mod attachments;
#[path = "importer/fts.rs"]
mod fts;
use rusqlite::types::Value;

#[derive(Debug, Clone)]
pub struct ImportPlan {
    pub source_path: String,
    pub normalized_passphrase: String,
    pub source_filename: String,
    pub source_hash: String,
}

pub fn normalize_passphrase(raw: &str) -> Result<String, CoreError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidPassphrase("passphrase is empty".to_string()));
    }
    let normalized: String = trimmed.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if normalized.len() != 30 {
        return Err(CoreError::InvalidPassphrase(
            "passphrase must be 30 digits".to_string(),
        ));
    }
    if !normalized.chars().all(|c| c.is_ascii_digit()) {
        return Err(CoreError::InvalidPassphrase(
            "passphrase must contain only digits".to_string(),
        ));
    }
    Ok(normalized)
}

pub fn plan_import(source_path: &Path, passphrase: &str) -> Result<ImportPlan, CoreError> {
    plan_import_with_progress(source_path, passphrase, |_| {})
}

pub fn plan_import_with_progress<F>(
    source_path: &Path,
    passphrase: &str,
    progress: F,
) -> Result<ImportPlan, CoreError>
where
    F: Fn(&str),
{
    let normalized = normalize_passphrase(passphrase)?;
    if !source_path.exists() {
        return Err(CoreError::InvalidArgument("backup file not found".to_string()));
    }
    if source_path.extension().and_then(|s| s.to_str()) != Some("backup") {
        return Err(CoreError::InvalidArgument("file must have .backup extension".to_string()));
    }
    let metadata = fs::metadata(source_path)
        .map_err(|e| CoreError::InvalidArgument(e.to_string()))?;
    if metadata.len() == 0 {
        return Err(CoreError::InvalidArgument("backup file is empty".to_string()));
    }
    progress("Preparing import...");
    let source_hash = hash_file_sha256_with_progress(source_path, &progress)?;
    let source_filename = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("backup.backup")
        .to_string();
    Ok(ImportPlan {
        source_path: source_path.display().to_string(),
        normalized_passphrase: normalized,
        source_filename,
        source_hash,
    })
}

fn hash_file_sha256_with_progress<F>(path: &Path, progress: F) -> Result<String, CoreError>
where
    F: Fn(&str),
{
    let mut file = fs::File::open(path)
        .map_err(|e| CoreError::InvalidArgument(format!("backup open failed: {}", e)))?;
    let metadata = fs::metadata(path)
        .map_err(|e| CoreError::InvalidArgument(format!("backup stat failed: {}", e)))?;
    let total = metadata.len().max(1);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut processed: u64 = 0;
    let mut last_percent: u64 = 0;
    loop {
        let n = std::io::Read::read(&mut file, &mut buf)
            .map_err(|e| CoreError::InvalidArgument(format!("backup read failed: {}", e)))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        processed = processed.saturating_add(n as u64);
        let percent = (processed * 100) / total;
        if percent > last_percent {
            last_percent = percent;
            if percent <= 100 {
                progress(&format!("Preparing import... {}%", percent));
            }
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn import_backup(plan: &ImportPlan, archive_path: &Path) -> Result<(), CoreError> {
    import_backup_with_progress(plan, archive_path, |_| {})
}

pub fn import_backup_with_progress<F>(
    plan: &ImportPlan,
    archive_path: &Path,
    progress: F,
) -> Result<(), CoreError>
where
    F: Fn(&str),
{
    progress("Decoding backup...");
    let import_id = Uuid::new_v4().to_string();
    let temp_dir = tempfile::tempdir()
        .map_err(|e| CoreError::InvalidArgument(format!("temp dir failed: {}", e)))?;
    check_disk_space(&temp_dir.path(), archive_path, &plan.source_path)?;
    let db_path = temp_dir.path().join("signal.sqlite");
    let frames_dir = temp_dir.path().join("frames");
    fs::create_dir_all(&frames_dir)
        .map_err(|e| CoreError::InvalidArgument(format!("frames dir failed: {}", e)))?;

    let mut archive = open_archive(archive_path)?;
    let import_started_at = Utc::now().timestamp_millis();
    let exists: Option<String> = archive
        .conn
        .query_row(
            "SELECT id FROM imports WHERE source_hash = ?1 AND status = 'success' LIMIT 1;",
            params![plan.source_hash],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_some() {
        return Err(CoreError::InvalidArgument(
            "archive already loaded for this backup".to_string(),
        ));
    }

    archive.conn.execute(
        "INSERT INTO imports (id, imported_at, source_filename, source_hash, detected_version, status, stats_json)
         VALUES (?1, ?2, ?3, ?4, NULL, 'running', NULL);",
        params![
            import_id,
            import_started_at,
            plan.source_filename,
            plan.source_hash
        ],
    )?;

    if let Err(err) = signalbackup::decode_backup(
        Path::new(&plan.source_path),
        &plan.normalized_passphrase,
        &db_path,
        &frames_dir,
        true,
    ) {
        let err_msg = err.to_string();
        if err_msg.to_lowercase().contains("unsupported") {
            return Err(CoreError::InvalidArgument(
                "unsupported backup version detected".to_string(),
            ));
        }
        // keep temp dir for inspection
        let keep_dir = temp_dir.keep().to_string_lossy().to_string();
        let log_path = Path::new(&keep_dir).join("frames").join("decode.log");
        let log_tail = fs::read_to_string(&log_path).unwrap_or_default();
        let msg = if log_tail.trim().is_empty() {
            format!("{} (logs: {})", err, log_path.display())
        } else {
            format!("{} (logs: {})\n{}", err, log_path.display(), log_tail)
        };
        let _ = archive.conn.execute(
            "UPDATE imports SET status = 'failed', stats_json = ?2 WHERE id = ?1;",
            params![import_id, format!(r#"{{"error":{}}}"#, serde_json::to_string(&msg).unwrap_or("null".to_string()))],
        );
        return Err(CoreError::InvalidArgument(msg));
    }

    progress("Opening decrypted database...");
    let signal_conn = Connection::open(&db_path)?;
    let attachments_dir = archive_path
        .parent()
        .ok_or_else(|| CoreError::InvalidArgument("archive path missing parent".to_string()))?
        .join("attachments");
    let stats = match map_signal_db(
        &signal_conn,
        &mut archive.conn,
        &progress,
        &frames_dir,
        &attachments_dir,
    ) {
        Ok(stats) => stats,
        Err(err) => {
            let msg = err.to_string();
            let _ = archive.conn.execute(
                "UPDATE imports SET status = 'failed', stats_json = ?2 WHERE id = ?1;",
                params![
                    import_id,
                    format!(
                        r#"{{"error":{}}}"#,
                        serde_json::to_string(&msg).unwrap_or("null".to_string())
                    )
                ],
            );
            return Err(err);
        }
    };
    let _ = archive.conn.execute(
        "UPDATE imports SET status = 'success', stats_json = ?2 WHERE id = ?1;",
        params![import_id, stats],
    );
    Ok(())
}

fn check_disk_space(temp_dir: &Path, archive_path: &Path, source_path: &str) -> Result<(), CoreError> {
    let source_meta = fs::metadata(source_path)
        .map_err(|e| CoreError::InvalidArgument(format!("backup stat failed: {}", e)))?;
    let backup_size = source_meta.len();
    let required_temp = backup_size.saturating_mul(2).saturating_add(100 * 1024 * 1024);
    let required_archive = backup_size.saturating_add(100 * 1024 * 1024);
    if let Some(free_temp) = available_space(temp_dir) {
        if free_temp < required_temp {
            return Err(CoreError::InvalidArgument(format!(
                "insufficient disk space for import (temp dir): need ~{}, have {}",
                format_bytes(required_temp),
                format_bytes(free_temp)
            )));
        }
    }
    if let Some(parent) = archive_path.parent() {
        if let Some(free_archive) = available_space(parent) {
            if free_archive < required_archive {
                return Err(CoreError::InvalidArgument(format!(
                    "insufficient disk space for import (archive): need ~{}, have {}",
                    format_bytes(required_archive),
                    format_bytes(free_archive)
                )));
            }
        }
    }
    Ok(())
}

fn available_space(path: &Path) -> Option<u64> {
    let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes()).ok()?;
    let mut stat: statvfs = unsafe { std::mem::zeroed() };
    let res = unsafe { statvfs(c_path.as_ptr(), &mut stat) };
    if res != 0 {
        return None;
    }
    let avail = (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64);
    Some(avail)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn import_from_signal_db_for_tests(
    signal_db_path: &Path,
    archive_path: &Path,
    export_dir: &Path,
) -> Result<(), CoreError> {
    let mut archive = open_archive(archive_path)?;
    let signal_conn = Connection::open(signal_db_path)?;
    let attachments_dir = archive_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("attachments");
    let progress = |_msg: &str| {};
    map_signal_db(&signal_conn, &mut archive.conn, &progress, export_dir, &attachments_dir)?;
    Ok(())
}

pub(super) fn table_exists(conn: &Connection, name: &str) -> Result<bool, CoreError> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name = ?1;",
            params![name],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}

pub(super) fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, CoreError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({});", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn pick_column(conn: &Connection, table: &str, preferred: &[&str]) -> Result<Option<String>, CoreError> {
    for col in preferred {
        if column_exists(conn, table, col)? {
            return Ok(Some((*col).to_string()));
        }
    }
    Ok(None)
}

fn map_signal_db<F>(
    signal: &Connection,
    archive: &mut Connection,
    progress: &F,
    export_dir: &Path,
    attachments_dir: &Path,
) -> Result<String, CoreError>
where
    F: Fn(&str),
{
    progress("Importing recipients...");
    let tx = archive.transaction()?;

    let mms_table = if table_exists(signal, "message")? {
        "message".to_string()
    } else if table_exists(signal, "mms")? {
        "mms".to_string()
    } else {
        return Err(CoreError::InvalidArgument("signal DB missing message/mms table".to_string()));
    };

    let thread_recipient_col =
        pick_column(signal, "thread", &["recipient_id", "thread_recipient_id", "recipient_ids"])?;
    let thread_message_count_col =
        pick_column(signal, "thread", &["meaningful_messages", "message_count"])?;
    let sms_recipient_col = pick_column(signal, "sms", &["recipient_id", "address"])?;
    let sms_date_col = pick_column(signal, "sms", &["date_received", "date"])?;
    let mms_type_col = pick_column(signal, &mms_table, &["type", "msg_box"])?;
    let mms_recipient_col = pick_column(signal, &mms_table, &["from_recipient_id", "recipient_id", "address"])?;
    let mms_date_sent_col = pick_column(signal, &mms_table, &["date_sent", "date"])?;
    let mms_date_received_col = pick_column(signal, &mms_table, &["date_received", "date"])?;

    let rec_aci = pick_column(signal, "recipient", &["aci", "uuid"])?;
    let rec_e164 = pick_column(signal, "recipient", &["e164", "phone"])?;
    let rec_system = pick_column(signal, "recipient", &["system_joined_name", "system_display_name"])?;
    let rec_profile = pick_column(signal, "recipient", &["profile_given_name", "signal_profile_name"])?;

    // recipients
    let mut rec_stmt = signal.prepare(&format!(
        "SELECT _id, {aci}, {e164}, {system}, {profile} FROM recipient;",
        aci = rec_aci.as_deref().unwrap_or("NULL"),
        e164 = rec_e164.as_deref().unwrap_or("NULL"),
        system = rec_system.as_deref().unwrap_or("NULL"),
        profile = rec_profile.as_deref().unwrap_or("NULL"),
    ))?;
    let rec_rows = rec_stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let aci: Option<String> = row.get(1)?;
        let e164: Option<String> = row.get(2)?;
        let system_name: Option<String> = row.get(3)?;
        let profile_name: Option<String> = row.get(4)?;
        Ok((id, aci, e164, system_name, profile_name))
    })?;
    for rec in rec_rows {
        let (id, _aci, e164, system_name, profile_name) = rec?;
        tx.execute(
            "INSERT OR IGNORE INTO recipients (id, phone_e164, profile_name, contact_name) VALUES (?1, ?2, ?3, ?4);",
            params![id.to_string(), e164, profile_name, system_name],
        )?;
    }

    // threads
    progress("Importing threads...");
    if let Some(thread_recipient_col) = thread_recipient_col {
        let msg_count_expr = thread_message_count_col
            .as_deref()
            .map(|col| format!("thread.{col}"))
            .unwrap_or_else(|| "NULL".to_string());
        let mut thread_stmt = signal.prepare(&format!(
            "SELECT thread._id, thread.{rec_col}, thread.date, {msg_count}, groups.title, recipient.{system}, recipient.{profile}, recipient.{e164}
             FROM thread
             LEFT JOIN recipient ON recipient._id = thread.{rec_col}
             LEFT JOIN groups ON recipient.group_id = groups.group_id;",
            rec_col = thread_recipient_col,
            msg_count = msg_count_expr,
            system = rec_system.as_deref().unwrap_or("NULL"),
            profile = rec_profile.as_deref().unwrap_or("NULL"),
            e164 = rec_e164.as_deref().unwrap_or("NULL"),
        ))?;

        let thread_rows = thread_stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let rec_id: Option<i64> = row.get(1)?;
            let date: Option<i64> = row.get(2)?;
            let message_count: Option<i64> = row.get(3)?;
            let group_title: Option<String> = row.get(4)?;
            let system_name: Option<String> = row.get(5)?;
            let profile_name: Option<String> = row.get(6)?;
            let e164: Option<String> = row.get(7)?;
            let name = group_title.or(system_name).or(profile_name).or(e164);
            Ok((id, rec_id, date, message_count.unwrap_or(0), name))
        })?;

        for row in thread_rows {
            let (id, rec_id, date, message_count, name) = row?;
            tx.execute(
                "INSERT OR IGNORE INTO threads (id, name, last_message_at) VALUES (?1, ?2, ?3);",
                params![id.to_string(), name, date],
            )?;
            if let Some(rec_id) = rec_id {
                tx.execute(
                    "INSERT OR IGNORE INTO thread_members (thread_id, recipient_id) VALUES (?1, ?2);",
                    params![id.to_string(), rec_id.to_string()],
                )?;
            }
            if message_count > 0 {
                // optional update of last_message_at could happen later
            }
        }
    }

    let mut sms_count: i64 = 0;
    let mut sms_inserted: i64 = 0;
    let mut sms_total: Option<i64> = None;
    let sms_quote_id_col = pick_column(signal, "sms", &["quote_id", "quote_id"])?
        .unwrap_or_else(|| "NULL".to_string());
    let sms_quote_author_col =
        pick_column(signal, "sms", &["quote_author", "quote_author_id", "quote_author_recipient_id"])?
            .unwrap_or_else(|| "NULL".to_string());
    let sms_quote_body_col =
        pick_column(signal, "sms", &["quote_body", "quote_text", "quote"])?
            .unwrap_or_else(|| "NULL".to_string());
    // sms messages
    if table_exists(signal, "sms")? {
        sms_total = Some(signal
            .query_row("SELECT COUNT(1) FROM sms;", [], |row| row.get(0))
            .unwrap_or(0));
        progress("Importing SMS messages...");
        let sms_recipient_col = sms_recipient_col.clone().unwrap_or_else(|| "recipient_id".to_string());
        let sms_date_col = sms_date_col.clone().unwrap_or_else(|| "date".to_string());
        let mut sms_stmt = signal.prepare(&format!(
            "SELECT _id, thread_id, body, {date_col} AS date_recv, date_sent, type, {rec_col} AS recipient_id, \
                    {quote_id} AS quote_id, {quote_author} AS quote_author, {quote_body} AS quote_body \
             FROM sms;",
            date_col = sms_date_col,
            rec_col = sms_recipient_col,
            quote_id = sms_quote_id_col,
            quote_author = sms_quote_author_col,
            quote_body = sms_quote_body_col,
        ))?;
        let sms_rows = sms_stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let thread_id: i64 = row.get(1)?;
            let body: Option<String> = row.get(2)?;
            let date_recv: Option<i64> = row.get(3)?;
            let date_sent: Option<i64> = row.get(4)?;
            let msg_type: Option<i64> = row.get(5)?;
            let recipient_id: Option<i64> = row.get(6)?;
            let quote_id: Option<i64> = row.get(7)?;
            let quote_author: Option<i64> = row.get(8)?;
            let quote_body: Option<String> = row.get(9)?;
            Ok((id, thread_id, body, date_recv, date_sent, msg_type, recipient_id, quote_id, quote_author, quote_body))
        })?;
        let mut sms_batch: Vec<MessageRowData> = Vec::with_capacity(100);
        for row in sms_rows {
            let (id, thread_id, body, date_recv, date_sent, msg_type, recipient_id, quote_id, quote_author, quote_body) = row?;
            let is_outgoing = msg_type.map(is_outgoing_type).unwrap_or(false);
            let sender_id = if is_outgoing { None } else { recipient_id.map(|v| v.to_string()) };
            let msg_id = format!("sms:{}", id);
            let quote_message_id = quote_id.map(|v| format!("sms:{}", v));
            let metadata_json = if quote_body.is_some() || quote_author.is_some() {
                Some(
                    serde_json::json!({
                        "quote_body": quote_body,
                        "quote_author_id": quote_author,
                    })
                    .to_string(),
                )
            } else {
                None
            };
            let dedupe_key = if id > 0 {
                format!("sms:{}", id)
            } else {
                fallback_dedupe_key(
                    "sms",
                    &thread_id.to_string(),
                    sender_id.as_deref(),
                    date_sent.or(date_recv),
                    "text",
                    body.as_deref(),
                    is_outgoing,
                )
            };
            sms_batch.push(MessageRowData {
                id: msg_id,
                thread_id: thread_id.to_string(),
                sender_id,
                sent_at: date_sent.or(date_recv),
                received_at: date_recv,
                message_type: "text".to_string(),
                body,
                is_outgoing: if is_outgoing { 1 } else { 0 },
                is_view_once: 0,
                quote_message_id,
                metadata_json,
                dedupe_key,
            });
            if sms_batch.len() >= 100 {
                sms_inserted += insert_message_batch(&tx, &sms_batch)?;
                sms_batch.clear();
            }
            sms_count += 1;
            if sms_count % 5000 == 0 {
                let total = sms_total.unwrap_or(0);
                if total > 0 {
                    let msg = format!("Importing SMS messages... {}/{}", sms_count, total);
                    progress(&msg);
                }
            }
        }
        if !sms_batch.is_empty() {
            sms_inserted += insert_message_batch(&tx, &sms_batch)?;
        }
    }

    // mms/messages table
    let mms_total: i64 = signal
        .query_row(&format!("SELECT COUNT(1) FROM {};", mms_table), [], |row| row.get(0))
        .unwrap_or(0);
    let mut mms_count: i64 = 0;
    let mut mms_inserted: i64 = 0;
    progress("Importing MMS messages...");
    let mms_type_col = mms_type_col.clone().unwrap_or_else(|| "type".to_string());
    let mms_recipient_col = mms_recipient_col.clone().unwrap_or_else(|| "recipient_id".to_string());
    let mms_date_sent_col = mms_date_sent_col.clone().unwrap_or_else(|| "date_sent".to_string());
    let mms_date_recv_col = mms_date_received_col.clone().unwrap_or_else(|| "date_received".to_string());
    let mms_quote_id_col =
        pick_column(signal, &mms_table, &["quote_id", "quote_id"])?.unwrap_or_else(|| "NULL".to_string());
    let mms_quote_author_col = pick_column(
        signal,
        &mms_table,
        &["quote_author", "quote_author_id", "quote_author_recipient_id"],
    )?
    .unwrap_or_else(|| "NULL".to_string());
    let mms_quote_body_col =
        pick_column(signal, &mms_table, &["quote_body", "quote_text", "quote"])?
            .unwrap_or_else(|| "NULL".to_string());

    let mut mms_stmt = signal.prepare(&format!(
        "SELECT _id, thread_id, body, {date_recv} AS date_recv, {date_sent} AS date_sent, \
                {type_col} AS msg_type, {rec_col} AS recipient_id, \
                {quote_id} AS quote_id, {quote_author} AS quote_author, {quote_body} AS quote_body \
         FROM {mms_table};",
        date_recv = mms_date_recv_col,
        date_sent = mms_date_sent_col,
        type_col = mms_type_col,
        rec_col = mms_recipient_col,
        quote_id = mms_quote_id_col,
        quote_author = mms_quote_author_col,
        quote_body = mms_quote_body_col,
        mms_table = mms_table,
    ))?;
    let mms_rows = mms_stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let thread_id: i64 = row.get(1)?;
        let body: Option<String> = row.get(2)?;
        let date_recv: Option<i64> = row.get(3)?;
        let date_sent: Option<i64> = row.get(4)?;
        let msg_type: Option<i64> = row.get(5)?;
        let recipient_id: Option<i64> = row.get(6)?;
        let quote_id: Option<i64> = row.get(7)?;
        let quote_author: Option<i64> = row.get(8)?;
        let quote_body: Option<String> = row.get(9)?;
        Ok((id, thread_id, body, date_recv, date_sent, msg_type, recipient_id, quote_id, quote_author, quote_body))
    })?;
    let mut mms_batch: Vec<MessageRowData> = Vec::with_capacity(100);
    for row in mms_rows {
        let (id, thread_id, body, date_recv, date_sent, msg_type, recipient_id, quote_id, quote_author, quote_body) = row?;
        let is_outgoing = msg_type.map(is_outgoing_type).unwrap_or(false);
        let sender_id = if is_outgoing { None } else { recipient_id.map(|v| v.to_string()) };
        let msg_id = format!("mms:{}", id);
        let quote_message_id = quote_id.map(|v| format!("mms:{}", v));
        let metadata_json = if quote_body.is_some() || quote_author.is_some() {
            Some(
                serde_json::json!({
                    "quote_body": quote_body,
                    "quote_author_id": quote_author,
                })
                .to_string(),
            )
        } else {
            None
        };
        let dedupe_key = if id > 0 {
            format!("mms:{}", id)
        } else {
            fallback_dedupe_key(
                "mms",
                &thread_id.to_string(),
                sender_id.as_deref(),
                date_sent.or(date_recv),
                "text",
                body.as_deref(),
                is_outgoing,
            )
        };
        mms_batch.push(MessageRowData {
            id: msg_id,
            thread_id: thread_id.to_string(),
            sender_id,
            sent_at: date_sent.or(date_recv),
            received_at: date_recv,
            message_type: "text".to_string(),
            body,
            is_outgoing: if is_outgoing { 1 } else { 0 },
            is_view_once: 0,
            quote_message_id,
            metadata_json,
            dedupe_key,
        });
        if mms_batch.len() >= 100 {
            mms_inserted += insert_message_batch(&tx, &mms_batch)?;
            mms_batch.clear();
        }
        mms_count += 1;
        if mms_count % 5000 == 0 && mms_total > 0 {
            let msg = format!("Importing MMS messages... {}/{}", mms_count, mms_total);
            progress(&msg);
        }
    }
    if !mms_batch.is_empty() {
        mms_inserted += insert_message_batch(&tx, &mms_batch)?;
    }

    let attachment_stats = attachments::map_attachments(signal, &tx, export_dir, attachments_dir, progress)?;
    map_reactions(signal, &tx, progress)?;

    progress("Updating thread activity...");
    update_thread_activity(&tx)?;
    fts::build_message_fts(&tx, progress)?;

    progress("Finalizing import...");
    tx.commit()?;

    let stats_json = serde_json::json!({
        "sms_total": sms_total.unwrap_or(sms_count),
        "sms_inserted": sms_inserted,
        "mms_total": mms_count,
        "mms_inserted": mms_inserted,
        "messages_inserted_total": sms_inserted + mms_inserted,
        "attachments_total": attachment_stats.total,
        "attachments_found": attachment_stats.found,
        "attachments_missing": attachment_stats.missing,
        "attachments_inserted": attachment_stats.inserted,
    })
    .to_string();
    Ok(stats_json)
}

fn is_outgoing_type(msg_type: i64) -> bool {
    let base = (msg_type as u64) & 0x1F;
    matches!(base, 21 | 22 | 23 | 24 | 25 | 26 | 2 | 11)
}

fn fallback_dedupe_key(
    kind: &str,
    thread_id: &str,
    sender_id: Option<&str>,
    timestamp: Option<i64>,
    message_type: &str,
    body: Option<&str>,
    is_outgoing: bool,
) -> String {
    let body_hash = body
        .map(|b| {
            let mut hasher = Sha256::new();
            hasher.update(b.as_bytes());
            hex::encode(hasher.finalize())
        })
        .unwrap_or_else(|| "none".to_string());
    let base = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        kind,
        thread_id,
        sender_id.unwrap_or(""),
        timestamp.unwrap_or(0),
        message_type,
        if is_outgoing { 1 } else { 0 },
        body_hash
    );
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    format!("fb:{}", hex::encode(hasher.finalize()))
}

fn update_thread_activity(tx: &rusqlite::Transaction) -> Result<(), CoreError> {
    tx.execute(
        "UPDATE threads
         SET last_message_at = (
           SELECT MAX(sort_ts)
           FROM messages m
           WHERE m.thread_id = threads.id
         );",
        [],
    )?;
    Ok(())
}

fn map_reactions<F>(signal: &Connection, tx: &rusqlite::Transaction, progress: &F) -> Result<(), CoreError>
where
    F: Fn(&str),
{
    let table = if table_exists(signal, "reaction")? {
        "reaction"
    } else if table_exists(signal, "reactions")? {
        "reactions"
    } else {
        return Ok(());
    };
    let msg_col = pick_column(signal, table, &["message_id", "message", "mid", "mms_id"])?;
    let emoji_col = pick_column(signal, table, &["emoji", "reaction", "emote"])?;
    let author_col = pick_column(signal, table, &["author_id", "author", "sender_id", "recipient_id"])?;
    let date_col = pick_column(signal, table, &["date", "date_sent", "timestamp", "reacted_at"])?;
    if msg_col.is_none() || emoji_col.is_none() {
        return Ok(());
    }
    progress("Importing reactions...");
    let query = format!(
        "SELECT {msg}, {emoji}, {author}, {date} FROM {table};",
        msg = msg_col.unwrap(),
        emoji = emoji_col.unwrap(),
        author = author_col.unwrap_or_else(|| "NULL".to_string()),
        date = date_col.unwrap_or_else(|| "NULL".to_string()),
        table = table
    );
    let mut stmt = signal.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        let msg_raw: Value = row.get(0)?;
        let emoji: Option<String> = row.get(1)?;
        let author_id: Option<i64> = row.get(2)?;
        let reacted_at: Option<i64> = row.get(3)?;
        Ok((msg_raw, emoji, author_id, reacted_at))
    })?;
    let mut batch: Vec<(String, String, String, Option<i64>)> = Vec::with_capacity(200);
    for row in rows {
        let (msg_raw, emoji, author_id, reacted_at) = row?;
        let msg_id = match msg_raw {
            Value::Integer(v) => format!("mms:{}", v),
            Value::Text(v) => {
                if v.contains(':') {
                    v
                } else {
                    format!("mms:{}", v)
                }
            }
            _ => continue,
        };
        let emoji = match emoji {
            Some(v) => v,
            None => continue,
        };
        let author = author_id
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        batch.push((msg_id, author, emoji, reacted_at));
        if batch.len() >= 500 {
            insert_reaction_batch(tx, &batch)?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        insert_reaction_batch(tx, &batch)?;
    }
    Ok(())
}

fn insert_reaction_batch(
    tx: &rusqlite::Transaction,
    batch: &[(String, String, String, Option<i64>)],
) -> Result<(), CoreError> {
    let mut sql = String::from(
        "INSERT OR IGNORE INTO reactions (message_id, reactor_id, emoji, reacted_at) VALUES ",
    );
    let mut params_vec: Vec<Value> = Vec::with_capacity(batch.len() * 4);
    for (idx, row) in batch.iter().enumerate() {
        if idx > 0 {
            sql.push(',');
        }
        sql.push_str("(?, ?, ?, ?)");
        params_vec.push(Value::from(row.0.clone()));
        params_vec.push(Value::from(row.1.clone()));
        params_vec.push(Value::from(row.2.clone()));
        match row.3 {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
    }
    tx.execute(&sql, rusqlite::params_from_iter(params_vec))?;
    Ok(())
}


struct MessageRowData {
    id: String,
    thread_id: String,
    sender_id: Option<String>,
    sent_at: Option<i64>,
    received_at: Option<i64>,
    message_type: String,
    body: Option<String>,
    is_outgoing: i64,
    is_view_once: i64,
    quote_message_id: Option<String>,
    metadata_json: Option<String>,
    dedupe_key: String,
}

fn insert_message_batch(tx: &rusqlite::Transaction, batch: &[MessageRowData]) -> Result<i64, CoreError> {
    if batch.is_empty() {
        return Ok(0);
    }
    let mut sql = String::from(
        "INSERT OR IGNORE INTO messages (id, thread_id, sender_id, sent_at, received_at, sort_ts, type, body, is_outgoing, is_view_once, quote_message_id, metadata_json, dedupe_key) VALUES ",
    );
    let mut params_vec: Vec<Value> = Vec::with_capacity(batch.len() * 13);
    for (idx, row) in batch.iter().enumerate() {
        if idx > 0 {
            sql.push(',');
        }
        sql.push_str("(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)");
        params_vec.push(Value::from(row.id.clone()));
        params_vec.push(Value::from(row.thread_id.clone()));
        match &row.sender_id {
            Some(v) => params_vec.push(Value::from(v.clone())),
            None => params_vec.push(Value::Null),
        }
        match row.sent_at {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
        match row.received_at {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
        params_vec.push(Value::from(row.sent_at.or(row.received_at).unwrap_or(0)));
        params_vec.push(Value::from(row.message_type.clone()));
        match &row.body {
            Some(v) => params_vec.push(Value::from(v.clone())),
            None => params_vec.push(Value::Null),
        }
        params_vec.push(Value::from(row.is_outgoing));
        params_vec.push(Value::from(row.is_view_once));
        match &row.quote_message_id {
            Some(v) => params_vec.push(Value::from(v.clone())),
            None => params_vec.push(Value::Null),
        }
        match &row.metadata_json {
            Some(v) => params_vec.push(Value::from(v.clone())),
            None => params_vec.push(Value::Null),
        }
        params_vec.push(Value::from(row.dedupe_key.clone()));
    }
    let changes = tx.execute(&sql, rusqlite::params_from_iter(params_vec))?;
    Ok(changes as i64)
}
