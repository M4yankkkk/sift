import React, { useEffect, useRef, useState } from "react";
import type { IndexStatus, ResourceUsage } from "../types/contract";
import { FolderCheck, Sliders, RefreshCw, Cpu, MemoryStick, Sparkles, Database } from "lucide-react";
import { SiftBrandmark } from "./SiftLogo";
import { SiftAPI } from "../services/tauriClient";

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

/** "Upgrading search model: 1200 / 4968 files" → 0.24 */
const progressOf = (op: string | null | undefined): number | null => {
  const m = op?.match(/(\d[\d,]*)\s*\/\s*(\d[\d,]*)/);
  if (!m) return null;
  const done = Number(m[1].replace(/,/g, ""));
  const total = Number(m[2].replace(/,/g, ""));
  return total > 0 ? Math.min(done / total, 1) : null;
};

const formatMb = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${Math.round(mb)} MB`);

const ResourcePopover: React.FC<{ status: IndexStatus | null; onClose: () => void }> = ({ status, onClose }) => {
  const [usage, setUsage] = useState<ResourceUsage | null>(null);
  const ref = useRef<HTMLDivElement | null>(null);

  // Poll only while open; the first sample primes the CPU counter.
  useEffect(() => {
    let alive = true;
    const tick = () =>
      SiftAPI.getResourceUsage()
        .then((u) => alive && setUsage(u))
        .catch(() => {});
    tick();
    const interval = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    // Capture phase: Esc closes the popover only, instead of also clearing the query.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [onClose]);

  const busy = status?.is_indexing || status?.is_scanning;
  const uiMb = usage ? Math.max(usage.memory_mb - usage.core_memory_mb, 0) : 0;

  return (
    <div className="resource-popover" ref={ref} role="dialog" aria-label="Resource usage">
      <div className="resource-popover-title">{busy ? "Working" : "Idle"}</div>
      <div className="resource-row">
        <MemoryStick size={13} />
        <span className="resource-label">Memory</span>
        <span className="resource-value">{usage ? formatMb(usage.memory_mb) : "…"}</span>
      </div>
      {usage && (
        <div className="resource-sub">
          search core {formatMb(usage.core_memory_mb)} · interface {formatMb(uiMb)}
        </div>
      )}
      <div className="resource-row">
        <Cpu size={13} />
        <span className="resource-label">CPU</span>
        <span className="resource-value">
          {usage ? `${Math.round(usage.cpu_percent)}%` : "…"}
        </span>
      </div>
      {usage && <div className="resource-sub">of one core · {usage.cpu_cores} available</div>}
      <div className="resource-row">
        <Sparkles size={13} />
        <span className="resource-label">Model</span>
        <span className="resource-value">{usage?.model_loaded ? "loaded" : "unloaded"}</span>
      </div>
      <div className="resource-sub">
        {usage?.model_loaded ? "freed after 5 idle minutes" : "loads on the next search (~1 s)"}
      </div>
      <div className="resource-row">
        <Database size={13} />
        <span className="resource-label">Index</span>
        <span className="resource-value">{(status?.files_indexed ?? 0).toLocaleString()} files</span>
      </div>
      <div className="resource-sub">{(status?.total_embeddings ?? 0).toLocaleString()} vectors</div>
    </div>
  );
};

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
  const [showUsage, setShowUsage] = useState(false);
  const isBusy = status?.is_indexing || status?.is_scanning;
  const progress = isBusy ? progressOf(status?.current_operation) : null;

  return (
    <footer className="action-bar">
      {progress !== null && (
        <div className="action-bar-progress" style={{ transform: `scaleX(${progress})` }} />
      )}

      <div className="index-status-indicator">
        <button
          className="status-trigger"
          onClick={() => setShowUsage((v) => !v)}
          title="Memory, CPU and index details"
        >
          <SiftBrandmark size={11} style={{ opacity: 0.9 }} />
          <span className={`indicator-dot ${isBusy ? "active" : ""}`} />
          <span className="status-text">
            {isBusy
              ? status?.current_operation || "Indexing…"
              : `${(status?.files_indexed ?? 0).toLocaleString()} files indexed`}
          </span>
        </button>
        <button
          className="icon-btn icon-btn-sm"
          onClick={onRefreshScan}
          disabled={isBusy}
          title="Rescan folders for changes"
        >
          <RefreshCw size={11} className={isBusy ? "spin" : ""} />
        </button>
        {showUsage && <ResourcePopover status={status} onClose={() => setShowUsage(false)} />}
      </div>

      <div className="keycap-shortcuts">
        <span className="keycap-group">
          <span className="keycap">↵</span>
          <span>Open</span>
        </span>
        <button
          className={`action-icon-btn ${showPreview ? "active" : ""}`}
          onClick={onTogglePreview}
          title="Toggle preview (Tab)"
        >
          <span className="keycap">Tab</span>
          <span>Preview</span>
        </button>

        <span className="keycap-divider" />

        <button
          className={`action-icon-btn ${isFoldersOpen ? "active" : ""}`}
          onClick={onOpenFolders}
          title="Indexed folders (Ctrl+Shift+F)"
        >
          <FolderCheck size={13} />
          <span>Folders</span>
        </button>
        <button
          className={`action-icon-btn ${isSettingsOpen ? "active" : ""}`}
          onClick={onOpenSettings}
          title="Settings (Ctrl+,)"
        >
          <Sliders size={13} />
          <span>Settings</span>
        </button>

        {onOpenActionMenu && (
          <button
            className={`action-icon-btn action-pill-btn ${isActionMenuOpen ? "active" : ""}`}
            onClick={onOpenActionMenu}
            title="All actions (Ctrl+K)"
          >
            <span className="keycap keycap-action">Ctrl+K</span>
            <span>Actions</span>
          </button>
        )}
      </div>
    </footer>
  );
};
