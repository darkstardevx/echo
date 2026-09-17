//! Reads WraithFlow's capture log (`{pipeline, direction, at, format,
//! rendered}` JSON lines — see wraithflow's `wf-proxy/src/lib.rs`
//! `CaptureRecord`) and the tail-new-lines-only logic Echo's live-follow
//! mode uses.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Flow {
    pub pipeline: String,
    pub direction: String,
    pub at: DateTime<Utc>,
    pub format: String,
    pub rendered: String,
}

/// Reads every flow currently in the log, oldest first. A malformed line
/// (caught mid-write) is skipped rather than aborting the whole read —
/// same reasoning as Argus's `events::read_all`.
pub fn read_all(log_path: &Path) -> Result<Vec<Flow>> {
    if !log_path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(log_path)?;
    Ok(raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_real_wraithflow_capture_line() {
        let dir = std::env::temp_dir().join(format!("echo-flow-test1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("captures.jsonl");
        std::fs::write(
            &path,
            r#"{"pipeline":"http-gateway","direction":"OUTBOUND","at":"2026-09-17T22:09:10.448920208+00:00","format":"compact","rendered":"[OUTBOUND] 53B \"hello\""}
"#,
        )
        .unwrap();

        let flows = read_all(&path).unwrap();
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].pipeline, "http-gateway");
        assert_eq!(flows[0].direction, "OUTBOUND");
        assert_eq!(flows[0].format, "compact");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skips_malformed_lines_without_failing_the_whole_read() {
        let dir = std::env::temp_dir().join(format!("echo-flow-test2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("captures.jsonl");
        std::fs::write(&path, "not json\n{\"broken\":\n").unwrap();

        let flows = read_all(&path).unwrap();
        assert!(flows.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_log_file_returns_empty_not_an_error() {
        let path = Path::new("/tmp/echo-test-definitely-does-not-exist.jsonl");
        let flows = read_all(path).unwrap();
        assert!(flows.is_empty());
    }
}
