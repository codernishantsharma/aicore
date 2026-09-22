export default function Dashboard() {
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">System Status</h2>
        <span className="px-3 py-1 bg-green-500/10 text-green-400 border border-green-500/20 rounded-full text-xs font-semibold">
          Service Active
        </span>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="bg-gray-800 p-6 rounded-xl border border-gray-700">
          <h3 className="text-sm font-medium text-gray-400">AICore Daemon</h3>
          <p className="text-2xl font-bold text-white mt-2">Running</p>
          <p className="text-xs text-gray-400 mt-1">Socket: /run/user/1000/aicore.sock</p>
        </div>

        <div className="bg-gray-800 p-6 rounded-xl border border-gray-700">
          <h3 className="text-sm font-medium text-gray-400">ChatGPT Web Session</h3>
          <p className="text-2xl font-bold text-green-400 mt-2">Connected</p>
          <p className="text-xs text-gray-400 mt-1">Token Active & Refreshed</p>
        </div>

        <div className="bg-gray-800 p-6 rounded-xl border border-gray-700">
          <h3 className="text-sm font-medium text-gray-400">Connected Applications</h3>
          <p className="text-2xl font-bold text-indigo-400 mt-2">3 Apps</p>
          <p className="text-xs text-gray-400 mt-1">FluxNotes, VSCode, CLI</p>
        </div>
      </div>

      <div className="bg-gray-800 p-6 rounded-xl border border-gray-700">
        <h3 className="text-lg font-bold mb-4">Recent Activity</h3>
        <div className="space-y-3 text-sm font-mono text-gray-300">
          <div className="flex justify-between border-b border-gray-700/50 pb-2">
            <span>[10:14:02] FluxNotes requested chat.stream</span>
            <span className="text-gray-500">200 OK</span>
          </div>
          <div className="flex justify-between border-b border-gray-700/50 pb-2">
            <span>[10:12:45] ChatGPT Sentinel requirements refreshed</span>
            <span className="text-gray-500">PoW Solved</span>
          </div>
          <div className="flex justify-between border-b border-gray-700/50 pb-2">
            <span>[10:10:00] Client connected: fluxnotes (v1.0.0)</span>
            <span className="text-gray-500">Authorized</span>
          </div>
        </div>
      </div>
    </div>
  );
}
