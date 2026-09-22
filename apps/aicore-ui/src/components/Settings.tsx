export default function Settings() {
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Settings</h2>

      <div className="bg-gray-800 p-6 rounded-xl border border-gray-700 space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-300">Unix Domain Socket Path</label>
          <input
            type="text"
            readOnly
            value="/run/user/1000/aicore.sock"
            className="mt-1 w-full bg-gray-900 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-300 font-mono"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-300">Logging Level</label>
          <select className="mt-1 w-full bg-gray-900 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-300">
            <option>info</option>
            <option>debug</option>
            <option>warn</option>
            <option>error</option>
          </select>
        </div>
      </div>
    </div>
  );
}
