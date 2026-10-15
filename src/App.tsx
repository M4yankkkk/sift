import React, { useState, useEffect, useCallback, useMemo } from "react";
import { SearchBar, type SearchFilter } from "./components/SearchBar";
import { ResultsList } from "./components/ResultsList";
import { PreviewPanel } from "./components/PreviewPanel";
import { StatusBar } from "./components/StatusBar";
import { ActionMenu } from "./components/ActionMenu";
import { SetupPage } from "./pages/SetupPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SiftAPI } from "./services/tauriClient";
import type {
  SearchResultItem,
  PreviewResponse,
  IndexStatus,
  ModeCounts,
} from "./types/contract";

export const App: React.FC = () => {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<SearchFilter>("all");
  const [showPreview, setShowPreview] = useState(true);
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [modeCounts, setModeCounts] = useState<Partial<ModeCounts> | null>(null);
  const [searchMs, setSearchMs] = useState<number | null>(null);
  const [gridColumns, setGridColumns] = useState(1);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewResponse | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);

  // Modal sheets & action menu
  const [isFoldersOpen, setIsFoldersOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isActionMenuOpen, setIsActionMenuOpen] = useState(false);
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

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event")
      .then(({ listen }) => {
        listen("open-settings", () => {
          setIsSettingsOpen(true);
        }).then((u) => {
          unlisten = u;
        });
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const currentSearchSeqRef = React.useRef(0);

  // Search logic
  const performSearch = useCallback(
    async (q: string, filter: SearchFilter) => {
      const seq = ++currentSearchSeqRef.current;
      if (!q.trim()) {
        setResults([]);
        setModeCounts(null);
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
        setSearchMs(resp.execution_time_ms);
        setModeCounts(resp.mode_counts ?? { filename: resp.results.length });
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

  const handlePageChange = async (newPage: number) => {
    if (!selectedItem || newPage < 1) return;
    setIsPreviewLoading(true);
    try {
      const p = await SiftAPI.previewResult(selectedItem.id, newPage);
      setPreview(p);
    } catch (err) {
      console.error("Failed to load page preview:", err);
    } finally {
      setIsPreviewLoading(false);
    }
  };

  // Keyboard navigation (ArrowUp, ArrowDown, Enter, Tab, Escape)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Fast window dismissal via Ctrl+W / Ctrl+Q
      if ((e.ctrlKey && e.key.toLowerCase() === "w") || (e.ctrlKey && e.key.toLowerCase() === "q")) {
        e.preventDefault();
        SiftAPI.hideWindow().catch(console.error);
        return;
      }

      // Settings / Folders modal sheet toggle shortcuts (Ctrl+, and Ctrl+Shift+F)
      if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        setIsSettingsOpen((prev) => !prev);
        return;
      }

      if (e.ctrlKey && (e.key === "F" || (e.shiftKey && e.key.toLowerCase() === "f"))) {
        e.preventDefault();
        setIsFoldersOpen((prev) => !prev);
        return;
      }

      // Focus search input on Ctrl+L
      if (e.ctrlKey && e.key.toLowerCase() === "l") {
        e.preventDefault();
        const input = document.querySelector<HTMLInputElement>(".search-input-field");
        if (input) {
          input.focus();
          input.select();
        }
        return;
      }

      // Quick Action Menu toggle via Ctrl+K
      if (e.ctrlKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setIsActionMenuOpen((prev) => !prev);
        return;
      }

      // Escape handling: closes sheets/action menu -> clears query -> hides window
      if (e.key === "Escape") {
        if (isActionMenuOpen) {
          setIsActionMenuOpen(false);
          e.preventDefault();
          return;
        }
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
        if (query && !e.shiftKey) {
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

      // Enter or Ctrl+O opens selected result
      if ((e.key === "Enter" || (e.ctrlKey && e.key.toLowerCase() === "o")) && selectedItem) {
        e.preventDefault();
        const targetPage = preview?.page_number || selectedItem.page_number;
        SiftAPI.openResult(selectedItem.path, targetPage);
        return;
      }

      // Ctrl+R reveals selected file in folder
      if (e.ctrlKey && e.key.toLowerCase() === "r" && selectedItem) {
        e.preventDefault();
        SiftAPI.revealResult(selectedItem.path);
        return;
      }

      // Ctrl+T opens in terminal
      if (e.ctrlKey && e.key.toLowerCase() === "t" && selectedItem) {
        e.preventDefault();
        SiftAPI.openInTerminal(selectedItem.path);
        return;
      }

      // Navigation through results (Arrows or Ctrl+N / Ctrl+P). In the Visual grid, ↑/↓ move by
      // row and ←/→ by tile — but only while the caret sits at the end of the query, so the arrow
      // keys still edit text normally.
      if (results.length > 0) {
        const currentIndex = results.findIndex((r) => r.id === selectedId);
        const isGrid = activeFilter === "visual";
        const rowStep = isGrid ? gridColumns : 1;
        const input = document.activeElement as HTMLInputElement | null;
        const caretAtEnd =
          !input ||
          input.tagName !== "INPUT" ||
          (input.selectionStart === input.value.length && input.selectionEnd === input.value.length);
        const move = (delta: number, wrap: boolean) => {
          e.preventDefault();
          let next = currentIndex + delta;
          if (wrap) {
            next = (next + results.length) % results.length;
          } else {
            next = Math.min(Math.max(next, 0), results.length - 1);
          }
          setSelectedId(results[next].id);
        };

        if (e.key === "ArrowDown") {
          move(rowStep, !isGrid);
        } else if (e.key === "ArrowUp") {
          move(-rowStep, !isGrid);
        } else if (e.ctrlKey && e.key.toLowerCase() === "n") {
          move(1, true);
        } else if (e.ctrlKey && e.key.toLowerCase() === "p") {
          move(-1, true);
        } else if (isGrid && caretAtEnd && !e.shiftKey && !e.ctrlKey && e.key === "ArrowRight") {
          move(1, false);
        } else if (isGrid && caretAtEnd && !e.shiftKey && !e.ctrlKey && e.key === "ArrowLeft") {
          move(-1, false);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [results, selectedId, selectedItem, isFoldersOpen, isSettingsOpen, query, activeFilter, gridColumns]);

  return (
    <div className="floating-window">
      {/* Top Floating Search Bar */}
      <SearchBar
        query={query}
        onChange={setQuery}
        onClear={() => setQuery("")}
        activeFilter={activeFilter}
        onFilterChange={setActiveFilter}
        modeCounts={modeCounts}
        showPreview={showPreview}
        onTogglePreview={() => setShowPreview((prev) => !prev)}
        isSearching={isSearching}
      />

      {/* Main Content: Compact Results + Expandable Preview Panel */}
      <main className="launcher-body">
        <ResultsList
          results={results}
          selectedId={selectedId}
          onSelect={(item) => setSelectedId(item.id)}
          isLoading={isSearching && results.length === 0}
          withPreview={showPreview}
          query={query}
          executionMs={searchMs}
          grid={activeFilter === "visual"}
          onGridColumnsChange={setGridColumns}
          onExampleQuery={setQuery}
        />

        {showPreview && (
          <PreviewPanel
            item={selectedItem}
            preview={preview}
            isLoading={isPreviewLoading}
            onPageChange={handlePageChange}
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
        onOpenActionMenu={() => setIsActionMenuOpen((prev) => !prev)}
        isActionMenuOpen={isActionMenuOpen}
      />

      {/* Quick Action Palette Modal (Ctrl+K) */}
      <ActionMenu
        isOpen={isActionMenuOpen}
        onClose={() => setIsActionMenuOpen(false)}
        item={selectedItem}
        previewContent={preview?.preview_content || preview?.snippet}
        showPreview={showPreview}
        onTogglePreview={() => setShowPreview((prev) => !prev)}
        onOpenFolders={() => setIsFoldersOpen(true)}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      {/* Modal Sheets for Folders and Preferences */}
      {isFoldersOpen && <SetupPage onClose={() => setIsFoldersOpen(false)} />}
      {isSettingsOpen && (
        <SettingsPage
          onClose={() => setIsSettingsOpen(false)}
          indexStatus={indexStatus}
          onRefreshStatus={fetchStatus}
        />
      )}
    </div>
  );
};

export default App;
