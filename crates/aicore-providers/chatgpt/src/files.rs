use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FileError {
    #[error("Invalid path or path traversal detected: {0}")]
    InvalidPath(String),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Network(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFileRequest {
    pub file_name: String,
    pub file_size: u64,
    pub use_case: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFileResponse {
    pub file_id: String,
    pub upload_url: Option<String>,
}

pub struct ChatGPTFileClient;

impl ChatGPTFileClient {
    pub fn validate_file_path(path_str: &str) -> Result<PathBuf, FileError> {
        let path = Path::new(path_str);
        if path.components().any(|c| c.as_os_str() == "..") {
            return Err(FileError::InvalidPath(format!("Path traversal rejected: {}", path_str)));
        }
        Ok(path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_path_validation() {
        assert!(ChatGPTFileClient::validate_file_path("/tmp/safe.png").is_ok());
        assert!(ChatGPTFileClient::validate_file_path("../secret.txt").is_err());
        assert!(ChatGPTFileClient::validate_file_path("/home/user/../../etc/passwd").is_err());
    }
}
