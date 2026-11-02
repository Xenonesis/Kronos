use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeStreamEvent {
    ContentBlockStart {
        index: usize,
        content_block: Value,
    },
    ContentBlockDelta {
        index: usize,
        delta: Value,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageStart {
        message: Value,
    },
    MessageDelta {
        delta: Value,
    },
    MessageStop,
    Ping,
    Error {
        error: Value,
    },
}

pub struct ClaudeCliClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl ClaudeCliClient {
    pub async fn spawn(cwd: &Path, model: Option<&str>) -> Result<Self, String> {
        let mut cmd = Command::new("claude");
        cmd.arg("--output-format")
            .arg("stream-json")
            .arg("--input-format")
            .arg("stream-json")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn Claude CLI: {e}"))?;
        let stdin = child.stdin.take().ok_or("Failed to capture Claude stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture Claude stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }

    pub async fn send_prompt(&mut self, text: &str) -> Result<(), String> {
        let payload = serde_json::json!({
            "type": "user_message",
            "text": text
        });
        let line = format!("{}\n", payload);
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed writing to Claude stdin: {e}"))?;
        self.stdin.flush().await.map_err(|e| format!("Failed flushing Claude stdin: {e}"))?;
        Ok(())
    }

    pub async fn read_event(&mut self) -> Result<Option<Value>, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed reading from Claude stdout: {e}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let val: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("Invalid JSON from Claude: {e} (line: {trimmed})"))?;
        Ok(Some(val))
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|e| format!("Failed killing Claude process: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_stream_delta_serialization() {
        let raw = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": "Hello from Claude"
            }
        });

        let event: ClaudeStreamEvent = serde_json::from_value(raw).unwrap();
        match event {
            ClaudeStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                assert_eq!(delta["text"], "Hello from Claude");
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }
}
