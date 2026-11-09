use std::fs;
use std::path::Path;
use subtle::ConstantTimeEq;

pub const DAEMON_PASSWORD_BCRYPT_COST: u32 = 12;
const LOCAL_CREDENTIAL_FILE: &str = "local-credential";

/// Hashes a password string with bcrypt at standard daemon cost 12.
pub fn hash_daemon_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, DAEMON_PASSWORD_BCRYPT_COST)
}

/// Verifies a plain candidate password against a stored bcrypt hash.
pub fn verify_daemon_password(candidate: &str, hash: &str) -> bool {
    bcrypt::verify(candidate, hash).unwrap_or(false)
}

/// Generates and persists a cryptographically secure 32-byte base64url local credential token
/// into `{home}/local-credential` via an atomic temporary file rename.
pub fn write_local_credential(home: &Path) -> std::io::Result<String> {
    fs::create_dir_all(home)?;

    let token = generate_local_credential()?;

    use std::io::Write;
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp_file_path = home.join(format!(
        "{LOCAL_CREDENTIAL_FILE}.{}.{nonce}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp_file_path)?;
        file.write_all(token.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_file_path, home.join(LOCAL_CREDENTIAL_FILE))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_file_path);
    }
    result?;

    Ok(token)
}

pub fn generate_local_credential() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom_bytes(&mut bytes)?;
    Ok(base64_url_encode(&bytes))
}

/// Reads the local credential token from `{home}/local-credential`.
/// Validates length (must be 43 base64url characters).
pub fn read_local_credential(home: &Path) -> Option<String> {
    let path = home.join(LOCAL_CREDENTIAL_FILE);
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if is_valid_local_credential_token(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Deletes `{home}/local-credential`.
pub fn delete_local_credential(home: &Path) -> std::io::Result<()> {
    let path = home.join(LOCAL_CREDENTIAL_FILE);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Constant-time comparison to prevent timing attacks.
pub fn matches_local_credential(expected: &str, candidate: &str) -> bool {
    let a = expected.as_bytes();
    let b = candidate.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

/// Extracts Bearer token from standard Authorization header (e.g. "Bearer <token>").
pub fn extract_http_bearer_token(header_value: &str) -> Option<String> {
    let parts: Vec<&str> = header_value.split_whitespace().collect();
    if parts.len() == 2 && parts[0] == "Bearer" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Extracts Bearer token from WebSocket subprotocol negotiation (e.g. "paseo.bearer.<token>").
pub fn extract_ws_bearer_protocol(header_value: &str) -> Option<String> {
    for protocol in header_value.split(',') {
        let trimmed = protocol.trim();
        let segments: Vec<&str> = trimmed.split('.').collect();
        if segments.len() >= 3 && segments[0] == "paseo" && segments[1] == "bearer" {
            return Some(segments[2..].join("."));
        }
    }
    None
}

fn is_valid_local_credential_token(token: &str) -> bool {
    token.len() == 43
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn base64_url_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn getrandom_bytes(dest: &mut [u8]) -> std::io::Result<()> {
    // Fill buffer using Windows BCryptGenRandom or /dev/urandom
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::BCryptGenRandom;
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                dest.as_mut_ptr(),
                dest.len() as u32,
                windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("BCryptGenRandom failed with status 0x{status:08x}"),
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        let mut f = fs::File::open("/dev/urandom")?;
        f.read_exact(dest)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_bcrypt_password_hashing_and_verification() {
        let pass = "super-secret-password";
        let hash = hash_daemon_password(pass).expect("Hash password");
        assert!(verify_daemon_password(pass, &hash));
        assert!(!verify_daemon_password("wrong-password", &hash));
    }

    #[test]
    fn test_local_credential_lifecycle() {
        let dir = tempdir().unwrap();
        let home = dir.path();

        assert_eq!(read_local_credential(home), None);

        let token = write_local_credential(home).expect("Write local credential");
        assert_eq!(token.len(), 43);

        let read_token = read_local_credential(home).expect("Read token");
        assert_eq!(token, read_token);

        assert!(matches_local_credential(&token, &read_token));
        assert!(!matches_local_credential(&token, "invalid_length_token"));

        delete_local_credential(home).expect("Delete credential");
        assert_eq!(read_local_credential(home), None);
    }

    #[test]
    fn test_extract_bearer_tokens() {
        assert_eq!(
            extract_http_bearer_token("Bearer secret_tok_123"),
            Some("secret_tok_123".to_string())
        );
        assert_eq!(extract_http_bearer_token("Basic dXNlcjpwYXNz"), None);
        assert_eq!(extract_http_bearer_token(""), None);

        assert_eq!(
            extract_ws_bearer_protocol("paseo.v1, paseo.bearer.abc_xyz_789"),
            Some("abc_xyz_789".to_string())
        );
        assert_eq!(extract_ws_bearer_protocol("paseo.v1, something.else"), None);
    }
}
