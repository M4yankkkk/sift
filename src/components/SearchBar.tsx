import React from "react";
import { Search, X } from "lucide-react";

interface SearchBarProps {
  query: string;
  onChange: (val: string) => void;
  onClear: () => void;
  placeholder?: string;
}

export const SearchBar: React.FC<SearchBarProps> = ({
  query,
  onChange,
  onClear,
  placeholder = "Search files by name, text, or meaning...",
}) => {
  return (
    <div className="search-bar-container">
      <Search size={18} color="var(--accent-blue)" />
      <input
        type="text"
        className="search-input"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoFocus
        spellCheck={false}
      />
      {query ? (
        <button
          onClick={onClear}
          style={{
            background: "none",
            border: "none",
            cursor: "pointer",
            color: "var(--text-muted)",
            display: "flex",
            alignItems: "center",
          }}
          aria-label="Clear query"
        >
          <X size={16} />
        </button>
      ) : (
        <span className="shortcut-pill">Alt + Space</span>
      )}
    </div>
  );
};
