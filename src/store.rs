use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn serialize_log_entry_produces_correct_json() {
        let entry = LogEntry {
            ts: "2026-03-30T14:32:01.123Z".to_string(),
            cmd: "git push origin main".to_string(),
            cwd: "/Users/dev/myproject".to_string(),
            exit_code: 0,
            shell: "/bin/zsh".to_string(),
            ppid: 48291,
            duration_ms: 1423,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["ts"], "2026-03-30T14:32:01.123Z");
        assert_eq!(parsed["cmd"], "git push origin main");
        assert_eq!(parsed["cwd"], "/Users/dev/myproject");
        assert_eq!(parsed["exit_code"], 0);
        assert_eq!(parsed["shell"], "/bin/zsh");
        assert_eq!(parsed["ppid"], 48291);
        assert_eq!(parsed["duration_ms"], 1423);
    }

    #[test]
    fn deserialize_json_to_log_entry() {
        let json = r#"{
            "ts": "2026-03-30T14:32:01.123Z",
            "cmd": "cargo test",
            "cwd": "/Users/dev/project",
            "exit_code": 1,
            "shell": "/bin/bash",
            "ppid": 12345,
            "duration_ms": 5000
        }"#;

        let entry: LogEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.ts, "2026-03-30T14:32:01.123Z");
        assert_eq!(entry.cmd, "cargo test");
        assert_eq!(entry.cwd, "/Users/dev/project");
        assert_eq!(entry.exit_code, 1);
        assert_eq!(entry.shell, "/bin/bash");
        assert_eq!(entry.ppid, 12345);
        assert_eq!(entry.duration_ms, 5000);
    }

    #[test]
    fn round_trip_produces_identical_entry() {
        let original = LogEntry {
            ts: "2026-03-30T00:00:00.000Z".to_string(),
            cmd: "echo 'hello world'".to_string(),
            cwd: "/tmp".to_string(),
            exit_code: 127,
            shell: "/bin/zsh".to_string(),
            ppid: 1,
            duration_ms: 0,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: LogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(original, deserialized);
    }
}
