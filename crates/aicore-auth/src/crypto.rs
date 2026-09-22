use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption error: {0}")]
    Encrypt(String),
    #[error("Decryption error: {0}")]
    Decrypt(String),
    #[error("Data too short")]
    TooShort,
}

pub struct Crypto {
    cipher: Aes256Gcm,
}

impl Crypto {
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::Encrypt(e.to_string()))?;
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if data.len() < 12 + 16 {
            return Err(CryptoError::TooShort);
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::Decrypt(e.to_string()))
    }
}

pub fn derive_key_from_seed(seed: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let salt = b"ai-core-chatgpt-session-v1";
    hasher.update(salt);
    let d1 = hasher.finalize();
    let mut h2 = Sha256::new();
    h2.update(d1);
    h2.update(salt);
    let d2 = h2.finalize();
    let mut k = [0u8; 32];
    k.copy_from_slice(&d2);
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let key = [42u8; 32];
        let crypto = Crypto::new(&key);
        let plaintext = b"Hello, secret AICore credentials!";

        let encrypted = crypto.encrypt(plaintext).unwrap();
        assert_ne!(encrypted, plaintext);

        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_derivation() {
        let k1 = derive_key_from_seed("machine_id_1");
        let k2 = derive_key_from_seed("machine_id_1");
        let k3 = derive_key_from_seed("machine_id_2");

        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
