use crate::files::{ChatGPTFileClient, CreateFileRequest, CreateFileResponse, FileError};
use crate::pow::{ChatRequirements, PowSolver};
use crate::sse::SseParser;
use aicore_auth::load_session;
use aicore_protocol::Response;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde_json::json;
use tokio::sync::mpsc;

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

pub struct ChatGPTProvider {
    client: reqwest::Client,
}

impl Default for ChatGPTProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatGPTProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_requirements(&self, access_token: &str) -> Result<(Option<String>, Option<String>), String> {
        let url = "https://chatgpt.com/backend-api/sentinel/chat-requirements";
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token))
                .map_err(|e| e.to_string())?,
        );
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));

        let resp = self
            .client
            .post(url)
            .headers(headers)
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch chat requirements: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Requirements endpoint returned status {}", resp.status()));
        }

        let reqs: ChatRequirements = resp
            .json()
            .await
            .map_err(|e| format!("Bad requirements JSON: {e}"))?;

        let req_token = reqs.token;
        let proof_token = if let Some(pow) = reqs.proofofwork {
            if pow.required {
                let diff = pow.difficulty.as_deref().unwrap_or("0");
                PowSolver::solve(&pow.seed, diff, DEFAULT_USER_AGENT)
            } else {
                None
            }
        } else {
            None
        };

        Ok((req_token, proof_token))
    }

    pub async fn send_conversation(
        &self,
        _session_id: &str,
        message: &str,
        conversation_id: Option<&str>,
        parent_message_id: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let session = load_session().map_err(|_| "AUTH_REQUIRED".to_string())?;
        let access_token = session
            .access_token
            .as_deref()
            .ok_or_else(|| "AUTH_REQUIRED".to_string())?;

        let (req_token, proof_token) = self.fetch_requirements(access_token).await.unwrap_or((None, None));

        let msg_id = format!("{}", uuid::Uuid::new_v4());
        let parent_id = parent_message_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}", uuid::Uuid::new_v4()));

        let payload = json!({
            "action": "next",
            "messages": [{
                "id": msg_id,
                "author": { "role": "user" },
                "content": { "content_type": "text", "parts": [message] }
            }],
            "parent_message_id": parent_id,
            "conversation_id": conversation_id,
            "model": "auto",
            "timezone_offset_min": -300,
            "history_and_training_disabled": false
        });

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token)).map_err(|e| e.to_string())?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));

        if let Some(rt) = req_token {
            if let Ok(v) = HeaderValue::from_str(&rt) {
                headers.insert("openai-sentinel-chat-requirements-token", v);
            }
        }
        if let Some(pt) = proof_token {
            if let Ok(v) = HeaderValue::from_str(&pt) {
                headers.insert("openai-sentinel-proof-token", v);
            }
        }

        let resp = self
            .client
            .post("https://chatgpt.com/backend-api/conversation")
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Conversation request failed: {e}"))?;

        if resp.status().as_u16() == 401 {
            return Err("AUTH_EXPIRED".to_string());
        }

        if !resp.status().is_success() {
            return Err(format!("Conversation endpoint returned HTTP {}", resp.status()));
        }

        Ok(json!({
            "messageId": msg_id,
            "conversationId": conversation_id.unwrap_or("conv_new"),
            "status": "success"
        }))
    }

    pub async fn stream_conversation(
        &self,
        req_id: &str,
        _session_id: &str,
        message: &str,
        conversation_id: Option<&str>,
        parent_message_id: Option<&str>,
        tx: mpsc::Sender<Response>,
    ) -> Result<(), String> {
        let session = match load_session() {
            Ok(s) => s,
            Err(_) => {
                let err = Response::error(req_id, "AUTH_REQUIRED", "Authentication required");
                let _ = tx.send(err).await;
                return Ok(());
            }
        };

        let access_token = match session.access_token.as_deref() {
            Some(t) => t,
            None => {
                let err = Response::error(req_id, "AUTH_REQUIRED", "Access token missing");
                let _ = tx.send(err).await;
                return Ok(());
            }
        };

        let (req_token, proof_token) = self.fetch_requirements(access_token).await.unwrap_or((None, None));

        let msg_id = format!("{}", uuid::Uuid::new_v4());
        let parent_id = parent_message_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}", uuid::Uuid::new_v4()));

        let payload = json!({
            "action": "next",
            "messages": [{
                "id": msg_id,
                "author": { "role": "user" },
                "content": { "content_type": "text", "parts": [message] }
            }],
            "parent_message_id": parent_id,
            "conversation_id": conversation_id,
            "model": "auto",
            "timezone_offset_min": -300
        });

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token)).map_err(|e| e.to_string())?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));

        if let Some(rt) = req_token {
            if let Ok(v) = HeaderValue::from_str(&rt) {
                headers.insert("openai-sentinel-chat-requirements-token", v);
            }
        }
        if let Some(pt) = proof_token {
            if let Ok(v) = HeaderValue::from_str(&pt) {
                headers.insert("openai-sentinel-proof-token", v);
            }
        }

        let resp = self
            .client
            .post("https://chatgpt.com/backend-api/conversation")
            .headers(headers)
            .json(&payload)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                let err = Response::error(req_id, "PROVIDER_ERROR", e.to_string());
                let _ = tx.send(err).await;
                return Ok(());
            }
        };

        if resp.status().as_u16() == 401 {
            let err = Response::error(req_id, "AUTH_EXPIRED", "ChatGPT session expired");
            let _ = tx.send(err).await;
            return Ok(());
        }

        let mut stream = resp.bytes_stream();
        let mut sse_parser = SseParser::new();
        let mut line_buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            if let Ok(chunk) = chunk_res {
                if let Ok(str_chunk) = std::str::from_utf8(&chunk) {
                    line_buffer.push_str(str_chunk);
                    while let Some(pos) = line_buffer.find('\n') {
                        let line = line_buffer[..pos].to_string();
                        line_buffer = line_buffer[pos + 1..].to_string();
                        if let Some(response) = sse_parser.parse_line(req_id, &line) {
                            let _ = tx.send(response).await;
                        }
                    }
                }
            }
        }

        let _ = tx.send(Response::done(req_id)).await;
        Ok(())
    }

    pub async fn upload_file(&self, path_str: &str) -> Result<String, FileError> {
        let path = ChatGPTFileClient::validate_file_path(path_str)?;
        let file_bytes = std::fs::read(&path)?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("uploaded_file")
            .to_string();

        let session = load_session().map_err(|_| FileError::Network("AUTH_REQUIRED".to_string()))?;
        let access_token = session
            .access_token
            .as_deref()
            .ok_or_else(|| FileError::Network("AUTH_REQUIRED".to_string()))?;

        // 1. Request upload URL
        let create_req = CreateFileRequest {
            file_name: file_name.clone(),
            file_size: file_bytes.len() as u64,
            use_case: "multimodal".to_string(),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token))
                .map_err(|e| FileError::Network(e.to_string()))?,
        );
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));

        let resp = self
            .client
            .post("https://chatgpt.com/backend-api/files")
            .headers(headers.clone())
            .json(&create_req)
            .send()
            .await
            .map_err(|e| FileError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(FileError::Network(format!(
                "File creation API returned HTTP {}",
                resp.status()
            )));
        }

        let file_res: CreateFileResponse = resp
            .json()
            .await
            .map_err(|e| FileError::Network(e.to_string()))?;

        // 2. Upload to Azure Blob if URL returned
        if let Some(upload_url) = file_res.upload_url {
            let blob_resp = self
                .client
                .put(&upload_url)
                .header("x-ms-blob-type", "BlockBlob")
                .body(file_bytes)
                .send()
                .await
                .map_err(|e| FileError::Network(e.to_string()))?;

            if !blob_resp.status().is_success() {
                return Err(FileError::Network(format!(
                    "Azure Blob upload returned HTTP {}",
                    blob_resp.status()
                )));
            }

            // 3. Complete upload
            let complete_url = format!(
                "https://chatgpt.com/backend-api/files/{}/uploaded",
                file_res.file_id
            );
            let _ = self
                .client
                .post(&complete_url)
                .headers(headers)
                .send()
                .await;
        }

        Ok(file_res.file_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chatgpt_provider_instantiation() {
        let provider = ChatGPTProvider::new();
        assert!(provider.client.get("https://chatgpt.com").build().is_ok());
    }
}
