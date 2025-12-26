# PRD: Golden Thread - Signal Archive Viewer for macOS

## Problem
Signal histories and media collections can exceed comfortable phone storage. You want to free phone space without deleting memories, while keeping custody of the data.

## Users
- primary: you
- secondary: your partner
- both are trusted custodians of the archive

## Goals
1. Import one or more Signal Android `.backup` files using the 30-digit passphrase.
2. Create a local archive that is read-only for imported Signal data (local annotations like tags are allowed).
3. Browse conversations and messages smoothly for 5+ years of history.
4. Search across all messages quickly, with filters.
5. View media with lazy loading and responsive UI.

## Non-goals
- send/receive messages
- modify Signal data on phone
- sync or cloud features
- ingest from Signal Desktop or iOS
- multi-user support beyond the two local users
- sharing, publishing, or distribution tooling

## Assumptions
- Android `.backup` files are the single source of truth.
- The user has passphrase custody.
- Backups may be imported repeatedly over time (monthly, weekly, etc.).

## Functional requirements

### Import
- Select one or more `.backup` files.
- Enter passphrase (accept spaces; normalize).
- Validate passphrase quickly before long work.
- Transactional import:
  - failure leaves existing archive unchanged
- Incremental import:
  - importing a newer backup adds only missing data
  - avoids duplicates for messages and attachments
- Progress UI:
  - phases: decrypt, parse, write db, copy media, index
  - counts: messages processed, attachments processed

### Browse
- Thread list sorted by last activity.
- Conversation view:
  - infinite scroll
  - jump to date
  - show timestamps, sender, delivery/system events as applicable
- Render message types:
  - text
  - attachments (image/video/audio/file)
  - stickers (at minimum as an attachment-like item)
  - reactions
  - quoted replies
  - calls and other system events (as system messages)

### Search
- Full-text search within the active thread (with an option to list all matches).
- Filters:
  - thread
  - sender
  - date range
  - has media
  - message type
- Search results:
  - open message in context
  - highlight matched terms (best effort)

### Media
- Per-thread media gallery.
- Thumbnail grid with lazy generation.
- Media viewer:
  - images: zoom + pan (basic)
  - video/audio: play/pause, scrub (basic)
- Optional: export a single attachment file (does not modify archive).

## Non-functional requirements

### Performance
- After indexing, typical searches return first results quickly on a laptop-class machine.
- UI remains responsive during media loading and scrolling.
- Import should be resumable by re-running (not necessarily checkpoint-resume mid-import).

### Reliability
- Archive schema is versioned and migratable.
- Import is safe against crashes and power loss.
- Corrupt or unsupported backup yields a clear error and no partial archive mutation.

### Privacy and security
- No network calls by default.
- No passphrase written to disk.
- No sensitive content in logs.
- Provide guidance to store archive outside Spotlight indexing, or inside an app folder with clear instructions.

## Out of scope boundaries (hard)
- Supporting any non-Android source
- Supporting real-time sync
- Supporting shared web links or publishing
- Supporting advanced analytics over messages

## Success criteria
- You can import a real backup and browse messages and media.
- You can find any old message with search in seconds.
- Incremental imports do not duplicate data and do not balloon storage unnecessarily.
