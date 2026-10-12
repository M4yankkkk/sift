import React from "react";
import type { PreviewResponse, SearchResultItem } from "../types/contract";
import { ExternalLink, FolderOpen, FileQuestion, Sparkles } from "lucide-react";
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
      <div className="preview-container">
        <div className="empty-state">
          <FileQuestion size={28} className="empty-state-icon" />
          <div className="empty-title">Select a file to preview</div>
          <div className="empty-desc">
            Use the arrow keys or click a search result to view details and code/text excerpts.
          </div>
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
    <div className="preview-container">
      {/* Top Header */}
      <div className="preview-topbar">
        <div className="preview-file-info">
          <div className="preview-title" title={item.filename}>
            {item.filename}
          </div>
          <div className="preview-chips">
            <span>{(item.size_bytes / 1024).toFixed(1)} KB</span>
            <span>•</span>
            <span style={{ textTransform: "uppercase" }}>{item.extension}</span>
            <span>•</span>
            <span>{new Date(item.modified_at).toLocaleDateString()}</span>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="preview-action-group">
          <button
            className="btn-reveal"
            onClick={handleReveal}
            title="Reveal in file manager"
          >
            <FolderOpen size={13} />
            Reveal
          </button>
          <button
            className="btn-open"
            onClick={handleOpen}
            title="Open file with default system application (Enter)"
          >
            <ExternalLink size={13} />
            Open
          </button>
        </div>
      </div>

      {/* Matched Snippet Callout (if available) */}
      {item.snippet && (
        <div className="matched-snippet-callout">
          <div style={{ display: "flex", alignItems: "center", gap: 5, marginBottom: 4, fontWeight: 600 }}>
            <Sparkles size={12} />
            Matched Context:
          </div>
          <div style={{ fontFamily: "var(--font-mono)", fontSize: 11.5 }}>
            {item.snippet}
          </div>
        </div>
      )}

      {/* Preview Content Body */}
      <div className="preview-content-box">
        {isLoading ? (
          <div style={{ color: "var(--text-muted)", textAlign: "center", padding: 24 }}>
            Loading preview...
          </div>
        ) : preview?.preview_content ? (
          preview.preview_content
        ) : (
          <div style={{ color: "var(--text-muted)", fontStyle: "italic" }}>
            Binary or non-text document. Use "Open" or "Reveal" to view in native viewer.
          </div>
        )}
      </div>
    </div>
  );
};
