//! Kronos 1-Click Public Preview Tunnel
//!
//! Exposes running local development servers (e.g. `http://localhost:3000`)
//! to teammates and mobile test devices via real `cloudflared` or `bore` subprocess tunnels.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TunnelProvider {
    Cloudflare,
    Bore,
    Localhost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum TunnelState {
    Inactive,
    Starting { local_port: u16 },
    Active {
        local_port: u16,
        public_url: String,
        provider: TunnelProvider,
    },
    Failed { error: String },
}

impl Default for TunnelState {
    fn default() -> Self {
        TunnelState::Inactive
    }
}

pub struct PreviewTunnelManager {
    current_state: TunnelState,
    child: Option<Child>,
}

impl Drop for PreviewTunnelManager {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Extracts a live public URL from `cloudflared` or `bore` output lines.
pub fn extract_tunnel_url(line: &str, provider: &TunnelProvider) -> Option<String> {
    match provider {
        TunnelProvider::Cloudflare => {
            if let Some(idx) = line.find("https://") {
                let tail = &line[idx..];
                let url_part = tail
                    .split(|c: char| c.is_whitespace() || matches!(c, '|' | '"' | '\'' | '<' | '>' | ')'))
                    .next()
                    .unwrap_or("");
                if url_part.contains("trycloudflare.com") {
                    return Some(url_part.trim_end_matches('/').to_string());
                }
            }
            None
        }
        TunnelProvider::Bore => {
            if let Some(idx) = line.find("bore.pub:") {
                let tail = &line[idx..];
                let host_port = tail
                    .split(|c: char| c.is_whitespace() || matches!(c, '|' | '"' | '\'' | '<' | '>' | ')'))
                    .next()
                    .unwrap_or("");
                if host_port.len() > "bore.pub:".len() {
                    return Some(format!("http://{host_port}"));
                }
            }
            None
        }
        TunnelProvider::Localhost => None,
    }
}

fn spawn_line_forwarder<R: std::io::Read + Send + 'static>(
    reader: R,
    tx: mpsc::Sender<String>,
) {
    std::thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            // Keep forwarding until receiver drops, then drain silently so child pipe never blocks
            let _ = tx.send(line);
        }
    });
}

impl PreviewTunnelManager {
    pub fn new() -> Self {
        Self {
            current_state: TunnelState::Inactive,
            child: None,
        }
    }

    pub fn state(&self) -> &TunnelState {
        &self.current_state
    }

    /// Starts a real tunnel process forwarding `127.0.0.1:<local_port>` to a public URL.
    pub fn start(&mut self, local_port: u16, provider: TunnelProvider) -> Result<TunnelState, String> {
        self.stop();

        if local_port == 0 {
            let err = "Invalid local port 0 for preview tunnel".to_string();
            self.current_state = TunnelState::Failed { error: err.clone() };
            return Err(err);
        }

        if provider == TunnelProvider::Localhost {
            let state = TunnelState::Active {
                local_port,
                public_url: format!("http://localhost:{local_port}"),
                provider,
            };
            self.current_state = state.clone();
            return Ok(state);
        }

        self.current_state = TunnelState::Starting { local_port };

        let (bin_name, args): (&str, Vec<String>) = match provider {
            TunnelProvider::Cloudflare => (
                "cloudflared",
                vec![
                    "tunnel".to_string(),
                    "--url".to_string(),
                    format!("http://127.0.0.1:{local_port}"),
                    "--no-autoupdate".to_string(),
                ],
            ),
            TunnelProvider::Bore => (
                "bore",
                vec![
                    "local".to_string(),
                    local_port.to_string(),
                    "--to".to_string(),
                    "bore.pub".to_string(),
                ],
            ),
            TunnelProvider::Localhost => unreachable!(),
        };

        let bin_path = match crate::agent::registry::resolve_command(bin_name) {
            Some(path) => path,
            None => {
                let err = format!(
                    "'{bin_name}' CLI not found on PATH. Install {bin_name} to share a live public preview URL."
                );
                self.current_state = TunnelState::Failed { error: err.clone() };
                return Err(err);
            }
        };

        let mut cmd = Command::new(&bin_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err = format!("Failed to spawn '{bin_name}': {e}");
                self.current_state = TunnelState::Failed { error: err.clone() };
                return Err(err);
            }
        };

        let (tx, rx) = mpsc::channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            spawn_line_forwarder(stdout, tx.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_line_forwarder(stderr, tx);
        }

        let deadline = Instant::now() + Duration::from_secs(12);
        let mut recent_logs = Vec::new();
        while Instant::now() < deadline {
            if let Ok(Some(status)) = child.try_wait() {
                let detail = if recent_logs.is_empty() {
                    format!("'{bin_name}' exited with status {status}")
                } else {
                    recent_logs.join(" | ")
                };
                self.current_state = TunnelState::Failed { error: detail.clone() };
                return Err(detail);
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(remaining.min(Duration::from_millis(200))) {
                Ok(line) => {
                    if let Some(url) = extract_tunnel_url(&line, &provider) {
                        self.child = Some(child);
                        let state = TunnelState::Active {
                            local_port,
                            public_url: url,
                            provider,
                        };
                        self.current_state = state.clone();
                        return Ok(state);
                    }
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        recent_logs.push(trimmed.chars().take(180).collect::<String>());
                        if recent_logs.len() > 4 {
                            recent_logs.remove(0);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let _ = child.kill();
        let _ = child.wait();
        let err = format!(
            "Timed out waiting for public URL from '{bin_name}' on port {local_port}"
        );
        self.current_state = TunnelState::Failed { error: err.clone() };
        Err(err)
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.current_state = TunnelState::Inactive;
    }
}

impl Default for PreviewTunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_TUNNEL_MANAGER: LazyLock<Arc<Mutex<PreviewTunnelManager>>> =
    LazyLock::new(|| Arc::new(Mutex::new(PreviewTunnelManager::new())));

pub fn get_tunnel_manager() -> &'static Arc<Mutex<PreviewTunnelManager>> {
    &GLOBAL_TUNNEL_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cloudflare_and_bore_urls() {
        let cf_line = "2026-11-13T12:00:00Z INF +----------------------------------------------------+ |  https://silver-cloud-demo.trycloudflare.com  |";
        assert_eq!(
            extract_tunnel_url(cf_line, &TunnelProvider::Cloudflare).as_deref(),
            Some("https://silver-cloud-demo.trycloudflare.com")
        );

        let bore_line = "2026-11-13 INFO bore_cli::client: listening at bore.pub:43210";
        assert_eq!(
            extract_tunnel_url(bore_line, &TunnelProvider::Bore).as_deref(),
            Some("http://bore.pub:43210")
        );
    }

    #[test]
    fn test_localhost_tunnel_lifecycle() {
        let mut manager = PreviewTunnelManager::new();
        assert_eq!(*manager.state(), TunnelState::Inactive);

        let state = manager
            .start(3000, TunnelProvider::Localhost)
            .expect("Localhost tunnel start");
        assert_eq!(
            state,
            TunnelState::Active {
                local_port: 3000,
                public_url: "http://localhost:3000".to_string(),
                provider: TunnelProvider::Localhost,
            }
        );

        manager.stop();
        assert_eq!(*manager.state(), TunnelState::Inactive);
    }
}
