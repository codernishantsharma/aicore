// AICore — Persistent ChatGPT WebView Background App
// Provides: authenticated web session management, system tray, encrypted storage, Unix socket IPC

use aicore_auth::{clear_session, load_session, save_session, SessionData};
use aicore_core::RequestRouter;
use aicore_protocol::{Request as ProtocolRequest, Response as ProtocolResponse, PROTOCOL_VERSION};
use aicore_storage::Storage;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewWindow,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{mpsc, Mutex},
};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthState {
    Starting,
    InitializingWebview,
    CheckingSession,
    Authenticated,
    Unauthenticated,
    LoginRequired,
    UserLogin,
    Ready,
    Background,
}

#[derive(Default)]
struct AppStateInternal {
    auth_state: AuthState,
    session: Option<SessionData>,
    webview: Option<WebviewWindow>,
    pending_calls: HashMap<String, tokio::sync::oneshot::Sender<String>>,
    login_notifier: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Default for AuthState {
    fn default() -> Self {
        AuthState::Starting
    }
}

#[derive(Clone, Default)]
pub struct AppState {
    inner: Arc<Mutex<AppStateInternal>>,
    router: Arc<Mutex<Option<Arc<RequestRouter>>>>,
}

fn determine_socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("aicore.sock")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".ai-core").join("aicore.sock")
    }
}

fn set_socket_permissions(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o700);
        let _ = fs::set_permissions(path, perms);
    }
}

fn is_chatgpt_host(url_str: &str) -> bool {
    let Ok(u) = Url::parse(url_str) else {
        return false;
    };
    if u.scheme() != "https" {
        return false;
    }
    let host = u.host_str().unwrap_or_default();
    host == "chatgpt.com" || host.ends_with(".chatgpt.com")
}

fn is_chatgpt_root(url_str: &str) -> bool {
    let Ok(u) = Url::parse(url_str) else {
        return false;
    };
    if u.scheme() != "https" {
        return false;
    }
    let host = u.host_str().unwrap_or_default();
    if !(host == "chatgpt.com" || host == "www.chatgpt.com") {
        return false;
    }
    let path = u.path();
    path.is_empty() || path == "/"
}

fn build_injected_js() -> String {
    r#"
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
    try {
      let r;
      switch (method) {
        case "auth_status":
          r = { loggedIn: !!window.__aiCoreSession, session: window.__aiCoreSession || null };
          break;
        case "raw_auth_session": {
          const res = await fetch("https://chatgpt.com/api/auth/session", { credentials: "include" });
          if (!res.ok) throw new Error("session fetch HTTP " + res.status);
          const data = await res.json();
          window.__aiCoreSession = data;
          let did = null;
          try {
            const cs = document.cookie.split(";").map(c => c.trim());
            for (const c of cs) if (c.startsWith("oai-did=")) did = c.slice(8);
          } catch (_) {}
          r = { auth_session: data, device_id: did };
          break;
        }
        default:
          throw new Error("unknown bridge method: " + method);
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
"#.to_string()
}

fn register_navigation_handler(win: &WebviewWindow, state: AppState) -> Result<(), String> {
    let state_clone = state.clone();

    win.on_navigation(move |url, _evt| {
        let s = url.as_str();

        if is_chatgpt_root(s) {
            let st = state_clone.clone();
            tauri::async_runtime::spawn(async move {
                let mut inner = st.inner.lock().await;
                if let Some(tx) = inner.login_notifier.take() {
                    let _ = tx.send(());
                }
            });
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
                        let decoded = B64
                            .decode(payload_b64.as_bytes())
                            .ok()
                            .and_then(|b| String::from_utf8(b).ok())
                            .unwrap_or_else(|| {
                                format!(r#"{{"ok":false,"error":"bad payload encoding"}}"#)
                            });
                        let state = state_clone.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut inner = state.inner.lock().await;
                            if let Some(tx) = inner.pending_calls.remove(&nonce) {
                                let _ = tx.send(decoded);
                            }
                        });
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
    pending: &Arc<Mutex<AppStateInternal>>,
    method: &str,
    params: &serde_json::Value,
) -> Result<String, String> {
    let nonce = format!("{}", rand::random::<u64>());
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut inner = pending.lock().await;
        inner.pending_calls.insert(nonce.clone(), tx);
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

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(_)) => Err("rpc channel closed".to_string()),
        Err(_) => {
            let mut inner = pending.lock().await;
            inner.pending_calls.remove(&nonce);
            Err("rpc timed out".to_string())
        }
    }
}

async fn capture_session_via_webview(
    win: &WebviewWindow,
    state_internal: &Arc<Mutex<AppStateInternal>>,
) -> Result<SessionData, String> {
    let js = build_injected_js();
    let _ = win.eval(&js);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let raw = webview_rpc(win, state_internal, "raw_auth_session", &serde_json::json!({})).await?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("bad session json: {e}"))?;

    if parsed.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unauthenticated session response")
            .to_string());
    }

    let result = parsed.get("result").cloned().unwrap_or_default();
    let auth_session = result
        .get("auth_session")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let access_token = auth_session
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let user_val = auth_session.get("user");

    if access_token.is_none() || user_val.is_none() || user_val.unwrap().is_null() {
        return Err("session does not contain valid user/accessToken".to_string());
    }

    let device_id = result
        .get("device_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let session_data = SessionData {
        stored_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        auth_session,
        device_id,
        access_token,
        cookies: None,
    };

    Ok(session_data)
}

pub async fn trigger_login(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    {
        let mut inner = state.inner.lock().await;
        inner.auth_state = AuthState::LoginRequired;
    }

    let win = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let _ = win.show();
    let _ = win.navigate(Url::parse("https://chatgpt.com/auth/login").unwrap());

    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut inner = state.inner.lock().await;
        inner.login_notifier = Some(tx);
        inner.auth_state = AuthState::UserLogin;
    }

    let _ = tokio::time::timeout(Duration::from_secs(600), rx).await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    {
        let mut inner = state.inner.lock().await;
        inner.auth_state = AuthState::CheckingSession;
    }

    match capture_session_via_webview(&win, &state.inner).await {
        Ok(sess) => {
            let _ = save_session(&sess);
            let mut inner = state.inner.lock().await;
            inner.session = Some(sess);
            inner.auth_state = AuthState::Ready;
            let _ = win.hide();
            Ok(())
        }
        Err(e) => {
            let mut inner = state.inner.lock().await;
            inner.auth_state = AuthState::LoginRequired;
            Err(e)
        }
    }
}

pub async fn trigger_logout(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    clear_session();

    let win = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let _ = win.show();
    let _ = win.navigate(Url::parse("https://chatgpt.com/auth/login").unwrap());

    let mut inner = state.inner.lock().await;
    inner.session = None;
    inner.auth_state = AuthState::LoginRequired;

    Ok(())
}

async fn initialize_and_check_session(app_handle: AppHandle, state: AppState) {
    {
        let mut inner = state.inner.lock().await;
        inner.auth_state = AuthState::InitializingWebview;
    }

    let win = match app_handle.get_webview_window("main") {
        Some(w) => w,
        None => return,
    };

    {
        let mut inner = state.inner.lock().await;
        inner.webview = Some(win.clone());
    }

    let _ = win.navigate(Url::parse("https://chatgpt.com/").unwrap());
    tokio::time::sleep(Duration::from_secs(3)).await;

    {
        let mut inner = state.inner.lock().await;
        inner.auth_state = AuthState::CheckingSession;
    }

    match capture_session_via_webview(&win, &state.inner).await {
        Ok(sess) => {
            let _ = save_session(&sess);
            let mut inner = state.inner.lock().await;
            inner.session = Some(sess);
            inner.auth_state = AuthState::Ready;
            let _ = win.hide();
            println!("[ai-core] ChatGPT WebView session verified and active");
        }
        Err(e) => {
            println!("[ai-core] Session unauthenticated or expired ({e}); navigating to login");
            clear_session();
            let _ = trigger_login(&app_handle, &state).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Socket dispatch & JSON-RPC handling
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SocketReq {
    #[serde(default)]
    version: Option<u32>,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

async fn dispatch_socket_request(
    app_handle: &AppHandle,
    state: &AppState,
    req: SocketReq,
) -> (serde_json::Value, Option<mpsc::Receiver<ProtocolResponse>>) {
    let req_id = req
        .id
        .as_ref()
        .and_then(|v| v.as_str())
        .unwrap_or("req_1")
        .to_string();

    match req.method.as_str() {
        "auth.status" | "auth_status" => {
            let inner = state.inner.lock().await;
            let is_authed = inner.session.is_some() && inner.auth_state == AuthState::Ready;
            let user_info = inner
                .session
                .as_ref()
                .map(|s| s.get_user_info())
                .unwrap_or(serde_json::Value::Null);

            let res = serde_json::json!({
                "version": PROTOCOL_VERSION,
                "id": req_id,
                "type": "result",
                "result": {
                    "running": true,
                    "authenticated": is_authed,
                    "user": user_info,
                    "authState": inner.auth_state
                }
            });
            (res, None)
        }
        "auth.login" | "auth_login" => {
            let handle = app_handle.clone();
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = trigger_login(&handle, &state_clone).await;
            });
            let res = serde_json::json!({
                "version": PROTOCOL_VERSION,
                "id": req_id,
                "type": "result",
                "result": { "ok": true }
            });
            (res, None)
        }
        "auth.logout" | "auth_logout" => {
            let handle = app_handle.clone();
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = trigger_logout(&handle, &state_clone).await;
            });
            let res = serde_json::json!({
                "version": PROTOCOL_VERSION,
                "id": req_id,
                "type": "result",
                "result": { "ok": true }
            });
            (res, None)
        }
        _ => {
            let router = {
                let r_guard = state.router.lock().await;
                r_guard.clone()
            };

            if let Some(router) = router {
                if req.method == "chat.stream" {
                    let (tx, rx) = mpsc::channel::<ProtocolResponse>(32);
                    let router_clone = router.clone();
                    let proto_req = ProtocolRequest::new(req_id.clone(), req.method, req.params);
                    tokio::spawn(async move {
                        let _ = router_clone.route_stream(proto_req, tx).await;
                    });
                    (serde_json::Value::Null, Some(rx))
                } else {
                    let proto_req = ProtocolRequest::new(req_id, req.method, req.params);
                    let resp = router.route_request(proto_req).await;
                    let val = serde_json::to_value(&resp).unwrap_or_default();
                    (val, None)
                }
            } else {
                let err_res = serde_json::json!({
                    "version": PROTOCOL_VERSION,
                    "id": req_id,
                    "type": "error",
                    "error": {
                        "code": "PROVIDER_UNAVAILABLE",
                        "message": "Router not initialized"
                    }
                });
                (err_res, None)
            }
        }
    }
}

async fn handle_socket_connection(app_handle: AppHandle, state: AppState, stream: UnixStream) {
    let (rx, mut tx) = tokio::io::split(stream);
    let mut reader = BufReader::new(rx);
    let mut line = String::new();

    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: SocketReq = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err = serde_json::json!({
                    "version": PROTOCOL_VERSION,
                    "id": "req_unknown",
                    "type": "error",
                    "error": { "code": "INVALID_REQUEST", "message": e.to_string() }
                });
                let mut out = serde_json::to_string(&err).unwrap_or_default();
                out.push('\n');
                let _ = tx.write_all(out.as_bytes()).await;
                continue;
            }
        };

        let (res_val, stream_rx) = dispatch_socket_request(&app_handle, &state, req).await;

        if let Some(mut rx) = stream_rx {
            while let Some(item) = rx.recv().await {
                if let Ok(mut json) = serde_json::to_string(&item) {
                    json.push('\n');
                    if tx.write_all(json.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        } else if !res_val.is_null() {
            if let Ok(mut out) = serde_json::to_string(&res_val) {
                out.push('\n');
                let _ = tx.write_all(out.as_bytes()).await;
            }
        }
    }
}

async fn run_socket_server(app_handle: AppHandle, state: AppState) {
    let path = determine_socket_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => {
            set_socket_permissions(&path);
            println!("[ai-core] Unix socket listener started at {}", path.display());
            l
        }
        Err(e) => {
            eprintln!("[ai-core] Cannot bind Unix socket at {}: {e}", path.display());
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let handle = app_handle.clone();
                let st = state.clone();
                tokio::spawn(async move {
                    handle_socket_connection(handle, st, stream).await;
                });
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// System Tray Setup
// ---------------------------------------------------------------------------

fn setup_system_tray(app: &AppHandle, state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let header = MenuItem::with_id(app, "header", "AICore Background App", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let status = MenuItem::with_id(
        app,
        "auth_status",
        "Authentication Status",
        true,
        None::<&str>,
    )?;
    let login = MenuItem::with_id(app, "login", "Login", true, None::<&str>)?;
    let logout = MenuItem::with_id(app, "logout", "Logout", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&header, &open, &status, &login, &logout, &quit])?;

    let handle = app.clone();
    let state_clone = state.clone();

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |_app, event| match event.id.as_ref() {
            "open" | "auth_status" => {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "login" => {
                let h = handle.clone();
                let st = state_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = trigger_login(&h, &st).await;
                });
            }
            "logout" => {
                let h = handle.clone();
                let st = state_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = trigger_logout(&h, &st).await;
                });
            }
            "quit" => {
                std::process::exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn auth_status_command(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let inner = state.inner.lock().await;
    let is_authed = inner.session.is_some() && inner.auth_state == AuthState::Ready;
    let user_info = inner
        .session
        .as_ref()
        .map(|s| s.get_user_info())
        .unwrap_or(serde_json::Value::Null);

    Ok(serde_json::json!({
        "running": true,
        "authenticated": is_authed,
        "authState": inner.auth_state,
        "user": user_info,
        "socketPath": determine_socket_path().to_string_lossy().to_string(),
    }))
}

// ---------------------------------------------------------------------------
// Application Entrypoint
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::default();

    // Initialize storage & router
    let socket_p = determine_socket_path();
    let db_path = socket_p
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .join("aicore.db");
    if let Ok(storage) = Storage::new(db_path) {
        let storage_arc = Arc::new(storage);
        let router = Arc::new(RequestRouter::new(storage_arc));
        let router_mutex = state.router.clone();
        tauri::async_runtime::spawn(async move {
            *router_mutex.lock().await = Some(router);
        });
    }

    let state_for_setup = state.clone();
    let state_for_socket = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![auth_status_command])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Setup system tray
            let _ = setup_system_tray(&handle, state_for_setup.clone());

            // Window close requested event -> hide to tray instead of quitting
            if let Some(win) = handle.get_webview_window("main") {
                let _ = register_navigation_handler(&win, state_for_setup.clone());
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                });
            }

            // Spawn startup check & Unix socket server
            let handle_for_check = app.handle().clone();
            let state_check = state_for_setup.clone();
            tauri::async_runtime::spawn(async move {
                initialize_and_check_session(handle_for_check, state_check).await;
            });

            let handle_for_socket = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                run_socket_server(handle_for_socket, state_for_socket).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
