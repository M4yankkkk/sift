import React from "react";
import type { PreviewResponse, SearchResultItem } from "../types/contract";
import { ExternalLink, FolderOpen, Info } from "lucide-react";
import { SiftAPI } from "../services/tauriClient";

interface PreviewPanelProps {
  item: SearchResultItem | null;
  preview: PreviewResponse | null;
  isLoading?: boolean;
}

export const PreviewPanel: React.FC<PreviewPanelProps> = ({
  item,
  preview,
  isLoading = false,
}) => {
  if (!item) {
    return (
      <div
        className="preview-panel"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
        }}
      >
        <div style={{ textAlign: "center", padding: 24 }}>
          <Info size={32} style={{ opacity: 0.5, marginBottom: 8 }} />
          <p>Select a result to preview contents</p>
        </div>
      </div>
    );
  }

  const handleOpen = () => {
    SiftAPI.openResult(item.path);
  };

  const handleReveal = () => {
    SiftAPI.revealResult(item.path);
  };

  return (
    <div className="preview-panel">
      <div className="preview-header">
        <div style={{ overflow: "hidden", marginRight: 12 }}>
          <h4
            style={{
              fontSize: 14,
              fontWeight: 600,
              textOverflow: "ellipsis",
              overflow: "hidden",
              whiteSpace: "nowrap",
            }}
          >
            {item.filename}
          </h4>
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {(item.size_bytes / 1024).toFixed(1)} KB • {item.extension.toUpperCase()}
          </span>
        </div>
        <div className="preview-actions">
          <button
            className="btn-secondary"
            onClick={handleReveal}
            title="Reveal in file manager"
          >
            <FolderOpen size={14} style={{ marginRight: 4, verticalAlign: "middle" }} />
            Reveal
          </button>
          <button
            className="btn-primary"
            onClick={handleOpen}
            title="Open file with default system application"
          >
            <ExternalLink size={14} style={{ marginRight: 4, verticalAlign: "middle" }} />
            Open
          </button>
        </div>
      </div>

      <div className="preview-body">
        {isLoading ? (
          <div style={{ textAlign: "center", color: "var(--text-muted)", marginTop: 24 }}>
            Loading preview...
          </div>
        ) : preview?.preview_content ? (
          preview.preview_content
        ) : (
          <div style={{ color: "var(--text-muted)" }}>
            [Binary or empty preview: use Open or Reveal to inspect file]
          </div>
        )}
      </div>
    </div>
  );
};
