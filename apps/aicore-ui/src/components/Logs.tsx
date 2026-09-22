export default function Logs() {
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Logs & Diagnostics</h2>

      <div className="bg-gray-950 p-4 rounded-xl border border-gray-800 font-mono text-xs text-gray-300 h-96 overflow-y-auto space-y-1">
        <div>[INFO] Starting AICore Background Service...</div>
        <div>[INFO] AICore IPC server listening on /run/user/1000/aicore.sock</div>
        <div>[INFO] Loaded encrypted session from ~/.ai-core/session.enc.json</div>
        <div>[INFO] Client connected: fluxnotes (FluxNotes v1.0.0)</div>
        <div>[DEBUG] Sentinel chat-requirements token acquired</div>
        <div>[DEBUG] PoW solution generated for seed: test_seed_123</div>
        <div>[INFO] Request chat.stream handled in 420ms</div>
      </div>
    </div>
  );
}
