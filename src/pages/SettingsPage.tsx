import React, { useState, useEffect } from "react";
import { Settings as SettingsIcon, Cpu, Zap, Shield, Save } from "lucide-react";
import { SiftAPI } from "../services/tauriClient";
import type { AppSettings, ResourceMode } from "../types/contract";

export const SettingsPage: React.FC = () => {
  const [settings, setSettings] = useState<AppSettings>({
    global_shortcut: "Alt+Space",
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
    <div style={{ flex: 1, padding: 24, overflowY: "auto" }}>
      <div style={{ maxWidth: 640, margin: "0 auto" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 16 }}>
          <SettingsIcon size={20} color="var(--accent-blue)" />
          <h2 style={{ fontSize: 18, fontWeight: 600 }}>Application Settings</h2>
        </div>

        {/* Global Shortcut */}
        <div
          style={{
            backgroundColor: "var(--bg-secondary)",
            border: "1px solid var(--border-color)",
            borderRadius: "var(--radius-md)",
            padding: 16,
            marginBottom: 16,
          }}
        >
          <label style={{ display: "block", fontWeight: 500, fontSize: 14, marginBottom: 6 }}>
            Global Launch Shortcut
          </label>
          <p style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 12 }}>
            Press this system-wide key combination to bring Sift to the foreground.
          </p>
          <input
            type="text"
            className="search-input"
            style={{
              backgroundColor: "var(--bg-tertiary)",
              border: "1px solid var(--border-color)",
              borderRadius: "var(--radius-sm)",
              padding: "8px 12px",
              width: 200,
            }}
            value={settings.global_shortcut}
            onChange={(e) =>
              setSettings({ ...settings, global_shortcut: e.target.value })
            }
          />
        </div>

        {/* Resource Mode */}
        <div
          style={{
            backgroundColor: "var(--bg-secondary)",
            border: "1px solid var(--border-color)",
            borderRadius: "var(--radius-md)",
            padding: 16,
            marginBottom: 16,
          }}
        >
          <label style={{ display: "block", fontWeight: 500, fontSize: 14, marginBottom: 6 }}>
            Resource Profile
          </label>
          <p style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 14 }}>
            Controls background CPU thread limits and memory consumption during indexing.
          </p>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
            <div
              onClick={() => setSettings({ ...settings, resource_mode: "eco" as ResourceMode })}
              style={{
                border: `1px solid ${
                  settings.resource_mode === "eco" ? "var(--accent-emerald)" : "var(--border-color)"
                }`,
                backgroundColor:
                  settings.resource_mode === "eco" ? "rgba(16, 185, 129, 0.08)" : "var(--bg-tertiary)",
                borderRadius: "var(--radius-sm)",
                padding: 12,
                cursor: "pointer",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                <Cpu size={16} color="var(--accent-emerald)" />
                <span style={{ fontWeight: 600, fontSize: 13 }}>Eco Mode</span>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
                Low CPU priority, throttled background queue, idle RAM &lt; 100MB.
              </p>
            </div>

            <div
              onClick={() => setSettings({ ...settings, resource_mode: "performance" as ResourceMode })}
              style={{
                border: `1px solid ${
                  settings.resource_mode === "performance" ? "var(--accent-blue)" : "var(--border-color)"
                }`,
                backgroundColor:
                  settings.resource_mode === "performance" ? "rgba(59, 130, 246, 0.08)" : "var(--bg-tertiary)",
                borderRadius: "var(--radius-sm)",
                padding: 12,
                cursor: "pointer",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                <Zap size={16} color="var(--accent-blue)" />
                <span style={{ fontWeight: 600, fontSize: 13 }}>Performance</span>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
                Uses available cores for faster initial index passes.
              </p>
            </div>
          </div>
        </div>

        {/* Offline Privacy Guarantee */}
        <div
          style={{
            backgroundColor: "var(--bg-secondary)",
            border: "1px solid var(--border-color)",
            borderRadius: "var(--radius-md)",
            padding: 16,
            marginBottom: 24,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
            <Shield size={16} color="var(--accent-blue)" />
            <span style={{ fontWeight: 600, fontSize: 14 }}>Offline Privacy Guarantee</span>
          </div>
          <p style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
            Sift is strictly local-only. No telemetry, cloud queries, or external network requests
            are made during indexing or search. All SQLite, Tantivy, and vector data remain
            encrypted or stored solely on your machine.
          </p>
        </div>

        <button
          className="btn-primary"
          onClick={handleSave}
          style={{ display: "flex", alignItems: "center", gap: 6, padding: "8px 18px", fontSize: 13 }}
        >
          <Save size={15} />
          {isSaved ? "Saved!" : "Save Changes"}
        </button>
      </div>
    </div>
  );
};
