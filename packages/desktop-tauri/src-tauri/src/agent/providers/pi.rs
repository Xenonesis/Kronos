use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct PiAgentClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl PiAgentClient {
    pub async fn spawn(cwd: &Path, extension_paths: &[&str]) -> Result<Self, String> {
        let mut cmd = Command::new("pi");
        cmd.arg("--json")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for ext in extension_paths {
            cmd.arg("--extension").arg(ext);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn Pi CLI: {e}"))?;
        let stdin = child.stdin.take().ok_or("Failed to capture Pi stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture Pi stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }

    pub async fn send_line(&mut self, text: &str) -> Result<(), String> {
        let line = format!("{}\n", text.trim());
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed writing to Pi stdin: {e}"))?;
        self.stdin.flush().await.map_err(|e| format!("Failed flushing Pi stdin: {e}"))?;
        Ok(())
    }

    pub async fn read_event(&mut self) -> Result<Option<Value>, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed reading from Pi stdout: {e}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let val: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("Invalid JSON from Pi: {e} (line: {trimmed})"))?;
        Ok(Some(val))
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|e| format!("Failed killing Pi process: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_event_framing() {
        let raw = "{\"type\":\"tool_call\",\"name\":\"read_file\",\"path\":\"src/main.rs\"}\n";
        let parsed: Value = serde_json::from_str(raw.trim()).unwrap();
        assert_eq!(parsed["type"], "tool_call");
        assert_eq!(parsed["name"], "read_file");
    }
}
