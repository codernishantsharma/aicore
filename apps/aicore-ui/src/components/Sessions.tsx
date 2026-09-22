export default function Sessions() {
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Active Sessions</h2>

      <div className="bg-gray-800 p-6 rounded-xl border border-gray-700 space-y-4">
        <div className="flex justify-between items-center pb-4 border-b border-gray-700">
          <div>
            <p className="font-mono text-sm text-indigo-400">fluxnotes-note-123</p>
            <p className="text-xs text-gray-400">ChatGPT Web Integration • Model: Auto (GPT-4o)</p>
          </div>
          <button className="px-3 py-1 bg-red-600/20 text-red-400 border border-red-500/30 rounded text-xs">
            Reset Session
          </button>
        </div>
      </div>
    </div>
  );
}
