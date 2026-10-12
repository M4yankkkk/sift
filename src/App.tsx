import React, { useState, useEffect } from "react";
import { SearchPage } from "./pages/SearchPage";
import { SetupPage } from "./pages/SetupPage";
import { SettingsPage } from "./pages/SettingsPage";
import { StatusBar } from "./components/StatusBar";
import { SiftAPI } from "./services/tauriClient";
import type { IndexStatus } from "./types/contract";
import { Search, FolderCheck, Sliders, Sparkles } from "lucide-react";

type Tab = "search" | "setup" | "settings";

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<Tab>("search");
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);

  const fetchStatus = async () => {
    try {
      const s = await SiftAPI.getIndexStatus();
      setIndexStatus(s);
    } catch (err) {
      console.error("Failed to fetch index status:", err);
    }
  };

  const handleRefreshScan = async () => {
    try {
      const s = await SiftAPI.startScan();
      setIndexStatus(s);
    } catch (err) {
      console.error("Failed to start scan:", err);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 3000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="app-container">
      {/* Header */}
      <header className="app-header">
        <div className="brand-section">
          <div className="brand-badge">
            <Sparkles size={14} />
            SIFT
          </div>
          <span style={{ fontSize: 12, color: "var(--text-muted)", letterSpacing: 0.2 }}>
            Local Desktop Search
          </span>
        </div>

        {/* Navigation Tabs */}
        <nav className="nav-tabs">
          <button
            className={`nav-tab-btn ${activeTab === "search" ? "active" : ""}`}
            onClick={() => setActiveTab("search")}
          >
            <Search size={14} />
            Search
          </button>
          <button
            className={`nav-tab-btn ${activeTab === "setup" ? "active" : ""}`}
            onClick={() => setActiveTab("setup")}
          >
            <FolderCheck size={14} />
            Folders
          </button>
          <button
            className={`nav-tab-btn ${activeTab === "settings" ? "active" : ""}`}
            onClick={() => setActiveTab("settings")}
          >
            <Sliders size={14} />
            Settings
          </button>
        </nav>
      </header>

      {/* Main Content */}
      <main className="main-content">
        {activeTab === "search" && <SearchPage />}
        {activeTab === "setup" && <SetupPage />}
        {activeTab === "settings" && <SettingsPage />}
      </main>

      {/* Persistent Status Bar */}
      <StatusBar status={indexStatus} onRefreshScan={handleRefreshScan} />
    </div>
  );
};

export default App;
