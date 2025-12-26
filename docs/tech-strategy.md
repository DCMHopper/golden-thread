# Importer strategy (GPL-allowed)

This document restarts the technical strategy now that GPL/open-source reuse is allowed.

## Goal
Import Signal Android `.backup` files into a local archive (SQLite + attachments), with incremental merge and a read-only UI.

## Candidate upstreams
### A) signalbackup-tools (GPL)
The README describes a mature CLI tool for Signal Android backups with features beyond simple decoding:
- Decrypts backups and can dump the decrypted database and media.
- Can repair broken backups.
- Can export to multiple formats (HTML/TXT/CSV/XML).
- Can crop to threads/date ranges, merge backups, and import from Signal Desktop/JSON exports.
- Notes that Signal’s database format changes periodically and the tool sometimes breaks until updated.

Implications for us:
- It already contains robust parsing/crypto logic and a deep understanding of the Android backup/DB format.
- The README indicates it can dump the decrypted DB and media, which matches our import pipeline need.
- Since GPL is allowed, we can reuse components directly instead of re-deriving.

### B) signal-backup-decode (GPL)
A smaller Rust tool that implements:
- Backup framing, AES-CTR decryption, truncated HMAC verification.
- Protobuf `BackupFrame` parsing with the same field numbers/types.
- Exposes a simple frame iterator model.

Implications for us:
- Easier to embed into `/core` (Rust), but it is smaller and less feature-rich than signalbackup-tools.
- Best fit if we want a lightweight decoder and then build our own mapping logic to the archive schema.

## Recommended strategy (least reinvention)
Given the current goals and GPL allowed, the fastest path is to reuse existing decoding logic and focus our custom work on mapping to the archive schema and incremental merges.

### Option 1 (fastest overall): embed signalbackup-tools logic
- Use its decoding/repair logic to produce a decrypted SQLite db + media. citeturn4search0
- Then implement a translator that maps Signal’s DB tables to our archive schema.
- Pros: Most mature and likely most robust with edge cases.
- Cons: It’s C++ and heavier to integrate into the Rust core (FFI or separate module). Also larger dependency surface.

### Option 2 (balanced): embed signal-backup-decode logic
- Keep everything inside Rust core.
- Implement streaming decode + SQL replay into a temp Signal DB.
- Then map Signal DB into archive schema.
- Pros: Rust-native, smaller, easier integration with existing core.
- Cons: Fewer recovery/repair features; may need more edge-case handling.

## Proposed implementation path (if we choose Option 2)
1) Add decoder module to `/core` (ported/embedded from signal-backup-decode).
2) Rebuild Signal SQLite via replaying `SqlStatement` frames into a temp DB.
3) Extract threads, recipients, messages, attachments, reactions from temp DB.
4) Store attachments by sha256 and map messages to archive schema.
5) Merge into existing archive with dedupe rules.

## Near-term decision
With GPL allowed, we will use **signalbackup-tools** for maturity and breadth of backup/version handling.

Integration options for signalbackup-tools (chosen):
1) **Static library + FFI** (selected)
   - Build the C++ sources into a static lib via `build.rs` (CMake).
   - Expose a small C API wrapper (`gt_decode_backup(...)`).
   - Rust calls directly into the lib; no external process.
2) **Bundled CLI invoked by core** (not chosen)
   - Build the CLI during app build and ship it alongside the app bundle.
   - Rust spawns it with controlled args and parses structured output (JSON or logs).
   - Simpler initially, but adds process management and stdout parsing.

If you choose signal-backup-decode, we proceed with a Rust-native importer module and move immediately to SQL replay + mapping.

## Notes on format volatility
Signal’s database and backup formats change over time; upstream tools note occasional breakage until updates. We should:
- detect DB version and store it in `imports`.
- fail gracefully with clear errors if version is unknown.
- keep unknown fields in `metadata_json`.
