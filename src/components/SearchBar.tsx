import React from "react";
import { X, PanelRightClose, PanelRightOpen, Layers, FileText, AlignLeft, Image, Loader2 } from "lucide-react";
import { SiftFullLogo } from "./SiftLogo";
import type { ModeCounts } from "../types/contract";

export type SearchFilter = "all" | "filename" | "content" | "visual";

interface SearchBarProps {
  query: string;
  onChange: (val: string) => void;
  onClear: () => void;
  activeFilter: SearchFilter;
  onFilterChange: (filter: SearchFilter) => void;
  modeCounts?: Partial<ModeCounts> | null;
  showPreview: boolean;
  onTogglePreview: () => void;
  isSearching?: boolean;
  placeholder?: string;
}

export const SearchBar: React.FC<SearchBarProps> = ({
  query,
  onChange,
  onClear,
  activeFilter,
  onFilterChange,
  modeCounts,
  showPreview,
  onTogglePreview,
  isSearching = false,
  placeholder = "Search files, content, or meaning...",
}) => {
  const countBadge = (filter: SearchFilter) => {
    const n = modeCounts?.[filter];
    if (n === undefined) return null;
    return <span className={`filter-chip-count ${n === 0 ? "zero" : ""}`}>{n}</span>;
  };

  return (
    <div className="search-header" data-tauri-drag-region>
      {/* Brand Badge */}
      <div
        className="search-brand-badge"
        data-tauri-drag-region
        title="Sift — Neural Desktop Search"
      >
        <SiftFullLogo height={21} />
        <div className="brand-badge-divider" />
      </div>

      {/* Main Search Input */}
      <input
        type="text"
        className="search-input-field"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoFocus
        spellCheck={false}
      />

      {/* Actions and Segmented Control */}
      <div className="header-actions">
        {/* Active Search Shimmer / Spinner */}
        {isSearching && (
          <div title="Searching index..." style={{ display: "flex", alignItems: "center", color: "var(--accent-violet-light)" }}>
            <Loader2 size={15} className="spin" />
          </div>
        )}

        {/* Quick Filter Segmented Control */}
        <div className="filter-chip-group">
          <button
            className={`filter-chip ${activeFilter === "all" ? "active" : ""}`}
            onClick={() => onFilterChange("all")}
            title="Best matches across filenames, content, and images (picked automatically per query)"
          >
            <Layers size={12} />
            <span>All</span>
            {countBadge("all")}
          </button>
          <button
            className={`filter-chip ${activeFilter === "filename" ? "active" : ""}`}
            onClick={() => onFilterChange("filename")}
            title="Search filenames only (<1ms SQLite exact match)"
          >
            <FileText size={12} />
            <span>Filename</span>
            {countBadge("filename")}
          </button>
          <button
            className={`filter-chip ${activeFilter === "content" ? "active" : ""}`}
            onClick={() => onFilterChange("content")}
            title="Search document content and OCR text"
          >
            <AlignLeft size={12} />
            <span>Content</span>
            {countBadge("content")}
          </button>
          <button
            className={`filter-chip ${activeFilter === "visual" ? "active" : ""}`}
            onClick={() => onFilterChange("visual")}
            title="Search images and photos via visual multimodal embeddings"
          >
            <Image size={12} />
            <span>Visual</span>
            {countBadge("visual")}
          </button>
        </div>

        {/* Clear Button if query exists */}
        {query && (
          <button
            onClick={onClear}
            className="clear-query-btn"
            title="Clear query (Esc)"
          >
            <X size={15} />
          </button>
        )}

        {/* Toggle Expandable Preview Panel with Keycap Hint */}
        <button
          className={`preview-toggle-btn ${showPreview ? "active" : ""}`}
          onClick={onTogglePreview}
          title={showPreview ? "Hide preview panel (Tab)" : "Show preview panel (Tab)"}
        >
          {showPreview ? <PanelRightClose size={14} /> : <PanelRightOpen size={14} />}
          <span className="preview-toggle-keycap">Tab</span>
        </button>
      </div>
    </div>
  );
};
