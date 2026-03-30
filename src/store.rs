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