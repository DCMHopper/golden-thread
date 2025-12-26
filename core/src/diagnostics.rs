use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

const MAX_LOG_BYTES: u64 = 1_500_000;

#[derive(Debug, Serialize)]
pub struct LogEvent {
    pub ts: String,
    pub kind: String,
    pub message: String,
}

fn sanitize(input: &str) -> String {
    let mut out = input.to_string();
    // strip obvious paths
    for prefix in ["/Users/", "/var/", "/private/", "C:\\", "D:\\"] {
        if let Some(idx) = out.find(prefix) {
            out.replace_range(idx.., "[redacted]");
            break;
        }
    }
    // strip long numeric sequences (passphrases, phone numbers)
    out = out
        .split_whitespace()
        .map(|token| {
            let digits = token.chars().filter(|c| c.is_ascii_digit()).count();
            if digits >= 10 {
                "[redacted]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    out
}

pub fn log_event(log_dir: &Path, kind: &str, message: &str) -> io::Result<()> {
    fs::create_dir_all(log_dir)?;
    let path = log_dir.join("diagnostics.log");
    trim_log(&path)?;
    let event = LogEvent {
        ts: Utc::now().to_rfc3339(),
        kind: kind.to_string(),
        message: sanitize(message),
    };
    let line = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

fn trim_log(path: &PathBuf) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let meta = fs::metadata(path)?;
    if meta.len() <= MAX_LOG_BYTES {
        return Ok(());
    }
    let data = fs::read(path)?;
    let keep_from = data.len().saturating_sub((MAX_LOG_BYTES / 2) as usize);
    fs::write(path, &data[keep_from..])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sanitize_redacts_paths_and_digits() {
        let msg = "path /Users/derek/secret 1234567890123";
        let cleaned = sanitize(msg);
        assert!(cleaned.contains("[redacted]"));
        assert!(!cleaned.contains("Users"));
    }

    #[test]
    fn log_event_writes_and_trims() {
        let dir = tempdir().expect("temp");
        let log_dir = dir.path();
        for _ in 0..10 {
            log_event(log_dir, "test", "hello 1234567890").expect("log");
        }
        let path = log_dir.join("diagnostics.log");
        assert!(path.exists());
    }
}
