# Golden Thread

### ❗Warning: this repo is vibe-coded
### ❗Warning: this app can decrypt sensitive Signal data
### ❗Warning: this repo is not actively maintained

## Overview
Golden Thread is a view-only interface for Signal messages, built on [signalbackup-tools](https://github.com/bepaald/signalbackup-tools) for Mac. Its features include:
- GUI-based Signal backup decryption (Android only).
- Automatic deduping from incremental backup files.
- Message search and "Jump to date" within thread capabilities.
- Advanced gallery controls.
- Tagging system for organizing important messages.
- Snappy, clean interface.

This app was built for personal use, entirely through Claude Code/Codex-CLI. This app is not meant for nor recommended for general use, but I figured I would make it publicly available for the sake of curiosity. I have intentionally not touched a single line of code in this repo directly.

My partner and I spent 5 years long distance after the pandemic. During this time, we generated almost half a million Signal messages. These messages have sentimental value, but also take up a lot of storage space on our phones. This app allows us to clear old message data without losing the memories by moving our message history into a desktop archive with more advanced browsing and organizational functionality than the base Signal app.

This idea of "apps built for one" has become popular this year thanks to AI-assisted development, and the scope of what can be built keeps expanding as capabilities keep growing. I was able to build Golden Thread in a few days, and deploy it for myself and my partner. It contains features that only we would care about, such as tag organization. It's built according to my personal aesthetic taste. It does exactly what I need and nothing that I don't.

The `docs/` directory has a bunch of AI-specific clutter. This README is the only human-written file in the entire repo.

## Privacy & Security
- Local‑only functionality.
- Passphrases not stored and wiped from memory after use.
- Easy local data deletion.
- Basic CSP in place.
- I am not responsible for what happens if you plug sensitive data into a gen-AI app.

## Getting Started
### Prerequisites
- macOS 13.3 or newer
- Android Signal .backup file
- Tested with Node v25.2.1 and rustc 1.92.0
- signalback-tools not included

### Build
1. Clone this repo, create a `vendor/` directory in its root, and clone signalbackup-tools into `vendor/`.
2. Apply the patches in `patches/signalbackup-tools`.
3. From `app/`, use `npm run tauri dev` or `npm run tauri build` to generate the app.

### Usage
From the Options dropdown, select either "Import backup" or "Load demo data". Loading from backup may take several minutes, depending on how much data you have (and especially how many attachments).

## License
GPLv3
