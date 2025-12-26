# implementation plan

## Milestone 0: repo skeleton and mac packaging
Outputs
- monorepo scaffold:
  - `/app` tauri app + ui
  - `/core` rust crate (importer, db, queries)
  - `/docs` specs
- core/app wiring:
  - tauri commands for read-only queries
  - shared types between app and core
- app launches on mac
- empty state UI with “import backup” CTA

Acceptance
- `pnpm dev` (or equivalent) runs the app
- `pnpm build` produces a mac app bundle

## Milestone 1: archive schema and read query API
Tasks
- Implement SQLite migrations v1 in `/core`.
- Create FTS5 index for message bodies.
- Implement query endpoints:
  - list threads
  - fetch messages page
  - search messages with filters
  - list media

Acceptance
- Seeded demo data renders in UI.
- Search works against demo data.

## Milestone 2: minimal Android `.backup` ingest (text only)
Tasks
- Implement:
  - file selection and passphrase entry UI
  - passphrase normalization and early validation
  - decrypt + parse enough to extract:
    - threads
    - recipients
    - text messages
- Transactional write into SQLite.

Acceptance
- Import one real backup and display conversations + text messages.
- Wrong passphrase fails fast and leaves archive unchanged.

## Milestone 3: attachments ingest + dedupe
Tasks
- Parse attachment metadata and payload.
- Stream attachment bytes to disk and compute sha256.
- Deduplicate by sha256 across imports.
- Store attachment records linked to messages.
- Add basic media rendering in conversation view.
- Batch attachment inserts and add progress stats for large imports.

Acceptance
- Images and files appear and open.
- Importing a newer backup does not duplicate media already stored.

## Milestone 4: incremental import correctness
Tasks
- Implement incremental merge rules:
  - prefer stable ids from the backup if present
  - fallback composite key strategy
- Record per-import stats and keep import history.
- Store source hash to detect repeat imports and set import status (running/success/failed).
- Update thread last_message_at after import.
- Add attachment uniqueness constraints (message_id + sha256).
- Record per-import message/attachment counts in stats_json.

Acceptance
- Import backup A then backup B:
  - message counts increase only by new messages
  - attachments increase only by new content
  - threads update last activity correctly

## Milestone 5: UI polish for real use
Tasks
- Thread list with search box and sorting.
- Conversation view:
  - infinite scroll virtualization
  - jump to date
  - quote rendering
  - reactions rendering
- Search page:
  - filters and result navigation to context
- Media gallery:
  - per thread
  - thumbnails lazily generated

Acceptance
- UI remains responsive on large archives.
- Search results open in correct location.

## Milestone 6: hardening and tests
Tasks
- Robust error handling for:
  - corrupt backup file
  - unsupported version
  - insufficient disk space
- Tests:
  - schema migration tests
  - importer unit tests with synthetic fixtures
  - “golden query” tests for search and pagination
- Diagnostics:
  - local debug logs with strict redaction
  - “copy diagnostics” output without sensitive content

Acceptance
- Crashes during import do not corrupt the archive.
- Test suite covers core importer and query functions.

## Deliverables checklist
- signed mac build (optional)
- deterministic archive folder location
- documentation: how to create android backups and import them (high-level)

## Acceptance checkpoints (rolling)
- M0: app shell runs; empty state with import CTA; core crate builds with tests.
- M1: seeded demo data displays thread list + conversation view; search returns matches.
- M2: import of real backup (text-only) succeeds; wrong passphrase fails fast with clear error.
- M3: attachments render and open; dedupe confirmed across imports.
- M4: incremental import adds only new messages/attachments; no duplicates.
- M5: UI remains responsive with virtualization; search opens match in context.
- M6: tests cover migrations/import/query; import crash leaves archive intact.
