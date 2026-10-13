import React, { useEffect, useRef } from "react";
import type { SearchResultItem } from "../types/contract";
import {
  FileText,
  FileCode,
  FileImage,
  File,
  SearchX,
  CornerDownLeft,
  Sparkles,
  Zap,
  Search,
  Command,
  Compass,
  FolderOpen,
} from "lucide-react";

interface ResultsListProps {
  results: SearchResultItem[];
  selectedId: string | null;
  onSelect: (item: SearchResultItem) => void;
  isLoading?: boolean;
  withPreview?: boolean;
  query?: string;
}

const getFileDetails = (ext: string) => {
  const e = ext.toLowerCase();
  switch (e) {
    case "pdf":
      return {
        icon: <FileText size={14} />,
        className: "icon-pdf",
        label: "PDF",
      };
    case "md":
    case "txt":
    case "doc":
    case "docx":
    case "rtf":
      return {
        icon: <FileText size={14} />,
        className: "icon-doc",
        label: "DOC",
      };
    case "rs":
    case "ts":
    case "tsx":
    case "js":
    case "jsx":
    case "json":
    case "py":
    case "go":
    case "c":
    case "cpp":
    case "toml":
    case "yaml":
    case "yml":
      return {
        icon: <FileCode size={14} />,
        className: "icon-code",
        label: "CODE",
      };
    case "png":
    case "jpg":
    case "jpeg":
    case "webp":
    case "svg":
    case "gif":
      return {
        icon: <FileImage size={14} />,
        className: "icon-image",
        label: "IMG",
      };
    default:
      return {
        icon: <File size={14} />,
        className: "",
        label: e.toUpperCase() || "FILE",
      };
  }
};

/** Formats a path into sleek breadcrumb segments */
const formatBreadcrumbs = (fullPath: string) => {
  const parts = fullPath.split("/").filter(Boolean);
  if (parts.length <= 1) return fullPath;
  const dirParts = parts.slice(0, -1);
  // Show at most last 3 directories
  const visible = dirParts.slice(-3);
  return visible.join(" › ");
};

export const ResultsList: React.FC<ResultsListProps> = ({
  results,
  selectedId,
  onSelect,
  isLoading = false,
  withPreview = false,
  query = "",
}) => {
  const selectedItemRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (
      selectedItemRef.current &&
      typeof selectedItemRef.current.scrollIntoView === "function"
    ) {
      selectedItemRef.current.scrollIntoView({
        block: "nearest",
        inline: "nearest",
      });
    }
  }, [selectedId, results]);

  if (isLoading) {
    return (
      <div className={`results-container ${withPreview ? "with-preview" : ""}`}>
        <div className="empty-state">
          <div className="spinner-dot" style={{ marginBottom: 14 }} />
          <div className="empty-title">Scanning indexed files...</div>
          <div className="empty-desc">
            Evaluating filename matches, BM25 text indices, and neural embeddings.
          </div>
        </div>
      </div>
    );
  }

  // Welcome state when no query is typed yet
  if (!query && results.length === 0) {
    return (
      <div className={`results-container ${withPreview ? "with-preview" : ""}`}>
        <div className="empty-state">
          <Compass size={32} className="empty-state-icon" />
          <div className="empty-title">Ready to Search</div>
          <div className="empty-desc">
            Type filenames, code identifiers, or natural descriptions to search across your system.
          </div>

          <div className="welcome-grid">
            <div className="welcome-card">
              <div className="welcome-card-header">
                <Search size={14} color="var(--accent-violet-light)" />
                <span>Instant Retrieval</span>
              </div>
              <div className="welcome-card-desc">
                Sub-millisecond filename indexing via SQLite and Tantivy BM25 full-text engine.
              </div>
            </div>

            <div className="welcome-card">
              <div className="welcome-card-header">
                <Sparkles size={14} color="var(--accent-emerald)" />
                <span>Semantic & OCR</span>
              </div>
              <div className="welcome-card-desc">
                Find images, scanned receipts, and documents by meaning with EmbeddingGemma 2.
              </div>
            </div>

            <div className="welcome-card">
              <div className="welcome-card-header">
                <Command size={14} color="var(--accent-amber)" />
                <span>Keyboard First</span>
              </div>
              <div className="welcome-card-desc">
                Navigate with <span className="keycap">↑</span> <span className="keycap">↓</span>, open with <span className="keycap">↵</span>, and preview with <span className="keycap">Tab</span>.
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // No matching results
  if (results.length === 0) {
    return (
      <div className={`results-container ${withPreview ? "with-preview" : ""}`}>
        <div className="empty-state">
          <SearchX size={34} className="empty-state-icon" />
          <div className="empty-title">No matching files found</div>
          <div className="empty-desc">
            No indexed documents matched "{query}". Try checking your approved search directories in the Folders menu (<span className="keycap">Ctrl+Shift+F</span>).
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`results-container ${withPreview ? "with-preview" : ""}`}>
      {results.map((item) => {
        const isSelected = item.id === selectedId;
        const details = getFileDetails(item.extension);
        const breadcrumb = formatBreadcrumbs(item.path);

        return (
          <div
            key={item.id}
            ref={isSelected ? selectedItemRef : null}
            data-id={item.id}
            className={`result-item ${isSelected ? "selected" : ""}`}
            onClick={() => onSelect(item)}
          >
            <div className="result-item-main">
              {/* File Icon Pill */}
              <div className={`result-icon-pill ${details.className}`}>
                {details.icon}
              </div>

              {/* Text Info */}
              <div className="result-text-stack">
                <div className="result-title-row">
                  <span className="result-title">{item.filename}</span>
                  {item.page_number && (
                    <span className="result-page-badge">Page {item.page_number}</span>
                  )}
                </div>

                {/* Breadcrumbs */}
                <div className="result-breadcrumb-row" title={item.path}>
                  <FolderOpen size={11} style={{ opacity: 0.6, flexShrink: 0 }} />
                  <span>{breadcrumb}</span>
                </div>

                {/* Match Snippet */}
                {item.snippet && (
                  <div
                    className="result-snippet-preview"
                    dangerouslySetInnerHTML={{ __html: item.snippet }}
                  />
                )}
              </div>
            </div>

            {/* Right-aligned Badges & Action Keycap */}
            <div className="result-meta-right">
              {item.match_type === "ocr" ? (
                <span className="badge-tag badge-ocr">
                  <Zap size={10} />
                  <span>OCR</span>
                </span>
              ) : item.match_type === "semantic" ? (
                <span className="badge-tag badge-semantic">
                  <Sparkles size={10} />
                  <span>Semantic</span>
                </span>
              ) : (
                <span className="badge-tag">
                  <span>{item.match_type}</span>
                </span>
              )}

              {/* Quick Action Hint */}
              <div className="result-item-action-hint">
                <span>Open</span>
                <CornerDownLeft size={11} />
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
};
