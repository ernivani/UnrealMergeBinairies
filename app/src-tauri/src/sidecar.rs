//! Spawns the UE commandlet (or test mock), sends JSON-RPC frames over stdin,
//! and extracts balanced top-level JSON objects from stdout.
//!
//! Mirrors `tools/run-commandlet.ps1`'s logic; the launcher script is kept as
//! the canonical reference and for ad-hoc shell smoke testing.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct SidecarConfig {
    /// Path to the executable to spawn (UnrealEditor.exe in production, mock in tests).
    pub executable: PathBuf,
    /// Args passed to the executable BEFORE the JSON-RPC stdio session starts.
    pub args: Vec<String>,
    /// When true, prepend `{"id":0,"cmd":"_warmup"}` to the stdin payload
    /// to absorb UE's first-stdin-line eating. Set false for the mock.
    pub prepend_warmup: bool,
    /// If set, pass `-AbsLog=<path>` to UE so its logs don't interleave with
    /// our JSON frames on stdout. Ignored for the mock.
    pub log_redirect: Option<PathBuf>,
}

pub struct Sidecar {
    cfg: SidecarConfig,
}

impl Sidecar {
    pub fn new(cfg: SidecarConfig) -> Self {
        Self { cfg }
    }

    /// Send a sequence of JSON requests and return the captured JSON responses.
    /// Always appends a `{"cmd":"quit"}` if the caller hasn't already, so the
    /// child exits cleanly.
    pub fn run_batch(&self, requests: &[serde_json::Value]) -> Result<Vec<serde_json::Value>> {
        let mut payload = String::new();
        if self.cfg.prepend_warmup {
            payload.push_str("{\"id\":0,\"cmd\":\"_warmup\"}\n");
        }
        for req in requests {
            payload.push_str(&serde_json::to_string(req)?);
            payload.push('\n');
        }
        if !requests
            .iter()
            .any(|r| r.get("cmd").and_then(|c| c.as_str()) == Some("quit"))
        {
            payload.push_str("{\"cmd\":\"quit\"}\n");
        }

        let mut cmd = Command::new(&self.cfg.executable);
        cmd.args(&self.cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(log) = &self.cfg.log_redirect {
            cmd.arg(format!("-AbsLog={}", log.display()));
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawning {}", self.cfg.executable.display()))?;

        // Write stdin in a scope so it drops (and the pipe closes) before we wait.
        {
            let stdin = child
                .stdin
                .as_mut()
                .context("child has no stdin")?;
            stdin
                .write_all(payload.as_bytes())
                .context("writing stdin payload")?;
        }

        let output = child
            .wait_with_output()
            .context("waiting for child")?;
        // We don't check exit code — UE may exit nonzero for incidental reasons
        // even when all our exports succeeded. Trust the in-band JSON instead.

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(extract_json_objects(&stdout))
    }
}

/// Walk a string and return every balanced top-level JSON object found in it,
/// parsed via serde_json. Robust to text noise around or between objects.
pub fn extract_json_objects(text: &str) -> Vec<serde_json::Value> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        // Scan for the matching close brace, tracking string/escape state.
        let mut depth: i32 = 0;
        let mut in_str = false;
        let mut esc = false;
        let start = i;
        let mut end = None;
        let mut j = i;
        while j < bytes.len() {
            let c = bytes[j];
            if in_str {
                if esc {
                    esc = false;
                } else if c == b'\\' {
                    esc = true;
                } else if c == b'"' {
                    in_str = false;
                }
            } else if c == b'"' {
                in_str = true;
            } else if c == b'{' {
                depth += 1;
            } else if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(j);
                    break;
                }
            }
            j += 1;
        }
        match end {
            Some(e) => {
                let slice = &text[start..=e];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(slice) {
                    out.push(val);
                }
                i = e + 1;
            }
            None => break, // unclosed; stop scanning
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_two_objects_with_noise_between() {
        let text = "garbage {\"id\":1} more garbage {\"id\":2,\"x\":\"a }\"} end";
        let objs = extract_json_objects(text);
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0]["id"], 1);
        assert_eq!(objs[1]["id"], 2);
    }

    #[test]
    fn handles_nested_braces_inside_strings() {
        let text = "noise {\"a\":\"contains } and { inside\",\"b\":3} end";
        let objs = extract_json_objects(text);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0]["b"], 3);
    }

    #[test]
    fn handles_escaped_quotes() {
        let text = r#"{"a":"with \"quotes\" inside","b":1}"#;
        let objs = extract_json_objects(text);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0]["b"], 1);
    }
}
