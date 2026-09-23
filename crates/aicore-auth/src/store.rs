use crate::crypto::{derive_key_from_seed, Crypto};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("No session found")]
    NoSession,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SessionData {
    pub stored_at: u64,
    pub auth_session: serde_json::Value,
    pub device_id: Option<String>,
    pub access_token: Option<String>,
    pub cookies: Option<String>,
}

impl SessionData {
    pub fn get_user_info(&self) -> serde_json::Value {
        if let Some(user) = self.auth_session.get("user") {
            let id = user.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = user.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let email = user.get("email").and_then(|v| v.as_str()).unwrap_or_default();
            serde_json::json!({
                "id": id,
                "name": name,
                "email": email,
            })
        } else {
            serde_json::Value::Null
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrapped_key: Option<String>,
    payload: String,
}

pub fn data_dir() -> PathBuf {
    ProjectDirs::from("in", "nishant", "ai-core")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".ai-core")
        })
}

fn session_file() -> PathBuf {
    data_dir().join("session.enc.json")
}

fn key_file() -> PathBuf {
    data_dir().join("key.bin")
}

pub fn load_or_create_key() -> [u8; 32] {
    let kf = key_file();
    if kf.exists() {
        if let Ok(bytes) = fs::read(&kf) {
            if bytes.len() == 32 {
                let mut k = [0u8; 32];
                k.copy_from_slice(&bytes);
                return k;
            }
        }
    }
    let seed = fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_else(|_| "default-ai-core-machine-seed-id".to_string());
    let seed = seed.trim().to_string();
    let key = derive_key_from_seed(&seed);
    if let Some(parent) = kf.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&kf, &key);
    key
}

pub fn save_session(session: &SessionData) -> Result<(), AuthError> {
    let key = load_or_create_key();
    let crypto = Crypto::new(&key);
    let plain = serde_json::to_vec(session)?;
    let enc = crypto.encrypt(&plain)?;
    let sf = session_file();
    if let Some(parent) = sf.parent() {
        fs::create_dir_all(parent)?;
    }
    let stored = StoredFile {
        wrapped_key: None,
        payload: B64.encode(enc),
    };
    let json = serde_json::to_string_pretty(&stored)?;
    fs::write(&sf, json)?;
    Ok(())
}

pub fn load_session() -> Result<SessionData, AuthError> {
    let sf = session_file();
    if !sf.exists() {
        return Err(AuthError::NoSession);
    }
    let json = fs::read_to_string(&sf)?;
    let stored: StoredFile = serde_json::from_str(&json)?;
    let enc = B64.decode(stored.payload.as_bytes())
        .map_err(|e| AuthError::Crypto(crate::crypto::CryptoError::Decrypt(e.to_string())))?;
    let key = load_or_create_key();
    let crypto = Crypto::new(&key);
    let plain = crypto.decrypt(&enc)?;
    let session = serde_json::from_slice(&plain)?;
    Ok(session)
}

pub fn clear_session() {
    let sf = session_file();
    let _ = fs::remove_file(&sf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_load_clear_session() {
        let sess = SessionData {
            stored_at: 1000,
            auth_session: serde_json::json!({"user": "test"}),
            device_id: Some("did_123".to_string()),
            access_token: Some("token_456".to_string()),
            cookies: Some("oai-did=did_123".to_string()),
        };

        save_session(&sess).unwrap();
        let loaded = load_session().unwrap();
        assert_eq!(loaded, sess);

        clear_session();
        assert!(load_session().is_err());
    }
}
