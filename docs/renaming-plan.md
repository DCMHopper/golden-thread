# Renaming Action Plan: LDR History → Golden Thread

**Status:** Executed (2025-12-26)
**Target Name:** Golden Thread
**Created:** 2025-12-26

---

## Overview

This document outlines all changes required to rename the project from "LDR History" to "Golden Thread". The renaming affects package names, user-facing text, configuration files, and internal code references.

**Estimated files to modify:** ~20 files
**Estimated time:** 30-45 minutes + testing

---

## Pre-Execution Checklist

Before starting the rename:
- [ ] Commit all current work
- [ ] Create a backup or git branch for the rename
- [ ] Decide on migration strategy for existing user data (see Gotchas section)
- [ ] Confirm final naming conventions:
  - Rust crate: `golden_thread_core` or alternate?
  - Bundle ID: `com.goldenthread.app` or `com.golden.thread`?
  - App data dir: `golden-thread` or `goldenthread`?
  - localStorage prefix: `gt_` or `goldenthread_`?

---

## Step-by-Step Execution Plan

### Phase 1: Package and Crate Names

#### 1.1 Core Rust Crate
**File:** `core/Cargo.toml`
```toml
# Change line 2:
name = "ldr_core"  →  name = "golden_thread_core"

# Change line 7:
name = "ldr_core"  →  name = "golden_thread_core"
```

#### 1.2 Tauri App Crate
**File:** `app/src-tauri/Cargo.toml`
```toml
# Change line 2:
name = "ldr_history_app"  →  name = "golden_thread_app"

# Change line 13:
ldr_core = { path = "../../core" }  →  golden_thread_core = { path = "../../core" }
```

#### 1.3 Node Package
**File:** `app/package.json`
```json
// Change line 2:
"name": "ldr-history-app"  →  "name": "golden-thread-app"
```

#### 1.4 Package Lock
**File:** `app/package-lock.json`
```json
// Change lines 2, 8 (3 occurrences total):
"name": "ldr-history-app"  →  "name": "golden-thread-app"
```

---

### Phase 2: App Configuration

#### 2.1 Tauri Configuration
**File:** `app/src-tauri/tauri.conf.json`
```json
// Line 3:
"productName": "LDR History"  →  "productName": "Golden Thread"

// Line 5:
"identifier": "com.ldr.history"  →  "identifier": "com.goldenthread.app"

// Line 18 (filesystem scope):
"scope": ["$APPDATA/ldr-history/**"]  →  "scope": ["$APPDATA/golden-thread/**"]

// Line 23:
"title": "LDR History"  →  "title": "Golden Thread"
```

#### 2.2 Capabilities Configuration
**File:** `app/src-tauri/capabilities/default.json`
```json
// Line 3:
"description": "Default permissions for LDR History"
  →  "description": "Default permissions for Golden Thread"
```

---

### Phase 3: User Interface

#### 3.1 HTML Title and Header
**File:** `app/index.html`
```html
<!-- Line 6: -->
<title>LDR History</title>  →  <title>Golden Thread</title>

<!-- Line 12: -->
<h1>LDR History</h1>  →  <h1>Golden Thread</h1>
```

---

### Phase 4: Source Code References

#### 4.1 Tauri Backend Main
**File:** `app/src-tauri/src/main.rs`
```rust
// Line 5:
use ldr_core::{diagnostics, open_archive, seed, CoreError};
  →  use golden_thread_core::{diagnostics, open_archive, seed, CoreError};

// Line 6:
use ldr_core::importer;
  →  use golden_thread_core::importer;

// Line 7:
use ldr_core::models::{...};
  →  use golden_thread_core::models::{...};

// Line 8:
use ldr_core::query::{...};
  →  use golden_thread_core::query::{...};

// Line 54 (app data directory):
let archive_dir = base.join("ldr-history");
  →  let archive_dir = base.join("golden-thread");

// Line 90 (type reference):
db: Mutex<Option<ldr_core::ArchiveDb>>,
  →  db: Mutex<Option<golden_thread_core::ArchiveDb>>,

// Line 95 (type reference):
F: FnOnce(&ldr_core::ArchiveDb) -> Result<T, CoreError>,
  →  F: FnOnce(&golden_thread_core::ArchiveDb) -> Result<T, CoreError>,

// Line 211 (return type):
Result<Vec<ldr_core::models::ReactionSummary>, String>
  →  Result<Vec<golden_thread_core::models::ReactionSummary>, String>
```

#### 4.2 Frontend Main
**File:** `app/src/main.ts`
```typescript
// Line 201:
const stored = localStorage.getItem("ldr_media_click");
  →  const stored = localStorage.getItem("gt_media_click");

// Line 207:
const savedTheme = localStorage.getItem("ldr_dark_mode");
  →  const savedTheme = localStorage.getItem("gt_dark_mode");

// Line 213:
const savedAccent = localStorage.getItem("ldr_accent_color") || "amber";
  →  const savedAccent = localStorage.getItem("gt_accent_color") || "amber";

// Line 1688:
localStorage.setItem("ldr_media_click", String(newState));
  →  localStorage.setItem("gt_media_click", String(newState));

// Line 1708:
localStorage.setItem("ldr_dark_mode", String(newState));
  →  localStorage.setItem("gt_dark_mode", String(newState));

// Line 1728:
localStorage.setItem("ldr_accent_color", selectedColor);
  →  localStorage.setItem("gt_accent_color", selectedColor);
```

#### 4.3 Tauri Build Script
**File:** `app/src-tauri/build.rs`
```rust
// Line 25:
let mut lib = std::env::var("DEP_LDR_CORE_SIGNALBACKUP_LIB").ok()
  →  let mut lib = std::env::var("DEP_GOLDEN_THREAD_CORE_SIGNALBACKUP_LIB").ok()

// Line 26:
std::env::var("DEP_LDR_CORE_SIGNALBACKUP_LIB_DIR")
  →  std::env::var("DEP_GOLDEN_THREAD_CORE_SIGNALBACKUP_LIB_DIR")
```

#### 4.4 Core Build Script
**File:** `core/build.rs`
```rust
// Line 3:
println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/ldr_bridge/ldr_bridge.cc");
  →  println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/gt_bridge/gt_bridge.cc");

// Line 4:
println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/ldr_bridge/ldr_bridge.h");
  →  println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/gt_bridge/gt_bridge.h");
```

---

### Phase 5: FFI Layer (Requires Vendor Changes)

#### 5.1 FFI Function Names
**File:** `core/src/ffi/signalbackup.rs`
```rust
// Line 9:
fn ldr_decode_backup(...)
  →  fn gt_decode_backup(...)

// Line 45:
ldr_decode_backup(...)
  →  gt_decode_backup(...)
```

**⚠️ IMPORTANT:** This requires matching changes in vendor C++ code:
- Rename `vendor/signalbackup-tools/ldr_bridge/` directory to `gt_bridge/`
- Update function names in `.cc` and `.h` files
- Ensure exported symbol names match

---

### Phase 6: Test Files

**All files in:** `core/tests/`
- `hardening_tests.rs`
- `importer_fixtures.rs`
- `migration_tests.rs`
- `pipeline_tests.rs`
- `query_tests.rs`
- `query.rs`
- `tag_tests.rs`

**Change in each file:**
```rust
use ldr_core::*;
  →  use golden_thread_core::*;
```

---

### Phase 7: Documentation

#### 7.1 README
**File:** `README.md`
```markdown
# LDR History  →  # Golden Thread
```
Update all placeholder content to reflect new name.

#### 7.2 Product Requirements
**File:** `docs/prd.md`
```markdown
# PRD: LDR-History signal archive viewer for mac
  →  # PRD: Golden Thread - Signal Archive Viewer for macOS
```

#### 7.3 Future Improvements
**File:** `docs/future-improvements.md`
```markdown
# Future Improvements for LDR History App
  →  # Future Improvements for Golden Thread

This document contains planned improvements and feature ideas for the LDR History application.
  →  This document contains planned improvements and feature ideas for Golden Thread.
```

#### 7.4 Technical Strategy
**File:** `docs/tech-strategy.md`
```markdown
// Line 61 (function reference):
ldr_decode_backup(...)  →  gt_decode_backup(...)
```

---

### Phase 8: Optional - Repository Folder

**Current:** `/Users/derek/workspace/ldr-history/`
**Target:** `/Users/derek/workspace/golden-thread/`

This is optional but recommended for consistency. If renaming:
1. Commit all changes
2. Close any open editors/terminals in the directory
3. `mv /Users/derek/workspace/ldr-history /Users/derek/workspace/golden-thread`
4. Update any IDE/editor workspace files

---

## Post-Execution Steps

After completing all changes:

1. **Clean build artifacts:**
   ```bash
   cd core
   cargo clean
   cd ../app/src-tauri
   cargo clean
   cd ../..
   rm -rf app/node_modules
   rm -rf app/dist
   ```

2. **Reinstall dependencies:**
   ```bash
   cd app
   npm install
   ```

3. **Rebuild and test:**
   ```bash
   npm run dev
   ```

4. **Verify functionality:**
   - [ ] App launches successfully
   - [ ] Window title shows "Golden Thread"
   - [ ] Import flow works
   - [ ] Archive location is `~/Library/Application Support/golden-thread/`
   - [ ] All features work as expected

5. **Update lock files:**
   - Cargo will regenerate `Cargo.lock` files automatically
   - `package-lock.json` is already updated from npm install

---

## Migration Concerns & Gotchas

### 1. localStorage Keys Change
**Impact:** Existing user settings will be lost (dark mode, accent color, media click preference)

**Migration Option:** Add one-time migration code in `app/src/main.ts`:
```typescript
// Run once on app load to migrate old settings
function migrateLocalStorage() {
  const oldKeys = ['ldr_media_click', 'ldr_dark_mode', 'ldr_accent_color'];
  const newKeys = ['gt_media_click', 'gt_dark_mode', 'gt_accent_color'];

  oldKeys.forEach((oldKey, i) => {
    const value = localStorage.getItem(oldKey);
    if (value !== null && localStorage.getItem(newKeys[i]) === null) {
      localStorage.setItem(newKeys[i], value);
      localStorage.removeItem(oldKey);
    }
  });
}
```

### 2. App Data Directory Change
**Impact:** App will look for archives in new location; existing data won't be found

**Options:**
1. **Manual migration:** User manually moves `~/Library/Application Support/ldr-history/` to `~/Library/Application Support/golden-thread/`
2. **Auto-migration:** Add code to check for old directory and migrate on first launch
3. **Keep old directory name:** Don't change line 54 in `main.rs` (simplest, but inconsistent)

**Auto-migration example:**
```rust
// In app/src-tauri/src/main.rs, around line 54:
let old_dir = base.join("ldr-history");
let new_dir = base.join("golden-thread");

if old_dir.exists() && !new_dir.exists() {
    std::fs::rename(&old_dir, &new_dir)?;
}

let archive_dir = new_dir;
```

### 3. FFI Function Names
**Impact:** Requires changes in vendor C++ code that may be tracked separately

**Options:**
1. **Rename in vendor:** Keep FFI consistent with project name
2. **Keep old name:** Don't change FFI layer, only affects internal code
3. **Add wrapper:** Keep old FFI name but wrap it with new name

### 4. Build Cache
**Impact:** Old package names may persist in build caches

**Solution:** Always run `cargo clean` after renaming before rebuilding

---

## Alternative Naming Conventions

If you want different conventions, here are alternatives:

### Shorter Prefix Option
- localStorage: `gt_*` (as planned)
- Rust crate: `gt_core` instead of `golden_thread_core`
- Bundle ID: `com.gt.app`
- App data: `golden-thread` (as planned)

### Full Name Option
- localStorage: `goldenthread_*` instead of `gt_*`
- Rust crate: `golden_thread_core` (as planned)
- Bundle ID: `com.goldenthread.app` (as planned)
- App data: `golden-thread` (as planned)

### No Separator Option
- Rust crate: `goldenthreadcore` instead of `golden_thread_core`
- App data: `goldenthread` instead of `golden-thread`

**Recommendation:** Stick with the plan as outlined (snake_case for Rust, kebab-case for directories, `gt_` prefix for localStorage)

---

## Rollback Plan

If issues arise after renaming:

1. **Git revert:** If changes are committed, use `git revert` or `git reset`
2. **Manual revert:** Use this document in reverse, changing new names back to old names
3. **Clean rebuild:** Run clean steps and rebuild from scratch

---

## Notes

- This rename does not affect the core functionality of the app
- The Signal backup format and parsing logic remain unchanged
- User data in the archive (messages, attachments) is unaffected
- Only configuration, package names, and UI text change

---

**Last Updated:** 2025-12-26
**Ready for Execution:** Yes (completed)
