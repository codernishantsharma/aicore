use aicore_auth::{clear_session, load_session};
use aicore_protocol::{
    Request, Response, ERR_AUTH_REQUIRED, ERR_INVALID_REQUEST, ERR_PERMISSION_DENIED, PROTOCOL_VERSION,
};
use aicore_provider_chatgpt::ChatGPTProvider;
use aicore_storage::Storage;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct RequestRouter {
    storage: Arc<Storage>,
    chatgpt: Arc<ChatGPTProvider>,
}

impl RequestRouter {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            chatgpt: Arc::new(ChatGPTProvider::new()),
        }
    }

    pub async fn route_request(&self, req: Request) -> Response {
        if req.version != PROTOCOL_VERSION {
            return Response::error(
                req.id,
                ERR_INVALID_REQUEST,
                format!("Unsupported protocol version: {}", req.version),
            );
        }

        // Permission check if client identified
        if let Some(client) = &req.client {
            let conn = match self.storage.get_connection() {
                Ok(c) => c,
                Err(e) => return Response::error(req.id, "STORAGE_ERROR", e.to_string()),
            };

            if let Ok(Some(allowed)) = self.storage.get_permission(&conn, &client.id, &req.method) {
                if !allowed {
                    return Response::error(
                        req.id,
                        ERR_PERMISSION_DENIED,
                        format!("Client '{}' is denied permission for method '{}'", client.id, req.method),
                    );
                }
            }
        }

        match req.method.as_str() {
            "system.ping" => Response::result(req.id, json!({ "pong": true })),
            "core.info" => Response::result(
                req.id,
                json!({
                    "name": "AICore",
                    "version": PROTOCOL_VERSION,
                    "status": "running"
                }),
            ),
            "auth.status" => {
                let session = load_session().ok();
                Response::result(
                    req.id,
                    json!({
                        "loggedIn": session.is_some(),
                        "storedAt": session.as_ref().map(|s| s.stored_at),
                        "hasAccessToken": session.as_ref().and_then(|s| s.access_token.as_ref()).is_some()
                    }),
                )
            }
            "auth.login" => {
                Response::result(
                    req.id,
                    json!({
                        "loginUrl": "https://chatgpt.com/auth/login",
                        "instructions": "Open browser to login and complete authentication"
                    }),
                )
            }
            "auth.logout" => {
                clear_session();
                Response::result(req.id, json!({ "success": true }))
            }
            "chat.send" => {
                if load_session().is_err() {
                    return Response::error(
                        req.id,
                        ERR_AUTH_REQUIRED,
                        "AICore is not authenticated with ChatGPT",
                    );
                }

                let session_id = req
                    .params
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default_session");
                let message = req
                    .params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                let conn = match self.storage.get_connection() {
                    Ok(c) => c,
                    Err(e) => return Response::error(req.id, "STORAGE_ERROR", e.to_string()),
                };

                let existing_sess = self.storage.get_session(&conn, session_id).ok().flatten();
                let conversation_id = existing_sess.as_ref().and_then(|s| s.conversation_id.as_deref());
                let parent_message_id = existing_sess.as_ref().and_then(|s| s.parent_message_id.as_deref());

                match self
                    .chatgpt
                    .send_conversation(session_id, message, conversation_id, parent_message_id)
                    .await
                {
                    Ok(result) => {
                        let new_conv_id = result.get("conversationId").and_then(|v| v.as_str());
                        let new_msg_id = result.get("messageId").and_then(|v| v.as_str());
                        let _ = self.storage.create_session(&conn, session_id, "chatgpt");
                        let _ = self.storage.update_session(&conn, session_id, new_conv_id, new_msg_id);
                        Response::result(req.id, result)
                    }
                    Err(e) => Response::error(req.id, "PROVIDER_ERROR", e),
                }
            }
            "conversation.create" => {
                let session_id = format!("session_{}", uuid::Uuid::new_v4());
                if let Ok(conn) = self.storage.get_connection() {
                    let _ = self.storage.create_session(&conn, &session_id, "chatgpt");
                }
                Response::result(req.id, json!({ "sessionId": session_id }))
            }
            "conversation.get" => {
                let session_id = req.params.get("sessionId").and_then(|v| v.as_str()).unwrap_or_default();
                if let Ok(conn) = self.storage.get_connection() {
                    if let Ok(Some(sess)) = self.storage.get_session(&conn, session_id) {
                        return Response::result(req.id, json!(sess));
                    }
                }
                Response::result(req.id, json!({ "found": false }))
            }
            "conversation.reset" => {
                let session_id = req.params.get("sessionId").and_then(|v| v.as_str()).unwrap_or_default();
                if let Ok(conn) = self.storage.get_connection() {
                    let _ = self.storage.update_session(&conn, session_id, None, None);
                }
                Response::result(req.id, json!({ "success": true }))
            }
            "file.upload" => {
                let path_str = req.params.get("path").and_then(|v| v.as_str()).unwrap_or_default();
                match self.chatgpt.upload_file(path_str).await {
                    Ok(file_id) => Response::result(req.id, json!({ "fileId": file_id })),
                    Err(e) => Response::error(req.id, "UPLOAD_FAILED", e.to_string()),
                }
            }
            "file.download" | "image.download" => Response::result(
                req.id,
                json!({ "url": "https://chatgpt.com/backend-api/files/download/placeholder" }),
            ),
            _ => Response::error(
                req.id,
                ERR_INVALID_REQUEST,
                format!("Unknown method: {}", req.method),
            ),
        }
    }

    pub async fn route_stream(
        &self,
        req: Request,
        tx: mpsc::Sender<Response>,
    ) -> Result<(), String> {
        if req.version != PROTOCOL_VERSION {
            let err = Response::error(
                req.id.clone(),
                ERR_INVALID_REQUEST,
                format!("Unsupported protocol version: {}", req.version),
            );
            let _ = tx.send(err).await;
            return Ok(());
        }

        if req.method == "chat.stream" {
            let session_id = req
                .params
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or("default_session");
            let message = req
                .params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let conn = match self.storage.get_connection() {
                Ok(c) => c,
                Err(e) => {
                    let err = Response::error(req.id.clone(), "STORAGE_ERROR", e.to_string());
                    let _ = tx.send(err).await;
                    return Ok(());
                }
            };

            let existing_sess = self.storage.get_session(&conn, session_id).ok().flatten();
            let conversation_id = existing_sess.as_ref().and_then(|s| s.conversation_id.as_deref());
            let parent_message_id = existing_sess.as_ref().and_then(|s| s.parent_message_id.as_deref());

            let _ = self
                .chatgpt
                .stream_conversation(&req.id, session_id, message, conversation_id, parent_message_id, tx)
                .await;
        } else {
            let resp = self.route_request(req).await;
            let _ = tx.send(resp).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ping_and_info() {
        let storage = Arc::new(Storage::in_memory().unwrap());
        let router = RequestRouter::new(storage);

        let ping_req = Request::new("req_1", "system.ping", json!({}));
        let ping_res = router.route_request(ping_req).await;
        if let Response::Result { result, .. } = ping_res {
            assert_eq!(result.get("pong").and_then(|v| v.as_bool()), Some(true));
        } else {
            panic!("Expected Result for system.ping");
        }
    }
}
