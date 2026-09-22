use aicore_auth::{save_session, SessionData};
use tracing::{info, warn};

pub struct AuthRetryManager;

impl AuthRetryManager {
    pub fn is_unauthorized_status(status_code: u16) -> bool {
        status_code == 401
    }

    pub async fn refresh_session_from_endpoint(
        client: &reqwest::Client,
        cookies_header: Option<&str>,
    ) -> Result<SessionData, String> {
        info!("Attempting session refresh via /api/auth/session...");

        let mut req = client.get("https://chatgpt.com/api/auth/session");
        if let Some(cookies) = cookies_header {
            req = req.header("Cookie", cookies);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Refresh request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Refresh endpoint returned HTTP {}", resp.status()));
        }

        let session_json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Bad JSON response from session endpoint: {e}"))?;

        let access_token = session_json
            .get("accessToken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if access_token.is_none() {
            warn!("Session response did not contain valid accessToken");
            return Err("AUTH_EXPIRED".to_string());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let updated_session = SessionData {
            stored_at: now,
            auth_session: session_json,
            device_id: None,
            access_token,
            cookies: cookies_header.map(|s| s.to_string()),
        };

        let _ = save_session(&updated_session);
        Ok(updated_session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_unauthorized_status() {
        assert!(AuthRetryManager::is_unauthorized_status(401));
        assert!(!AuthRetryManager::is_unauthorized_status(200));
        assert!(!AuthRetryManager::is_unauthorized_status(403));
        assert!(!AuthRetryManager::is_unauthorized_status(500));
    }
}
