# Handoff Notes (Dec 2025)

## Current status
- Import pipeline works (Signal Android `.backup` via `signalbackup-tools`).
- UI supports: thread list, per-thread message view, per-thread media gallery, search with navigation, jump-to-date, lightbox media viewer.
- **Tagging**: Create custom tags, apply to messages, browse tagged messages in Scrapbook view (cross-thread, with discontinuity indicators).
- Media handling includes lazy thumbnails for images and click-to-load for large/video/audio.
- Diagnostics logging + copy diagnostics to clipboard.
- Tests cover migrations, queries, pipeline, importer fixtures, and tagging (including discontinuity detection).

## How to run
- UI: `npm run tauri dev` (from `app/`)
- Core tests: `cargo test` (from `core/`)

## Key locations
- Importer: `core/src/importer.rs` + `core/src/importer/attachments.rs`
- Queries: `core/src/query.rs`
- Migrations: `core/src/migrations.rs`
- UI entry: `app/src/main.ts` + `app/index.html`
- UI modules: `app/src/ui/*`
- Styles: `app/src/styles.css` + `app/src/styles/*`
- Tauri commands: `app/src-tauri/src/main.rs`
- Diagnostics: `core/src/diagnostics.rs`

## ID normalization
- Messages use `sms:<id>` / `mms:<id>` to avoid collisions.
- Reactions/attachments normalize integer IDs into that format.

## Known constraints / assumptions
- Search is scoped to the active thread by design.
- Media thumbnails are image-only (video/audio use file source).
- Importer expects standard Signal tables; best-effort parsing for missing columns.

## Vendor dependency
- `vendor/signalbackup-tools` is required locally but intentionally not tracked in git. Each contributor should clone it before building the importer.
- Local changes to signalbackup-tools are captured as a patch series in `patches/signalbackup-tools/*.patch`; reapply after pulling upstream updates.

## Test coverage summary
- Importer fixtures: `core/tests/importer_fixtures.rs`
- Pipeline test: `core/tests/pipeline_tests.rs`
- Migration tests: `core/tests/migration_tests.rs`
- Query tests: `core/tests/query_tests.rs`
