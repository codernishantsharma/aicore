export default function ConnectedApps() {
  const apps = [
    { name: "FluxNotes", id: "fluxnotes", version: "1.0.0", activeSessions: 2, status: "Connected" },
    { name: "VS Code Extension", id: "vscode-aicore", version: "0.5.2", activeSessions: 1, status: "Connected" },
    { name: "AICore CLI", id: "aicore-cli", version: "0.1.0", activeSessions: 0, status: "Idle" },
  ];

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Connected Applications</h2>

      <div className="bg-gray-800 rounded-xl border border-gray-700 overflow-hidden">
        <table className="w-full text-left text-sm text-gray-300">
          <thead className="bg-gray-900/50 text-gray-400 border-b border-gray-700">
            <tr>
              <th className="px-6 py-3 font-semibold">Application</th>
              <th className="px-6 py-3 font-semibold">Client ID</th>
              <th className="px-6 py-3 font-semibold">Version</th>
              <th className="px-6 py-3 font-semibold">Active Sessions</th>
              <th className="px-6 py-3 font-semibold">Status</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-700/50">
            {apps.map((app) => (
              <tr key={app.id} className="hover:bg-gray-750">
                <td className="px-6 py-4 font-medium text-white">{app.name}</td>
                <td className="px-6 py-4 text-gray-400 font-mono text-xs">{app.id}</td>
                <td className="px-6 py-4">{app.version}</td>
                <td className="px-6 py-4">{app.activeSessions}</td>
                <td className="px-6 py-4">
                  <span className="px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-500/10 text-green-400 border border-green-500/20">
                    {app.status}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
