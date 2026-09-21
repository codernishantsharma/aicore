import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface AuthStatus {
  loggedIn: boolean;
  hasWebview: boolean;
  storedAt?: number;
  socketPath: string;
  sessionFile: string;
}

function formatAge(ms?: number): string {
  if (!ms) return "n/a";
  const diff = Date.now() - ms;
  const m = Math.floor(diff / 60000);
  if (m < 1) return "just now";
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${m % 60}m ago`;
  const d = Math.floor(h / 24);
  return `${d}d ${h % 24}h ago`;
}

function App() {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [socketCmd, setSocketCmd] = useState(
    'echo \'{"id":1,"method":"auth_status"}\' | nc -U /tmp/ai-core-test.sock'
  );

  const refresh = async () => {
    try {
      const s = await invoke<AuthStatus>("auth_status_command");
      setStatus(s);
      setSocketCmd(
        `echo '{"id":1,"method":"auth_status"}' | nc -U "${s.socketPath}"`
      );
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 2000);
    return () => clearInterval(id);
  }, []);

  return (
    <main className="container" style={{ textAlign: "left", maxWidth: 720 }}>
      <h1>AI Core</h1>
      <p className="tagline">Reverse-engineered ChatGPT access service</p>

      {status === null ? (
        <p>Loading…</p>
      ) : (
        <>
          <section className="card">
            <h2>Auth</h2>
            <div className="row">
              <span className={`badge ${status.loggedIn ? "ok" : "err"}`}>
                {status.loggedIn ? "LOGGED IN" : "NOT LOGGED IN"}
              </span>
              <span className={`badge ${status.hasWebview ? "ok" : "dim"}`}>
                {status.hasWebview ? "Webview alive" : "No webview"}
              </span>
            </div>
            <ul className="kv">
              <li>
                <span>Session captured</span>
                <code>{formatAge(status.storedAt)}</code>
              </li>
              <li>
                <span>Session file</span>
                <code>{status.sessionFile}</code>
              </li>
              <li>
                <span>Socket</span>
                <code>{status.socketPath}</code>
              </li>
            </ul>
            <button onClick={refresh}>Refresh status</button>
          </section>

          <section className="card">
            <h2>Socket API</h2>
            <p className="muted">
              External apps (e.g. the VSCode extension) communicate via JSON-RPC
              over the Unix socket. Example:
            </p>
            <pre>
              <code>{socketCmd}</code>
            </pre>
            <p className="muted">
              Available methods: <code>auth_status</code>, <code>auth_session_raw</code>,
              <code>auth_logout</code>, <code>chat_send</code>, <code>chat_new_conversation</code>,
              <code>chat_upload_file</code>, <code>chat_get_session</code>,
              <code>chat_set_session</code>, <code>chat_download_sandbox_image</code>.
            </p>
          </section>

          <section className="card">
            <h2>Troubleshooting</h2>
            <p className="muted">
              If login is needed, the app should have automatically opened the
              ChatGPT login page. If not, close and re-launch. If the session
              becomes stale, invoke <code>auth_logout</code> via the socket and
              restart.
            </p>
          </section>
        </>
      )}
    </main>
  );
}

export default App;

