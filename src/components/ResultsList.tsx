import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ModelStatus, SearchResultItem } from "../types/contract";
import {
  FileText,
  FileCode,
  FileImage,
  File,
  SearchX,
  CornerDownLeft,
  Sparkles,
  ScanText,
  Image as ImageIcon,
  TextSearch,
  Type,
  Command,
  Compass,
} from "lucide-react";
import { SiftAPI } from "../services/tauriClient";

interface ResultsListProps {
  results: SearchResultItem[];
  selectedId: string | null;
  onSelect: (item: SearchResultItem) => void;
  isLoading?: boolean;
  withPreview?: boolean;
  query?: string;
  /** Backend search time for the current results. */
  executionMs?: number | null;
  /** Show results as a thumbnail grid (Visual mode). */
  grid?: boolean;
  /** Reports how many grid columns fit, so arrow keys can move by row. */
  onGridColumnsChange?: (columns: number) => void;
  onExampleQuery?: (query: string) => void;
  modelStatus?: ModelStatus | null;
  onRetryModel?: () => void;
}

/** First-run card: what works now, and how far the search model download is. */
const ModelSetupCard: React.FC<{ status: ModelStatus; onRetry?: () => void }> = ({ status, onRetry }) => {
  if (status.state === "ready" || status.state === "disabled") return null;
  if (status.state === "failed") {
    return (
      <div className="setup-card setup-card-failed">
        <div className="setup-card-title">Meaning search couldn't be set up</div>
        <div className="setup-card-desc">{status.message}</div>
        <div className="setup-card-desc">Filename, keyword, and text-in-image search still work.</div>
        {onRetry && (
          <button className="setup-card-btn" onClick={onRetry}>
            Retry download
          </button>
        )}
      </div>
    );
  }
  const pct =
    status.state === "downloading" && status.total_bytes > 0
      ? Math.round((status.downloaded_bytes / status.total_bytes) * 100)
      : null;
  const mb = (b: number) => Math.round(b / 1048576);
  return (
    <div className="setup-card">
      <div className="setup-card-title">
        {status.state === "loading" ? "Loading the search model…" : "Setting up meaning search"}
        {pct !== null && <span className="setup-card-pct">{pct}%</span>}
      </div>
      <div className="setup-card-track">
        <div
          className={`setup-card-fill ${pct === null ? "indeterminate" : ""}`}
          style={pct === null ? undefined : { transform: `scaleX(${pct / 100})` }}
        />
      </div>
      <div className="setup-card-desc">
        {status.state === "downloading"
          ? `One-time download of EmbeddingGemma 2 · ${mb(status.downloaded_bytes)} / ${mb(status.total_bytes)} MB`
          : "One-time setup of EmbeddingGemma 2"}
        . Filename, keyword, and text-in-image search already work.
      </div>
    </div>
  );
};

const modelNotReady = (m: ModelStatus | null | undefined) =>
  !!m && m.state !== "ready" && m.state !== "disabled";

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp", "gif", "bmp"]);
const isImageExt = (ext: string) => IMAGE_EXTENSIONS.has(ext.toLowerCase());

const EXAMPLE_QUERIES = [
  "sunset photo",
  "error screenshot",
  "resume pdf",
  "notes about databases",
  "type:pdf invoice",
];

const getFileDetails = (ext: string) => {
  const e = ext.toLowerCase();
  if (e === "pdf") return { icon: <FileText size={14} />, className: "icon-pdf" };
  if (["md", "txt", "doc", "docx", "rtf"].includes(e)) {
    return { icon: <FileText size={14} />, className: "icon-doc" };
  }
  if (["rs", "ts", "tsx", "js", "jsx", "json", "py", "go", "c", "cpp", "toml", "yaml", "yml", "sql", "sh"].includes(e)) {
    return { icon: <FileCode size={14} />, className: "icon-code" };
  }
  if (isImageExt(e) || e === "svg") return { icon: <FileImage size={14} />, className: "icon-image" };
  return { icon: <File size={14} />, className: "" };
};

/** Why this result matched, in plain words: the strongest evidence wins. */
export const matchLabel = (item: SearchResultItem) => {
  const visual = item.match_reasons.some((r) => r.startsWith("Visual"));
  if (item.match_type === "ocr") return { label: "Text in image", className: "badge-ocr", icon: <ScanText size={10} /> };
  if (item.match_type === "semantic" && visual) return { label: "Visual", className: "badge-visual", icon: <ImageIcon size={10} /> };
  if (item.match_type === "semantic") return { label: "Meaning", className: "badge-meaning", icon: <Sparkles size={10} /> };
  if (item.match_type === "keyword") return { label: "Keyword", className: "badge-keyword", icon: <TextSearch size={10} /> };
  return { label: "Name", className: "badge-name", icon: <Type size={10} /> };
};

/** "/home/me/Downloads/a/b/file.pdf" → "~/Downloads/a/b" (last three folders at most). */
export const displayDir = (fullPath: string) => {
  const home = fullPath.match(/^\/home\/[^/]+/)?.[0];
  const rel = home ? "~" + fullPath.slice(home.length) : fullPath;
  const parts = rel.split("/");
  parts.pop();
  if (parts.length <= 4) return parts.join("/") || "/";
  return [parts[0], "…", ...parts.slice(-3)].join("/");
};

// Thumbnails are small data URLs fetched lazily per image result and kept for the session.
const thumbnailCache = new Map<string, string | null>();
const thumbnailRequests = new Map<string, Promise<string | null>>();

const useThumbnail = (id: string, enabled: boolean) => {
  const [thumb, setThumb] = useState<string | null>(() => thumbnailCache.get(id) ?? null);
  useEffect(() => {
    if (!enabled) return;
    if (thumbnailCache.has(id)) {
      setThumb(thumbnailCache.get(id) ?? null);
      return;
    }
    let cancelled = false;
    let request = thumbnailRequests.get(id);
    if (!request) {
      request = SiftAPI.getThumbnail(id).catch(() => null);
      thumbnailRequests.set(id, request);
    }
    request.then((url) => {
      thumbnailCache.set(id, url);
      thumbnailRequests.delete(id);
      if (!cancelled) setThumb(url);
    });
    return () => {
      cancelled = true;
    };
  }, [id, enabled]);
  return thumb;
};

const ResultIcon: React.FC<{ item: SearchResultItem }> = ({ item }) => {
  const image = isImageExt(item.extension);
  const thumb = useThumbnail(item.id, image);
  const details = getFileDetails(item.extension);
  return (
    <div className="result-leading">
      {image && thumb ? (
        <img className="result-thumb" src={thumb} alt="" draggable={false} />
      ) : (
        <div className={`result-icon-pill ${details.className}`}>{details.icon}</div>
      )}
    </div>
  );
};

const GridTile: React.FC<{
  item: SearchResultItem;
  selected: boolean;
  onSelect: () => void;
  innerRef?: React.Ref<HTMLDivElement>;
}> = ({ item, selected, onSelect, innerRef }) => {
  const thumb = useThumbnail(item.id, isImageExt(item.extension));
  const match = matchLabel(item);
  return (
    <div
      ref={innerRef}
      className={`grid-tile ${selected ? "selected" : ""}`}
      onClick={onSelect}
      onDoubleClick={() => SiftAPI.openResult(item.path, item.page_number)}
      title={item.path}
    >
      <div className="grid-tile-image">
        {thumb ? (
          <img src={thumb} alt="" draggable={false} />
        ) : (
          <div className="grid-tile-placeholder">{getFileDetails(item.extension).icon}</div>
        )}
        <span className={`grid-tile-badge ${match.className}`}>
          {match.icon}
          <span>{match.label}</span>
        </span>
      </div>
      <div className="grid-tile-name">{item.filename}</div>
    </div>
  );
};

const TILE_MIN_WIDTH = 132;
const GRID_GAP = 10;

export const ResultsList: React.FC<ResultsListProps> = ({
  results,
  selectedId,
  onSelect,
  isLoading = false,
  withPreview = false,
  query = "",
  executionMs = null,
  grid = false,
  onGridColumnsChange,
  onExampleQuery,
  modelStatus,
  onRetryModel,
}) => {
  const selectedItemRef = useRef<HTMLDivElement | null>(null);
  const gridRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (selectedItemRef.current && typeof selectedItemRef.current.scrollIntoView === "function") {
      selectedItemRef.current.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  }, [selectedId, results]);

  // Same formula as the CSS grid (auto-fill, minmax(TILE_MIN_WIDTH, 1fr)), so arrows match rows.
  useLayoutEffect(() => {
    const el = gridRef.current;
    if (!grid || !el || !onGridColumnsChange) return;
    const report = () => {
      const width = el.clientWidth - 2 * GRID_GAP;
      onGridColumnsChange(Math.max(1, Math.floor((width + GRID_GAP) / (TILE_MIN_WIDTH + GRID_GAP))));
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(el);
    return () => observer.disconnect();
  }, [grid, onGridColumnsChange, results.length]);

  const containerClass = `results-container ${withPreview ? "with-preview" : ""}`;

  if (isLoading && results.length === 0) {
    return (
      <div className={containerClass}>
        <div className="empty-state">
          <div className="spinner-dot" style={{ marginBottom: 14 }} />
          <div className="empty-title">Searching…</div>
        </div>
      </div>
    );
  }

  if (!query && results.length === 0) {
    return (
      <div className={containerClass}>
        <div className="empty-state">
          {modelStatus && <ModelSetupCard status={modelStatus} onRetry={onRetryModel} />}
          <Compass size={30} className="empty-state-icon" />
          <div className="empty-title">Search by what's in your files</div>
          <div className="empty-desc">
            Describe a photo, quote a line, or name a file. Sift checks names, exact text, the text
            inside images, and meaning at once.
          </div>

          <div className="example-queries">
            {EXAMPLE_QUERIES.map((q) => (
              <button key={q} className="example-query-chip" onClick={() => onExampleQuery?.(q)}>
                {q}
              </button>
            ))}
          </div>

          <div className="welcome-list">
            <div className="welcome-row">
              <Sparkles size={13} color="var(--accent-emerald)" />
              <span className="welcome-row-title">Meaning</span>
              <span className="welcome-row-desc">describe a photo or idea; EmbeddingGemma 2 finds it</span>
            </div>
            <div className="welcome-row">
              <ScanText size={13} color="var(--accent-amber)" />
              <span className="welcome-row-title">Text in images</span>
              <span className="welcome-row-desc">error codes inside screenshots are searchable</span>
            </div>
            <div className="welcome-row">
              <Command size={13} color="var(--accent-violet-light)" />
              <span className="welcome-row-title">Keyboard</span>
              <span className="welcome-row-desc">
                <span className="keycap">↑↓</span> move · <span className="keycap">↵</span> open ·{" "}
                <span className="keycap">Ctrl+K</span> actions
              </span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className={containerClass}>
        <div className="empty-state">
          <SearchX size={32} className="empty-state-icon" />
          <div className="empty-title">Nothing matched "{query}"</div>
          <div className="empty-desc">
            Try describing it differently, switch to <strong>All</strong>, or check which folders are
            indexed (<span className="keycap">Ctrl+Shift+F</span>).
          </div>
        </div>
      </div>
    );
  }

  const header = (
    <div className="results-header-info">
      <span className="results-count-badge">
        {results.length} {results.length === 1 ? "result" : "results"}
        {executionMs != null && <span className="results-timing"> · {Math.round(executionMs)} ms</span>}
        {modelNotReady(modelStatus) && (
          <span className="results-timing"> · meaning search is still setting up</span>
        )}
      </span>
    </div>
  );

  if (grid) {
    return (
      <div className={containerClass}>
        {header}
        <div className="results-grid" ref={gridRef}>
          {results.map((item) => {
            const selected = item.id === selectedId;
            return (
              <GridTile
                key={item.id}
                item={item}
                selected={selected}
                onSelect={() => onSelect(item)}
                innerRef={selected ? selectedItemRef : undefined}
              />
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className={containerClass}>
      {header}
      {results.map((item) => {
        const isSelected = item.id === selectedId;
        const match = matchLabel(item);

        return (
          <div
            key={item.id}
            ref={isSelected ? selectedItemRef : null}
            data-id={item.id}
            className={`result-item ${isSelected ? "selected" : ""}`}
            onClick={() => onSelect(item)}
            onDoubleClick={() => SiftAPI.openResult(item.path, item.page_number)}
          >
            <div className="result-item-main">
              <ResultIcon item={item} />

              <div className="result-text-stack">
                <div className="result-title-row">
                  <span className="result-title">{item.filename}</span>
                  {item.page_number && <span className="result-page-badge">p. {item.page_number}</span>}
                </div>

                <div className="result-breadcrumb-row" title={item.path}>
                  <span>{displayDir(item.path)}</span>
                </div>

                {/* Snippets arrive HTML-escaped from sift-core; only <b> highlights are markup. */}
                {item.snippet && (
                  <div className="result-snippet-preview" dangerouslySetInnerHTML={{ __html: item.snippet }} />
                )}
              </div>
            </div>

            <div className="result-meta-right">
              <span className={`badge-tag ${match.className}`}>
                {match.icon}
                <span>{match.label}</span>
              </span>
              {isSelected && (
                <span className="result-item-action-hint">
                  <CornerDownLeft size={11} />
                </span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};
