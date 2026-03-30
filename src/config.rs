use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub shell: String,
    pub log_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shell: "/bin/zsh".to_string(),
            log_path: default_log_path().to_string_lossy().to_string(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        toml::from_str(&content).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, content)
    }
}

pub fn resolve_shell(config_path: &Path) -> String {
    if let Ok(val) = std::env::var("SHLOG_REAL_SHELL") {
        if !val.is_empty() {
            return val;
        }
    }
    let config = Config::load(config_path);
    config.shell
}

pub fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".shlog/config.toml")
}

pub fn default_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".shlog/logs/commands.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_returns_defaults_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("nonexistent/config.toml");

        let config = Config::load(&config_path);

        assert_eq!(config.shell, "/bin/zsh");
        assert!(config.log_path.ends_with(".shlog/logs/commands.jsonl"));
    }

    #[test]
    fn save_creates_file_and_directories() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("nested/dir/config.toml");

        let config = Config {
            shell: "/bin/bash".to_string(),
            log_path: "/custom/path/commands.jsonl".to_string(),
        };

        config.save(&config_path).unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn save_then_load_round_trips() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let original = Config {
            shell: "/bin/bash".to_string(),
            log_path: "/custom/log.jsonl".to_string(),
        };

        original.save(&config_path).unwrap();
        let loaded = Config::load(&config_path);

        assert_eq!(loaded.shell, original.shell);
        assert_eq!(loaded.log_path, original.log_path);
    }

    #[test]
    fn resolve_shell_prefers_env_var() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        // Save config with /bin/bash
        let config = Config {
            shell: "/bin/bash".to_string(),
            log_path: "unused".to_string(),
        };
        config.save(&config_path).unwrap();

        // Env var should take precedence
        unsafe { std::env::set_var("SHLOG_REAL_SHELL", "/bin/fish") };
        let result = resolve_shell(&config_path);
        unsafe { std::env::remove_var("SHLOG_REAL_SHELL") };

        assert_eq!(result, "/bin/fish");
    }

    #[test]
    fn resolve_shell_falls_back_to_config() {
        // Ensure env var is unset
        unsafe { std::env::remove_var("SHLOG_REAL_SHELL") };

        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let config = Config {
            shell: "/bin/bash".to_string(),
            log_path: "unused".to_string(),
        };
        config.save(&config_path).unwrap();

        let result = resolve_shell(&config_path);
        assert_eq!(result, "/bin/bash");
    }

    #[test]
    fn resolve_shell_falls_back_to_zsh() {
        unsafe { std::env::remove_var("SHLOG_REAL_SHELL") };

        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("nonexistent.toml");

        let result = resolve_shell(&config_path);
        assert_eq!(result, "/bin/zsh");
    }
}
