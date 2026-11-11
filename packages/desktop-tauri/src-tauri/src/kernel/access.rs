use axum::http::{header, HeaderMap, Uri};
use serde_json::Value;
use std::path::Path;

use super::auth::{matches_local_credential, verify_daemon_password, write_local_credential};

/// Never send this daemon's credential to a saved remote host.
pub fn is_managed_target(listen: &str) -> bool {
    #[cfg(windows)]
    if listen == r"pipe://\\.\pipe\kronos-ipc" || listen == r"\\.\pipe\kronos-ipc" {
        return true;
    }
    #[cfg(not(windows))]
    if listen == "unix:///tmp/kronos-ipc.sock" || listen == "/tmp/kronos-ipc.sock" {
        return true;
    }
    let normalized;
    let candidate = if listen.contains("://") {
        listen
    } else {
        normalized = format!("tcp://{listen}");
        &normalized
    };
    let Ok(url) = url::Url::parse(candidate) else {
        return false;
    };
    let active_port = crate::daemon::get_active_daemon_port();
    matches!(url.scheme(), "tcp" | "ws" | "http")
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        && (url.port_or_known_default() == Some(6767) || url.port_or_known_default() == Some(active_port))
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "" | "/" | "/ws")
        && url.query().is_none()
        && url.fragment().is_none()
}

/// One authority per listening daemon. Never authenticate by Origin alone.
#[derive(Clone)]
pub struct AccessControl {
    pub(crate) local_credential: String,
    password_hash: Option<String>,
    port: u16,
    allowed_origins: Vec<String>,
}

impl AccessControl {
    pub fn load(home: &Path, port: u16) -> std::io::Result<Self> {
        let config = match std::fs::read(home.join("config.json")) {
            Ok(bytes) => serde_json::from_slice::<super::config_store::DaemonConfig>(&bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
            Err(e) => return Err(e),
        };
        let allowed_origins = std::env::var("KRONOS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|origin| {
                let uri: Uri = origin.parse().map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid KRONOS_ALLOWED_ORIGINS entry",
                    )
                })?;
                if !matches!(uri.scheme_str(), Some("http" | "https"))
                    || uri.authority().is_none()
                    || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
                    || origin.ends_with('/')
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Allowed origins must be exact http(s) origins without paths or wildcards",
                    ));
                }
                if uri
                    .authority()
                    .is_some_and(|a| a.as_str().contains(['*', '@']))
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid origin authority",
                    ));
                }
                Ok(origin.to_string())
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(Self {
            local_credential: write_local_credential(home)?,
            password_hash: config.password_hash,
            port,
            allowed_origins,
        })
    }

    pub fn allows_headers(&self, headers: &HeaderMap) -> bool {
        if headers.get_all(header::HOST).iter().count() != 1 {
            return false;
        }
        let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
            return false;
        };
        let Ok(authority) = host.parse::<axum::http::uri::Authority>() else {
            return false;
        };
        if !matches!(authority.host(), "127.0.0.1" | "localhost" | "[::1]")
            || authority.port_u16().unwrap_or(80) != self.port
        {
            return false;
        }
        if headers.get_all(header::ORIGIN).iter().count() > 1 {
            return false;
        }
        let Some(origin) = headers.get(header::ORIGIN) else {
            return true;
        };
        let Ok(origin) = origin.to_str() else {
            return false;
        };
        if matches!(
            origin,
            "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
        ) || self.allowed_origins.iter().any(|allowed| allowed == origin)
        {
            return true;
        }
        // The daemon's own origin is permitted, but still needs credentials.
        if origin.strip_prefix("http://") == Some(host) {
            return true;
        }
        #[cfg(debug_assertions)]
        if matches!(origin, "http://localhost:8081" | "http://127.0.0.1:8081") {
            return true;
        }
        false
    }

    pub fn accepts_bearer(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|candidate| matches_local_credential(&self.local_credential, candidate))
    }

    pub async fn authenticate_hello(&self, hello: &Value) -> Result<(), &'static str> {
        if hello["type"].as_str() != Some("hello") {
            return Err("password_required");
        }
        if hello["protocolVersion"]
            .as_u64()
            .is_none_or(|version| version < 1)
        {
            return Err("incompatible_protocol");
        }
        match hello["auth"]["kind"].as_str() {
            Some("localCredential") => {
                if hello["auth"]["token"].as_str().is_some_and(|candidate| {
                    matches_local_credential(&self.local_credential, candidate)
                }) {
                    Ok(())
                } else {
                    Err("incorrect_password")
                }
            }
            Some("password") => {
                let Some(hash) = self.password_hash.clone() else {
                    return Err("incorrect_password");
                };
                let Some(candidate) = hello["auth"]["password"].as_str() else {
                    return Err("incorrect_password");
                };
                // Bound bcrypt input and keep expensive verification off the socket runtime.
                if candidate.len() > 1024 {
                    return Err("incorrect_password");
                }
                let candidate = candidate.to_string();
                if tokio::task::spawn_blocking(move || verify_daemon_password(&candidate, &hash))
                    .await
                    .unwrap_or(false)
                {
                    Ok(())
                } else {
                    Err("incorrect_password")
                }
            }
            _ => Err("password_required"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn desktop_credentials_are_scoped_to_the_managed_listener() {
        for target in [
            "127.0.0.1:6767",
            "tcp://127.0.0.1:6767",
            "ws://localhost:6767/ws",
        ] {
            assert!(is_managed_target(target), "{target}");
        }
        for target in [
            "remote.example:6767",
            "127.0.0.1:6768",
            "127.0.0.1.evil:6767",
            "ws://user@localhost:6767",
            "ws://localhost:6767/ws?target=remote",
            "unix:///tmp/other.sock",
        ] {
            assert!(!is_managed_target(target), "{target}");
        }
    }

    fn authority() -> AccessControl {
        AccessControl {
            local_credential: "a".repeat(43),
            password_hash: None,
            port: 19102,
            allowed_origins: vec!["http://localhost:19103".into()],
        }
    }

    #[test]
    fn rejects_rebinding_and_untrusted_browser_origins() {
        let access = authority();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:19102"));
        for origin in [
            "http://evil.test",
            "null",
            "http://tauri.localhost.evil.test",
            "http://localhost:9999",
        ] {
            headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
            assert!(!access.allows_headers(&headers), "{origin}");
        }
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://tauri.localhost"),
        );
        assert!(access.allows_headers(&headers));
        headers.insert(header::HOST, HeaderValue::from_static("evil.test:19102"));
        assert!(!access.allows_headers(&headers));
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:6767"));
        assert!(!access.allows_headers(&headers));
    }

    #[tokio::test]
    async fn local_origin_and_missing_origin_are_not_credentials() {
        let access = authority();
        assert_eq!(
            access
                .authenticate_hello(&serde_json::json!({"type":"hello","protocolVersion":1}))
                .await,
            Err("password_required")
        );
        assert_eq!(access.authenticate_hello(&serde_json::json!({"type":"session","auth":{"kind":"localCredential","token":"a".repeat(43)}})).await,
            Err("password_required"));
        assert_eq!(
            access
                .authenticate_hello(&serde_json::json!({"type":"hello","protocolVersion":1,
            "auth":{"kind":"localCredential","token":"b".repeat(43)}}))
                .await,
            Err("incorrect_password")
        );
        assert!(access
            .authenticate_hello(&serde_json::json!({"type":"hello","protocolVersion":1,
            "auth":{"kind":"localCredential","token":"a".repeat(43)}}))
            .await
            .is_ok());
    }
}
