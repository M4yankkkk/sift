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
  onOpenActionMenu?: () => void;
  isActionMenuOpen?: boolean;
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
  onOpenActionMenu,
  isActionMenuOpen,
}) => {
  const isBusy = status?.is_indexing || status?.is_scanning;

  return (
    <footer className="action-bar">
      {/* Index Status & Smart Incremental Details */}
      <div
        className="index-status-indicator"
        title="Smart Incremental Indexing: Sift fingerprints file mtime + size in <0.5s on launch. Unchanged files are never re-embedded!"
      >
        <SiftBrandmark size={11} style={{ marginRight: 2, opacity: 0.9 }} />
        <span className={`indicator-dot ${isBusy ? "active" : ""}`} />
        <span>
          {isBusy
            ? status?.current_operation ||
              `Indexing files... ${status?.total_embeddings ? `(${status.total_embeddings.toLocaleString()} vectors)` : ""}`
            : `${(status?.files_indexed ?? 0).toLocaleString()} files • ${(status?.total_embeddings ?? 0).toLocaleString()} embeddings`}
        </span>

        {/* Force reconciliation scan button */}
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
            padding: "2px 4px",
            marginLeft: 4,
            borderRadius: "var(--radius-xs)",
          }}
          title="Force incremental reconciliation scan (<0.5s check)"
        >
          <RefreshCw size={11} className={isBusy ? "spin" : ""} />
        </button>
      </div>

      {/* Action shortcuts & sheet triggers */}
      <div className="keycap-shortcuts">
        <span className="keycap-group" title="Open selected file">
          <span className="keycap">↵</span>
          <span>Open</span>
        </span>

        <button
          className={`action-icon-btn ${showPreview ? "active" : ""}`}
          onClick={onTogglePreview}
          title="Toggle preview panel (Tab)"
        >
          <span className="keycap">Tab</span>
          <span>Preview</span>
        </button>

        <span className="keycap-group" title="Reveal in system folder (Ctrl+R)">
          <span className="keycap">Ctrl+R</span>
          <span>Reveal</span>
        </span>

        <span className="keycap-group" title="Dismiss or clear search (Esc)">
          <span className="keycap">Esc</span>
          <span>Close</span>
        </span>

        <span style={{ color: "var(--border-subtle)" }}>|</span>

        {/* Folders Sheet Trigger */}
        <button
          className={`action-icon-btn ${isFoldersOpen ? "active" : ""}`}
          onClick={onOpenFolders}
          title="Manage approved search directories (Ctrl+Shift+F)"
        >
          <FolderCheck size={13} color="var(--accent-violet-light)" />
          <span>Folders</span>
        </button>

        {/* Settings Sheet Trigger */}
        <button
          className={`action-icon-btn ${isSettingsOpen ? "active" : ""}`}
          onClick={onOpenSettings}
          title="Configure Sift settings & shortcuts (Ctrl+,)"
        >
          <Sliders size={13} color="var(--accent-violet-light)" />
          <span>Settings</span>
        </button>

        {onOpenActionMenu && (
          <button
            className={`action-icon-btn action-pill-btn ${isActionMenuOpen ? "active" : ""}`}
            onClick={onOpenActionMenu}
            title="Quick Actions Palette (Ctrl+K)"
          >
            <span className="keycap keycap-action">Ctrl+K</span>
            <span>Actions</span>
          </button>
        )}
      </div>
    </footer>
  );
};
