//! Media operations module - handles thumbnail generation, decryption, and caching.
//!
//! This module replaces the media worker subprocess with in-process operations
//! that run on tokio's blocking thread pool via `spawn_blocking`.

use std::collections::HashMap;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use golden_thread_core::crypto::{self, MasterKey};
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::ColorType;

const MAX_MEDIA_FILES: usize = 20;
const MEDIA_TTL: Duration = Duration::from_secs(300);
const PARALLEL_DECRYPT_THRESHOLD: u64 = 10 * 1024 * 1024;
const PARALLEL_DECRYPT_WORKERS: usize = 4;

/// Shared state for media operations.
pub struct MediaState {
    pub key: Arc<MasterKey>,
    pub attachments_dir: PathBuf,
    pub thumbs_dir: PathBuf,
    pub media_dir: PathBuf,
    pub cache: Mutex<MediaCache>,
}

impl MediaState {
    pub fn new(
        key: MasterKey,
        attachments_dir: PathBuf,
        thumbs_dir: PathBuf,
        media_dir: PathBuf,
    ) -> Self {
        std::fs::create_dir_all(&media_dir).ok();
        std::fs::create_dir_all(&thumbs_dir).ok();
        Self {
            key: Arc::new(key),
            attachments_dir,
            thumbs_dir,
            media_dir,
            cache: Mutex::new(MediaCache::new()),
        }
    }
}

/// Generate or load a cached thumbnail, returning a data URL.
pub fn generate_thumbnail(
    state: &MediaState,
    sha256: &str,
    max_size: u32,
) -> Result<String, String> {
    // Wrap in catch_unwind for crash isolation
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generate_thumbnail_inner(state, sha256, max_size)
    }))
    .map_err(|_| "thumbnail generation panicked".to_string())?
}

fn generate_thumbnail_inner(
    state: &MediaState,
    sha256: &str,
    max_size: u32,
) -> Result<String, String> {
    let encrypted_thumb = state
        .thumbs_dir
        .join(format!("{}_{}.bin", sha256, max_size));

    // Check for cached encrypted thumbnail
    if encrypted_thumb.exists() {
        let data = decrypt_to_bytes(&encrypted_thumb, &state.key)?;
        let encoded = BASE64_STANDARD.encode(data);
        return Ok(format!("data:image/webp;base64,{}", encoded));
    }

    // Generate from source attachment
    let attachment_path = state.attachments_dir.join(sha256);
    if !attachment_path.exists() {
        return Err("attachment missing".to_string());
    }

    let data = decrypt_to_bytes(&attachment_path, &state.key)?;
    let img = image::load_from_memory(&data).map_err(|e| e.to_string())?;
    let resized = img.resize(max_size, max_size, FilterType::Triangle);
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut webp_bytes: Vec<u8> = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut webp_bytes);
    encoder
        .encode(&rgba, w, h, ColorType::Rgba8.into())
        .map_err(|e| e.to_string())?;

    // Cache the encrypted thumbnail
    std::fs::create_dir_all(&state.thumbs_dir).map_err(|e| e.to_string())?;
    let mut reader = std::io::Cursor::new(&webp_bytes);
    let mut temp =
        tempfile::NamedTempFile::new_in(&state.thumbs_dir).map_err(|e| e.to_string())?;
    crypto::encrypt_stream(&mut reader, &mut temp, &state.key).map_err(|e| e.to_string())?;

    // Atomic rename (handle race condition)
    match temp.persist(&encrypted_thumb) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }

    let encoded = BASE64_STANDARD.encode(webp_bytes);
    Ok(format!("data:image/webp;base64,{}", encoded))
}

/// Decrypt attachment to a preview file, returning the file path.
pub fn decrypt_to_preview(
    state: &MediaState,
    sha256: &str,
    mime: Option<&str>,
) -> Result<String, String> {
    // Wrap in catch_unwind for crash isolation
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decrypt_to_preview_inner(state, sha256, mime)
    }))
    .map_err(|_| "media decryption panicked".to_string())?
}

fn decrypt_to_preview_inner(
    state: &MediaState,
    sha256: &str,
    mime: Option<&str>,
) -> Result<String, String> {
    let ext = mime.and_then(mime_extension).unwrap_or("bin");
    let cache_key = format!("{}:{}", sha256, ext);

    // Check in-memory cache
    if let Ok(mut cache) = state.cache.lock() {
        if let Some(path) = cache.get(&cache_key) {
            return Ok(path.to_string_lossy().to_string());
        }
    }

    let attachment_path = state.attachments_dir.join(sha256);
    if !attachment_path.exists() {
        return Err("attachment missing".to_string());
    }

    let plaintext_len = crypto::encrypted_plaintext_len(&attachment_path).ok();
    std::fs::create_dir_all(&state.media_dir).map_err(|e| e.to_string())?;

    let preview_path = state.media_dir.join(format!("{}.{}", sha256, ext));
    let mut temp =
        tempfile::NamedTempFile::new_in(&state.media_dir).map_err(|e| e.to_string())?;

    // Use parallel decryption for large files
    if let Some(len) = plaintext_len {
        if len >= PARALLEL_DECRYPT_THRESHOLD {
            crypto::decrypt_file_parallel(
                &attachment_path,
                temp.path(),
                &state.key,
                PARALLEL_DECRYPT_WORKERS,
            )
            .map_err(|e| e.to_string())?;
        } else if len > 0 {
            let mut reader =
                std::fs::File::open(&attachment_path).map_err(|e| e.to_string())?;
            let file = temp.as_file_mut();
            let _ = file.set_len(len);
            let _ = file.seek(SeekFrom::Start(0));
            crypto::decrypt_stream(&mut reader, &mut temp, &state.key)
                .map_err(|e| e.to_string())?;
        }
    } else {
        let mut reader =
            std::fs::File::open(&attachment_path).map_err(|e| e.to_string())?;
        crypto::decrypt_stream(&mut reader, &mut temp, &state.key)
            .map_err(|e| e.to_string())?;
    }

    // Atomic rename
    match temp.persist(&preview_path) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }

    // Update cache
    if let Ok(mut cache) = state.cache.lock() {
        cache.insert(cache_key, preview_path.clone());
    }

    Ok(preview_path.to_string_lossy().to_string())
}

/// Generate a data URL for small attachments.
pub fn generate_data_url(
    state: &MediaState,
    sha256: &str,
    mime: &str,
    max_bytes: u64,
) -> Result<String, String> {
    let attachment_path = state.attachments_dir.join(sha256);
    if !attachment_path.exists() {
        return Err("attachment missing".to_string());
    }

    let meta = std::fs::metadata(&attachment_path).map_err(|e| e.to_string())?;
    if meta.len() > max_bytes {
        return Err("media too large to preview".to_string());
    }

    let data = decrypt_to_bytes(&attachment_path, &state.key)?;
    let encoded = BASE64_STANDARD.encode(data);
    Ok(format!("data:{};base64,{}", mime, encoded))
}

/// Clear all cached preview files.
pub fn clear_cache(state: &MediaState) {
    if let Ok(mut cache) = state.cache.lock() {
        cache.clear();
    }
}

/// Drain list of evicted SHA256s for frontend cache sync.
pub fn drain_evictions(state: &MediaState) -> Vec<String> {
    if let Ok(mut cache) = state.cache.lock() {
        cache.drain_evictions()
    } else {
        Vec::new()
    }
}

// --- Helper functions ---

fn decrypt_to_bytes(path: &Path, key: &MasterKey) -> Result<Vec<u8>, String> {
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

// --- MediaCache ---

pub struct MediaCache {
    entries: HashMap<String, MediaCacheEntry>,
    evicted: Vec<String>,
}

struct MediaCacheEntry {
    path: PathBuf,
    last_access: Instant,
}

impl MediaCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            evicted: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<PathBuf> {
        self.evict_expired();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = Instant::now();
            return Some(entry.path.clone());
        }
        None
    }

    pub fn insert(&mut self, key: String, path: PathBuf) {
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

    pub fn clear(&mut self) {
        for entry in self.entries.values() {
            let _ = std::fs::remove_file(&entry.path);
        }
        self.entries.clear();
        self.evicted.clear();
    }

    pub fn drain_evictions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.evicted)
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
}

impl Default for MediaCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_cache_eviction() {
        let mut cache = MediaCache::new();
        for idx in 0..(MAX_MEDIA_FILES + 2) {
            cache.insert(
                format!("k{}", idx),
                PathBuf::from(format!("/tmp/{}.bin", idx)),
            );
        }
        assert!(cache.entries.len() <= MAX_MEDIA_FILES);
    }

    #[test]
    fn mime_extension_mapping() {
        assert_eq!(mime_extension("image/jpeg"), Some("jpg"));
        assert_eq!(mime_extension("video/mp4"), Some("mp4"));
        assert_eq!(mime_extension("audio/mpeg"), Some("mp3"));
        assert_eq!(mime_extension("application/pdf"), None);
    }
}
