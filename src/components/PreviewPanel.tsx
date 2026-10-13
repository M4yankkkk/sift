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
  const [pdfTab, setPdfTab] = useState<"visual" | "text">("visual");
  const [showOcrBoxes, setShowOcrBoxes] = useState(true);

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
      const hasOcr = preview.ocr_regions && preview.ocr_regions.length > 0;

      return (
        <div className="preview-image-container">
          <div className="preview-image-viewport">
            <div className="preview-image-wrapper">
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
              {showOcrBoxes &&
                imgDims &&
                hasOcr &&
                preview.ocr_regions!.map((region, idx) => {
                  const leftPct = (region.bbox.left / imgDims.width) * 100;
                  const topPct = (region.bbox.top / imgDims.height) * 100;
                  const widthPct = (region.bbox.width / imgDims.width) * 100;
                  const heightPct = (region.bbox.height / imgDims.height) * 100;

                  return (
                    <div
                      key={idx}
                      className="ocr-bounding-box"
                      style={{
                        left: `${leftPct}%`,
                        top: `${topPct}%`,
                        width: `${widthPct}%`,
                        height: `${heightPct}%`,
                      }}
                      title={`${region.text} (${Math.round(region.confidence)}% conf)`}
                    />
                  );
                })}
            </div>
          </div>

          <div className="preview-image-meta">
            <div className="image-meta-left">
              <ImageIcon size={13} color="var(--accent-amber)" />
              <span>
                {imgDims ? `${imgDims.width} × ${imgDims.height} px • ` : ""}
                {(item.size_bytes / 1024).toFixed(1)} KB • {item.extension.toUpperCase()}
              </span>
            </div>

            {hasOcr && (
              <div className="ocr-meta-toggle">
                <button
                  className={`ocr-toggle-pill ${showOcrBoxes ? "active" : ""}`}
                  onClick={() => setShowOcrBoxes((v) => !v)}
                  title="Toggle OCR match bounding boxes"
                >
                  <Sparkles size={11} />
                  <span>OCR Boxes ({preview.ocr_regions!.length})</span>
                </button>
              </div>
            )}
          </div>

          {hasOcr && preview.snippet && (
            <div className="ocr-text-card">
              <div className="ocr-text-header">
                <span className="ocr-badge-label">
                  OCR Text Match {preview.confidence ? `• ${Math.round(preview.confidence)}% Confidence` : ""}
                </span>
              </div>
              <div
                className="ocr-text-snippet"
                dangerouslySetInnerHTML={{ __html: preview.snippet }}
              />
            </div>
          )}
        </div>
      );
    }

    // 2. PDF Document Preview
    if (isPdf) {
      const hasVisual = preview.preview_content?.startsWith("data:image/");
      const textContent = preview.snippet || (!hasVisual ? preview.preview_content : "");

      return (
        <div className="pdf-preview-wrapper">
          <div className="pdf-top-controls">
            <div className="pdf-badge-header">
              <FileText size={13} />
              <span>PDF • Page {preview.page_number || item.page_number || 1} Preview</span>
            </div>

            {hasVisual && textContent && (
              <div className="pdf-toggle-group">
                <button
                  className={`pdf-toggle-btn ${pdfTab === "visual" ? "active" : ""}`}
                  onClick={() => setPdfTab("visual")}
                >
                  Visual
                </button>
                <button
                  className={`pdf-toggle-btn ${pdfTab === "text" ? "active" : ""}`}
                  onClick={() => setPdfTab("text")}
                >
                  Text
                </button>
              </div>
            )}
          </div>

          {pdfTab === "visual" && hasVisual ? (
            <div className="preview-image-viewport">
              <img
                src={preview.preview_content}
                alt={item.filename}
                className="preview-image-element"
              />
            </div>
          ) : textContent ? (
            <div className="preview-code-block">
              {textContent}
            </div>
          ) : (
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
          )}
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

      {/* Matched Snippet & Reasons Callout */}
      {(item.snippet || (item.match_reasons && item.match_reasons.length > 0)) && (
        <div className="matched-snippet-callout">
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4, fontWeight: 600 }}>
            <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
              <Sparkles size={12} color={item.match_type === "semantic" ? "#34d399" : "var(--accent-violet-light)"} />
              {item.match_type === "semantic" ? "Semantic Match Context:" : "Matched Context:"}
            </span>
            {item.match_reasons && item.match_reasons.length > 0 && (
              <span style={{ fontSize: 10, color: item.match_type === "semantic" ? "#34d399" : "var(--accent-violet-light)", fontWeight: 500 }}>
                {item.match_reasons[0]}
              </span>
            )}
          </div>
          {item.snippet && (
            <div
              style={{ fontFamily: "var(--font-mono)", fontSize: 11.5 }}
              dangerouslySetInnerHTML={{ __html: item.snippet }}
            />
          )}
        </div>
      )}

      {/* Preview Content Body */}
      <div className="preview-content-box">
        {renderContent()}
      </div>
    </div>
  );
};

