## Encryption at rest + decryption flow

### Goals (v1)
- All imported Signal data is read-only.
- Sensitive data is encrypted at rest on disk (database + attachments + thumbs).
- Decryption happens only on demand and only for display.
- UI stays responsive: heavy decrypt work runs on a blocking thread pool via `spawn_blocking`.
- No network calls by default.

### What is encrypted at rest
- `archive.sqlite` is encrypted via SQLCipher. The master key is stored in macOS Keychain and loaded once on first use.
- `attachments/` store encrypted blobs, named by SHA256.
- `thumbs/` store encrypted WebP thumbnails (per size, per attachment).
- `previews/session/media/` stores *temporary* decrypted media files for playback. These are not durable and are cleared on exit or eviction.

### Decryption model (high level)
- **Tauri commands are async** - they return Promises to the frontend.
- **CPU-bound work uses `spawn_blocking`** - decryption and thumbnail generation run on tokio's blocking thread pool, keeping the async runtime responsive.
- **Single process** - all operations run in the main Tauri process; no subprocess IPC overhead.
- Responses return either:
  - a data URL (for small items / thumbnails), or
  - a temp file path (for large video/audio/image).

### Tauri commands (media operations)
- `attachment_thumbnail_cmd`: returns a data URL for image thumbnails.
- `attachment_data_url_cmd`: returns a data URL (small media only).
- `attachment_path_cmd`: decrypts to a temp file, returns a path for `convertFileSrc()`.
- `clear_media_cache_cmd`: clears temp preview cache.
- `drain_media_evictions_cmd`: returns SHA256s of media files evicted by LRU/TTL.

### Temp media cache + eviction
- An in-memory LRU cache tracks decrypted media files (max count + TTL).
- When evicted, SHA256s are returned on `drain_media_evictions_cmd`.
- UI polls evictions while Gallery is active and resets matching cards to "Click to load".
- On gallery exit, UI clears media cache and restores placeholders.

### Thumbnail flow (gallery)
- Gallery thumbnails are loaded lazily via `IntersectionObserver`.
- A small LIFO queue prioritizes most recently viewed items, avoiding long spinner tails.
- While loading, the placeholder shows a spinner and the underlying `<img>` is collapsed.

### Performance knobs already in place
- AES-GCM chunk size: 1MB default, 4MB for large attachments (>=10MB).
- Parallel decryption: Files >=10MB use 4 worker threads with positional I/O (`read_at`/`write_at`) to avoid seek contention.
- Decrypt temp pre-allocation: worker computes plaintext size and pre-allocates output file.
- Thumbnail concurrency cap: 4.
- Cache sizes are kept modest to avoid UI stalls.
- Multi-tier frontend caching: weighted LRU for thumbnails (64MB by weight), count-based LRU for file URLs (200 items) and data URLs (40 items).

### Security notes
- Passphrase is never written to disk.
- Logs are redacted and avoid secrets.
- UI shows placeholders when media is not yet decrypted; decrypted data should be cleared on exit.

### Security tradeoffs (documented)

The following are intentional tradeoffs for a personal, local-only application. These would need reconsideration before any broader distribution.

#### 1. SQLCipher cipher_memory_security = OFF
**Location:** `core/src/db.rs`

SQLCipher normally zeros decrypted database pages after use. Setting `cipher_memory_security = OFF` disables this behavior, leaving decrypted data in memory for performance (~20-30% query improvement).

**Risk:** A memory dump could expose decrypted message content.

**Mitigation:** Acceptable for personal use on a single-user machine. For broader distribution, consider making this configurable or defaulting to ON.

#### 2. Master key cache does not use Zeroizing wrapper
**Location:** `core/src/crypto.rs` (MASTER_KEY_CACHE)

The master key is cached in a static `OnceLock<[u8; 32]>` for the process lifetime. While individual `MasterKey` instances use `Zeroizing<[u8; 32]>`, the cached copy persists as raw bytes.

**Risk:** The master key remains in process memory until exit.

**Mitigation:** For a personal app that holds the key for its entire runtime anyway, this is acceptable. The key is never written to disk (stored in macOS Keychain).

#### 3. Decrypted preview files persist on disk
**Location:** `previews/session/media/`

Decrypted media files are written to disk for playback (video/audio require file paths, not data URLs). These are cleared on normal app exit or LRU eviction, but may persist after crashes or force-quit.

**Risk:** Unencrypted media files could remain on disk after abnormal termination.

**Mitigation options:**
- For personal use: Document as known limitation
- For broader distribution: Investigate macOS Data Vault integration or startup cleanup

#### 4. Environment variable key override (debug/test only)
**Location:** `core/src/crypto.rs`

The `GT_MASTER_KEY_HEX` environment variable allows overriding the master key for testing. This is restricted to debug/test builds via `#[cfg(any(debug_assertions, test))]`.

**Rationale:** Environment variables are visible via `ps eww` on macOS, which would expose the key to other processes. Release builds must use the macOS Keychain.

### Where to look in code
- Media operations: `app/src-tauri/src/media_ops.rs`
- Tauri commands: `app/src-tauri/src/main.rs`
- Gallery UI + placeholders: `app/src/ui/gallery.ts`
- Crypto primitives: `core/src/crypto.rs`
- Importer encryption: `core/src/importer/attachments.rs`

