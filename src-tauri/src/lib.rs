// AI Core - Reverse-engineered ChatGPT access layer
// Provides: authenticated web session management, encrypted storage, Unix socket IPC

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::Arc,
};
use tauri::{Manager, WebviewWindow};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Mutex,
};
use url::Url;

// ---------------------------------------------------------------------------
// Encryption
// ---------------------------------------------------------------------------

struct Crypto {
    cipher: Aes256Gcm,
}

impl Crypto {
    fn new(key_bytes: &[u8; 32]) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        Self { cipher: Aes256Gcm::new(key) }
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| format!("encrypt failed: {e}"))?;
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if data.len() < 12 + 16 {
            return Err("ciphertext too short".to_string());
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("decrypt failed: {e}"))
    }
}

fn derive_key_from_seed(seed: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
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

fn generate_random_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

// ---------------------------------------------------------------------------
// Session storage
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionData {
    stored_at: u64,
    auth_session: serde_json::Value,
    device_id: Option<String>,
    access_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrapped_key: Option<String>,
    payload: String,
}

fn data_dir() -> PathBuf {
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

fn load_or_create_key() -> [u8; 32] {
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
    let seed = std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_else(|_| {
            let k = generate_random_key();
            let _ = fs::create_dir_all(kf.parent().unwrap());
            let _ = fs::write(&kf, &k);
            return B64.encode(k);
        });
    let seed = seed.trim().to_string();
    let key = derive_key_from_seed(&seed);
    let _ = fs::create_dir_all(kf.parent().unwrap());
    let _ = fs::write(&kf, &key);
    key
}

fn save_session(session: &SessionData) -> Result<(), String> {
    let key = load_or_create_key();
    let crypto = Crypto::new(&key);
    let plain = serde_json::to_vec(session).map_err(|e| e.to_string())?;
    let enc = crypto.encrypt(&plain)?;
    let sf = session_file();
    let _ = fs::create_dir_all(sf.parent().unwrap());
    let stored = StoredFile { wrapped_key: None, payload: B64.encode(enc) };
    let json = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    fs::write(&sf, json).map_err(|e| e.to_string())
}

fn load_session() -> Option<SessionData> {
    let sf = session_file();
    if !sf.exists() {
        return None;
    }
    let json = fs::read_to_string(&sf).ok()?;
    let stored: StoredFile = serde_json::from_str(&json).ok()?;
    let enc = B64.decode(stored.payload.as_bytes()).ok()?;
    let key = load_or_create_key();
    let crypto = Crypto::new(&key);
    let plain = crypto.decrypt(&enc).ok()?;
    serde_json::from_slice(&plain).ok()
}

fn clear_session() {
    let sf = session_file();
    let _ = fs::remove_file(&sf);
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AppState {
    webview: Arc<Mutex<Option<WebviewWindow>>>,
    session: Arc<Mutex<Option<SessionData>>>,
    pending_calls: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
    login_notifier: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

// ---------------------------------------------------------------------------
// Auth flow
// ---------------------------------------------------------------------------

fn is_home_url(raw: &str) -> bool {
    let Ok(u) = Url::parse(raw) else {
        return false;
    };
    let host = u.host_str().unwrap_or_default();
    if !(host == "chatgpt.com" || host.ends_with(".chatgpt.com")) {
        return false;
    }
    matches!(u.path(), "" | "/")
}

fn build_injected_js() -> String {
    let engine_path = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            let c1 = cwd.join("../../../chatgpt.js");
            if c1.exists() {
                return Some(c1);
            }
            let c2 = cwd.parent()?.join("chatgpt.js");
            if c2.exists() {
                return Some(c2);
            }
            None
        });

    let engine_src = if let Some(p) = engine_path {
        fs::read_to_string(&p).unwrap_or_default()
    } else {
        String::new()
    };

    let bridge = r#"
(function () {
  if (window.__aiCoreBridgeInstalled) return;
  window.__aiCoreBridgeInstalled = true;

  window.__aiCoreSendResponse = function (nonce, responseJsonString) {
    try {
      const b64 = btoa(unescape(encodeURIComponent(responseJsonString)));
      window.location.href = "aicores://r/" + encodeURIComponent(nonce) + "?payload=" + encodeURIComponent(b64);
    } catch (e) {
      try {
        const b64 = btoa(unescape(encodeURIComponent(responseJsonString)));
        document.title = "ac:" + nonce + ":" + b64;
      } catch (_) {}
    }
  };

  async function __aiCoreCall(method, params) {
    const P = params || {};
    const E = window.__fluxnotesChatGPT;
    try {
      let r;
      switch (method) {
        case "auth_status":
          r = { loggedIn: !!E, session: window.__aiCoreSession || null };
          break;
        case "chat_send":
          r = E && await E.send(P.message, P.engine, P.attachments, P.sessionId);
          break;
        case "chat_new_conversation":
          r = E && E.newConversation(P.sessionId);
          r = { ok: true };
          break;
        case "chat_upload_file":
          r = E && await E.uploadFileToChatGPT(P.fileBase64, P.filename, P.mimeType);
          r = { fileId: r };
          break;
        case "chat_get_session":
          r = E && E.getSession(P.sessionId);
          break;
        case "chat_set_session":
          r = E && E.setSession(P.sessionId, P.session);
          r = { ok: true };
          break;
        case "chat_download_sandbox_image":
          r = E && await E.downloadSandboxImage(P.imagePath, P.messageId, P.sessionId);
          break;
        case "raw_auth_session": {
          const res = await fetch("/api/auth/session", { credentials: "include" });
          if (!res.ok) throw new Error("session fetch HTTP " + res.status);
          const data = await res.json();
          window.__aiCoreSession = data;
          let did = null;
          try {
            const cs = document.cookie.split(";").map(c => c.trim());
            for (const c of cs) if (c.startsWith("oai-did=")) did = c.slice(7);
          } catch (_) {}
          r = { auth_session: data, device_id: did };
          break;
        }
        default:
          throw new Error("unknown method: " + method);
      }
      return JSON.stringify({ ok: true, result: r === undefined ? null : r });
    } catch (err) {
      return JSON.stringify({ ok: false, error: String(err && err.message || err) });
    }
  }
  window.__aiCoreCall = __aiCoreCall;

  window.__aiCoreRpc = async function (nonce, method, paramsJson) {
    try {
      const params = paramsJson ? JSON.parse(paramsJson) : {};
      const result = await __aiCoreCall(method, params);
      window.__aiCoreSendResponse(nonce, result);
    } catch (e) {
      const msg = JSON.stringify({ ok: false, error: String(e && e.message || e) });
      window.__aiCoreSendResponse(nonce, msg);
    }
  };
})();
"#;

    format!("{engine_src}\n{bridge}")
}

fn register_navigation_handler(win: &WebviewWindow, state: AppState) -> Result<(), String> {
    let pending = state.pending_calls.clone();
    let login_tx = state.login_notifier.clone();

    win.on_navigation(move |url, _evt| {
        let s = url.as_str();

        if is_home_url(s) {
            if let Ok(mut g) = login_tx.try_lock() {
                if let Some(tx) = g.take() {
                    let _ = tx.send(());
                }
            }
        }

        if s.starts_with("aicores://") {
            let u = Url::parse(s).ok();
            if let Some(u) = u {
                if u.host_str() == Some("r") {
                    let nonce = u.path().trim_start_matches('/').to_string();
                    let payload_b64 = u
                        .query_pairs()
                        .find(|(k, _)| k == "payload")
                        .map(|(_, v)| v.to_string())
                        .unwrap_or_default();
                    if !nonce.is_empty() {
                        use base64::Engine as _;
                        let decoded = B64
                            .decode(payload_b64.as_bytes())
                            .ok()
                            .and_then(|b| String::from_utf8(b).ok())
                            .unwrap_or_else(|| {
                                format!(r#"{{"ok":false,"error":"bad payload encoding"}}"#)
                            });
                        if let Ok(mut p) = pending.try_lock() {
                            if let Some(tx) = p.remove(&nonce) {
                                let _ = tx.send(decoded);
                            }
                        }
                    }
                }
            }
            return Err("[ai-core] intercepted aicores:// response".to_string());
        }

        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn webview_rpc(
    win: &WebviewWindow,
    pending: &Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
    method: &str,
    params: &serde_json::Value,
) -> Result<String, String> {
    let nonce = format!("{}", rand::random::<u64>());
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        pending.lock().await.insert(nonce.clone(), tx);
    }
    let params_json = params.to_string();
    let expr = format!(
        r#"(async () => {{
            if (!window.__aiCoreBridgeInstalled) {{ try {{ {engine} }} catch(_) {{}} }}
            const rpc = window.__aiCoreRpc;
            if (!rpc) {{ throw new Error("bridge missing"); }}
            await rpc({nonce:?}, {method:?}, {params_json:?});
        }})();"#,
        engine = build_injected_js(),
        nonce = nonce,
        method = method,
        params_json = params_json,
    );
    win.eval(&expr).map_err(|e| format!("eval failed: {e}"))?;

    match tokio::time::timeout(std::time::Duration::from_secs(600), rx).await {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(_)) => Err("rpc channel closed".to_string()),
        Err(_) => {
            let mut p = pending.lock().await;
            p.remove(&nonce);
            Err("rpc timed out".to_string())
        }
    }
}

async fn capture_session_via_webview(
    win: &WebviewWindow,
    pending: &Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
) -> Result<SessionData, String> {
    let js = build_injected_js();
    let _ = win.eval(&js);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let raw = webview_rpc(win, pending, "raw_auth_session", &serde_json::json!({})).await?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("bad session json: {e}"))?;

    if parsed.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown session error")
            .to_string());
    }
    let result = parsed.get("result").cloned().unwrap_or_default();
    let auth_session = result
        .get("auth_session")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let device_id = result
        .get("device_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let access_token = auth_session
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if access_token.is_none() {
        return Err("session did not contain accessToken; not logged in?".to_string());
    }

    Ok(SessionData {
        stored_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        auth_session,
        device_id,
        access_token,
    })
}

async fn run_auth_flow(app_handle: tauri::AppHandle, state: &AppState) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        *state.login_notifier.lock().await = Some(tx);
    }

    let win = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let _ = win.show();
    let _ = win.navigate(Url::parse("https://chatgpt.com/auth/login").unwrap());

    tokio::time::timeout(std::time::Duration::from_secs(600), rx)
        .await
        .map_err(|_| "login timed out".to_string())?
        .map_err(|_| "login channel closed".to_string())?;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let sess = capture_session_via_webview(&win, &state.pending_calls).await?;
    save_session(&sess)?;

    *state.session.lock().await = Some(sess);
    let _ = win.hide();
    *state.webview.lock().await = Some(win);
    Ok(())
}

async fn try_restore_without_ui(
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<bool, String> {
    if load_session().is_none() {
        return Ok(false);
    }

    let win = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let _ = win.navigate(Url::parse("https://chatgpt.com/").unwrap());
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    match capture_session_via_webview(&win, &state.pending_calls).await {
        Ok(new_sess) => {
            save_session(&new_sess)?;
            *state.session.lock().await = Some(new_sess);
            *state.webview.lock().await = Some(win.clone());
            let _ = win.hide();
            Ok(true)
        }
        Err(_) => {
            clear_session();
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// Unix socket JSON-RPC server
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SocketRequest {
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct SocketResponse<'a> {
    id: Option<&'a serde_json::Value>,
    #[serde(flatten)]
    body: SocketResponseBody,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum SocketResponseBody {
    Result { result: serde_json::Value },
    Error { error: SocketError },
}

#[derive(Debug, Serialize)]
struct SocketError {
    code: i32,
    message: String,
}

fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("ai-core.sock")
    } else {
        let d = data_dir();
        let _ = fs::create_dir_all(&d);
        d.join("ai-core.sock")
    }
}

async fn dispatch_call(
    state: &AppState,
    method: &str,
    params: Option<serde_json::Map<String, serde_json::Value>>,
) -> SocketResponseBody {
    if method == "auth_status" {
        let sess = state.session.lock().await;
        let has_webview = state.webview.lock().await.is_some();
        let info = serde_json::json!({
            "loggedIn": sess.is_some(),
            "hasWebview": has_webview,
            "storedAt": sess.as_ref().map(|s| s.stored_at),
            "hasAccessToken": sess.as_ref().and_then(|s| s.access_token.as_ref()).is_some(),
            "deviceId": sess.as_ref().and_then(|s| s.device_id.clone()),
            "socketPath": socket_path().to_string_lossy().to_string(),
            "sessionFile": session_file().to_string_lossy().to_string(),
        });
        return SocketResponseBody::Result { result: info };
    }
    if method == "auth_session_raw" {
        let sess = state.session.lock().await;
        let value = sess.as_ref().map(|s| s.auth_session.clone());
        return SocketResponseBody::Result {
            result: serde_json::json!(value),
        };
    }
    if method == "auth_logout" {
        clear_session();
        *state.session.lock().await = None;
        *state.webview.lock().await = None;
        return SocketResponseBody::Result {
            result: serde_json::json!({ "ok": true }),
        };
    }

    let win = {
        let g = state.webview.lock().await;
        match g.as_ref() {
            Some(w) => w.clone(),
            None => {
                return SocketResponseBody::Error {
                    error: SocketError {
                        code: -32002,
                        message: "not logged in; no webview available".to_string(),
                    },
                };
            }
        }
    };

    let params_value = params
        .map(serde_json::Value::Object)
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let raw = match webview_rpc(&win, &state.pending_calls, method, &params_value).await {
        Ok(s) => s,
        Err(e) => {
            return SocketResponseBody::Error {
                error: SocketError { code: -32000, message: e },
            };
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return SocketResponseBody::Error {
                error: SocketError {
                    code: -32000,
                    message: format!("bad response json: {e}"),
                },
            };
        }
    };

    if parsed.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        SocketResponseBody::Result {
            result: parsed.get("result").cloned().unwrap_or(serde_json::Value::Null),
        }
    } else {
        let msg = parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error")
            .to_string();
        SocketResponseBody::Error {
            error: SocketError { code: -1, message: msg },
        }
    }
}

async fn handle_socket_conn(state: AppState, stream: UnixStream) {
    let (rx, mut tx) = tokio::io::split(stream);
    let mut reader = BufReader::new(rx);

    loop {
        let mut line = String::new();
        let n = match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln!("socket read error: {e}");
                break;
            }
        };
        if n == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let send_err = |code: i32,
                        msg: String,
                        id: Option<&serde_json::Value>|
         -> Option<String> {
            let resp = SocketResponse {
                id,
                body: SocketResponseBody::Error {
                    error: SocketError { code, message: msg },
                },
            };
            serde_json::to_string(&resp)
                .map(|mut s| {
                    s.push('\n');
                    s
                })
                .ok()
        };

        let reqs: Vec<SocketRequest> = if line.starts_with('[') {
            match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    if let Some(buf) = send_err(-32700, e.to_string(), None) {
                        let _ = tx.write_all(buf.as_bytes()).await;
                    }
                    continue;
                }
            }
        } else {
            match serde_json::from_str::<SocketRequest>(line) {
                Ok(r) => vec![r],
                Err(e) => {
                    if let Some(buf) = send_err(-32700, e.to_string(), None) {
                        let _ = tx.write_all(buf.as_bytes()).await;
                    }
                    continue;
                }
            }
        };

        let mut out: Vec<serde_json::Value> = Vec::with_capacity(reqs.len());
        for req in &reqs {
            let body = dispatch_call(&state, &req.method, req.params.clone()).await;
            let resp = SocketResponse {
                id: req.id.as_ref(),
                body,
            };
            match serde_json::to_value(&resp) {
                Ok(v) => out.push(v),
                Err(e) => {
                    let err = SocketResponse {
                        id: req.id.as_ref(),
                        body: SocketResponseBody::Error {
                            error: SocketError {
                                code: -32603,
                                message: e.to_string(),
                            },
                        },
                    };
                    if let Ok(v) = serde_json::to_value(&err) {
                        out.push(v);
                    }
                }
            }
        }

        let buf = if line.starts_with('[') {
            serde_json::to_string(&out).map(|mut s| {
                s.push('\n');
                s
            })
        } else {
            out.into_iter()
                .next()
                .ok_or_else(|| "empty".to_string())
                .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
                .map(|mut s| {
                    s.push('\n');
                    s
                })
        };
        match buf {
            Ok(buf) => {
                if tx.write_all(buf.as_bytes()).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("socket serialize error: {e}");
                break;
            }
        }
    }
}

async fn socket_server_loop(state: AppState) {
    let path = socket_path();
    let _ = fs::remove_file(&path);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => {
            println!("[ai-core] socket listening on {}", path.display());
            l
        }
        Err(e) => {
            eprintln!("[ai-core] FATAL: cannot bind socket {}: {e}", path.display());
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state = state.clone();
                tokio::spawn(async move {
                    handle_socket_conn(state, stream).await;
                });
            }
            Err(e) => {
                eprintln!("[ai-core] socket accept error: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands (minimal — primary API is the socket)
// ---------------------------------------------------------------------------

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn auth_status_command(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let s = &*state;
    let sess = s.session.lock().await;
    let has_webview = s.webview.lock().await.is_some();
    Ok(serde_json::json!({
        "loggedIn": sess.is_some(),
        "hasWebview": has_webview,
        "storedAt": sess.as_ref().map(|x| x.stored_at),
        "socketPath": socket_path().to_string_lossy().to_string(),
        "sessionFile": session_file().to_string_lossy().to_string(),
    }))
}

#[tauri::command]
fn socket_path_command() -> String {
    socket_path().to_string_lossy().to_string()
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = fs::create_dir_all(data_dir());

    let state = AppState::default();
    let state_for_setup = state.clone();
    let state_for_socket = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            greet,
            auth_status_command,
            socket_path_command
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            {
                let win = handle
                    .get_webview_window("main")
                    .expect("main window must exist");
                register_navigation_handler(&win, state_for_setup.clone())
                    .expect("failed to register navigation handler");
            }

            tauri::async_runtime::spawn(async move {
                let restored = try_restore_without_ui(&handle, &state_for_setup)
                    .await
                    .unwrap_or(false);

                if !restored {
                    println!("[ai-core] no valid session; starting login flow");
                    if let Err(e) = run_auth_flow(handle.clone(), &state_for_setup).await {
                        eprintln!("[ai-core] auth flow failed: {e}");
                    } else {
                        println!("[ai-core] login successful");
                    }
                } else {
                    println!("[ai-core] session restored without UI");
                }

                socket_server_loop(state_for_socket).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
