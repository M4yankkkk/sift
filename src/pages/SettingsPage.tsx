import React, { useState, useEffect } from "react";
import { Cpu, Zap, Shield, Save, X, RefreshCw } from "lucide-react";
import { SiftAPI } from "../services/tauriClient";
import { SiftBrandmark } from "../components/SiftLogo";
import type { AppSettings, IndexStatus, ResourceMode } from "../types/contract";

interface SettingsPageProps {
  onClose?: () => void;
  indexStatus?: IndexStatus | null;
  onRefreshStatus?: () => void;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({
  onClose,
  indexStatus,
  onRefreshStatus,
}) => {
  const [settings, setSettings] = useState<AppSettings>({
    global_shortcut: "Ctrl+Alt+Space",
    resource_mode: "eco",

    ml_enabled: true,
    ocr_enabled: true,
    default_exclusions: [".git", "node_modules", "target", "dist", ".cache"],
    max_file_size_mb: 25,
  });
  const [isSaved, setIsSaved] = useState(false);
  const [isReindexing, setIsReindexing] = useState(false);
  const [liveStatus, setLiveStatus] = useState<IndexStatus | null>(indexStatus || null);

  useEffect(() => {
    if (indexStatus) setLiveStatus(indexStatus);
  }, [indexStatus]);

  const isBusy = isReindexing || !!liveStatus?.is_scanning || !!liveStatus?.is_indexing;

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    if (isBusy) {
      interval = setInterval(async () => {
        try {
          const s = await SiftAPI.getIndexStatus();
          setLiveStatus(s);
          if (!s.is_scanning && !s.is_indexing) {
            setIsReindexing(false);
            if (onRefreshStatus) onRefreshStatus();
          }
        } catch {
          // ignore transient poll error
        }
      }, 1000);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [isBusy, onRefreshStatus]);

  useEffect(() => {
    SiftAPI.getSettings().then(setSettings).catch(console.error);
  }, []);

  const handleSave = async () => {
    try {
      const updated = await SiftAPI.setSettings(settings);
      setSettings(updated);
      setIsSaved(true);
      setTimeout(() => setIsSaved(false), 2000);
    } catch (err) {
      console.error("Failed to save settings:", err);
    }
  };

  return (
    <div className="modal-overlay">
      <div className="modal-header">
        <div className="modal-title-stack" style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <SiftBrandmark size={14} />
          <h3 style={{ fontSize: 15, fontWeight: 600 }}>Preferences</h3>
        </div>
        {onClose && (
          <button className="modal-close-btn" onClick={onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        )}
      </div>

      <div className="modal-content-area">
        {/* Global Shortcut */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-sm)",
            padding: 14,
            marginBottom: 14,
          }}
        >
          <label style={{ display: "block", fontWeight: 500, fontSize: 13, marginBottom: 4 }}>
            Global Launch Shortcut
          </label>
          <p style={{ fontSize: 11.5, color: "var(--text-muted)", marginBottom: 10 }}>
            System-wide key combination to toggle Sift (auto-synced with GNOME Shortcuts &amp; sift-toggle on Linux).
          </p>
          <input
            type="text"
            className="search-input-field"
            style={{
              backgroundColor: "var(--bg-surface-elevated)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-xs)",
              padding: "6px 10px",
              width: 180,
              fontSize: 12.5,
              fontFamily: "var(--font-mono)",
            }}
            value={settings.global_shortcut}
            onChange={(e) =>
              setSettings({ ...settings, global_shortcut: e.target.value })
            }
          />
        </div>

        {/* Resource Profile */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-sm)",
            padding: 14,
            marginBottom: 14,
          }}
        >
          <label style={{ display: "block", fontWeight: 500, fontSize: 13, marginBottom: 4 }}>
            Resource Profile
          </label>
          <p style={{ fontSize: 11.5, color: "var(--text-muted)", marginBottom: 12 }}>
            Governs background CPU and memory throttling during indexing passes.
          </p>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
            <div
              onClick={() => setSettings({ ...settings, resource_mode: "eco" as ResourceMode })}
              style={{
                border: `1px solid ${
                  settings.resource_mode === "eco" ? "var(--accent-emerald)" : "var(--border-subtle)"
                }`,
                backgroundColor:
                  settings.resource_mode === "eco" ? "rgba(16, 185, 129, 0.08)" : "var(--bg-surface-elevated)",
                borderRadius: "var(--radius-xs)",
                padding: 10,
                cursor: "pointer",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 3 }}>
                <Cpu size={14} color="var(--accent-emerald)" />
                <span style={{ fontWeight: 600, fontSize: 12 }}>Eco Mode</span>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
                Restricted CPU threads, idle RAM &lt; 100MB.
              </p>
            </div>

            <div
              onClick={() => setSettings({ ...settings, resource_mode: "performance" as ResourceMode })}
              style={{
                border: `1px solid ${
                  settings.resource_mode === "performance" ? "var(--accent-violet)" : "var(--border-subtle)"
                }`,
                backgroundColor:
                  settings.resource_mode === "performance" ? "var(--accent-violet-subtle)" : "var(--bg-surface-elevated)",
                borderRadius: "var(--radius-xs)",
                padding: 10,
                cursor: "pointer",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 3 }}>
                <Zap size={14} color="var(--accent-violet-light)" />
                <span style={{ fontWeight: 600, fontSize: 12 }}>Performance</span>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
                Utilizes multi-core parallel indexing.
              </p>
            </div>
          </div>
        </div>

        {/* AI Semantic Engine (Google EmbeddingGemma 2) */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-sm)",
            padding: 14,
            marginBottom: 14,
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 6 }}>
            <div>
              <label style={{ display: "block", fontWeight: 600, fontSize: 13, marginBottom: 2 }}>
                Google EmbeddingGemma 2
              </label>
              <span
                style={{
                  fontSize: 10.5,
                  padding: "2px 6px",
                  borderRadius: 4,
                  backgroundColor: "rgba(179, 128, 255, 0.15)",
                  color: "#B380FF",
                  fontFamily: "var(--font-mono)",
                  fontWeight: 600,
                }}
              >
                768-dim Unified Multimodal ONNX
              </span>
            </div>
            <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer", fontSize: 12 }}>
              <input
                type="checkbox"
                checked={settings.ml_enabled}
                onChange={(e) => setSettings({ ...settings, ml_enabled: e.target.checked })}
                style={{ accentColor: "var(--accent-violet)" }}
              />
              <span style={{ fontWeight: 500 }}>Enabled</span>
            </label>
          </div>
          <p style={{ fontSize: 11.5, color: "var(--text-muted)", lineHeight: 1.4, marginBottom: 10 }}>
            Maps text, code, PDFs, and images into a unified 768-dimensional semantic space with asymmetric search instruction prefixes (<code>task: search result | query:</code>).
          </p>
          <button
            onClick={async () => {
              setIsReindexing(true);
              try {
                const s = await SiftAPI.reindexAll();
                setLiveStatus(s);
                if (onRefreshStatus) onRefreshStatus();
              } catch (e) {
                console.error("Reindex error:", e);
                setIsReindexing(false);
              }
            }}
            disabled={isBusy}
            style={{
              fontSize: 11.5,
              padding: "6px 14px",
              borderRadius: "var(--radius-xs)",
              background: isBusy ? "rgba(139, 92, 246, 0.15)" : "var(--bg-surface-elevated)",
              border: isBusy ? "1px solid var(--accent-violet)" : "1px solid var(--border-subtle)",
              color: "var(--text-primary)",
              cursor: isBusy ? "not-allowed" : "pointer",
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <RefreshCw size={12} className={isBusy ? "spin" : ""} />
            {isBusy ? "Re-indexing files in background..." : "Re-index All Files (Apply New Embeddings)"}
          </button>

          {isBusy && (
            <div
              style={{
                marginTop: 10,
                padding: "8px 12px",
                background: "rgba(139, 92, 246, 0.08)",
                borderRadius: "var(--radius-xs)",
                border: "1px solid rgba(139, 92, 246, 0.25)",
                fontSize: 11,
                display: "flex",
                flexDirection: "column",
                gap: 4,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--accent-violet-light)", fontWeight: 500 }}>
                <span className="indicator-dot active" style={{ width: 6, height: 6 }} />
                <span>{liveStatus?.current_operation || "Generating EmbeddingGemma 2 vectors..."}</span>
              </div>
              <div style={{ color: "var(--text-muted)", fontSize: 10.5 }}>
                {liveStatus?.files_indexed ?? 0} files discovered • Background ML pass active (~2–5 mins for large libraries). Filename & text search remain instantly usable!
              </div>
            </div>
          )}

          {!isBusy && liveStatus?.last_scan_completed_at && (
            <div style={{ marginTop: 8, fontSize: 10.5, color: "var(--text-muted)" }}>
              Index ready • {liveStatus.files_indexed} files indexed
            </div>
          )}
        </div>

        {/* Offline Privacy Guarantee */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-sm)",
            padding: 14,
            marginBottom: 18,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
            <Shield size={14} color="var(--accent-violet-light)" />
            <span style={{ fontWeight: 600, fontSize: 12.5 }}>Offline Privacy Guarantee</span>
          </div>
          <p style={{ fontSize: 11.5, color: "var(--text-muted)", lineHeight: 1.4 }}>
            All SQLite indices, queries, and file contents remain 100% on this machine.
            Zero telemetry and zero cloud calls.
          </p>
        </div>

        <button
          className="btn-open"
          onClick={handleSave}
          style={{ display: "inline-flex", alignItems: "center", gap: 6, padding: "8px 16px" }}
        >
          <Save size={14} />
          {isSaved ? "Saved!" : "Save Preferences"}
        </button>
      </div>
    </div>
  );
};
