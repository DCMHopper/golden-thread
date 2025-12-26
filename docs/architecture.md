# architecture and scope boundaries

## Target platform
- macOS (Apple Silicon + Intel if feasible)

## Licensing note
- GPL is allowed for this project. We currently embed `signalbackup-tools` via a static library + FFI.

## High-level architecture
Three layers.

1. Importer (write path)
- Reads `.backup` + passphrase.
- Decrypts and parses into a normalized archive store.
- Only component that writes to the archive.

2. Archive store (data)
- SQLite database as canonical store.
- FTS index for message search.
- Attachments stored on disk, referenced by hash from SQLite.

3. UI (read-only)
- All interactions are reads against the archive store via a query API.
- No direct writes to archive store.
- Frontend is organized as a small module set under `app/src/ui/` with a typed Tauri API wrapper.

## Recommended stack
Option A (recommended):
- Tauri app
- Rust core crate for importer + query API
- UI: web (React/Preact/Svelte) inside Tauri

Option B:
- SwiftUI app
- Rust core as a dynamic library or separate helper process

This repo assumes Option A unless explicitly changed.

## Data model (v1)
SQLite file: `archive.sqlite`

Tables (suggested)
- `imports`
  - id, imported_at, source_filename, source_hash, detected_version, status
- `threads`
  - id (stable), name, last_message_at, avatar_attachment_hash (optional)
- `recipients`
  - id (stable), phone/e164 (optional), profile_name, contact_name
- `thread_members`
  - thread_id, recipient_id
- `messages`
  - id (stable if available), thread_id, sender_id, sent_at, received_at
  - type (enum), body (text), is_outgoing, is_view_once (flag if available)
  - quote_message_id (optional), metadata_json (optional for unknown fields)
- `attachments`
  - id, message_id, sha256, mime, size_bytes, original_filename
  - kind (image/video/audio/file/sticker), width/height/duration_ms (optional)
- `reactions`
  - message_id, reactor_id, emoji, reacted_at
- `message_fts`
  - FTS5 virtual table indexing `messages.body` plus optionally sender/thread tokens
- `tags`
  - id (timestamp-based), name (unique), color (hex), created_at, display_order
- `message_tags`
  - message_id, tag_id, tagged_at (when tag was applied)
  - CASCADE DELETE on both foreign keys

### ID normalization
Signal uses separate `sms` and `mms` tables with overlapping integer IDs. We store a unified `messages` table, so IDs are normalized as strings to avoid collisions:

- SMS message id `42` → `sms:42`
- MMS message id `42` → `mms:42`

Any ingest or query layer that references message IDs must either:
1) Preserve the `sms:`/`mms:` prefix when a string id is provided, or
2) Assume an MMS id for raw integers (the common case for attachments/reactions tables).

This normalization avoids collisions and simplifies queries by keeping a single `messages` table. If a future refactor adds `source_table` + `source_id`, this section should be updated.

Attachment storage
- `attachments/sha256_<hash>` or `<hash>` as filename
- `thumbs/<hash>_<size>.jpg` (or png/webp) generated lazily

## Import invariants
- Import is transactional.
- If import fails, archive remains unchanged.
- Incremental imports:
  - Identify duplicates by stable message id if available.
  - Otherwise, use a composite key (thread + sender + timestamp + body hash) as fallback.
- Attachment dedupe:
  - compute sha256 while streaming
  - store once, reference many

## Backup format volatility strategy
- Detect and persist backup version information when possible.
- If an unsupported version is detected:
  - fail fast
  - do not write partial data
  - show actionable error (“unsupported backup version X”)

## Query API surface (backend)
- list threads
- get thread summary (members, last message)
- paginate messages (anchor + direction + limit)
- search messages (query + filters)
- list media (global, per thread)
- fetch attachment by hash (and thumbnail path)
- tag management (create, update, delete, list)
- message tagging (get tags for message, set tags)
- scrapbook view (list tagged messages with discontinuity detection)

## Frontend structure (app)
- `app/src/main.ts`: main entry and wiring
- `app/src/ui/`: UI modules + typed API wrapper
- `app/src/styles.css`: stylesheet entrypoint importing `app/src/styles/*`

## Privacy and security defaults
- disable all network usage at build time if possible
- no remote fonts
- no auto-update checks unless explicitly enabled and still privacy-safe
- redact logs by default; provide an optional local debug log toggle
