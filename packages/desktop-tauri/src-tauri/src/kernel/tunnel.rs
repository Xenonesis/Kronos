//! Kronos 1-Click Public Preview Tunnel
//!
//! Exposes running local development servers (e.g. `http://localhost:3000`)
//! to teammates and mobile test devices via lightweight public secure tunnels.

use serde::{Deserialize, Serialize};

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
}

impl PreviewTunnelManager {
    pub fn new() -> Self {
        Self {
            current_state: TunnelState::Inactive,
        }
    }

    pub fn state(&self) -> &TunnelState {
        &self.current_state
    }

    /// Creates an active tunnel representation for the given port and provider
    pub fn simulate_tunnel_start(&mut self, local_port: u16, provider: TunnelProvider) -> String {
        let slug = format!("kronos-preview-{}", local_port);
        let public_url = match provider {
            TunnelProvider::Cloudflare => format!("https://{}.trycloudflare.com", slug),
            TunnelProvider::Bore => format!("http://bore.pub:{}", local_port + 10000),
            TunnelProvider::Localhost => format!("http://localhost:{}", local_port),
        };

        self.current_state = TunnelState::Active {
            local_port,
            public_url: public_url.clone(),
            provider,
        };

        public_url
    }

    pub fn stop(&mut self) {
        self.current_state = TunnelState::Inactive;
    }
}

impl Default for PreviewTunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_tunnel_lifecycle() {
        let mut manager = PreviewTunnelManager::new();
        assert_eq!(*manager.state(), TunnelState::Inactive);

        let url = manager.simulate_tunnel_start(3000, TunnelProvider::Cloudflare);
        assert!(url.contains("trycloudflare.com"));

        if let TunnelState::Active { local_port, public_url, .. } = manager.state() {
            assert_eq!(*local_port, 3000);
            assert_eq!(public_url, &url);
        } else {
            panic!("Expected active state");
        }

        manager.stop();
        assert_eq!(*manager.state(), TunnelState::Inactive);
    }
}
