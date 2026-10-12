import React, { useEffect, useRef } from "react";
import type { SearchResultItem } from "../types/contract";
import { FileText, FileCode, FileImage, File, SearchX, CornerDownLeft } from "lucide-react";

interface ResultsListProps {
  results: SearchResultItem[];
  selectedId: string | null;
  onSelect: (item: SearchResultItem) => void;
  isLoading?: boolean;
  withPreview?: boolean;
}

const getFileIcon = (ext: string) => {
  switch (ext.toLowerCase()) {
    case "md":
    case "txt":
    case "pdf":
    case "doc":
      return <FileText size={14} color="#a78bfa" />;
    case "rs":
    case "ts":
    case "tsx":
    case "js":
    case "json":
    case "py":
      return <FileCode size={14} color="#06b6d4" />;
    case "png":
    case "jpg":
    case "jpeg":
      return <FileImage size={14} color="#f59e0b" />;
    default:
      return <File size={14} color="#9ca3af" />;
  }
};

export const ResultsList: React.FC<ResultsListProps> = ({
  results,
  selectedId,
  onSelect,
  isLoading = false,
  withPreview = false,
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
          <div className="empty-title">Scanning indexed files...</div>
        </div>
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className={`results-container ${withPreview ? "with-preview" : ""}`}>
        <div className="empty-state">
          <SearchX size={32} className="empty-state-icon" />
          <div className="empty-title">No matching files found</div>
          <div className="empty-desc">
            Type a query or check your indexed folder paths in the Folders menu.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`results-container ${withPreview ? "with-preview" : ""}`}>
      {results.map((item) => {
        const isSelected = item.id === selectedId;
        return (
          <div
            key={item.id}
            ref={isSelected ? selectedItemRef : null}
            data-id={item.id}
            className={`result-item ${isSelected ? "selected" : ""}`}
            onClick={() => onSelect(item)}
          >
            <div className="result-item-main">
              <div className="result-icon-pill">
                {getFileIcon(item.extension)}
              </div>
              <div className="result-text-stack">
                <div className="result-title-row">
                  <span className="result-title">{item.filename}</span>
                  {item.page_number && (
                    <span className="result-page-badge">Page {item.page_number}</span>
                  )}
                </div>
                <span className="result-subtitle" title={item.path}>
                  {item.path}
                </span>
                {item.snippet && (
                  <div
                    className="result-snippet-preview"
                    dangerouslySetInnerHTML={{ __html: item.snippet }}
                  />
                )}
              </div>
            </div>

            <div className="result-meta-right">
              <span
                className={`badge-tag ${
                  item.match_type === "ocr"
                    ? "badge-ocr"
                    : item.match_type === "semantic"
                    ? "badge-semantic"
                    : ""
                }`}
              >
                {item.match_type}
              </span>
              {isSelected && (
                <span style={{ color: "var(--accent-violet-light)", display: "flex", alignItems: "center" }}>
                  <CornerDownLeft size={12} />
                </span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};
