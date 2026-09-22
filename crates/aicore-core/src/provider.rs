use async_trait::async_trait;
use aicore_protocol::Response;
use tokio::sync::mpsc;

#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn send(&self, session_id: &str, message: &str) -> Result<serde_json::Value, String>;
    async fn stream(
        &self,
        req_id: &str,
        session_id: &str,
        message: &str,
        tx: mpsc::Sender<Response>,
    ) -> Result<(), String>;
}
