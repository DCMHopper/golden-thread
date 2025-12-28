Code review focus: changes after commit d21f21f (encryption-at-rest + media worker + perf). Reviewed docs: `docs/encryption-at-rest.md`, `docs/architecture.md`, `docs/implementation-plan.md`, `docs/future-improvements.md`.

Goals alignment check
- The changes align with the mission: read-only UI, encrypted at rest, no network by default, lazy media loading, and performance isolation via a worker. Good trajectory for Milestones 3–5.
- Security posture is improved (SQLCipher + encrypted attachments/thumbs), but some implementation details could lead to UX regressions or intermittent failures under concurrency.

Findings (ordered by severity)

1) Potential race: concurrent decrypt requests can fail when the preview/thumbnail file already exists
- Why it matters: The UI can request the same media multiple times (gallery + lightbox + message view). If two worker requests race, both will decrypt and then `persist()` into the same destination. The second persist fails with “file exists,” causing a user-visible error even though the file is already present. This is especially likely with rapid scrolling + lightbox use.
- Where:
  - `app/src-tauri/src/media_worker.rs:308-312` (thumb persist)
  - `app/src-tauri/src/media_worker.rs:343-360` (media persist)
- Suggested fix:
  - If `persist()` fails with “already exists”, return success using the existing path; or
  - Write to a unique temp name, then `rename` with `std::fs::rename` and treat `AlreadyExists` as success; or
  - Check for existing path again right before persist and skip persist if it now exists.

2) Media worker data URL size gate uses encrypted size, not plaintext size
- Why it matters: `meta.len()` includes header + tags. For large-but-eligible media, the worker may reject a data URL even though plaintext is within limit. This manifests as inconsistent “too large to preview” errors for files near the limit.
- Where: `app/src-tauri/src/media_worker.rs:371-380`
- Suggested fix: Use `crypto::encrypted_plaintext_len()` for the comparison and fall back to `meta.len()` only if plaintext size cannot be derived.

3) File URL cache key ignores mime/extension, risking wrong path reuse
- Why it matters: The worker writes decrypted previews as `sha256.ext`, where ext depends on mime. The frontend caches by `sha256` alone, so a call with `mime=null` can cache a `sha256.bin` path that then gets reused for an image/video with a more specific ext. Some media elements (especially video) can be picky about extension/content type combos, and the cache may return a path that no longer exists after evictions.
- Where: `app/src/main.ts:1421-1429`
- Suggested fix: Use `const key = `${sha256}:${mime ?? ""}`` (or ext) for `attachmentFileCache` entries, and clear all entries for a sha256 on eviction.

4) Worker request timeout is fixed at 30s and may be too short for large decrypts
- Why it matters: Large attachments (e.g., long videos) can legitimately take >30s to decrypt on slower disks. A timeout will surface as a user-facing error even though the worker may complete shortly after. This also leaves a preview file on disk without a cache entry, increasing race potential.
- Where: `app/src-tauri/src/media_worker.rs:78-113`
- Suggested fix:
  - Use a longer timeout for `media` commands, or
  - Make timeout proportional to plaintext size (if known), or
  - Accept a “pending” response and let the UI poll for completion.

5) Unbounded memory use for thumbnail generation on huge images
- Why it matters: `handle_thumb` decrypts the full attachment into memory, then decodes it to RGBA before resizing. Very large images can trigger high memory usage and potential OOM. This hurts the “UI stays responsive” goal.
- Where: `app/src-tauri/src/media_worker.rs:262-307`
- Suggested fix:
  - Use `crypto::encrypted_plaintext_len` to reject images beyond a safe size and return an error/placeholder.
  - Consider streaming decode or using image crate’s resize with limits when possible.

6) Master key cached as raw bytes in process memory without zeroization
- Why it matters: `MasterKey` uses `Zeroizing`, but `MASTER_KEY_CACHE` stores raw `[u8; 32]` in a `OnceLock`, so the key remains in process memory for the lifetime of the app without automatic zeroization. Given the “encryption at rest” goal, this is a defensible perf tradeoff, but it should be intentional and documented.
- Where: `core/src/crypto.rs:39-58`
- Suggested fix: Store `Zeroizing<[u8;32]>` in `OnceLock` or explicitly note the tradeoff in `docs/encryption-at-rest.md` under “Security notes.”

7) `GT_MASTER_KEY_HEX` environment override is production-accessible
- Why it matters: If this env var is set in production (even unintentionally), it bypasses keychain storage and uses a potentially weak key. This is fine for tests but risky in prod.
- Where: `core/src/crypto.rs:51-59`
- Suggested fix: Gate env override behind `cfg!(test)` or a debug build flag; or treat it as a hard-fail in release builds unless explicitly configured.

Moderate/low-priority observations

- `cipher_memory_security = OFF` in `open_archive` improves perf but disables memory zeroing for SQLCipher. Consider documenting this tradeoff in `docs/encryption-at-rest.md` or gating it behind a “performance mode.”
  - Where: `core/src/db.rs:21-30`
- `handle_media` decrypts serially in the worker; a single large media request blocks all other media/thumbnail requests. Consider a small internal thread pool or at least splitting thumb/media workers to keep UI snappy.
  - Where: `app/src-tauri/src/media_worker.rs:226-369`

Tests/verification gaps
- No tests cover worker file collision or concurrent media requests. Consider adding a unit test or integration test that simulates two `media` requests for the same sha256 and ensures one succeeds without error.
- No tests cover thumbnail generation for large images or limits.

Suggested next steps (aligned with mission)
1) Fix the media/thumbnail persist race and file cache keying; these are the most likely sources of user-visible errors under normal use.
2) Switch data URL size check to plaintext size and consider per-command timeouts based on file size.
3) Decide on a documented policy for master key caching and SQLCipher memory security (performance vs. in-memory exposure).
4) Add a small concurrency test for media worker + update docs with the final behavior and tradeoffs.

Notes
- I did not run tests. If you want, I can run `cargo test -p golden_thread_core` and a minimal Tauri smoke build next.
