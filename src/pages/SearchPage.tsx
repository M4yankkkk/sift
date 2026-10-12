import React, { useState, useEffect, useCallback } from "react";
import { SearchBar } from "../components/SearchBar";
import { ResultsList } from "../components/ResultsList";
import { PreviewPanel } from "../components/PreviewPanel";
import { SiftAPI } from "../services/tauriClient";
import type {
  SearchResultItem,
  PreviewResponse,
} from "../types/contract";

export const SearchPage: React.FC = () => {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [selectedItem, setSelectedItem] = useState<SearchResultItem | null>(null);
  const [preview, setPreview] = useState<PreviewResponse | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);

  const performSearch = useCallback(async (q: string) => {
    if (!q.trim()) {
      setResults([]);
      setSelectedItem(null);
      setPreview(null);
      return;
    }

    setIsSearching(true);
    try {
      const resp = await SiftAPI.searchFiles({
        query: q,
        search_type: "filename",
      });
      setResults(resp.results);
      if (resp.results.length > 0) {
        setSelectedItem(resp.results[0]);
      } else {
        setSelectedItem(null);
        setPreview(null);
      }
    } catch (err) {
      console.error("Search failed:", err);
    } finally {
      setIsSearching(false);
    }
  }, []);

  // Debounced search
  useEffect(() => {
    const timer = setTimeout(() => {
      performSearch(query);
    }, 150);
    return () => clearTimeout(timer);
  }, [query, performSearch]);

  // Load preview when selected item changes
  useEffect(() => {
    if (!selectedItem) {
      setPreview(null);
      return;
    }

    let active = true;
    setIsPreviewLoading(true);

    SiftAPI.previewResult(selectedItem.id)
      .then((p) => {
        if (active) setPreview(p);
      })
      .catch((err) => {
        console.error("Preview failed:", err);
        if (active) setPreview(null);
      })
      .finally(() => {
        if (active) setIsPreviewLoading(false);
      });

    return () => {
      active = false;
    };
  }, [selectedItem]);

  return (
    <div className="search-view">
      <SearchBar
        query={query}
        onChange={setQuery}
        onClear={() => setQuery("")}
        activeFilter="all"
        onFilterChange={() => {}}
        showPreview={true}
        onTogglePreview={() => {}}
      />
      <div className="results-split-view">
        <ResultsList
          results={results}
          selectedId={selectedItem?.id || null}
          onSelect={setSelectedItem}
          isLoading={isSearching}
        />
        <PreviewPanel
          item={selectedItem}
          preview={preview}
          isLoading={isPreviewLoading}
        />
      </div>
    </div>
  );
};
