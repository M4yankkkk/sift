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
  Copy,
  Check,
  Zap,
  Terminal,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { SiftAPI } from "../services/tauriClient";
import { SyntaxHighlighter } from "./SyntaxHighlighter";

interface PreviewPanelProps {
  item: SearchResultItem | null;
  preview: PreviewResponse | null;
  isLoading?: boolean;
  onPageChange?: (page: number) => void;
}

export const PreviewPanel: React.FC<PreviewPanelProps> = ({
  item,
  preview,
  isLoading = false,
  onPageChange,
}) => {
  const [imgDims, setImgDims] = useState<{ width: number; height: number } | null>(null);
  const [pdfTab, setPdfTab] = useState<"visual" | "text">("visual");
  const [showOcrBoxes, setShowOcrBoxes] = useState(true);
  const [copiedPath, setCopiedPath] = useState(false);
  const [copiedContent, setCopiedContent] = useState(false);

  if (!item) {
    return (
      <div className="preview-container">
        <div className="empty-state">
          <FileQuestion size={32} className="empty-state-icon" />
          <div className="empty-title">Select a file to preview</div>
          <div className="empty-desc">
            Use the arrow keys or click a search result to view details, images, code excerpts, or OCR regions.
          </div>
        </div>
      </div>
    );
  }

  const handleOpen = () => {
    const targetPage = preview?.page_number || item.page_number;
    SiftAPI.openResult(item.path, targetPage);
  };

  const handleReveal = () => {
    SiftAPI.revealResult(item.path);
  };

  const handleTerminal = () => {
    SiftAPI.openInTerminal(item.path);
  };

  const handleCopyPath = () => {
    navigator.clipboard.writeText(item.path);
    setCopiedPath(true);
    setTimeout(() => setCopiedPath(false), 2000);
  };

  const handleCopyContent = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedContent(true);
    setTimeout(() => setCopiedContent(false), 2000);
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
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11.5, color: "var(--text-secondary)" }}>
              <ImageIcon size={13} color="var(--accent-amber)" />
              <span>
                {imgDims ? `${imgDims.width} × ${imgDims.height} px • ` : ""}
                {(item.size_bytes / 1024).toFixed(1)} KB • {item.extension.toUpperCase()}
              </span>
            </div>

            {hasOcr && (
              <button
                className={`ocr-toggle-pill ${showOcrBoxes ? "active" : ""}`}
                onClick={() => setShowOcrBoxes((v) => !v)}
                title="Toggle OCR match bounding boxes"
              >
                <Zap size={11} />
                <span>OCR Boxes ({preview.ocr_regions!.length})</span>
              </button>
            )}
          </div>

          {hasOcr && preview.snippet && (
            <div className="ocr-text-card">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: 10, fontWeight: 600, color: "var(--accent-amber)", textTransform: "uppercase", letterSpacing: "0.5px" }}>
                  OCR Recognized Text {preview.confidence ? `• ${Math.round(preview.confidence)}% Confidence` : ""}
                </span>
                <button
                  className="btn-copy"
                  style={{ padding: "2px 6px", fontSize: 10 }}
                  onClick={() => handleCopyContent(preview.snippet || "")}
                >
                  {copiedContent ? <Check size={10} color="var(--accent-emerald)" /> : <Copy size={10} />}
                  <span>{copiedContent ? "Copied" : "Copy"}</span>
                </button>
              </div>
              <div className="ocr-text-snippet">{preview.snippet}</div>
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
              {onPageChange && (
                <div style={{ display: "inline-flex", alignItems: "center", gap: 3, marginLeft: 6 }}>
                  <button
                    className="pdf-toggle-btn"
                    style={{ padding: "2px 5px", display: "inline-flex", alignItems: "center" }}
                    disabled={(preview.page_number || item.page_number || 1) <= 1 || isLoading}
                    onClick={() => onPageChange((preview.page_number || item.page_number || 1) - 1)}
                    title="Previous Page"
                  >
                    <ChevronLeft size={11} />
                  </button>
                  <button
                    className="pdf-toggle-btn"
                    style={{ padding: "2px 5px", display: "inline-flex", alignItems: "center" }}
                    disabled={isLoading}
                    onClick={() => onPageChange((preview.page_number || item.page_number || 1) + 1)}
                    title="Next Page"
                  >
                    <ChevronRight size={11} />
                  </button>
                </div>
              )}
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

    // 4. Text / Code / Markdown / Config
    const lines = preview.preview_content.split("\n");
    const isCode =
      preview.mime_type === "text/code" ||
      ["rs", "ts", "tsx", "js", "jsx", "py", "json", "toml", "yaml", "yml", "go", "cpp", "c", "h", "hpp", "sh", "bash", "sql", "html", "css", "md", "txt", "log"].includes(ext);

    return (
      <div className="preview-text-wrapper">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", display: "flex", alignItems: "center", gap: 6 }}>
            <span>{lines.length} lines</span>
            <span>•</span>
            <span style={{ textTransform: "uppercase" }}>{item.extension || "plain"}</span>
          </div>
          <button
            className={`btn-copy ${copiedContent ? "copied" : ""}`}
            onClick={() => handleCopyContent(preview.preview_content)}
            style={{ padding: "3px 8px", fontSize: 11 }}
            title="Copy file contents (Ctrl+Shift+C)"
          >
            {copiedContent ? <Check size={11} /> : <Copy size={11} />}
            <span>{copiedContent ? "Copied" : "Copy Content"}</span>
          </button>
        </div>

        {isCode ? (
          <SyntaxHighlighter code={preview.preview_content} extension={item.extension} />
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
            className={`btn-copy ${copiedPath ? "copied" : ""}`}
            onClick={handleCopyPath}
            title="Copy absolute file path (Ctrl+C)"
          >
            {copiedPath ? <Check size={13} /> : <Copy size={13} />}
            <span>{copiedPath ? "Copied" : "Path"}</span>
          </button>
          <button
            className="btn-reveal"
            onClick={handleReveal}
            title="Reveal in file manager (Ctrl+R)"
          >
            <FolderOpen size={13} />
            <span>Reveal</span>
          </button>
          <button
            className="btn-reveal"
            onClick={handleTerminal}
            title="Open folder in terminal (Ctrl+T)"
          >
            <Terminal size={13} />
            <span>Terminal</span>
          </button>
          <button
            className="btn-open"
            onClick={handleOpen}
            title={
              isPdf && (preview?.page_number || item.page_number)
                ? `Open PDF at Page ${preview?.page_number || item.page_number} (Enter)`
                : "Open file with default system application (Enter)"
            }
          >
            <ExternalLink size={13} />
            <span>
              {isPdf && (preview?.page_number || item.page_number)
                ? `Open Page ${preview?.page_number || item.page_number}`
                : "Open"}
            </span>
          </button>
        </div>
      </div>

      {/* Matched Snippet & Context Callout */}
      {(item.snippet || (item.match_reasons && item.match_reasons.length > 0)) && (
        <div className="matched-snippet-callout">
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4, fontWeight: 600 }}>
            <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
              <Sparkles size={12} color={item.match_type === "semantic" ? "var(--accent-emerald)" : "var(--accent-violet-light)"} />
              <span>{item.match_type === "semantic" ? "Neural Relevance Match:" : "Matched Context:"}</span>
            </span>
            {item.match_reasons && item.match_reasons.length > 0 && (
              <span style={{ fontSize: 10, color: item.match_type === "semantic" ? "var(--accent-emerald)" : "var(--accent-violet-light)", fontWeight: 500 }}>
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
