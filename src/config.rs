//! Finds WraithFlow's `capture_log` path the same way WraithFlow itself
//! resolves its own config — reading its real config file rather than
//! assuming a fixed location, so Echo always points at whatever WraithFlow
//! is actually configured to write.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Default)]
struct WraithflowConfig {
    #[serde(default)]
    capture_log: Option<String>,
}

/// Same lookup order as `wraithflow`'s own `default_config_path`.
pub fn default_wraithflow_config_path() -> Option<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok()?;
    let candidates = [
        config_home.join("wraithflow").join("config.toml"),
        PathBuf::from("config.toml"),
        PathBuf::from("config.json"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

/// Reads WraithFlow's config and returns its configured `capture_log`
/// path, expanded. `None` if no config was found, it couldn't be parsed,
/// or `capture_log` isn't set there (WraithFlow doesn't have capture
/// logging enabled yet).
pub fn find_capture_log_path(wraithflow_config: &std::path::Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(wraithflow_config).ok()?;
    let is_json = wraithflow_config.extension().and_then(|e| e.to_str()) == Some("json");
    let config: WraithflowConfig = if is_json {
        serde_json::from_str(&raw).ok()?
    } else {
        toml::from_str(&raw).ok()?
    };
    config.capture_log.map(|p| expand_home(&p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_real_wraithflow_config_shape() {
        let raw = r#"
stats_interval_secs = 30
capture_log = "~/.local/state/wraithflow/captures.jsonl"

[[proxies]]
name = "http-traffic-gateway"
listen = "127.0.0.1:45634"
target = "127.0.0.1:9000"
"#;
        let dir = std::env::temp_dir().join(format!("echo-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, raw).unwrap();

        let found = find_capture_log_path(&path).unwrap();
        assert!(found.ends_with(".local/state/wraithflow/captures.jsonl"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn returns_none_when_capture_log_is_not_configured() {
        let raw = r#"
stats_interval_secs = 30

[[proxies]]
name = "x"
listen = "127.0.0.1:1"
target = "127.0.0.1:2"
"#;
        let dir = std::env::temp_dir().join(format!("echo-config-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, raw).unwrap();

        assert!(find_capture_log_path(&path).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }
}
