# agent guide

## Mission
Build a mac app that imports Signal Android `.backup` files into a local archive and provides read-only browsing + search + media viewing (read-only for imported Signal data; local annotations like tags are allowed).

## Hard constraints
- mac only
- android `.backup` only
- read-only UI for imported Signal data (local annotations like tags are OK)
- no network calls by default
- you may not ever read user data during development: chats, thumbnails, contacts, etc. this information is private to the user

## Repo expectations
- `/core` contains all logic for:
  - decrypting/parsing `.backup`
  - writing SQLite archive
  - query API used by UI
- `/app` contains:
  - tauri shell
  - UI components
  - invokes `/core` via tauri commands

## Definition of done for v1
- Import a real `.backup` successfully with correct passphrase.
- Wrong passphrase fails fast with clear error.
- Thread list renders.
- Conversation view renders with infinite scroll.
- Search works across all messages and opens the match in context.
- Media loads lazily and does not freeze the UI.
- Importing a newer backup only adds new data (no duplicates).

## Implementation guidance
### Import safety
- Use a single SQLite transaction per import, or stage into a temp db then swap.
- Never write passphrase to disk.
- Avoid logging secrets. Redact aggressively.

### Storage
- `archive.sqlite` in app support directory.
- `attachments/` and `thumbs/` sibling directories.
- attachments are stored by sha256 to dedupe.

### Search
- Use SQLite FTS5 for message body indexing.
- Store enough metadata for filters without scanning whole tables.

### UI performance
- virtualize message list
- lazy-load thumbnails and media blobs

## Work order
Follow docs/implementation-plan.md milestones in order.
Do not expand scope beyond the “hard constraints.”
If the Android backup format is ambiguous:
- implement a best-effort parser
- fail gracefully when unknown versions are detected
- store unknown fields in `metadata_json` rather than dropping them silently

## Output expectations
- clean, buildable project
- minimal dependencies
- clear error messages
- tests for core importer and schema
