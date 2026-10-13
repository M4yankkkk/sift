import React, { useState, useEffect, useCallback, useMemo } from "react";
import { SearchBar, type SearchFilter } from "./components/SearchBar";
import { ResultsList } from "./components/ResultsList";
import { PreviewPanel } from "./components/PreviewPanel";
import { StatusBar } from "./components/StatusBar";
import { SetupPage } from "./pages/SetupPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SiftAPI } from "./services/tauriClient";
import type {
  SearchResultItem,
  PreviewResponse,
  IndexStatus,
} from "./types/contract";

export const App: React.FC = () => {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<SearchFilter>("all");
  const [showPreview, setShowPreview] = useState(false);
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewResponse | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);

  // Modal sheets
  const [isFoldersOpen, setIsFoldersOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);

  // Fetch status
  const fetchStatus = useCallback(async () => {
    try {
      const s = await SiftAPI.getIndexStatus();
      setIndexStatus(s);
    } catch (err) {
      console.error("Failed to fetch status:", err);
    }
  }, []);

  const handleRefreshScan = async () => {
    try {
      const s = await SiftAPI.startScan();
      setIndexStatus(s);
    } catch (err) {
      console.error("Failed to start scan:", err);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 4000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  const currentSearchSeqRef = React.useRef(0);

  // Search logic
  const performSearch = useCallback(
    async (q: string, filter: SearchFilter) => {
      const seq = ++currentSearchSeqRef.current;
      if (!q.trim()) {
        setResults([]);
        setSelectedId(null);
        setPreview(null);
        return;
      }

      setIsSearching(true);
      try {
        const searchType = filter === "all" ? "hybrid" : filter;
        const resp = await SiftAPI.searchFiles({
          query: q,
          search_type: searchType,
        });
        if (seq !== currentSearchSeqRef.current) {
          // Stale in-flight query response dropped
          return;
        }
        setResults(resp.results);
        if (resp.results.length > 0) {
          setSelectedId(resp.results[0].id);
        } else {
          setSelectedId(null);
          setPreview(null);
        }
      } catch (err) {
        if (seq === currentSearchSeqRef.current) {
          console.error("Search failed:", err);
        }
      } finally {
        if (seq === currentSearchSeqRef.current) {
          setIsSearching(false);
        }
      }
    },
    []
  );

  // Debounced search on query or filter change
  useEffect(() => {
    const timer = setTimeout(() => {
      performSearch(query, activeFilter);
    }, 120);
    return () => clearTimeout(timer);
  }, [query, activeFilter, performSearch]);

  const selectedItem = useMemo(() => {
    return results.find((r) => r.id === selectedId) || null;
  }, [results, selectedId]);

  // Load preview when selected item changes and preview is open
  useEffect(() => {
    if (!selectedItem || !showPreview) {
      setPreview(null);
      return;
    }

    let active = true;
    setIsPreviewLoading(true);

    SiftAPI.previewResult(selectedItem.id, selectedItem.page_number)
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
  }, [selectedItem, showPreview]);

  // Keyboard navigation (ArrowUp, ArrowDown, Enter, Tab, Escape)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Escape handling
      if (e.key === "Escape") {
        if (isFoldersOpen) {
          setIsFoldersOpen(false);
          e.preventDefault();
          return;
        }
        if (isSettingsOpen) {
          setIsSettingsOpen(false);
          e.preventDefault();
          return;
        }
        if (query) {
          setQuery("");
          e.preventDefault();
          return;
        }
        e.preventDefault();
        SiftAPI.hideWindow().catch(console.error);
        return;
      }


      // Tab toggles expandable preview
      if (e.key === "Tab") {
        e.preventDefault();
        setShowPreview((prev) => !prev);
        return;
      }

      // Enter opens selected result
      if (e.key === "Enter" && selectedItem) {
        e.preventDefault();
        SiftAPI.openResult(selectedItem.path);
        return;
      }

      // Navigation through results
      if (results.length > 0) {
        const currentIndex = results.findIndex((r) => r.id === selectedId);

        if (e.key === "ArrowDown") {
          e.preventDefault();
          const nextIndex =
            currentIndex < results.length - 1 ? currentIndex + 1 : 0;
          setSelectedId(results[nextIndex].id);
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          const prevIndex =
            currentIndex > 0 ? currentIndex - 1 : results.length - 1;
          setSelectedId(results[prevIndex].id);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [results, selectedId, selectedItem, isFoldersOpen, isSettingsOpen, query]);

  return (
    <div className="floating-window">
      {/* Top Floating Search Bar */}
      <SearchBar
        query={query}
        onChange={setQuery}
        onClear={() => setQuery("")}
        activeFilter={activeFilter}
        onFilterChange={setActiveFilter}
        showPreview={showPreview}
        onTogglePreview={() => setShowPreview((prev) => !prev)}
      />

      {/* Main Content: Compact Results + Expandable Preview Panel */}
      <main className="launcher-body">
        <ResultsList
          results={results}
          selectedId={selectedId}
          onSelect={(item) => setSelectedId(item.id)}
          isLoading={isSearching}
          withPreview={showPreview}
        />

        {showPreview && (
          <PreviewPanel
            item={selectedItem}
            preview={preview}
            isLoading={isPreviewLoading}
          />
        )}
      </main>

      {/* Action Bar / Status Footer */}
      <StatusBar
        status={indexStatus}
        onRefreshScan={handleRefreshScan}
        onOpenFolders={() => setIsFoldersOpen(true)}
        onOpenSettings={() => setIsSettingsOpen(true)}
        isFoldersOpen={isFoldersOpen}
        isSettingsOpen={isSettingsOpen}
        showPreview={showPreview}
        onTogglePreview={() => setShowPreview((prev) => !prev)}
      />

      {/* Modal Sheets for Folders and Preferences */}
      {isFoldersOpen && <SetupPage onClose={() => setIsFoldersOpen(false)} />}
      {isSettingsOpen && <SettingsPage onClose={() => setIsSettingsOpen(false)} />}
    </div>
  );
};

export default App;
