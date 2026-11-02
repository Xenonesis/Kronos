use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct OmpAgentClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl OmpAgentClient {
    pub async fn spawn(cwd: &Path) -> Result<Self, String> {
        let mut cmd = Command::new("omp");
        cmd.arg("--stdio")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn OMP CLI: {e}"))?;
        let stdin = child.stdin.take().ok_or("Failed to capture OMP stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture OMP stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }

    pub async fn send_json(&mut self, val: &Value) -> Result<(), String> {
        let line = format!("{}\n", val);
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed writing to OMP stdin: {e}"))?;
        self.stdin.flush().await.map_err(|e| format!("Failed flushing OMP stdin: {e}"))?;
        Ok(())
    }

    pub async fn read_json(&mut self) -> Result<Option<Value>, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed reading from OMP stdout: {e}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let val: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("Invalid JSON from OMP: {e} (line: {trimmed})"))?;
        Ok(Some(val))
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|e| format!("Failed killing OMP process: {e}"))
    }
}

#[cfg(test)]
mod tests {


    #[test]
    fn test_omp_rpc_framing() {
        let envelope = serde_json::json!({
            "type": "omp/turn",
            "instruction": "Fix memory leak"
        });
        let line = format!("{}\n", envelope);
        assert!(line.contains("omp/turn"));
    }
}
