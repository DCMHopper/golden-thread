## Encryption at rest + decryption flow (UI + worker)

### Goals (v1)
- All imported Signal data is read-only.
- Sensitive data is encrypted at rest on disk (database + attachments + thumbs).
- Decryption happens only on demand and only for display.
- UI stays responsive: heavy decrypt work runs in a separate process (media worker).
- No network calls by default.

### What is encrypted at rest
- `archive.sqlite` is encrypted via SQLCipher. The master key is stored in macOS Keychain and loaded on startup.
- `attachments/` store encrypted blobs, named by SHA256.
- `thumbs/` store encrypted WebP thumbnails (per size, per attachment).
- `previews/session/media/` stores *temporary* decrypted media files for playback. These are not durable and are cleared on exit or eviction.

### Decryption model (high level)
- **UI process never decrypts on its own.**
- A **media worker** process does all decryption and thumbnail work.
- UI talks to the worker via Tauri commands -> IPC over stdin/stdout JSON.
- Worker responses return either:
  - a data URL (for small items / thumbnails), or
  - a temp file path (for large video/audio/image).

### UI <-> Worker interaction (commands)
- `attachment_thumbnail_cmd`: returns a data URL for image thumbnails.
- `attachment_data_url_cmd`: returns a data URL (small media only).
- `attachment_path_cmd`: decrypts to a temp file, returns a path for `convertFileSrc()`.
- `clear_media_cache_cmd`: clears temp preview cache + signals worker to purge.
- `drain_media_evictions_cmd`: returns SHA256s of media files evicted by worker LRU/TTL.

### Temp media cache + eviction
- Worker keeps a small LRU cache of decrypted media files (max count + TTL).
- When evicted, worker returns SHA256s on `drain_media_evictions_cmd`.
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
- Worker + IPC: `app/src-tauri/src/media_worker.rs`, `app/src-tauri/src/media_ipc.rs`
- Tauri commands: `app/src-tauri/src/main.rs`
- Gallery UI + placeholders: `app/src/main.ts`
- Crypto primitives: `core/src/crypto.rs`
- Importer encryption: `core/src/importer/attachments.rs`

