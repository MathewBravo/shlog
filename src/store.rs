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

#[derive(Default)]
pub struct Filters {
    pub since: Option<String>,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
    pub cmd_pattern: Option<String>,
}

pub fn read(path: &Path, filters: &Filters) -> Vec<LogEntry> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter_map(|line| serde_json::from_str::<LogEntry>(line).ok())
        .filter(|entry| {
            if let Some(ref pattern) = filters.cmd_pattern {
                if !entry.cmd.contains(pattern.as_str()) {
                    return false;
                }
            }
            if let Some(ref cwd) = filters.cwd {
                if entry.cwd != *cwd {
                    return false;
                }
            }
            if let Some(code) = filters.exit_code {
                if entry.exit_code != code {
                    return false;
                }
            }
            if let Some(ref since) = filters.since {
                if entry.ts.as_str() <= since.as_str() {
                    return false;
                }
            }
            true
        })
        .collect()
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

    fn entry_at(ts: &str, cmd: &str, cwd: &str, exit_code: i32) -> LogEntry {
        LogEntry {
            ts: ts.to_string(),
            cmd: cmd.to_string(),
            cwd: cwd.to_string(),
            exit_code,
            shell: "/bin/zsh".to_string(),
            ppid: 1000,
            duration_ms: 100,
        }
    }

    fn populate_log(tmp: &TempDir) -> std::path::PathBuf {
        let log_path = tmp.path().join("commands.jsonl");
        append(&entry_at("2026-03-30T10:00:00.000Z", "git status", "/home/dev/projA", 0), &log_path).unwrap();
        append(&entry_at("2026-03-30T11:00:00.000Z", "git push origin main", "/home/dev/projA", 1), &log_path).unwrap();
        append(&entry_at("2026-03-30T12:00:00.000Z", "cargo test", "/home/dev/projB", 0), &log_path).unwrap();
        append(&entry_at("2026-03-30T13:00:00.000Z", "git diff", "/home/dev/projA", 0), &log_path).unwrap();
        log_path
    }

    #[test]
    fn read_returns_all_entries_with_no_filters() {
        let tmp = TempDir::new().unwrap();
        let log_path = populate_log(&tmp);

        let entries = read(&log_path, &Filters::default());
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn read_filters_by_cmd_pattern() {
        let tmp = TempDir::new().unwrap();
        let log_path = populate_log(&tmp);

        let entries = read(&log_path, &Filters {
            cmd_pattern: Some("git".to_string()),
            ..Default::default()
        });
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.cmd.contains("git")));
    }

    #[test]
    fn read_filters_by_cwd() {
        let tmp = TempDir::new().unwrap();
        let log_path = populate_log(&tmp);

        let entries = read(&log_path, &Filters {
            cwd: Some("/home/dev/projA".to_string()),
            ..Default::default()
        });
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.cwd == "/home/dev/projA"));
    }

    #[test]
    fn read_filters_by_exit_code() {
        let tmp = TempDir::new().unwrap();
        let log_path = populate_log(&tmp);

        let entries = read(&log_path, &Filters {
            exit_code: Some(1),
            ..Default::default()
        });
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cmd, "git push origin main");
    }

    #[test]
    fn read_filters_by_since() {
        let tmp = TempDir::new().unwrap();
        let log_path = populate_log(&tmp);

        let entries = read(&log_path, &Filters {
            since: Some("2026-03-30T11:30:00.000Z".to_string()),
            ..Default::default()
        });
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].cmd, "cargo test");
        assert_eq!(entries[1].cmd, "git diff");
    }

    #[test]
    fn read_combines_multiple_filters() {
        let tmp = TempDir::new().unwrap();
        let log_path = populate_log(&tmp);

        let entries = read(&log_path, &Filters {
            cmd_pattern: Some("git".to_string()),
            exit_code: Some(0),
            ..Default::default()
        });
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.cmd.contains("git") && e.exit_code == 0));
    }

    #[test]
    fn read_skips_malformed_lines() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("commands.jsonl");

        append(&sample_entry("cmd1", 0), &log_path).unwrap();
        // Write a malformed line directly
        let mut file = std::fs::OpenOptions::new().append(true).open(&log_path).unwrap();
        std::io::Write::write_all(&mut file, b"this is not json\n").unwrap();
        append(&sample_entry("cmd2", 0), &log_path).unwrap();

        let entries = read(&log_path, &Filters::default());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].cmd, "cmd1");
        assert_eq!(entries[1].cmd, "cmd2");
    }

    #[test]
    fn read_empty_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("commands.jsonl");
        std::fs::write(&log_path, "").unwrap();

        let entries = read(&log_path, &Filters::default());
        assert!(entries.is_empty());
    }

    #[test]
    fn read_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("nonexistent.jsonl");

        let entries = read(&log_path, &Filters::default());
        assert!(entries.is_empty());
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