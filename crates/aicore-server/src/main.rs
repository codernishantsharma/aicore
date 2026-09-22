use aicore_core::RequestRouter;
use aicore_protocol::{Request, Response};
use aicore_storage::Storage;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub fn determine_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("aicore.sock")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".ai-core").join("aicore.sock")
    }
}

pub fn set_socket_permissions(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o700);
        if let Err(e) = fs::set_permissions(path, perms) {
            warn!("Failed to set 0700 permissions on socket: {}", e);
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    router: Arc<RequestRouter>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let (tx, mut rx) = mpsc::channel::<Response>(32);

    // Spawn writer task
    let writer_handle = tokio::spawn(async move {
        while let Some(response) = rx.recv().await {
            if let Ok(mut json) = serde_json::to_string(&response) {
                json.push('\n');
                if writer.write_all(json.as_bytes()).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = buf_reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break; // Connection closed by client
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err = Response::error("req_unknown", "INVALID_REQUEST", e.to_string());
                let _ = tx.send(err).await;
                continue;
            }
        };

        if req.method == "chat.stream" {
            let router_clone = router.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let _ = router_clone.route_stream(req, tx_clone).await;
            });
        } else {
            let resp = router.route_request(req).await;
            let _ = tx.send(resp).await;
        }
    }

    drop(tx);
    let _ = writer_handle.await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting AICore Background Service...");

    let socket_path = determine_socket_path();
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if socket_path.exists() {
        info!("Removing stale socket file at {}", socket_path.display());
        let _ = fs::remove_file(&socket_path);
    }

    let listener = UnixListener::bind(&socket_path)?;
    set_socket_permissions(&socket_path);
    info!("AICore IPC server listening on {}", socket_path.display());

    let db_path = socket_path
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .join("aicore.db");
    let storage = Arc::new(Storage::new(db_path)?);
    let router = Arc::new(RequestRouter::new(storage));

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let router_clone = router.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, router_clone).await {
                        error!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Socket accept error: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_determination() {
        let path = determine_socket_path();
        assert!(path.to_string_lossy().contains("aicore.sock"));
    }
}
