import React, { useState, useEffect } from "react";
import {
  Cpu,
  Zap,
  Shield,
  Save,
  X,
  RefreshCw,
  Keyboard,
  HardDrive,
  Sparkles,
  Info,
  CheckCircle2,
} from "lucide-react";
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

  const handleReindex = async () => {
    setIsReindexing(true);
    try {
      const s = await SiftAPI.reindexAll();
      setLiveStatus(s);
      if (onRefreshStatus) onRefreshStatus();
    } catch (e) {
      console.error("Reindex error:", e);
      setIsReindexing(false);
    }
  };

  return (
    <div className="modal-overlay">
      {/* Header */}
      <div className="modal-header">
        <div className="modal-title-stack" style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <SiftBrandmark size={15} />
          <div>
            <h3 style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>Preferences & Engine Diagnostics</h3>
            <p style={{ fontSize: 11, color: "var(--text-muted)" }}>Control launcher behavior, shortcut bindings, and neural models</p>
          </div>
        </div>
        {onClose && (
          <button className="modal-close-btn" onClick={onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        )}
      </div>

      <div className="modal-content-area" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {/* SECTION 1: INDEXING ENGINE & LAUNCH BEHAVIOR (Directly answers user question) */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-md)",
            padding: 16,
            display: "flex",
            flexDirection: "column",
            gap: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <HardDrive size={16} color="var(--accent-violet-light)" />
              <span style={{ fontWeight: 600, fontSize: 13.5 }}>Indexing Engine & Startup Behavior</span>
            </div>
            <span
              style={{
                fontSize: 10.5,
                fontWeight: 600,
                color: "var(--accent-emerald)",
                background: "var(--accent-emerald-subtle)",
                border: "1px solid rgba(16, 185, 129, 0.3)",
                padding: "2px 7px",
                borderRadius: "var(--radius-xs)",
              }}
            >
              Smart Incremental Active
            </span>
          </div>

          {/* Explanation Banner */}
          <div
            style={{
              padding: "10px 14px",
              borderRadius: "var(--radius-sm)",
              backgroundColor: "rgba(139, 92, 246, 0.08)",
              border: "1px solid rgba(139, 92, 246, 0.22)",
              display: "flex",
              alignItems: "flex-start",
              gap: 10,
              fontSize: 12,
              lineHeight: 1.5,
              color: "#c4b5fd",
            }}
          >
            <Info size={16} style={{ flexShrink: 0, marginTop: 2, color: "var(--accent-violet-light)" }} />
            <div>
              <strong>Does Sift re-index everything on every launch?</strong>
              <div style={{ color: "var(--text-secondary)", marginTop: 2 }}>
                <strong>No.</strong> Sift uses smart timestamp and size fingerprinting. On application start, Sift reconciles your approved directories in <strong>&lt; 0.5s</strong>. Existing unchanged files are <strong>never</strong> re-embedded. Only modified or newly added files undergo BM25 tokenization and EmbeddingGemma 2 ONNX vector generation.
              </div>
            </div>
          </div>

          {/* Live Metric Cards Grid */}
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 10 }}>
            <div
              style={{
                background: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-xs)",
                padding: "10px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>Files Indexed</div>
              <div style={{ fontSize: 18, fontWeight: 700, color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
                {(liveStatus?.files_indexed ?? 0).toLocaleString()}
              </div>
            </div>

            <div
              style={{
                background: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-xs)",
                padding: "10px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>Neural Embeddings</div>
              <div style={{ fontSize: 18, fontWeight: 700, color: "var(--accent-violet-light)", fontFamily: "var(--font-mono)" }}>
                {(liveStatus?.total_embeddings ?? 0).toLocaleString()}
              </div>
            </div>

            <div
              style={{
                background: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-xs)",
                padding: "10px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>Indexing Engine State</div>
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginTop: 4 }}>
                <span className={`indicator-dot ${isBusy ? "active" : ""}`} />
                <span style={{ fontSize: 12, fontWeight: 500, color: isBusy ? "var(--accent-violet-light)" : "var(--accent-emerald)" }}>
                  {isBusy ? "Indexing Pass Active" : "Reconciled & Ready"}
                </span>
              </div>
            </div>
          </div>

          {/* Action to Force Full Reindex */}
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: 4 }}>
            <span style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
              Need to refresh all cached models or wipe stale embeddings?
            </span>
            <button
              onClick={handleReindex}
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
                transition: "all 0.15s ease",
              }}
            >
              <RefreshCw size={12} className={isBusy ? "spin" : ""} />
              {isBusy ? "Re-indexing files in background..." : "Force Full Re-index"}
            </button>
          </div>
        </div>

        {/* SECTION 2: KEYBOARD SHORTCUTS REFERENCE GUIDE */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-md)",
            padding: 16,
            display: "flex",
            flexDirection: "column",
            gap: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Keyboard size={16} color="var(--accent-violet-light)" />
            <span style={{ fontWeight: 600, fontSize: 13.5 }}>Complete Keyboard Shortcuts Guide</span>
          </div>
          <p style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
            Sift is 100% keyboard-navigable for distraction-free developer workflows.
          </p>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
            {/* Search & Navigation */}
            <div
              style={{
                backgroundColor: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-xs)",
                padding: "10px 12px",
              }}
            >
              <div
                style={{
                  fontSize: 10.5,
                  fontWeight: 600,
                  color: "var(--accent-violet-light)",
                  textTransform: "uppercase",
                  letterSpacing: "0.5px",
                  marginBottom: 8,
                }}
              >
                Search &amp; Navigation
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 8, fontSize: 11.5 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Navigate results</span>
                  <div style={{ display: "flex", gap: 4 }}>
                    <span className="keycap">↓</span>
                    <span className="keycap">↑</span>
                  </div>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Next / Prev (Vim style)</span>
                  <div style={{ display: "flex", gap: 4 }}>
                    <span className="keycap">Ctrl+N</span>
                    <span className="keycap">Ctrl+P</span>
                  </div>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Focus &amp; select search bar</span>
                  <span className="keycap">Ctrl+L</span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Toggle preview panel</span>
                  <span className="keycap">Tab</span>
                </div>
              </div>
            </div>

            {/* Actions & File Execution */}
            <div
              style={{
                backgroundColor: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-xs)",
                padding: "10px 12px",
              }}
            >
              <div
                style={{
                  fontSize: 10.5,
                  fontWeight: 600,
                  color: "var(--accent-emerald)",
                  textTransform: "uppercase",
                  letterSpacing: "0.5px",
                  marginBottom: 8,
                }}
              >
                Actions &amp; Opening
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 8, fontSize: 11.5 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Open file in default app</span>
                  <div style={{ display: "flex", gap: 4 }}>
                    <span className="keycap">↵ Enter</span>
                    <span className="keycap">Ctrl+O</span>
                  </div>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Reveal in file manager</span>
                  <span className="keycap">Ctrl+R</span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Open Preferences</span>
                  <span className="keycap">Ctrl+,</span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Manage Search Folders</span>
                  <span className="keycap">Ctrl+Shift+F</span>
                </div>
              </div>
            </div>

            {/* Window & Dismissal */}
            <div
              style={{
                backgroundColor: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-subtle)",
                borderRadius: "var(--radius-xs)",
                padding: "10px 12px",
                gridColumn: "span 2",
              }}
            >
              <div
                style={{
                  fontSize: 10.5,
                  fontWeight: 600,
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.5px",
                  marginBottom: 8,
                }}
              >
                Window &amp; Launcher Control
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px 20px", fontSize: 11.5 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Global Launch Shortcut</span>
                  <span
                    className="keycap"
                    style={{
                      color: "var(--accent-violet-light)",
                      borderColor: "rgba(139, 92, 246, 0.4)",
                      backgroundColor: "rgba(139, 92, 246, 0.12)",
                    }}
                  >
                    {settings.global_shortcut || "Ctrl+Alt+Space"}
                  </span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Clear search / Close sheet</span>
                  <span className="keycap">Esc</span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Instant window hide</span>
                  <span className="keycap">Shift+Esc</span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ color: "var(--text-secondary)" }}>Quick dismiss</span>
                  <div style={{ display: "flex", gap: 4 }}>
                    <span className="keycap">Ctrl+W</span>
                    <span className="keycap">Ctrl+Q</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* SECTION 3: GLOBAL SHORTCUT & SYSTEM CONFIG */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-md)",
            padding: 16,
          }}
        >
          <label style={{ display: "block", fontWeight: 600, fontSize: 13, marginBottom: 4 }}>
            System-Wide Global Shortcut
          </label>
          <p style={{ fontSize: 11.5, color: "var(--text-muted)", marginBottom: 10 }}>
            Toggles Sift from anywhere in Linux (Wayland GNOME custom shortcut daemon &amp; Unix socket synced).
          </p>
          <input
            type="text"
            className="search-input-field"
            style={{
              backgroundColor: "var(--bg-surface-elevated)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-xs)",
              padding: "7px 12px",
              width: 200,
              fontSize: 13,
              fontFamily: "var(--font-mono)",
            }}
            value={settings.global_shortcut}
            onChange={(e) =>
              setSettings({ ...settings, global_shortcut: e.target.value })
            }
          />
        </div>

        {/* SECTION 4: AI SEMANTIC & OCR MODELS */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-md)",
            padding: 16,
            display: "flex",
            flexDirection: "column",
            gap: 14,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Sparkles size={16} color="var(--accent-violet-light)" />
            <span style={{ fontWeight: 600, fontSize: 13.5 }}>AI Semantic Engine &amp; OCR Models</span>
          </div>

          {/* Model 1: EmbeddingGemma 2 */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "10px 12px",
              background: "var(--bg-surface-elevated)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-xs)",
            }}
          >
            <div>
              <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                <span style={{ fontWeight: 600, fontSize: 13 }}>Google EmbeddingGemma 2</span>
                <span
                  style={{
                    fontSize: 10,
                    padding: "1px 5px",
                    borderRadius: 3,
                    background: "rgba(167, 139, 250, 0.15)",
                    color: "var(--accent-violet-light)",
                    fontFamily: "var(--font-mono)",
                    fontWeight: 600,
                  }}
                >
                  256-dim fastembed ONNX
                </span>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                Asymmetric search queries with instruction prefixes (<code>task: search result | query:</code>)
              </p>
            </div>

            <label className="switch-container">
              <input
                type="checkbox"
                checked={settings.ml_enabled}
                onChange={(e) => setSettings({ ...settings, ml_enabled: e.target.checked })}
                style={{ display: "none" }}
              />
              <div className={`switch-track ${settings.ml_enabled ? "active" : ""}`}>
                <div className="switch-thumb" />
              </div>
            </label>
          </div>

          {/* Model 2: Tesseract OCR */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "10px 12px",
              background: "var(--bg-surface-elevated)",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--radius-xs)",
            }}
          >
            <div>
              <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                <span style={{ fontWeight: 600, fontSize: 13 }}>Tesseract OCR Engine</span>
                <span
                  style={{
                    fontSize: 10,
                    padding: "1px 5px",
                    borderRadius: 3,
                    background: "rgba(245, 158, 11, 0.15)",
                    color: "var(--accent-amber)",
                    fontFamily: "var(--font-mono)",
                    fontWeight: 600,
                  }}
                >
                  v5.x Engine
                </span>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                Extracts text and word bounding coordinates from scanned documents and PNG/JPEG images.
              </p>
            </div>

            <label className="switch-container">
              <input
                type="checkbox"
                checked={settings.ocr_enabled}
                onChange={(e) => setSettings({ ...settings, ocr_enabled: e.target.checked })}
                style={{ display: "none" }}
              />
              <div className={`switch-track ${settings.ocr_enabled ? "active" : ""}`}>
                <div className="switch-thumb" />
              </div>
            </label>
          </div>

          {/* Resource Profiles */}
          <div>
            <label style={{ display: "block", fontWeight: 500, fontSize: 12, marginBottom: 6, color: "var(--text-secondary)" }}>
              Resource Throttling Profile
            </label>
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
                  transition: "all 0.15s ease",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 3 }}>
                  <Cpu size={14} color="var(--accent-emerald)" />
                  <span style={{ fontWeight: 600, fontSize: 12 }}>Eco Mode (Default)</span>
                </div>
                <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  Throttled background worker threads, idle memory footprint &lt; 100MB.
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
                  transition: "all 0.15s ease",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 3 }}>
                  <Zap size={14} color="var(--accent-violet-light)" />
                  <span style={{ fontWeight: 600, fontSize: 12 }}>Performance Mode</span>
                </div>
                <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  Uncapped multi-core batch vectorization and parallel OCR processing.
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* SECTION 5: PRIVACY GUARANTEE */}
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-md)",
            padding: 14,
            display: "flex",
            alignItems: "center",
            gap: 12,
          }}
        >
          <Shield size={20} color="var(--accent-violet-light)" style={{ flexShrink: 0 }} />
          <div>
            <div style={{ fontWeight: 600, fontSize: 12.5, color: "var(--text-primary)" }}>
              100% Offline &amp; Private Desktop Search
            </div>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
              All SQLite databases, Tantivy inverted indices, neural embeddings, and queries remain strictly local. Zero analytics and zero network calls.
            </p>
          </div>
        </div>

        {/* Save Button */}
        <div>
          <button
            className="btn-open"
            onClick={handleSave}
            style={{ display: "inline-flex", alignItems: "center", gap: 6, padding: "8px 18px" }}
          >
            {isSaved ? <CheckCircle2 size={14} /> : <Save size={14} />}
            <span>{isSaved ? "Preferences Saved!" : "Save Preferences"}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
