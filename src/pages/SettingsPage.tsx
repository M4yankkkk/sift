import React, { useState, useEffect } from "react";
import { Sliders, Cpu, Zap, Shield, Save, X } from "lucide-react";
import { SiftAPI } from "../services/tauriClient";
import type { AppSettings, ResourceMode } from "../types/contract";

interface SettingsPageProps {
  onClose?: () => void;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({ onClose }) => {
  const [settings, setSettings] = useState<AppSettings>({
    global_shortcut: "Ctrl+Alt+Space",
    resource_mode: "eco",

    ml_enabled: false,
    ocr_enabled: false,
    default_exclusions: [".git", "node_modules", "target", "dist", ".cache"],
    max_file_size_mb: 25,
  });
  const [isSaved, setIsSaved] = useState(false);

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
        <div className="modal-title-stack">
          <Sliders size={18} color="var(--accent-violet-light)" />
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
