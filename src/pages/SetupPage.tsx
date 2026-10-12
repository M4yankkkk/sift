import React, { useState, useEffect } from "react";
import { Folder, Plus, Trash2, ShieldCheck, AlertCircle } from "lucide-react";
import { SiftAPI } from "../services/tauriClient";
import type { RootFolder } from "../types/contract";

export const SetupPage: React.FC = () => {
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
    <div style={{ flex: 1, padding: 24, overflowY: "auto" }}>
      <div style={{ maxWidth: 720, margin: "0 auto" }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>
          Approved Search Roots
        </h2>
        <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 20 }}>
          Sift strictly indexes only folders you approve. Hidden files, build
          artifacts, and `.git` repositories are excluded by default.
        </p>

        {error && (
          <div
            style={{
              padding: 12,
              borderRadius: 8,
              backgroundColor: "rgba(244, 63, 94, 0.1)",
              border: "1px solid var(--accent-rose)",
              color: "var(--accent-rose)",
              display: "flex",
              alignItems: "center",
              gap: 8,
              marginBottom: 16,
              fontSize: 13,
            }}
          >
            <AlertCircle size={16} />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleAddRoot} style={{ display: "flex", gap: 10, marginBottom: 24 }}>
          <input
            type="text"
            className="search-input"
            style={{
              backgroundColor: "var(--bg-secondary)",
              border: "1px solid var(--border-color)",
              borderRadius: "var(--radius-sm)",
              padding: "10px 14px",
              flex: 1,
            }}
            placeholder="/home/username/Documents or /path/to/folder"
            value={newPath}
            onChange={(e) => setNewPath(e.target.value)}
          />
          <button
            type="submit"
            className="btn-primary"
            disabled={isLoading || !newPath.trim()}
            style={{ display: "flex", alignItems: "center", gap: 6 }}
          >
            <Plus size={15} />
            Add Folder
          </button>
        </form>

        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {roots.map((root) => (
            <div
              key={root.id}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "12px 16px",
                backgroundColor: "var(--bg-secondary)",
                border: "1px solid var(--border-color)",
                borderRadius: "var(--radius-sm)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <Folder size={18} color="var(--accent-blue)" />
                <div>
                  <div style={{ fontWeight: 500, fontSize: 14 }}>{root.path}</div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                    {root.file_count} files indexed • Added {new Date(root.added_at).toLocaleDateString()}
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
                }}
                title="Remove folder from index"
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}

          {roots.length === 0 && (
            <div
              style={{
                padding: 32,
                textAlign: "center",
                border: "1px dashed var(--border-color)",
                borderRadius: "var(--radius-md)",
                color: "var(--text-muted)",
              }}
            >
              <ShieldCheck size={28} style={{ margin: "0 auto 8px auto", opacity: 0.6 }} />
              <p style={{ fontSize: 14 }}>No search roots configured yet.</p>
              <p style={{ fontSize: 12, marginTop: 4 }}>
                Add your document or project directories above to begin indexing.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
