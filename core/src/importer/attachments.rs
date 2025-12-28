use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use rusqlite::Connection;
use rusqlite::types::Value;
use tempfile::NamedTempFile;

use crate::crypto;
use crate::error::CoreError;

use super::{pick_column, table_exists};

const ATTACHMENT_BATCH_SIZE: usize = 500;
const ATTACHMENT_PROGRESS_EVERY: i64 = 2000;
const ATTACHMENT_WORKERS: usize = 4;
const SIZE_SMALL_MAX: i64 = 1 * 1024 * 1024 - 1;
const SIZE_MEDIUM_MAX: i64 = 10 * 1024 * 1024 - 1;

#[derive(Debug, Clone)]
pub(super) struct AttachmentImportStats {
    pub total: i64,
    pub found: i64,
    pub missing: i64,
    pub inserted: i64,
}

pub(super) fn map_attachments<F>(
    signal: &Connection,
    tx: &rusqlite::Transaction,
    export_dir: &Path,
    attachments_dir: &Path,
    progress: &F,
) -> Result<AttachmentImportStats, CoreError>
where
    F: Fn(&str),
{
    let master_key = crypto::load_or_create_master_key()?;
    let part_table = if table_exists(signal, "part")? {
        "part".to_string()
    } else if table_exists(signal, "attachment")? {
        "attachment".to_string()
    } else {
        return Ok(AttachmentImportStats {
            total: 0,
            found: 0,
            missing: 0,
            inserted: 0,
        });
    };

    let part_mid = pick_column(signal, &part_table, &["message_id", "mid"])?;
    if part_mid.is_none() {
        return Ok(AttachmentImportStats {
            total: 0,
            found: 0,
            missing: 0,
            inserted: 0,
        });
    }
    let part_unique = pick_column(signal, &part_table, &["unique_id"])?;
    let part_ct = pick_column(signal, &part_table, &["content_type", "ct"])?;
    let part_size = pick_column(signal, &part_table, &["data_size", "size"])?;
    let part_file = pick_column(signal, &part_table, &["file_name", "filename", "fileName"])?;
    let part_width = pick_column(signal, &part_table, &["width"])?;
    let part_height = pick_column(signal, &part_table, &["height"])?;
    let part_duration = pick_column(signal, &part_table, &["duration", "duration_ms"])?;

    fs::create_dir_all(&attachments_dir)
        .map_err(|e| CoreError::InvalidArgument(format!("attachments dir failed: {}", e)))?;

    let total_rows: i64 = signal
        .query_row(&format!("SELECT COUNT(1) FROM {part_table};"), [], |row| row.get(0))
        .unwrap_or(0);
    if total_rows == 0 {
        progress("No attachments found.");
        return Ok(AttachmentImportStats {
            total: 0,
            found: 0,
            missing: 0,
            inserted: 0,
        });
    }

    progress("Importing attachments...");
    let query = format!(
        "SELECT _id, {mid}, {unique}, {ct}, {size}, {file}, {width}, {height}, {duration} FROM {table};",
        mid = part_mid.as_deref().unwrap_or("NULL"),
        unique = part_unique.as_deref().unwrap_or("-1 AS unique_id"),
        ct = part_ct.as_deref().unwrap_or("NULL"),
        size = part_size.as_deref().unwrap_or("NULL"),
        file = part_file.as_deref().unwrap_or("NULL"),
        width = part_width.as_deref().unwrap_or("NULL"),
        height = part_height.as_deref().unwrap_or("NULL"),
        duration = part_duration.as_deref().unwrap_or("NULL"),
        table = part_table
    );
    let mut stmt = signal.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let mid: Option<i64> = row.get(1)?;
        let unique_id: Option<i64> = row.get(2)?;
        let mime: Option<String> = row.get(3)?;
        let data_size: Option<i64> = row.get(4)?;
        let file_name: Option<String> = row.get(5)?;
        let width: Option<i64> = row.get(6)?;
        let height: Option<i64> = row.get(7)?;
        let duration: Option<i64> = row.get(8)?;
        Ok((id, mid, unique_id, mime, data_size, file_name, width, height, duration))
    })?;

    let mut jobs: Vec<AttachmentJob> = Vec::with_capacity(total_rows as usize);

    for row in rows {
        let (id, mid, unique_id, mime, data_size, file_name, width, height, duration) = row?;
        let mid = match mid {
            Some(mid) => mid,
            None => continue,
        };
        let mut unique_id_val = unique_id.unwrap_or(-1);
        if unique_id_val == 0 {
            unique_id_val = -1;
        }
        let attachment_path = export_dir.join(format!("Attachment_{}_{}.bin", id, unique_id_val));
        jobs.push(AttachmentJob {
            mid,
            attachment_path,
            mime,
            data_size,
            file_name,
            width,
            height,
            duration_ms: duration,
        });
    }

    if jobs.is_empty() {
        progress("No attachments found.");
        return Ok(AttachmentImportStats {
            total: 0,
            found: 0,
            missing: 0,
            inserted: 0,
        });
    }

    let total: i64 = jobs.len() as i64;
    let worker_count = ATTACHMENT_WORKERS.min(jobs.len().max(1));
    let chunk_size = (jobs.len() + worker_count - 1) / worker_count;

    let (result_tx, result_rx) = mpsc::channel();
    let key = Arc::new(master_key);
    let dest_dir = Arc::new(attachments_dir.to_path_buf());

    for chunk in jobs.chunks(chunk_size) {
        let worker_jobs = chunk.to_vec();
        let worker_tx = result_tx.clone();
        let worker_key = Arc::clone(&key);
        let worker_dest = Arc::clone(&dest_dir);
        thread::spawn(move || {
            for job in worker_jobs {
                if !job.attachment_path.exists() {
                    let _ = worker_tx.send(AttachmentResult::Missing);
                    continue;
                }
                match copy_attachment(&job.attachment_path, worker_dest.as_path(), worker_key.as_ref()) {
                    Ok((sha256, file_size)) => {
                        let size_bytes = job.data_size.or(Some(file_size as i64));
                        let size_bucket = size_bytes.map(bucket_from_size);
                        let kind = job.mime.as_deref().map(infer_kind).unwrap_or_else(|| "file".to_string());
                        let message_id = format!("mms:{}", job.mid);
                        let attachment_id = format!("att:{}:{}", message_id, sha256);
                        let row = AttachmentRowData {
                            id: attachment_id,
                            message_id,
                            sha256,
                            mime: job.mime.clone(),
                            size_bytes,
                            size_bucket,
                            original_filename: job.file_name.clone(),
                            kind,
                            width: job.width,
                            height: job.height,
                            duration_ms: job.duration_ms,
                        };
                        let _ = worker_tx.send(AttachmentResult::Found(row));
                    }
                    Err(err) => {
                        let _ = worker_tx.send(AttachmentResult::Error(err.to_string()));
                        break;
                    }
                }
            }
        });
    }
    drop(result_tx);

    let mut processed: i64 = 0;
    let mut found_files: i64 = 0;
    let mut missing_files: i64 = 0;
    let mut inserted: i64 = 0;
    let mut batch: Vec<AttachmentRowData> = Vec::with_capacity(ATTACHMENT_BATCH_SIZE);

    for result in result_rx {
        match result {
            AttachmentResult::Found(row) => {
                processed += 1;
                found_files += 1;
                batch.push(row);
                if batch.len() >= ATTACHMENT_BATCH_SIZE {
                    inserted += insert_attachment_batch(tx, &batch)?;
                    batch.clear();
                }
            }
            AttachmentResult::Missing => {
                processed += 1;
                missing_files += 1;
            }
            AttachmentResult::Error(msg) => {
                return Err(CoreError::InvalidArgument(msg));
            }
        }

        if processed % ATTACHMENT_PROGRESS_EVERY == 0 {
            let msg = format!(
                "Importing attachments... {}/{} (found {}, missing {}, inserted {})",
                processed, total, found_files, missing_files, inserted
            );
            progress(&msg);
        }
    }

    if !batch.is_empty() {
        inserted += insert_attachment_batch(tx, &batch)?;
    }

    progress(&format!(
        "Attachments imported: total {}, found {}, missing {}, inserted {}",
        total, found_files, missing_files, inserted
    ));

    Ok(AttachmentImportStats {
        total,
        found: found_files,
        missing: missing_files,
        inserted,
    })
}

struct AttachmentRowData {
    id: String,
    message_id: String,
    sha256: String,
    mime: Option<String>,
    size_bytes: Option<i64>,
    size_bucket: Option<i64>,
    original_filename: Option<String>,
    kind: String,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
}

#[derive(Clone)]
struct AttachmentJob {
    mid: i64,
    attachment_path: std::path::PathBuf,
    mime: Option<String>,
    data_size: Option<i64>,
    file_name: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
}

enum AttachmentResult {
    Found(AttachmentRowData),
    Missing,
    Error(String),
}

fn bucket_from_size(size_bytes: i64) -> i64 {
    if size_bytes <= SIZE_SMALL_MAX {
        0
    } else if size_bytes <= SIZE_MEDIUM_MAX {
        1
    } else {
        2
    }
}

fn insert_attachment_batch(
    tx: &rusqlite::Transaction,
    batch: &[AttachmentRowData],
) -> Result<i64, CoreError> {
    if batch.is_empty() {
        return Ok(0);
    }
    let mut sql = String::from(
        "INSERT OR IGNORE INTO attachments (id, message_id, sha256, mime, size_bytes, size_bucket, original_filename, kind, width, height, duration_ms) VALUES ",
    );
    let mut params_vec: Vec<Value> = Vec::with_capacity(batch.len() * 11);
    for (idx, row) in batch.iter().enumerate() {
        if idx > 0 {
            sql.push(',');
        }
        sql.push_str("(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)");
        params_vec.push(Value::from(row.id.clone()));
        params_vec.push(Value::from(row.message_id.clone()));
        params_vec.push(Value::from(row.sha256.clone()));
        match &row.mime {
            Some(v) => params_vec.push(Value::from(v.clone())),
            None => params_vec.push(Value::Null),
        }
        match row.size_bytes {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
        match row.size_bucket {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
        match &row.original_filename {
            Some(v) => params_vec.push(Value::from(v.clone())),
            None => params_vec.push(Value::Null),
        }
        params_vec.push(Value::from(row.kind.clone()));
        match row.width {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
        match row.height {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
        match row.duration_ms {
            Some(v) => params_vec.push(Value::from(v)),
            None => params_vec.push(Value::Null),
        }
    }
    let changes = tx.execute(&sql, rusqlite::params_from_iter(params_vec))?;
    Ok(changes as i64)
}

fn copy_attachment(
    src: &Path,
    dest_dir: &Path,
    master_key: &crypto::MasterKey,
) -> Result<(String, u64), CoreError> {
    let mut file = fs::File::open(src)
        .map_err(|e| CoreError::InvalidArgument(format!("attachment open failed: {}", e)))?;
    let mut temp = NamedTempFile::new_in(dest_dir)
        .map_err(|e| CoreError::InvalidArgument(format!("attachment temp failed: {}", e)))?;
    let chunk_size = attachment_chunk_size(src).unwrap_or(1024 * 1024);
    let (hash, total) = crypto::encrypt_stream_with_hash_chunk(&mut file, &mut temp, master_key, chunk_size)?;
    let dest = dest_dir.join(&hash);
    if dest.exists() {
        return Ok((hash, total));
    }
    temp.persist(&dest)
        .map_err(|e| CoreError::InvalidArgument(format!("attachment persist failed: {}", e)))?;
    Ok((hash, total))
}

fn attachment_chunk_size(path: &Path) -> Option<usize> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();
    if size >= 10 * 1024 * 1024 {
        Some(4 * 1024 * 1024)
    } else {
        Some(1024 * 1024)
    }
}

fn infer_kind(mime: &str) -> String {
    if mime.starts_with("image/") {
        "image".to_string()
    } else if mime.starts_with("video/") {
        "video".to_string()
    } else if mime.starts_with("audio/") {
        "audio".to_string()
    } else {
        "file".to_string()
    }
}
