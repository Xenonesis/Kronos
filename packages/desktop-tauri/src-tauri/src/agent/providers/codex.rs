use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct CodexAppServerClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl CodexAppServerClient {
    /// Spawns the codex app-server child process.
    pub async fn spawn(cwd: &Path) -> Result<Self, String> {
        let mut cmd = Command::new("codex");
        cmd.arg("app-server")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn codex: {e}"))?;
        let stdin = child.stdin.take().ok_or("Failed to capture codex stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture codex stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }

    /// Sends a JSON line to codex app-server.
    pub async fn send_json(&mut self, val: &Value) -> Result<(), String> {
        let line = format!("{}\n", val);
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed writing to codex stdin: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("Failed flushing to codex stdin: {e}"))?;
        Ok(())
    }

    /// Reads the next JSON line from codex app-server.
    pub async fn read_json(&mut self) -> Result<Option<Value>, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed reading from codex stdout: {e}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let val: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("Invalid JSON from codex: {e} (line: {trimmed})"))?;
        Ok(Some(val))
    }

    /// Gracefully kills the codex app server process.
    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|e| format!("Failed killing codex process: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_jsonl_framing_logic() {
        let sample = serde_json::json!({
            "id": "turn-1",
            "method": "start_turn",
            "params": {
                "prompt": "Hello Codex"
            }
        });
        let line = format!("{}\n", sample);
        assert!(line.ends_with('\n'));
        let parsed: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed["id"], "turn-1");
    }
}
