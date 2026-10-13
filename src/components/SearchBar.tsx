import React from "react";
import { X, PanelRightClose, PanelRightOpen } from "lucide-react";
import { SiftBrandmark } from "./SiftLogo";

export type SearchFilter = "all" | "filename" | "content";

interface SearchBarProps {
  query: string;
  onChange: (val: string) => void;
  onClear: () => void;
  activeFilter: SearchFilter;
  onFilterChange: (filter: SearchFilter) => void;
  showPreview: boolean;
  onTogglePreview: () => void;
  placeholder?: string;
}

export const SearchBar: React.FC<SearchBarProps> = ({
  query,
  onChange,
  onClear,
  activeFilter,
  onFilterChange,
  showPreview,
  onTogglePreview,
  placeholder = "Search files, content, or meaning...",
}) => {
  return (
    <div className="search-header">
      <div className="search-icon-wrapper" style={{ paddingLeft: 2, paddingRight: 2 }}>
        <SiftBrandmark size={14} />
      </div>

      <input
        type="text"
        className="search-input-field"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoFocus
        spellCheck={false}
      />

      <div className="header-actions">
        {/* Quick Filter Chips */}
        <button
          className={`filter-chip ${activeFilter === "all" ? "active" : ""}`}
          onClick={() => onFilterChange("all")}
          title="Search all fields"
        >
          All
        </button>
        <button
          className={`filter-chip ${activeFilter === "filename" ? "active" : ""}`}
          onClick={() => onFilterChange("filename")}
          title="Search filenames only"
        >
          Filename
        </button>
        <button
          className={`filter-chip ${activeFilter === "content" ? "active" : ""}`}
          onClick={() => onFilterChange("content")}
          title="Search document contents"
        >
          Content
        </button>

        {/* Clear Button if query exists */}
        {query && (
          <button
            onClick={onClear}
            style={{
              background: "none",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              padding: 4,
            }}
            title="Clear query (Esc)"
          >
            <X size={15} />
          </button>
        )}

        {/* Toggle Expandable Preview Panel */}
        <button
          className={`preview-toggle-btn ${showPreview ? "active" : ""}`}
          onClick={onTogglePreview}
          title={showPreview ? "Hide preview panel (Tab)" : "Show preview panel (Tab)"}
        >
          {showPreview ? <PanelRightClose size={15} /> : <PanelRightOpen size={15} />}
        </button>
      </div>
    </div>
  );
};
