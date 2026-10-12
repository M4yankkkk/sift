import React, { useState } from "react";
import type { PreviewResponse, SearchResultItem } from "../types/contract";
import {
  ExternalLink,
  FolderOpen,
  FileQuestion,
  Sparkles,
  FileText,
  Image as ImageIcon,
  Binary,
} from "lucide-react";
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
  const [imgDims, setImgDims] = useState<{ width: number; height: number } | null>(null);

  if (!item) {
    return (
      <div className="preview-container">
        <div className="empty-state">
          <FileQuestion size={28} className="empty-state-icon" />
          <div className="empty-title">Select a file to preview</div>
          <div className="empty-desc">
            Use the arrow keys or click a search result to view details, images, or document excerpts.
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

  const ext = item.extension.toLowerCase();
  const isImage =
    preview?.mime_type?.startsWith("image/") ||
    preview?.preview_content?.startsWith("data:image/") ||
    ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico"].includes(ext);

  const isPdf = ext === "pdf" || preview?.mime_type === "application/pdf";
  const isBinaryContent =
    preview?.preview_content === "[Binary content or unsupported encoding]";

  const renderContent = () => {
    if (isLoading) {
      return (
        <div className="preview-loading-state">
          <div className="spinner-dot" />
          <span>Loading preview...</span>
        </div>
      );
    }

    if (!preview) {
      return (
        <div className="binary-card">
          <div className="binary-card-icon">
            <Binary size={22} />
          </div>
          <div className="binary-card-title">Preview Unavailable</div>
          <div className="binary-card-desc">
            Could not read preview data for this file.
          </div>
          <button className="btn-open" onClick={handleOpen} style={{ marginTop: 8 }}>
            <ExternalLink size={13} />
            Open File (Enter)
          </button>
        </div>
      );
    }

    // 1. Image Preview
    if (isImage && preview.preview_content?.startsWith("data:image/")) {
      return (
        <div className="preview-image-container">
          <div className="preview-image-viewport">
            <img
              src={preview.preview_content}
              alt={item.filename}
              className="preview-image-element"
              onLoad={(e) => {
                setImgDims({
                  width: e.currentTarget.naturalWidth,
                  height: e.currentTarget.naturalHeight,
                });
              }}
            />
          </div>
          <div className="preview-image-meta">
            <ImageIcon size={13} color="var(--accent-amber)" />
            <span>
              {imgDims ? `${imgDims.width} × ${imgDims.height} px • ` : ""}
              {(item.size_bytes / 1024).toFixed(1)} KB • {item.extension.toUpperCase()}
            </span>
          </div>
        </div>
      );
    }

    // 2. PDF Document Preview
    if (isPdf) {
      const isScannedOrEmpty =
        preview.preview_content.includes("Scanned or graphic document") ||
        isBinaryContent ||
        !preview.preview_content.trim();

      if (isScannedOrEmpty) {
        return (
          <div className="binary-card">
            <div className="binary-card-icon" style={{ color: "var(--accent-rose)", borderColor: "rgba(244, 63, 94, 0.3)", background: "rgba(244, 63, 94, 0.1)" }}>
              <FileText size={24} />
            </div>
            <div className="binary-card-title">PDF Document</div>
            <div className="binary-card-desc">
              Visual or scanned PDF without an embedded text layer. Open in your system document viewer to read.
            </div>
            <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
              <button className="btn-open" onClick={handleOpen}>
                <ExternalLink size={13} />
                Open PDF (Enter)
              </button>
              <button className="btn-reveal" onClick={handleReveal}>
                <FolderOpen size={13} />
                Reveal
              </button>
            </div>
          </div>
        );
      }

      return (
        <div className="pdf-preview-wrapper">
          <div className="pdf-badge-header">
            <FileText size={13} />
            <span>PDF Document • Text Excerpt (Pages 1–5)</span>
          </div>
          <div className="preview-code-block">
            {preview.preview_content}
          </div>
        </div>
      );
    }

    // 3. Binary / Non-text File
    if (isBinaryContent) {
      return (
        <div className="binary-card">
          <div className="binary-card-icon">
            <Binary size={22} />
          </div>
          <div className="binary-card-title">
            Binary File ({item.extension.toUpperCase() || "RAW"})
          </div>
          <div className="binary-card-desc">
            Binary stream without plain-text encoding. Open in its native application to view.
          </div>
          <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
            <button className="btn-open" onClick={handleOpen}>
              <ExternalLink size={13} />
              Open File (Enter)
            </button>
            <button className="btn-reveal" onClick={handleReveal}>
              <FolderOpen size={13} />
              Reveal
            </button>
          </div>
        </div>
      );
    }

    // 4. Text / Code / Markdown
    const lines = preview.preview_content.split("\n");
    const isCode = preview.mime_type === "text/code" || ["rs", "ts", "tsx", "js", "py", "json", "toml", "yaml"].includes(ext);

    return (
      <div className="preview-text-wrapper">
        {isCode ? (
          <div className="code-preview-layout">
            <div className="code-gutter">
              {lines.map((_, i) => (
                <div key={i} className="gutter-line-number">
                  {i + 1}
                </div>
              ))}
            </div>
            <div className="code-body">
              {preview.preview_content}
            </div>
          </div>
        ) : (
          <div className="preview-code-block">
            {preview.preview_content}
          </div>
        )}
      </div>
    );
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
        {renderContent()}
      </div>
    </div>
  );
};

