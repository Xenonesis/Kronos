use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

/// Encrypts plaintext bytes using ChaCha20-Poly1305 with a 32-byte symmetric key and 12-byte nonce.
pub fn encrypt_frame(
    key: &[u8; 32],
    nonce: &[u8; 12],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("ChaCha20Poly1305 encryption failed: {e}"))
}

/// Decrypts ciphertext bytes using ChaCha20-Poly1305 with a 32-byte symmetric key and 12-byte nonce.
pub fn decrypt_frame(
    key: &[u8; 32],
    nonce: &[u8; 12],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("ChaCha20Poly1305 decryption failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chacha20_poly1305_roundtrip() {
        let key = [0x42u8; 32];
        let nonce = [0x13u8; 12];
        let message = b"Kronos Relay E2EE Protected Frame";

        let encrypted = encrypt_frame(&key, &nonce, message).expect("Encrypt frame");
        assert_ne!(encrypted, message);

        let decrypted = decrypt_frame(&key, &nonce, &encrypted).expect("Decrypt frame");
        assert_eq!(decrypted, message);

        // Tampered ciphertext must fail authentication
        let mut tampered = encrypted.clone();
        tampered[0] ^= 0xFF;
        assert!(decrypt_frame(&key, &nonce, &tampered).is_err());
    }
}
