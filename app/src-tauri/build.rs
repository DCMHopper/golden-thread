use std::path::{Path, PathBuf};

fn find_file(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|s| s.to_str()) == Some(name) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file(&path, name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn main() {
    tauri_build::build();

    let mut lib = std::env::var("DEP_GOLDEN_THREAD_CORE_SIGNALBACKUP_LIB").ok().or_else(|| {
        std::env::var("DEP_GOLDEN_THREAD_CORE_SIGNALBACKUP_LIB_DIR")
            .ok()
            .map(|dir| format!("{}/libsignalbackup_tools_static.a", dir))
    });

    if lib.is_none() {
        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let mut target_dir = PathBuf::from(out_dir);
            for _ in 0..4 {
                if !target_dir.pop() {
                    break;
                }
            }
            if target_dir.ends_with("target") {
                lib = find_file(&target_dir, "libsignalbackup_tools_static.a", 6)
                    .map(|path| path.to_string_lossy().to_string());
            }
        }
    }

    if let Some(lib) = lib {
        println!("cargo:warning=force_load signalbackup-tools: {}", lib);
        println!("cargo:rustc-link-arg=-Wl,-force_load,{}", lib);
    } else {
        println!("cargo:warning=force_load signalbackup-tools: unable to locate libsignalbackup_tools_static.a");
    }
}
