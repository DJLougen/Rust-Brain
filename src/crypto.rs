use crate::{EncryptedPayload, RbmemError};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionKey {
    bytes: [u8; 32],
}

impl EncryptionKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn from_env_value(value: &str) -> Result<Self, RbmemError> {
        let trimmed = value.trim();
        // A raw key is exactly 32 bytes; anything else must be base64 of 32
        // bytes. Branching on length avoids misreading a 32-byte raw key that
        // happens to be valid base64 (which would decode to the wrong length).
        if trimmed.len() == 32 {
            return Self::from_slice(trimmed.as_bytes());
        }
        let decoded = STANDARD.decode(trimmed).map_err(|error| {
            RbmemError::Crypto(format!(
                "encryption key must be 32 raw bytes or base64-encoded 32 bytes: {error}"
            ))
        })?;
        Self::from_slice(&decoded)
    }

    pub fn resolve() -> Result<Self, RbmemError> {
        if let Ok(value) = std::env::var("RBMEM_ENCRYPTION_KEY") {
            if !value.trim().is_empty() {
                return Self::from_env_value(&value);
            }
        }

        if let Some(path) = default_key_path() {
            if path.exists() {
                return Self::from_env_value(&fs::read_to_string(path)?);
            }
        }

        if !io::stdin().is_terminal() {
            return Err(RbmemError::Crypto(
                "no encryption key found and stdin is not interactive; set RBMEM_ENCRYPTION_KEY \
                 or place key in ~/.rbmem/key"
                    .into(),
            ));
        }

        print!("RBMEM encryption key: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Self::from_env_value(&input)
    }

    fn from_slice(bytes: &[u8]) -> Result<Self, RbmemError> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
            RbmemError::Crypto("encryption key must be 32 bytes or base64-encoded 32 bytes".into())
        })?;
        Ok(Self { bytes })
    }
}

pub fn encrypt_content(
    plaintext: &str,
    key: &EncryptionKey,
    encrypted_at: DateTime<Utc>,
) -> Result<EncryptedPayload, RbmemError> {
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| RbmemError::Crypto("failed to generate encryption nonce".into()))?;

    let sealing_key = less_safe_key(key)?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = plaintext.as_bytes().to_vec();
    sealing_key
        .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| RbmemError::Crypto("failed to encrypt section".into()))?;

    Ok(EncryptedPayload {
        nonce: STANDARD.encode(nonce_bytes),
        ciphertext: STANDARD.encode(in_out),
        encrypted_at,
    })
}

pub fn decrypt_content(
    payload: &EncryptedPayload,
    key: &EncryptionKey,
) -> Result<String, RbmemError> {
    let nonce_bytes = STANDARD
        .decode(&payload.nonce)
        .map_err(|error| RbmemError::Crypto(format!("invalid encrypted section nonce: {error}")))?;
    let nonce_bytes: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| RbmemError::Crypto("encrypted section nonce must be 12 bytes".into()))?;
    let mut in_out = STANDARD.decode(&payload.ciphertext).map_err(|error| {
        RbmemError::Crypto(format!("invalid encrypted section ciphertext: {error}"))
    })?;

    let opening_key = less_safe_key(key)?;
    let plaintext = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut in_out,
        )
        .map_err(|_| RbmemError::Crypto("failed to decrypt section".into()))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|error| RbmemError::Crypto(format!("decrypted section is not UTF-8: {error}")))
}

fn less_safe_key(key: &EncryptionKey) -> Result<LessSafeKey, RbmemError> {
    let unbound = UnboundKey::new(&AES_256_GCM, key.as_bytes())
        .map_err(|_| RbmemError::Crypto("failed to initialize AES-256-GCM key".into()))?;
    Ok(LessSafeKey::new(unbound))
}

fn default_key_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .map(|home| home.join(".rbmem").join("key"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_32_byte_key_is_accepted_even_when_base64_shaped() {
        // 32 ASCII chars that are also valid base64 (would decode to 24 bytes).
        let raw = "abcdefghabcdefghabcdefghabcdefgh";
        assert_eq!(raw.len(), 32);
        let key = EncryptionKey::from_env_value(raw).unwrap();
        assert_eq!(key.as_bytes(), raw.as_bytes());
    }

    #[test]
    fn base64_encoded_32_bytes_round_trips() {
        let bytes = [7u8; 32];
        let encoded = STANDARD.encode(bytes);
        assert_ne!(encoded.len(), 32);
        let key = EncryptionKey::from_env_value(&encoded).unwrap();
        assert_eq!(key.as_bytes(), &bytes);
    }

    #[test]
    fn wrong_length_base64_key_is_rejected() {
        // Valid base64 ("YWJj" -> 3 bytes) but not 32 decoded bytes.
        let err = EncryptionKey::from_env_value("YWJj").unwrap_err();
        assert!(matches!(err, RbmemError::Crypto(_)));
    }
}
