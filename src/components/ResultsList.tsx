import React from "react";
import type { SearchResultItem } from "../types/contract";
import { FileText, FileCode, FileImage, File } from "lucide-react";

interface ResultsListProps {
  results: SearchResultItem[];
  selectedId: string | null;
  onSelect: (item: SearchResultItem) => void;
  isLoading?: boolean;
}

const getFileIcon = (ext: string) => {
  switch (ext.toLowerCase()) {
    case "md":
    case "txt":
    case "pdf":
    case "doc":
      return <FileText size={16} color="var(--accent-blue)" />;
    case "rs":
    case "ts":
    case "tsx":
    case "js":
    case "json":
    case "py":
      return <FileCode size={16} color="var(--accent-emerald)" />;
    case "png":
    case "jpg":
    case "jpeg":
      return <FileImage size={16} color="var(--accent-amber)" />;
    default:
      return <File size={16} color="var(--text-muted)" />;
  }
};

export const ResultsList: React.FC<ResultsListProps> = ({
  results,
  selectedId,
  onSelect,
  isLoading = false,
}) => {
  if (isLoading) {
    return (
      <div style={{ padding: 24, textAlign: "center", color: "var(--text-muted)" }}>
        Searching indexes...
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div style={{ padding: 32, textAlign: "center", color: "var(--text-muted)" }}>
        <p style={{ fontSize: 14 }}>No matches found</p>
        <p style={{ fontSize: 12, marginTop: 4 }}>Try a different filename, phrase, or check indexed folders.</p>
      </div>
    );
  }

  return (
    <div className="results-column">
      {results.map((item) => {
        const isSelected = item.id === selectedId;
        return (
          <div
            key={item.id}
            className={`result-card ${isSelected ? "selected" : ""}`}
            onClick={() => onSelect(item)}
          >
            <div className="result-header">
              <div style={{ display: "flex", alignItems: "center", gap: 8, overflow: "hidden" }}>
                {getFileIcon(item.extension)}
                <span className="result-filename">{item.filename}</span>
              </div>
              <span
                style={{
                  fontSize: 10,
                  textTransform: "uppercase",
                  padding: "2px 6px",
                  borderRadius: 4,
                  backgroundColor: "var(--bg-tertiary)",
                  color: "var(--text-secondary)",
                }}
              >
                {item.match_type}
              </span>
            </div>
            <div className="result-path" title={item.path}>
              {item.path}
            </div>
            {item.snippet && (
              <div className="result-snippet">
                {item.snippet}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
};
