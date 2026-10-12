import React from "react";
import type { IndexStatus } from "../types/contract";
import { Database, RefreshCw } from "lucide-react";

interface StatusBarProps {
  status: IndexStatus | null;
  onRefreshScan: () => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({ status, onRefreshScan }) => {
  const isBusy = status?.is_indexing || status?.is_scanning;

  return (
    <footer className="app-footer">
      <div className="status-badge">
        <span className={`status-dot ${isBusy ? "indexing" : ""}`} />
        <span>
          {isBusy
            ? status?.current_operation || "Indexing background queue..."
            : "Index synchronized"}
        </span>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Database size={13} />
          {status ? `${status.files_indexed} files ready` : "0 files"}
        </span>
        {status && status.queue_length > 0 && (
          <span style={{ color: "var(--accent-amber)" }}>
            ({status.queue_length} queued)
          </span>
        )}
        <button
          onClick={onRefreshScan}
          disabled={isBusy}
          style={{
            background: "none",
            border: "none",
            color: "var(--text-muted)",
            cursor: isBusy ? "not-allowed" : "pointer",
            display: "flex",
            alignItems: "center",
          }}
          title="Force reconciliation scan"
        >
          <RefreshCw size={13} className={isBusy ? "spin" : ""} />
        </button>
      </div>
    </footer>
  );
};
