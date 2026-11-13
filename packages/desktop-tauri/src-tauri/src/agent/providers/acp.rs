use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub struct AcpAgentClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl AcpAgentClient {
    /// Spawns an ACP-compatible agent command (e.g. cursor, copilot, or generic bridge).
    pub async fn spawn(command: &str, args: &[&str], cwd: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn ACP agent {command}: {e}"))?;
        let stdin = child.stdin.take().ok_or("Failed to capture ACP stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture ACP stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }

    /// Sends a JSON-RPC request to the ACP agent.
    pub async fn send_request(&mut self, id: &str, method: &str, params: Option<Value>) -> Result<(), String> {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            method: method.to_string(),
            params,
        };
        let line = format!("{}\n", serde_json::to_string(&req).unwrap());
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed writing to ACP stdin: {e}"))?;
        self.stdin.flush().await.map_err(|e| format!("Failed flushing ACP stdin: {e}"))?;
        Ok(())
    }

    /// Reads the next JSON-RPC line from the ACP agent.
    pub async fn read_response(&mut self) -> Result<Option<JsonRpcResponse>, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed reading from ACP stdout: {e}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let resp: JsonRpcResponse = serde_json::from_str(trimmed)
            .map_err(|e| format!("Invalid JSON-RPC from ACP agent: {e} (line: {trimmed})"))?;
        Ok(Some(resp))
    }

    /// Reads the next JSON-RPC line from the ACP agent as a raw Value (handles notifications and responses).
    pub async fn read_message(&mut self) -> Result<Option<Value>, String> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("Failed reading from ACP stdout: {e}"))?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        serde_json::from_str::<Value>(trimmed)
            .map(Some)
            .map_err(|e| format!("Invalid JSON-RPC from ACP agent: {e} (line: {trimmed})"))
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|e| format!("Failed killing ACP process: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_json_rpc_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: "req-42".to_string(),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({ "client": "kronos" })),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
    }
}
