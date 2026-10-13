import React, { useState, useEffect } from "react";
import { Folder, Plus, Trash2, X, AlertCircle } from "lucide-react";
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

  const handleAddRoot = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newPath.trim()) return;

    setIsLoading(true);
    setError(null);
    try {
      await SiftAPI.addRoot(newPath.trim());
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
      <div className="modal-header">
        <div className="modal-title-stack" style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <SiftBrandmark size={14} />
          <h3 style={{ fontSize: 15, fontWeight: 600 }}>Approved Search Roots</h3>
        </div>
        {onClose && (
          <button className="modal-close-btn" onClick={onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        )}
      </div>

      <div className="modal-content-area">
        <p style={{ fontSize: 12.5, color: "var(--text-secondary)", marginBottom: 16, lineHeight: 1.5 }}>
          Sift strictly indexes only folders you approve. Hidden directories, build
          artifacts, and `.git` caches are excluded by default.
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
              marginBottom: 14,
              fontSize: 12,
            }}
          >
            <AlertCircle size={15} />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleAddRoot} style={{ display: "flex", gap: 8, marginBottom: 18 }}>
          <input
            type="text"
            className="search-input-field"
            style={{
              backgroundColor: "var(--bg-surface-elevated)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-sm)",
              padding: "8px 12px",
              fontSize: 13,
              flex: 1,
            }}
            placeholder="/home/username/Documents or folder path..."
            value={newPath}
            onChange={(e) => setNewPath(e.target.value)}
          />
          <button
            type="submit"
            className="btn-open"
            disabled={isLoading || !newPath.trim()}
            style={{ padding: "8px 14px", display: "flex", alignItems: "center", gap: 5 }}
          >
            <Plus size={14} />
            Add Root
          </button>
        </form>

        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {roots.map((root) => (
            <div
              key={root.id}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "10px 14px",
                backgroundColor: "var(--bg-surface)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-sm)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 10, overflow: "hidden" }}>
                <Folder size={16} color="var(--accent-violet-light)" />
                <div style={{ overflow: "hidden" }}>
                  <div style={{ fontWeight: 500, fontSize: 13, whiteSpace: "nowrap", textOverflow: "ellipsis", overflow: "hidden" }}>
                    {root.path}
                  </div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                    {root.file_count} files • Added {new Date(root.added_at).toLocaleDateString()}
                  </div>
                </div>
              </div>

              <button
                onClick={() => handleRemoveRoot(root.path)}
                style={{
                  background: "none",
                  border: "none",
                  color: "var(--accent-rose)",
                  cursor: "pointer",
                  padding: 4,
                  opacity: 0.8,
                }}
                title="Remove folder from index"
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}

          {roots.length === 0 && (
            <div
              style={{
                padding: 24,
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
