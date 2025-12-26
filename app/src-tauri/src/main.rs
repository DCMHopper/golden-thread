use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use golden_thread_core::{diagnostics, open_archive, seed, CoreError};
use golden_thread_core::importer;
use golden_thread_core::models::{ArchiveStats, MediaRow, MessageRow, MessageTags, ScrapbookMessage, SearchHit, Tag, ThreadMediaRow, ThreadSummary};
use golden_thread_core::query::{
    archive_stats,
    create_tag,
    delete_tag,
    get_message,
    get_message_tags,
    get_message_tags_bulk,
    list_attachments_for_message,
    list_media,
    list_messages,
    list_messages_around,
    list_messages_after,
    list_reactions_for_messages,
    list_scrapbook_messages,
    list_tags,
    list_thread_media,
    list_threads,
    search_messages,
    set_message_tags,
    update_tag,
};
use tauri::{Emitter, Manager};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use image::imageops::FilterType;

fn validate_sha256(input: &str) -> Result<(), String> {
    if input.len() != 64 {
        return Err("invalid attachment id".to_string());
    }
    if !input.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid attachment id".to_string());
    }
    Ok(())
}

fn archive_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, CoreError> {
    let base = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CoreError::InvalidArgument(e.to_string()))?;
    let archive_dir = archive_dir(&base)?;
    Ok(archive_dir.join("archive.sqlite"))
}

fn archive_dir(base: &PathBuf) -> Result<PathBuf, CoreError> {
    let archive_dir = base.join("golden-thread");
    if !archive_dir.exists() {
        fs::create_dir_all(&archive_dir).map_err(|e| CoreError::InvalidArgument(e.to_string()))?;
    }
    Ok(archive_dir)
}

fn diagnostics_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, CoreError> {
    let base = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CoreError::InvalidArgument(e.to_string()))?;
    let archive_dir = archive_dir(&base)?;
    Ok(archive_dir.join("logs"))
}

fn attachments_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, CoreError> {
    let base = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CoreError::InvalidArgument(e.to_string()))?;
    let archive_dir = archive_dir(&base)?;
    Ok(archive_dir.join("attachments"))
}

fn thumbs_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, CoreError> {
    let base = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CoreError::InvalidArgument(e.to_string()))?;
    let archive_dir = archive_dir(&base)?;
    Ok(archive_dir.join("thumbs"))
}

#[derive(Default)]
struct DbState {
    db: Mutex<Option<golden_thread_core::ArchiveDb>>,
}

fn with_db<F, T>(app_handle: &tauri::AppHandle, state: &tauri::State<DbState>, f: F) -> Result<T, CoreError>
where
    F: FnOnce(&golden_thread_core::ArchiveDb) -> Result<T, CoreError>,
{
    let path = archive_path(app_handle)?;
    let mut guard = state
        .db
        .lock()
        .map_err(|_| CoreError::InvalidArgument("db lock poisoned".to_string()))?;
    let needs_open = match guard.as_ref() {
        Some(db) => !db.path.exists(),
        None => true,
    };
    if needs_open {
        *guard = Some(open_archive(&path)?);
    }
    let db = guard.as_ref().ok_or_else(|| CoreError::InvalidArgument("db unavailable".to_string()))?;
    f(db)
}

#[tauri::command]
fn list_threads_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ThreadSummary>, String> {
    let result = with_db(&app_handle, &state, |db| list_threads(&db.conn, limit, offset))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_threads failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_messages_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    thread_id: String,
    before_ts: Option<i64>,
    before_id: Option<String>,
    limit: i64,
) -> Result<Vec<MessageRow>, String> {
    let result = with_db(&app_handle, &state, |db| {
        list_messages(&db.conn, &thread_id, before_ts, before_id.as_deref(), limit)
    })
    .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_messages failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_messages_after_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    thread_id: String,
    after_ts: i64,
    after_id: Option<String>,
    limit: i64,
) -> Result<Vec<MessageRow>, String> {
    let result = with_db(&app_handle, &state, |db| {
        list_messages_after(&db.conn, &thread_id, after_ts, after_id.as_deref(), limit)
    })
    .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_messages_after failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn get_message_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_id: String,
) -> Result<MessageRow, String> {
    let result = with_db(&app_handle, &state, |db| get_message(&db.conn, &message_id))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("get_message failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_messages_around_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_id: String,
    before: i64,
    after: i64,
) -> Result<Vec<MessageRow>, String> {
    let result = with_db(&app_handle, &state, |db| list_messages_around(&db.conn, &message_id, before, after))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_messages_around failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_message_reactions_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_ids: Vec<String>,
) -> Result<Vec<golden_thread_core::models::ReactionSummary>, String> {
    let result = with_db(&app_handle, &state, |db| list_reactions_for_messages(&db.conn, &message_ids))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_reactions failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn search_messages_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    query: String,
    thread_id: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SearchHit>, String> {
    let result = with_db(&app_handle, &state, |db| search_messages(&db.conn, &query, thread_id.as_deref(), limit, offset))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("search_messages failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_media_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    thread_id: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<MediaRow>, String> {
    let result = with_db(&app_handle, &state, |db| list_media(&db.conn, thread_id.as_deref(), limit, offset))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_media failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_thread_media_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    thread_id: String,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    size_bucket: Option<i64>,
    sort: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<ThreadMediaRow>, String> {
    let result = with_db(&app_handle, &state, |db| {
        list_thread_media(
            &db.conn,
            &thread_id,
            from_ts,
            to_ts,
            size_bucket,
            &sort,
            limit,
            offset,
        )
    })
    .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_thread_media failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn list_message_attachments_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_id: String,
) -> Result<Vec<MediaRow>, String> {
    let result = with_db(&app_handle, &state, |db| list_attachments_for_message(&db.conn, &message_id))
        .map_err(|e| e.to_string());
    if let Err(ref err) = result {
        if let Ok(log_dir) = diagnostics_dir(&app_handle) {
            let _ = diagnostics::log_event(&log_dir, "query_error", &format!("list_attachments failed: {}", err));
        }
    }
    result
}

#[tauri::command]
fn attachment_data_url_cmd(
    app_handle: tauri::AppHandle,
    sha256: String,
    mime: String,
) -> Result<String, String> {
    validate_sha256(&sha256)?;
    if !(mime.starts_with("image/") || mime.starts_with("video/") || mime.starts_with("audio/")) {
        return Err("unsupported media type".to_string());
    }
    let path = attachments_dir(&app_handle).map_err(|e| e.to_string())?.join(&sha256);
    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    let max_bytes: u64 = if mime.starts_with("image/") {
        12 * 1024 * 1024
    } else if mime.starts_with("audio/") {
        25 * 1024 * 1024
    } else {
        35 * 1024 * 1024
    };
    if meta.len() > max_bytes {
        return Err("media too large to preview".to_string());
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let encoded = BASE64_STANDARD.encode(data);
    Ok(format!("data:{};base64,{}", mime, encoded))
}

#[tauri::command]
fn attachment_path_cmd(
    app_handle: tauri::AppHandle,
    sha256: String,
    mime: Option<String>,
) -> Result<String, String> {
    validate_sha256(&sha256)?;
    let base = attachments_dir(&app_handle).map_err(|e| e.to_string())?;
    let path = base.join(&sha256);
    if !path.exists() {
        return Err("attachment missing".to_string());
    }
    let ext = mime
        .as_deref()
        .and_then(mime_extension)
        .map(|s| s.to_string());
    if let Some(ext) = ext {
        let previews = base.join("previews");
        fs::create_dir_all(&previews).map_err(|e| e.to_string())?;
        let preview_path = previews.join(format!("{}.{}", sha256, ext));
        if !preview_path.exists() {
            if let Err(_) = fs::hard_link(&path, &preview_path) {
                let _ = std::os::unix::fs::symlink(&path, &preview_path);
            }
        }
        if preview_path.exists() {
            return Ok(preview_path.to_string_lossy().to_string());
        }
    }
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn attachment_thumbnail_cmd(
    app_handle: tauri::AppHandle,
    sha256: String,
    mime: Option<String>,
    max_size: u32,
) -> Result<String, String> {
    validate_sha256(&sha256)?;
    if let Some(m) = mime.as_deref() {
        if !m.starts_with("image/") {
            return Err("thumbnail only for images".to_string());
        }
    }
    let src = attachments_dir(&app_handle).map_err(|e| e.to_string())?.join(&sha256);
    if !src.exists() {
        return Err("attachment missing".to_string());
    }
    let thumbs = thumbs_dir(&app_handle).map_err(|e| e.to_string())?;
    fs::create_dir_all(&thumbs).map_err(|e| e.to_string())?;
    let thumb_path = thumbs.join(format!("{}_{}.jpg", sha256, max_size));
    if thumb_path.exists() {
        return Ok(thumb_path.to_string_lossy().to_string());
    }
    let img = image::open(&src).map_err(|e| e.to_string())?;
    let resized = img.resize(max_size, max_size, FilterType::Triangle);
    resized
        .save(&thumb_path)
        .map_err(|e| e.to_string())?;
    Ok(thumb_path.to_string_lossy().to_string())
}

fn mime_extension(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/heic" => Some("heic"),
        "video/mp4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "video/webm" => Some("webm"),
        "video/x-matroska" => Some("mkv"),
        "audio/mpeg" => Some("mp3"),
        "audio/mp4" => Some("m4a"),
        "audio/aac" => Some("aac"),
        "audio/ogg" => Some("ogg"),
        "audio/wav" => Some("wav"),
        _ => None,
    }
}

#[tauri::command]
fn archive_stats_cmd(app_handle: tauri::AppHandle, state: tauri::State<DbState>) -> Result<ArchiveStats, String> {
    with_db(&app_handle, &state, |db| archive_stats(&db.conn)).map_err(|e| e.to_string())
}

#[tauri::command]
fn seed_demo_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    primary_count: i64,
    secondary_threads: i64,
) -> Result<(), String> {
    with_db(&app_handle, &state, |db| seed::seed_demo(&db.conn, primary_count, secondary_threads))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_backup_cmd(app_handle: tauri::AppHandle, path: String, passphrase: String) -> Result<(), String> {
    let app = app_handle.clone();
    let log_dir = diagnostics_dir(&app_handle).map_err(|e| e.to_string())?;
    let _ = diagnostics::log_event(&log_dir, "import_start", "import requested");
    let handle = tauri::async_runtime::spawn_blocking(move || {
        let emit_status = |msg: &str| {
            let _ = app.emit("import_status", msg.to_string());
        };
        emit_status("Preparing import...");
        let plan = importer::plan_import_with_progress(std::path::Path::new(&path), &passphrase, emit_status)
            .map_err(|e| e.to_string())?;
        let archive = archive_path(&app).map_err(|e| e.to_string())?;
        importer::import_backup_with_progress(&plan, &archive, emit_status).map_err(|e| e.to_string())
    });
    match handle.await {
        Ok(result) => {
            if result.is_ok() {
                let _ = diagnostics::log_event(&log_dir, "import_success", "import completed");
            } else if let Err(ref err) = result {
                let _ = diagnostics::log_event(&log_dir, "import_error", &err.to_string());
            }
            result
        }
        Err(err) => {
            let _ = diagnostics::log_event(&log_dir, "import_error", &err.to_string());
            Err(err.to_string())
        }
    }
}

#[tauri::command]
fn reset_archive_cmd(app_handle: tauri::AppHandle, state: tauri::State<DbState>) -> Result<(), String> {
    let base = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let archive_dir = archive_dir(&base).map_err(|e| e.to_string())?;
    let archive_path = archive_dir.join("archive.sqlite");
    let archive_wal = archive_dir.join("archive.sqlite-wal");
    let archive_shm = archive_dir.join("archive.sqlite-shm");
    let attachments_dir = archive_dir.join("attachments");
    let thumbs_dir = archive_dir.join("thumbs");

    if archive_path.exists() {
        fs::remove_file(&archive_path).map_err(|e| e.to_string())?;
    }
    if archive_wal.exists() {
        fs::remove_file(&archive_wal).map_err(|e| e.to_string())?;
    }
    if archive_shm.exists() {
        fs::remove_file(&archive_shm).map_err(|e| e.to_string())?;
    }
    if attachments_dir.exists() {
        fs::remove_dir_all(&attachments_dir).map_err(|e| e.to_string())?;
    }
    if thumbs_dir.exists() {
        fs::remove_dir_all(&thumbs_dir).map_err(|e| e.to_string())?;
    }
    if let Ok(mut guard) = state.db.lock() {
        *guard = None;
    }

    Ok(())
}

#[tauri::command]
fn get_diagnostics_cmd(app_handle: tauri::AppHandle) -> Result<String, String> {
    let log_dir = diagnostics_dir(&app_handle).map_err(|e| e.to_string())?;
    let path = log_dir.join("diagnostics.log");
    if !path.exists() {
        return Ok("No diagnostics available.".to_string());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

// ===== Tag Commands =====

#[tauri::command]
fn list_tags_cmd(app_handle: tauri::AppHandle, state: tauri::State<DbState>) -> Result<Vec<Tag>, String> {
    with_db(&app_handle, &state, |db| list_tags(&db.conn)).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_tag_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    name: String,
    color: String,
) -> Result<Tag, String> {
    with_db(&app_handle, &state, |db| create_tag(&db.conn, &name, &color)).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_tag_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    id: String,
    name: String,
    color: String,
) -> Result<(), String> {
    with_db(&app_handle, &state, |db| update_tag(&db.conn, &id, &name, &color)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_tag_cmd(app_handle: tauri::AppHandle, state: tauri::State<DbState>, id: String) -> Result<(), String> {
    with_db(&app_handle, &state, |db| delete_tag(&db.conn, &id)).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_message_tags_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_id: String,
) -> Result<Vec<Tag>, String> {
    with_db(&app_handle, &state, |db| get_message_tags(&db.conn, &message_id)).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_message_tags_bulk_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_ids: Vec<String>,
) -> Result<Vec<MessageTags>, String> {
    with_db(&app_handle, &state, |db| get_message_tags_bulk(&db.conn, &message_ids))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_message_tags_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    message_id: String,
    tag_ids: Vec<String>,
) -> Result<(), String> {
    with_db(&app_handle, &state, |db| set_message_tags(&db.conn, &message_id, &tag_ids))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_scrapbook_messages_cmd(
    app_handle: tauri::AppHandle,
    state: tauri::State<DbState>,
    tag_id: String,
    before_ts: Option<i64>,
    before_id: Option<String>,
    limit: i64,
) -> Result<Vec<ScrapbookMessage>, String> {
    with_db(&app_handle, &state, |db| list_scrapbook_messages(&db.conn, &tag_id, before_ts, before_id.as_deref(), limit))
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(DbState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            if let Ok(log_dir) = diagnostics_dir(&app.handle()) {
                let _ = diagnostics::log_event(&log_dir, "app_start", "app started");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_threads_cmd,
            list_messages_cmd,
            list_messages_after_cmd,
            get_message_cmd,
            list_messages_around_cmd,
            list_message_reactions_cmd,
            search_messages_cmd,
            list_media_cmd,
            list_thread_media_cmd,
            list_message_attachments_cmd,
            attachment_data_url_cmd,
            attachment_path_cmd,
            attachment_thumbnail_cmd,
            archive_stats_cmd,
            get_diagnostics_cmd,
            seed_demo_cmd,
            import_backup_cmd,
            reset_archive_cmd,
            list_tags_cmd,
            create_tag_cmd,
            update_tag_cmd,
            delete_tag_cmd,
            get_message_tags_cmd,
            get_message_tags_bulk_cmd,
            set_message_tags_cmd,
            list_scrapbook_messages_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
