export default function Permissions() {
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Application Permissions</h2>

      <div className="bg-gray-800 p-6 rounded-xl border border-gray-700 space-y-4">
        <h3 className="text-lg font-semibold text-white">FluxNotes</h3>
        <div className="space-y-2 text-sm text-gray-300">
          <div className="flex items-center justify-between py-2 border-b border-gray-700/50">
            <span>chat.send & chat.stream</span>
            <span className="text-green-400 font-semibold">Allowed</span>
          </div>
          <div className="flex items-center justify-between py-2 border-b border-gray-700/50">
            <span>file.upload & file.download</span>
            <span className="text-green-400 font-semibold">Allowed</span>
          </div>
          <div className="flex items-center justify-between py-2 border-b border-gray-700/50">
            <span>image.download</span>
            <span className="text-green-400 font-semibold">Allowed</span>
          </div>
        </div>
      </div>
    </div>
  );
}
