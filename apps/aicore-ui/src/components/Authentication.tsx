export default function Authentication() {
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">ChatGPT Authentication</h2>

      <div className="bg-gray-800 p-6 rounded-xl border border-gray-700 space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-semibold text-white">ChatGPT Web Session</h3>
            <p className="text-sm text-gray-400">Manage browser login and access token status</p>
          </div>
          <span className="px-3 py-1 bg-green-500/10 text-green-400 border border-green-500/20 rounded-full text-xs font-semibold">
            Logged In
          </span>
        </div>

        <div className="pt-4 border-t border-gray-700 flex gap-4">
          <button
            onClick={() => window.open("https://chatgpt.com/auth/login", "_blank")}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm font-medium transition"
          >
            Re-authenticate via Browser
          </button>
          <button
            className="px-4 py-2 bg-red-600/20 hover:bg-red-600/30 text-red-400 border border-red-500/30 rounded-lg text-sm font-medium transition"
          >
            Log Out
          </button>
        </div>
      </div>
    </div>
  );
}
