# Signal Android .backup research (pre-implementation)

Goal: collect non-GPL knowledge about the Signal Android on-device backup format and outline a GPL-safe implementation plan.

## Constraints recap
- We can *read* public info, but we must not copy GPL code or vendor GPL tools.
- We should implement from first principles, or from independently derived specs.
- No network calls at runtime; importer runs locally.

## Sources reviewed
- Signal Support: On-device backups are encrypted, use a 30-digit passphrase, and the filename format is `signal-YYYY-MM-DD-HHMM.backup`.
- Signal Support: “Secure Backups” are a different cloud-based system with a 64-character recovery key (not our input).
- wngr blog (2021): reverse-engineering notes on Android `.backup` format and crypto framing.
- GPL source (Signal Android backup proto definitions) for *field numbers and types only*, no code reuse.
- GPL tools exist (signal-backup-decode, signal_for_android_decryption) but **cannot** be used or referenced for code.

Sources (URLs)
```
https://support.signal.org/hc/en-us/articles/10066926526362-Android-On-Device-Backups
https://support.signal.org/hc/en-us/articles/9708267671322-Signal-Secure-Backup
https://0xd.org/blog/2021-04-06_Sniffing-into-Signal-Backups.html
https://github.com/pajowu/signal-backup-decode/blob/master/proto/Backups.proto
https://raw.githubusercontent.com/pajowu/signal-backup-decode/master/proto/Backups.proto
https://github.com/mossblaser/signal_for_android_decryption
```

## High-level format (from non-GPL sources)
The backup file is a sequence of length-prefixed protobuf frames:

```
| 4-byte big-endian length | header frame bytes |
| 4-byte big-endian length | encrypted frame bytes | 10-byte MAC |
| 4-byte big-endian length | encrypted frame bytes | 10-byte MAC |
...
```

The first frame is *not* encrypted; it contains parameters for key derivation and encryption (salt + IV). Subsequent frames are encrypted and authenticated.

The protobuf “BackupFrame” acts like a one-of/union, containing exactly one of several payload types (header, SQL statements, preferences, attachments, etc.). The blog lists fields like:
`header`, `statement`, `preference`, `attachment`, `version`, `end`, `avatar`, `sticker`, `keyValue`.

## Crypto details (from non-GPL sources)
Per the reverse-engineering notes:
- Passphrase normalization: remove whitespace (and by our UX, dashes too), then validate 30 digits.
- Key derivation:
  - SHA-512 iterated ~250,000 times over (salt + passphrase) as described in the blog.
  - Use HKDF-SHA256 with info string `"Backup Export"` and a zeroed salt to derive 64 bytes.
  - Split derived bytes into `cipher_key` (first 32) and `mac_key` (last 32). citeturn2view0
- Encryption:
  - AES-256-CTR for each frame.
  - IV uses a counter in the first 4 bytes, and the remaining bytes from the header IV; counter increments per frame. citeturn2view0
- Authentication:
  - HMAC-SHA256 (implied by HKDF/SHA256) over encrypted frame bytes, compare only the first 10 bytes stored in file.

## BackupFrame schema (GPL source for field numbers/types)
This section is derived from `Backups.proto` (GPL). We use it *only* to capture field numbers/types for a clean reimplementation.

Top-level messages and fields:

- `Header`
  - `iv` (bytes, field 1)
  - `salt` (bytes, field 2)
- `SqlStatement`
  - `statement` (string, field 1)
  - `parameters` (repeated `SqlParameter`, field 2)
  - `SqlParameter` fields:
    - `stringParameter` (string, field 1) [note: typo “stringParamter” in proto]
    - `integerParameter` (uint64, field 2)
    - `doubleParameter` (double, field 3)
    - `blobParameter` (bytes, field 4)
    - `nullParameter` (bool, field 5) [note: “nullparameter” in proto]
- `SharedPreference`
  - `file` (string, 1)
  - `key` (string, 2)
  - `value` (string, 3)
  - `booleanValue` (bool, 4)
  - `stringSetValue` (repeated string, 5)
  - `isStringSetValue` (bool, 6)
- `Attachment`
  - `rowId` (uint64, 1)
  - `attachmentId` (uint64, 2)
  - `length` (uint32, 3)
- `Sticker`
  - `rowId` (uint64, 1)
  - `length` (uint32, 2)
- `Avatar`
  - `name` (string, 1)
  - `length` (uint32, 2)
  - `recipientId` (string, 3)
- `DatabaseVersion`
  - `version` (uint32, 1)
- `KeyValue`
  - `key` (string, 1)
  - `blobValue` (bytes, 2)
  - `booleanValue` (bool, 3)
  - `floatValue` (float, 4)
  - `integerValue` (int32, 5)
  - `longValue` (int64, 6)
  - `stringValue` (string, 7)

- `BackupFrame` union (all optional; only one set per frame)
  - `header` (Header, field 1)
  - `statement` (SqlStatement, field 2)
  - `preference` (SharedPreference, field 3)
  - `attachment` (Attachment, field 4)
  - `version` (DatabaseVersion, field 5)
  - `end` (bool, field 6)
  - `avatar` (Avatar, field 7)
  - `sticker` (Sticker, field 8)
  - `keyValue` (KeyValue, field 9)

Notes:
- `statement` frames contain SQL + parameters to recreate the Android DB.
- `attachment` and `sticker` frames indicate binary payload lengths; the payload bytes follow the protobuf frame in the stream.
- `avatar` frames are similar to attachment payloads but keyed to recipient.

## What this implies for our importer
We can implement a streaming reader:
1) Read header frame length + bytes.
2) Decode header protobuf to obtain `salt` + `iv`.
3) Derive keys from passphrase.
4) For each subsequent frame:
   - Read length + encrypted bytes + 10-byte MAC.
   - Verify MAC first (fail fast on mismatch).
   - Decrypt with AES-CTR using incrementing counter.
   - Decode protobuf payload and map it into our archive schema.
   - For `attachment`/`sticker`/`avatar` frames, read the specified `length` bytes from the stream after the protobuf frame.

## GPL safety / open questions
The wire format schema (protobuf field numbers/types) appears in Signal-Android’s `Backups.proto`, which is GPL. We should avoid copying it verbatim.

Potential GPL-safe paths:
1) **Independent schema discovery**: parse a sample backup, decode raw protobuf with a dynamic parser, and infer only the fields we need (field numbers, types). This is more work but keeps us clean.
2) **User permission to consult GPL source for *spec only***: if allowed, we can read field numbers/types and re-implement without copying code. This should be a clear, explicit permission step.

We should decide *before* implementing the parser.

## Planned knowledge artifacts
1) “Backup format quick spec” (human-readable, not code).
2) “Importer flow notes” (streaming parser, failure modes, counters).
3) “Schema mapping” (BackupFrame -> our SQLite tables).

## Next concrete steps
1) Decide on GPL-safe schema approach (independent inference vs. permission to read `Backups.proto`).
2) If independent inference:
   - Obtain a real `.backup` file and sample passphrase.
   - Write a local tool to inspect raw protobuf fields without a schema (length-delimited + varint parsing).
3) If permission granted to consult Signal-Android source:
   - Read only enough to capture field numbers and types.
   - Write our own schema definitions (not copying file comments or structure).

## Risk notes
- Backup format may evolve over time (new fields, versions).
- Must detect unsupported versions and fail gracefully.
- Some message types may be missing or encoded differently (stickers, avatars, reactions).

## Quick UI/UX tie-ins
- Passphrase normalization: strip spaces + dashes before validation.
- Wrong passphrase should fail fast on header MAC verification or first frame MAC check.
- Import should be transactional; do not write to archive until integrity checks pass.
