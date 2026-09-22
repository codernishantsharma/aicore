import { useState } from "react";
import Dashboard from "./components/Dashboard";
import Authentication from "./components/Authentication";
import ConnectedApps from "./components/ConnectedApps";
import Permissions from "./components/Permissions";
import Sessions from "./components/Sessions";
import Logs from "./components/Logs";
import Settings from "./components/Settings";

export default function App() {
  const [activeTab, setActiveTab] = useState<
    "dashboard" | "auth" | "apps" | "permissions" | "sessions" | "logs" | "settings"
  >("dashboard");

  return (
    <div className="flex h-screen bg-gray-900 text-gray-100">
      {/* Sidebar */}
      <aside className="w-64 bg-gray-950 border-r border-gray-800 flex flex-col">
        <div className="p-6 border-b border-gray-800">
          <h1 className="text-xl font-bold tracking-wider text-indigo-400">
            AICore
          </h1>
          <p className="text-xs text-gray-400 mt-1">Background AI Daemon</p>
        </div>

        <nav className="flex-1 p-4 space-y-1">
          {[
            { id: "dashboard", label: "Dashboard" },
            { id: "auth", label: "Authentication" },
            { id: "apps", label: "Connected Apps" },
            { id: "permissions", label: "Permissions" },
            { id: "sessions", label: "Sessions" },
            { id: "logs", label: "Logs & Diagnostics" },
            { id: "settings", label: "Settings" },
          ].map((item) => (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id as typeof activeTab)}
              className={`w-full text-left px-4 py-2.5 rounded-lg text-sm font-medium transition-colors ${
                activeTab === item.id
                  ? "bg-indigo-600 text-white"
                  : "text-gray-400 hover:bg-gray-800 hover:text-gray-200"
              }`}
            >
              {item.label}
            </button>
          ))}
        </nav>

        <div className="p-4 border-t border-gray-800 text-xs text-gray-500">
          AICore v0.1.0 (Daemon Active)
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto p-8 bg-gray-900">
        {activeTab === "dashboard" && <Dashboard />}
        {activeTab === "auth" && <Authentication />}
        {activeTab === "apps" && <ConnectedApps />}
        {activeTab === "permissions" && <Permissions />}
        {activeTab === "sessions" && <Sessions />}
        {activeTab === "logs" && <Logs />}
        {activeTab === "settings" && <Settings />}
      </main>
    </div>
  );
}
