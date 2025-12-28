use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::ColorType;
use serde::de::DeserializeOwned;
use serde_json::json;

use golden_thread_core::{crypto, diagnostics};

use crate::media_ipc::{
    DataUrlRequest, DataUrlResponse, MediaRequest, MediaResponse, Request, Response, ThumbRequest, ThumbResponse,
};

const WORKER_FLAG: &str = "--media-worker";
const ARCHIVE_ENV: &str = "GT_ARCHIVE_DIR";
const MAX_MEDIA_FILES: usize = 20;
const MEDIA_TTL: Duration = Duration::from_secs(300);
const PARALLEL_DECRYPT_THRESHOLD: u64 = 10 * 1024 * 1024;
const PARALLEL_DECRYPT_WORKERS: usize = 4;

pub fn maybe_run_worker() -> bool {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == WORKER_FLAG) {
        if let Err(err) = run_worker() {
            eprintln!("media worker failed: {}", err);
        }
        return true;
    }
    false
}

pub struct MediaWorkerClient {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Response>>>>,
    next_id: AtomicU64,
    log_dir: Option<PathBuf>,
}

impl MediaWorkerClient {
    pub fn spawn(archive_dir: PathBuf, log_dir: Option<PathBuf>) -> Result<Self, String> {
        let exe = env::current_exe().map_err(|e| e.to_string())?;
        let mut child = Command::new(exe)
            .arg(WORKER_FLAG)
            .env(ARCHIVE_ENV, &archive_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| e.to_string())?;

        let stdin = child.stdin.take().ok_or_else(|| "worker stdin missing".to_string())?;
        let stdout = child.stdout.take().ok_or_else(|| "worker stdout missing".to_string())?;

        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Response>>>> = Arc::new(Mutex::new(HashMap::new()));
        start_reader_thread(stdout, Arc::clone(&pending));

        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending,
            next_id: AtomicU64::new(1),
            log_dir,
        })
    }

    pub fn request(&self, cmd: &str, payload: serde_json::Value) -> Result<Response, String> {
        let start = Instant::now();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        let inflight = {
            let mut guard = self.pending.lock().map_err(|_| "pending lock poisoned".to_string())?;
            guard.insert(id, tx);
            guard.len()
        };
        let request = Request {
            id,
            cmd: cmd.to_string(),
            payload,
        };
        let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        {
            let mut stdin = self.stdin.lock().map_err(|_| "stdin lock poisoned".to_string())?;
            stdin
                .write_all(line.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .map_err(|e| e.to_string())?;
            stdin.flush().map_err(|e| e.to_string())?;
        }
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(resp) => {
                self.log_timing(cmd, resp.ok, start.elapsed(), inflight, None);
                Ok(resp)
            }
            Err(err) => {
                if let Ok(mut guard) = self.pending.lock() {
                    guard.remove(&id);
                }
                let msg = err.to_string();
                self.log_timing(cmd, false, start.elapsed(), inflight, Some(&msg));
                Err("worker timeout".to_string())
            }
        }
    }

    pub fn shutdown(&self) {
        let _ = self.request("shutdown", json!({}));
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }

    fn log_timing(&self, cmd: &str, ok: bool, elapsed: Duration, inflight: usize, err: Option<&str>) {
        let Some(log_dir) = self.log_dir.as_ref() else { return };
        let ms = elapsed.as_millis();
        let mut msg = format!("cmd={} ok={} ms={} inflight={}", cmd, ok as u8, ms, inflight);
        if let Some(err) = err {
            msg.push_str(" err=");
            msg.push_str(err);
        }
        let _ = diagnostics::log_event(log_dir, "media_timing", &msg);
    }
}

impl Drop for MediaWorkerClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn start_reader_thread(stdout: ChildStdout, pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Response>>>>) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(response) = serde_json::from_str::<Response>(&line) {
                    if let Ok(mut guard) = pending.lock() {
                        if let Some(tx) = guard.remove(&response.id) {
                            let _ = tx.send(response);
                        }
                    }
                }
            }
        }
    });
}

fn run_worker() -> Result<(), String> {
    let archive_dir = env::var(ARCHIVE_ENV).map_err(|_| "missing archive dir".to_string())?;
    let archive_dir = PathBuf::from(archive_dir);
    let attachments_dir = archive_dir.join("attachments");
    let thumbs_dir = archive_dir.join("thumbs");
    let media_dir = archive_dir.join("previews").join("session").join("media");
    std::fs::create_dir_all(&media_dir).map_err(|e| e.to_string())?;

    let key = crypto::load_or_create_master_key().map_err(|e| e.to_string())?;
    let state = Arc::new(WorkerState::new(key, attachments_dir, thumbs_dir, media_dir));

    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        let resp = handle_request(&state, req);
        let out_line = serde_json::to_string(&resp).map_err(|e| e.to_string())?;
        out.write_all(out_line.as_bytes()).map_err(|e| e.to_string())?;
        out.write_all(b"\n").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        if resp.ok && resp.payload.as_ref().and_then(|v| v.get("shutdown")).is_some() {
            break;
        }
    }
    Ok(())
}

struct WorkerState {
    key: crypto::MasterKey,
    attachments_dir: PathBuf,
    thumbs_dir: PathBuf,
    media_dir: PathBuf,
    media_cache: Mutex<MediaCache>,
}

impl WorkerState {
    fn new(key: crypto::MasterKey, attachments_dir: PathBuf, thumbs_dir: PathBuf, media_dir: PathBuf) -> Self {
        Self {
            key,
            attachments_dir,
            thumbs_dir,
            media_dir,
            media_cache: Mutex::new(MediaCache::new()),
        }
    }
}

fn handle_request(state: &WorkerState, req: Request) -> Response {
    match req.cmd.as_str() {
        "thumb" => match parse_payload::<ThumbRequest>(&req.payload) {
            Ok(payload) => match handle_thumb(state, &payload) {
                Ok(res) => ok(req.id, json!(res)),
                Err(err) => response_err(req.id, err),
            },
            Err(err) => response_err(req.id, err),
        },
        "media" => match parse_payload::<MediaRequest>(&req.payload) {
            Ok(payload) => match handle_media(state, &payload) {
                Ok(res) => ok(req.id, json!(res)),
                Err(err) => response_err(req.id, err),
            },
            Err(err) => response_err(req.id, err),
        },
        "data_url" => match parse_payload::<DataUrlRequest>(&req.payload) {
            Ok(payload) => match handle_data_url(state, &payload) {
                Ok(res) => ok(req.id, json!(res)),
                Err(err) => response_err(req.id, err),
            },
            Err(err) => response_err(req.id, err),
        },
        "drain_evictions" => {
            let evictions = if let Ok(mut cache) = state.media_cache.lock() {
                cache.drain_evictions()
            } else {
                Vec::new()
            };
            ok(req.id, json!({ "sha256s": evictions }))
        }
        "clear_cache" => {
            if let Ok(mut cache) = state.media_cache.lock() {
                cache.clear();
            }
            ok(req.id, json!({"cleared": true}))
        }
        "shutdown" => ok(req.id, json!({"shutdown": true})),
        _ => response_err(req.id, "unknown command".to_string()),
    }
}

fn ok(id: u64, payload: serde_json::Value) -> Response {
    Response {
        id,
        ok: true,
        payload: Some(payload),
        error: None,
    }
}

fn response_err(id: u64, message: String) -> Response {
    Response {
        id,
        ok: false,
        payload: None,
        error: Some(message),
    }
}

fn parse_payload<T: DeserializeOwned>(payload: &serde_json::Value) -> Result<T, String> {
    serde_json::from_value(payload.clone()).map_err(|e| e.to_string())
}

fn handle_thumb(state: &WorkerState, payload: &ThumbRequest) -> Result<ThumbResponse, String> {
    let encrypted_thumb = state
        .thumbs_dir
        .join(format!("{}_{}.bin", payload.sha256, payload.max_size));
    if encrypted_thumb.exists() {
        let data = decrypt_to_bytes(&encrypted_thumb, &state.key)?;
        let encoded = BASE64_STANDARD.encode(data);
        return Ok(ThumbResponse {
            data_url: format!("data:image/webp;base64,{}", encoded),
        });
    }

    let attachment_path = state.attachments_dir.join(&payload.sha256);
    if !attachment_path.exists() {
        return Err("attachment missing".to_string());
    }

    let data = decrypt_to_bytes(&attachment_path, &state.key)?;
    let img = image::load_from_memory(&data).map_err(|e| e.to_string())?;
    let resized = img.resize(payload.max_size, payload.max_size, FilterType::Triangle);
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut webp_bytes: Vec<u8> = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut webp_bytes);
    encoder
        .encode(&rgba, w, h, ColorType::Rgba8.into())
        .map_err(|e| e.to_string())?;

    std::fs::create_dir_all(&state.thumbs_dir).map_err(|e| e.to_string())?;
    let mut reader = std::io::Cursor::new(&webp_bytes);
    let mut temp = tempfile::NamedTempFile::new_in(&state.thumbs_dir).map_err(|e| e.to_string())?;
    crypto::encrypt_stream(&mut reader, &mut temp, &state.key).map_err(|e| e.to_string())?;
    // Handle race condition: if another request already created this file, that's fine
    match temp.persist(&encrypted_thumb) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }

    let encoded = BASE64_STANDARD.encode(webp_bytes);
    Ok(ThumbResponse {
        data_url: format!("data:image/webp;base64,{}", encoded),
    })
}

fn handle_media(state: &WorkerState, payload: &MediaRequest) -> Result<MediaResponse, String> {
    let ext = payload
        .mime
        .as_deref()
        .and_then(mime_extension)
        .unwrap_or("bin");
    let cache_key = format!("{}:{}", payload.sha256, ext);

    if let Ok(mut cache) = state.media_cache.lock() {
        if let Some(path) = cache.get(&cache_key) {
            return Ok(MediaResponse {
                path: path.to_string_lossy().to_string(),
            });
        }
    }

    let attachment_path = state.attachments_dir.join(&payload.sha256);
    if !attachment_path.exists() {
        return Err("attachment missing".to_string());
    }

    let plaintext_len = crypto::encrypted_plaintext_len(&attachment_path).ok();
    std::fs::create_dir_all(&state.media_dir).map_err(|e| e.to_string())?;
    let preview_path = state.media_dir.join(format!("{}.{}", payload.sha256, ext));
    let mut reader = std::fs::File::open(&attachment_path).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(&state.media_dir).map_err(|e| e.to_string())?;
    if let Some(len) = plaintext_len {
        if len >= PARALLEL_DECRYPT_THRESHOLD {
            drop(reader);
            crypto::decrypt_file_parallel(&attachment_path, temp.path(), &state.key, PARALLEL_DECRYPT_WORKERS)
                .map_err(|e| e.to_string())?;
        } else if len > 0 {
            let file = temp.as_file_mut();
            let _ = file.set_len(len);
            let _ = file.seek(SeekFrom::Start(0));
            crypto::decrypt_stream(&mut reader, &mut temp, &state.key).map_err(|e| e.to_string())?;
        }
    } else {
        crypto::decrypt_stream(&mut reader, &mut temp, &state.key).map_err(|e| e.to_string())?;
    }
    // Handle race condition: if another request already created this file, that's fine
    match temp.persist(&preview_path) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }

    if let Ok(mut cache) = state.media_cache.lock() {
        cache.insert(cache_key, preview_path.clone());
    }

    Ok(MediaResponse {
        path: preview_path.to_string_lossy().to_string(),
    })
}

fn handle_data_url(state: &WorkerState, payload: &DataUrlRequest) -> Result<DataUrlResponse, String> {
    let attachment_path = state.attachments_dir.join(&payload.sha256);
    if !attachment_path.exists() {
        return Err("attachment missing".to_string());
    }
    let meta = std::fs::metadata(&attachment_path).map_err(|e| e.to_string())?;
    if meta.len() > payload.max_bytes {
        return Err("media too large to preview".to_string());
    }
    let data = decrypt_to_bytes(&attachment_path, &state.key)?;
    let encoded = BASE64_STANDARD.encode(data);
    Ok(DataUrlResponse {
        data_url: format!("data:{};base64,{}", payload.mime, encoded),
    })
}

fn decrypt_to_bytes(path: &Path, key: &crypto::MasterKey) -> Result<Vec<u8>, String> {
    let mut reader = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut out: Vec<u8> = Vec::new();
    crypto::decrypt_stream(&mut reader, &mut out, key).map_err(|e| e.to_string())?;
    Ok(out)
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

struct MediaCacheEntry {
    path: PathBuf,
    last_access: Instant,
}

struct MediaCache {
    entries: HashMap<String, MediaCacheEntry>,
    evicted: Vec<String>,
}

impl MediaCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            evicted: Vec::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<PathBuf> {
        self.evict_expired();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = Instant::now();
            return Some(entry.path.clone());
        }
        None
    }

    fn insert(&mut self, key: String, path: PathBuf) {
        self.entries.insert(
            key,
            MediaCacheEntry {
                path,
                last_access: Instant::now(),
            },
        );
        self.evict_expired();
        self.evict_lru();
    }

    fn clear(&mut self) {
        for entry in self.entries.values() {
            let _ = std::fs::remove_file(&entry.path);
        }
        self.entries.clear();
        self.evicted.clear();
    }

    fn evict_expired(&mut self) {
        let now = Instant::now();
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_access) > MEDIA_TTL)
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired {
            if let Some(entry) = self.entries.remove(&key) {
                let _ = std::fs::remove_file(&entry.path);
                self.record_eviction(&key);
            }
        }
    }

    fn evict_lru(&mut self) {
        while self.entries.len() > MAX_MEDIA_FILES {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, entry)| (key.clone(), entry.path.clone()));
            if let Some((key, path)) = oldest {
                let _ = std::fs::remove_file(&path);
                self.entries.remove(&key);
                self.record_eviction(&key);
            } else {
                break;
            }
        }
    }

    fn record_eviction(&mut self, key: &str) {
        if let Some((sha, _)) = key.split_once(':') {
            self.evicted.push(sha.to_string());
        } else {
            self.evicted.push(key.to_string());
        }
    }

    fn drain_evictions(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        std::mem::swap(&mut out, &mut self.evicted);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn media_cache_eviction() {
        let mut cache = MediaCache::new();
        for idx in 0..(MAX_MEDIA_FILES + 2) {
            cache.insert(format!("k{}", idx), PathBuf::from(format!("/tmp/{}.bin", idx)));
        }
        assert!(cache.entries.len() <= MAX_MEDIA_FILES);
    }

    #[test]
    fn data_url_rejects_large() {
        std::env::set_var(
            "GT_MASTER_KEY_HEX",
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        );
        let dir = tempdir().expect("temp");
        let attach_dir = dir.path().join("attachments");
        std::fs::create_dir_all(&attach_dir).expect("attachments");
        let path = attach_dir.join("deadbeef");
        std::fs::write(&path, vec![0u8; 10]).expect("write");

        let key = crypto::load_or_create_master_key().expect("key");
        let state = WorkerState::new(key, attach_dir, dir.path().join("thumbs"), dir.path().join("media"));

        let req = DataUrlRequest {
            sha256: "deadbeef".to_string(),
            mime: "image/png".to_string(),
            max_bytes: 0,
        };
        let err = handle_data_url(&state, &req).err().unwrap();
        assert!(err.contains("too large"));
    }
}
