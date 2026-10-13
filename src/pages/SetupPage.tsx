import React, { useState, useEffect } from "react";
import { Folder, Plus, Trash2, X, AlertCircle, FolderPlus } from "lucide-react";
import { SiftAPI } from "../services/tauriClient";
import { SiftBrandmark } from "../components/SiftLogo";
import type { RootFolder } from "../types/contract";

interface SetupPageProps {
  onClose?: () => void;
}

export const SetupPage: React.FC<SetupPageProps> = ({ onClose }) => {
  const [roots, setRoots] = useState<RootFolder[]>([]);
  const [newPath, setNewPath] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadRoots = async () => {
    try {
      const data = await SiftAPI.listRoots();
      setRoots(data);
    } catch (err) {
      console.error("Failed to load roots:", err);
    }
  };

  useEffect(() => {
    loadRoots();
  }, []);

  const handleAddRoot = async (pathToAdd?: string) => {
    const target = (pathToAdd || newPath).trim();
    if (!target) return;

    setIsLoading(true);
    setError(null);
    try {
      await SiftAPI.addRoot(target);
      setNewPath("");
      await loadRoots();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleRemoveRoot = async (path: string) => {
    try {
      await SiftAPI.removeRoot(path);
      await loadRoots();
    } catch (err) {
      console.error("Failed to remove root:", err);
    }
  };

  return (
    <div className="modal-overlay">
      {/* Header */}
      <div className="modal-header">
        <div className="modal-title-stack" style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <SiftBrandmark size={15} />
          <div>
            <h3 style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>Approved Search Directories</h3>
            <p style={{ fontSize: 11, color: "var(--text-muted)" }}>Control which local folders Sift monitors and indexes</p>
          </div>
        </div>
        {onClose && (
          <button className="modal-close-btn" onClick={onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        )}
      </div>

      <div className="modal-content-area" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <p style={{ fontSize: 12.5, color: "var(--text-secondary)", lineHeight: 1.5 }}>
          Sift strictly indexes only directories you explicitly approve. System roots, hidden dotfiles, build artifacts (<code style={{ color: "var(--accent-violet-light)" }}>target/</code>, <code style={{ color: "var(--accent-violet-light)" }}>node_modules/</code>), and <code style={{ color: "var(--accent-violet-light)" }}>.git</code> caches are automatically excluded.
        </p>

        {error && (
          <div
            style={{
              padding: "10px 14px",
              borderRadius: "var(--radius-sm)",
              backgroundColor: "rgba(244, 63, 94, 0.12)",
              border: "1px solid var(--accent-rose)",
              color: "var(--accent-rose)",
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 12,
            }}
          >
            <AlertCircle size={15} />
            <span>{error}</span>
          </div>
        )}

        {/* Add Folder Form */}
        <form
          onSubmit={(e) => {
            e.preventDefault();
            handleAddRoot();
          }}
          style={{ display: "flex", gap: 8 }}
        >
          <input
            type="text"
            className="search-input-field"
            style={{
              backgroundColor: "var(--bg-surface)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-sm)",
              padding: "9px 12px",
              fontSize: 13,
              flex: 1,
            }}
            placeholder="/home/username/Documents or directory path..."
            value={newPath}
            onChange={(e) => setNewPath(e.target.value)}
          />
          <button
            type="submit"
            className="btn-open"
            disabled={isLoading || !newPath.trim()}
            style={{ padding: "8px 16px", display: "flex", alignItems: "center", gap: 6 }}
          >
            <Plus size={14} />
            <span>Add Folder</span>
          </button>
        </form>

        {/* Quick Directory Shortcuts */}
        <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>Suggested directories:</span>
          {["Documents", "Projects", "Downloads", "Desktop"].map((dir) => {
            const pathGuess = `/home/${window.location.pathname.split("/")[2] || "user"}/${dir}`;
            return (
              <button
                key={dir}
                type="button"
                className="filter-chip"
                onClick={() => handleAddRoot(pathGuess)}
                style={{ padding: "3px 8px", fontSize: 11 }}
              >
                <FolderPlus size={11} />
                <span>~/{dir}</span>
              </button>
            );
          })}
        </div>

        {/* Existing Approved Directories List */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginTop: 4 }}>
            Monitored Folders ({roots.length})
          </span>

          {roots.map((root) => (
            <div
              key={root.id}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "12px 14px",
                backgroundColor: "var(--bg-surface)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-sm)",
                transition: "all 0.15s ease",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 12, overflow: "hidden" }}>
                <div
                  style={{
                    width: 32,
                    height: 32,
                    borderRadius: "var(--radius-xs)",
                    backgroundColor: "var(--accent-violet-subtle)",
                    border: "1px solid rgba(139, 92, 246, 0.2)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flexShrink: 0,
                  }}
                >
                  <Folder size={16} color="var(--accent-violet-light)" />
                </div>
                <div style={{ overflow: "hidden" }}>
                  <div
                    style={{
                      fontWeight: 500,
                      fontSize: 13,
                      whiteSpace: "nowrap",
                      textOverflow: "ellipsis",
                      overflow: "hidden",
                      color: "var(--text-primary)",
                    }}
                    title={root.path}
                  >
                    {root.path}
                  </div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                    {root.file_count.toLocaleString()} files indexed • Added {new Date(root.added_at).toLocaleDateString()}
                  </div>
                </div>
              </div>

              <button
                onClick={() => handleRemoveRoot(root.path)}
                style={{
                  background: "transparent",
                  border: "none",
                  color: "var(--text-muted)",
                  cursor: "pointer",
                  padding: 6,
                  borderRadius: "var(--radius-xs)",
                  transition: "all 0.15s ease",
                }}
                onMouseEnter={(e) => (e.currentTarget.style.color = "var(--accent-rose)")}
                onMouseLeave={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
                title="Remove folder from index"
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}

          {roots.length === 0 && (
            <div
              style={{
                padding: 32,
                textAlign: "center",
                border: "1px dashed var(--border-subtle)",
                borderRadius: "var(--radius-md)",
                color: "var(--text-muted)",
                fontSize: 13,
              }}
            >
              No folders added yet. Add a directory above to start indexing.
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
