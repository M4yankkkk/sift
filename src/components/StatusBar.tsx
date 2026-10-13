import React from "react";
import type { IndexStatus } from "../types/contract";
import { FolderCheck, Sliders, RefreshCw } from "lucide-react";
import { SiftBrandmark } from "./SiftLogo";

interface StatusBarProps {
  status: IndexStatus | null;
  onRefreshScan: () => void;
  onOpenFolders: () => void;
  onOpenSettings: () => void;
  isFoldersOpen: boolean;
  isSettingsOpen: boolean;
  showPreview: boolean;
  onTogglePreview: () => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  status,
  onRefreshScan,
  onOpenFolders,
  onOpenSettings,
  isFoldersOpen,
  isSettingsOpen,
  showPreview,
  onTogglePreview,
}) => {
  const isBusy = status?.is_indexing || status?.is_scanning;

  return (
    <footer className="action-bar">
      {/* Index Status Indicator */}
      <div className="index-status-indicator">
        <SiftBrandmark size={10} style={{ marginRight: 4, opacity: 0.85 }} />
        <span className={`indicator-dot ${isBusy ? "active" : ""}`} />
        <span>
          {isBusy
            ? status?.current_operation || "Indexing files..."
            : `${status ? status.files_indexed : 0} files indexed`}
        </span>

        <button
          onClick={onRefreshScan}
          disabled={isBusy}
          style={{
            background: "none",
            border: "none",
            color: "var(--text-muted)",
            cursor: isBusy ? "not-allowed" : "pointer",
            display: "inline-flex",
            alignItems: "center",
            padding: 2,
            marginLeft: 4,
          }}
          title="Force reconciliation scan"
        >
          <RefreshCw size={11} className={isBusy ? "spin" : ""} />
        </button>
      </div>

      {/* Action shortcuts & sheet triggers */}
      <div className="keycap-shortcuts">
        <span className="keycap-group">
          <span className="keycap">↵</span>
          <span>Open</span>
        </span>

        <button
          className={`action-icon-btn ${showPreview ? "active" : ""}`}
          onClick={onTogglePreview}
          title="Toggle preview panel"
        >
          <span className="keycap">Tab</span>
          <span>Preview</span>
        </button>

        <span className="keycap-group" title="Press Escape to close or dismiss">
          <span className="keycap">Esc</span>
          <span>Close</span>
        </span>

        <span style={{ color: "var(--border-subtle)" }}>|</span>

        {/* Folders Sheet Trigger */}
        <button
          className={`action-icon-btn ${isFoldersOpen ? "active" : ""}`}
          onClick={onOpenFolders}
          title="Manage indexed folders"
        >
          <FolderCheck size={13} />
          <span>Folders</span>
        </button>

        {/* Settings Sheet Trigger */}
        <button
          className={`action-icon-btn ${isSettingsOpen ? "active" : ""}`}
          onClick={onOpenSettings}
          title="Configure Sift settings"
        >
          <Sliders size={13} />
          <span>Settings</span>
        </button>
      </div>
    </footer>
  );
};
