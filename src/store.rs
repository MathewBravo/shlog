use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub cmd: String,
    pub cwd: String,
    pub exit_code: i32,
    pub shell: String,
    pub ppid: u32,
    pub duration_ms: u64,
}

pub fn append(entry: &LogEntry, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut line = serde_json::to_string(entry).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    file.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_entry(cmd: &str, exit_code: i32) -> LogEntry {
        LogEntry {
            ts: "2026-03-30T14:32:01.123Z".to_string(),
            cmd: cmd.to_string(),
            cwd: "/tmp".to_string(),
            exit_code,
            shell: "/bin/zsh".to_string(),
            ppid: 1000,
            duration_ms: 100,
        }
    }

    #[test]
    fn append_creates_directory_and_file_if_missing() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("nested/dir/commands.jsonl");

        append(&sample_entry("echo hello", 0), &log_path).unwrap();

        assert!(log_path.exists());
    }

    #[test]
    fn append_writes_valid_jsonl_line() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("commands.jsonl");

        append(&sample_entry("git status", 0), &log_path).unwrap();

        let content = fs::read_to_string(&log_path).unwrap();
        let entry: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(entry.cmd, "git status");
        assert_eq!(entry.exit_code, 0);
    }

    #[test]
    fn append_multiple_entries_one_per_line() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("commands.jsonl");

        append(&sample_entry("cmd1", 0), &log_path).unwrap();
        append(&sample_entry("cmd2", 1), &log_path).unwrap();
        append(&sample_entry("cmd3", 0), &log_path).unwrap();

        let content = fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        let first: LogEntry = serde_json::from_str(lines[0]).unwrap();
        let second: LogEntry = serde_json::from_str(lines[1]).unwrap();
        let third: LogEntry = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(first.cmd, "cmd1");
        assert_eq!(second.cmd, "cmd2");
        assert_eq!(third.cmd, "cmd3");
    }

    #[test]
    fn append_concurrent_writes_produce_valid_lines() {
        use std::sync::Arc;
        use std::thread;

        let tmp = TempDir::new().unwrap();
        let log_path = Arc::new(tmp.path().join("commands.jsonl"));

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let path = Arc::clone(&log_path);
                thread::spawn(move || {
                    append(&sample_entry(&format!("cmd-{i}"), i), &path).unwrap();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let content = fs::read_to_string(log_path.as_ref()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 20);

        // Every line must be valid JSON
        for line in &lines {
            let entry: LogEntry = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("Invalid JSONL line: {e}\nLine: {line}"));
            assert!(entry.cmd.starts_with("cmd-"));
        }
    }
}