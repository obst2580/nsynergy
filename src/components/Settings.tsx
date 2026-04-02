import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SettingsData, PermissionCheck } from "../types";

interface SettingsProps {
  onSave: () => void;
}

function Settings({ onSave }: SettingsProps) {
  const [settings, setSettings] = useState<SettingsData>({
    machine_name: "",
    udp_port: 24800,
    tcp_port: 24801,
    edge_threshold: 2,
  });
  const [saved, setSaved] = useState(false);
  const [permissions, setPermissions] = useState<PermissionCheck | null>(null);
  const [permInstructions, setPermInstructions] = useState<string[]>([]);
  const [activeSection, setActiveSection] = useState<"general" | "network" | "permissions">(
    "general"
  );

  useEffect(() => {
    loadSettings();
    loadPermissions();
  }, []);

  async function loadSettings() {
    try {
      const result = await invoke<SettingsData>("get_settings");
      setSettings(result);
    } catch (_e) {
      // Settings load failed, keep defaults
    }
  }

  async function loadPermissions() {
    try {
      const [perms, instructions] = await Promise.all([
        invoke<PermissionCheck>("check_permissions"),
        invoke<string[]>("get_permission_instructions"),
      ]);
      setPermissions(perms);
      setPermInstructions(instructions);
    } catch (_e) {
      // Permission check not available
    }
  }

  async function handleSave() {
    try {
      await invoke("save_settings", { settings });
      setSaved(true);
      onSave();
      setTimeout(() => setSaved(false), 2000);
    } catch (_e) {
      // Save failed
    }
  }

  function renderSectionTab(
    section: "general" | "network" | "permissions",
    label: string
  ) {
    return (
      <button
        onClick={() => setActiveSection(section)}
        style={{
          ...styles.sectionTab,
          ...(activeSection === section ? styles.sectionTabActive : {}),
        }}
      >
        {label}
      </button>
    );
  }

  function renderGeneralSection() {
    return (
      <div>
        <div style={styles.field}>
          <label style={styles.label}>Machine Name</label>
          <input
            style={styles.input}
            value={settings.machine_name}
            onChange={(e) =>
              setSettings({ ...settings, machine_name: e.target.value })
            }
            placeholder="e.g. my-desktop"
          />
          <span style={styles.fieldHint}>
            Name shown to other devices on the network
          </span>
        </div>

        <div style={styles.field}>
          <label style={styles.label}>Edge Threshold (px)</label>
          <input
            style={styles.input}
            type="number"
            min={1}
            max={20}
            value={settings.edge_threshold}
            onChange={(e) =>
              setSettings({
                ...settings,
                edge_threshold: parseInt(e.target.value) || 0,
              })
            }
          />
          <span style={styles.fieldHint}>
            Pixels from screen edge to trigger cursor transition
          </span>
        </div>
      </div>
    );
  }

  function renderNetworkSection() {
    return (
      <div>
        <div style={styles.row}>
          <div style={styles.field}>
            <label style={styles.label}>UDP Port</label>
            <input
              style={styles.input}
              type="number"
              min={1024}
              max={65535}
              value={settings.udp_port}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  udp_port: parseInt(e.target.value) || 0,
                })
              }
            />
            <span style={styles.fieldHint}>Input events</span>
          </div>
          <div style={styles.field}>
            <label style={styles.label}>TCP Port</label>
            <input
              style={styles.input}
              type="number"
              min={1024}
              max={65535}
              value={settings.tcp_port}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  tcp_port: parseInt(e.target.value) || 0,
                })
              }
            />
            <span style={styles.fieldHint}>Clipboard / large data</span>
          </div>
        </div>

        <div style={styles.infoBox}>
          <div style={styles.infoTitle}>Network Info</div>
          <div style={styles.infoRow}>
            <span style={styles.infoLabel}>Discovery</span>
            <span style={styles.infoValue}>mDNS (automatic)</span>
          </div>
          <div style={styles.infoRow}>
            <span style={styles.infoLabel}>Encryption</span>
            <span style={styles.infoValue}>TLS 1.3 (self-signed)</span>
          </div>
        </div>
      </div>
    );
  }

  function renderPermissionStatus(label: string, status: string) {
    const isGranted = status === "Granted";
    const isNA = status === "NotApplicable";

    return (
      <div style={styles.permRow}>
        <div style={styles.permInfo}>
          <span
            style={{
              ...styles.permDot,
              background: isNA ? "#555" : isGranted ? "#4caf50" : "#f44336",
            }}
          />
          <span style={styles.permLabel}>{label}</span>
        </div>
        <span
          style={{
            ...styles.permStatus,
            color: isNA ? "#555" : isGranted ? "#4caf50" : "#f44336",
          }}
        >
          {isNA ? "N/A" : isGranted ? "Granted" : "Required"}
        </span>
      </div>
    );
  }

  function renderPermissionsSection() {
    return (
      <div>
        {permissions && (
          <div style={styles.permBox}>
            {renderPermissionStatus(
              "Accessibility",
              permissions.accessibility
            )}
            {renderPermissionStatus(
              "Input Monitoring",
              permissions.input_monitoring
            )}
          </div>
        )}

        {permInstructions.length > 0 && (
          <div style={styles.instructionBox}>
            <div style={styles.infoTitle}>Setup Required</div>
            {permInstructions.map((instruction, i) => (
              <p key={i} style={styles.instruction}>
                {i + 1}. {instruction}
              </p>
            ))}
            <button onClick={loadPermissions} style={styles.recheckBtn}>
              Re-check Permissions
            </button>
          </div>
        )}

        {permissions &&
          permissions.accessibility === "Granted" &&
          permissions.input_monitoring === "Granted" && (
            <div style={styles.allGoodBox}>
              <span style={styles.allGoodCheck}>&#10003;</span>
              All permissions granted
            </div>
          )}
      </div>
    );
  }

  return (
    <div>
      <h3 style={styles.title}>Settings</h3>

      <div style={styles.sectionTabs}>
        {renderSectionTab("general", "General")}
        {renderSectionTab("network", "Network")}
        {renderSectionTab("permissions", "Permissions")}
      </div>

      <div style={styles.sectionContent}>
        {activeSection === "general" && renderGeneralSection()}
        {activeSection === "network" && renderNetworkSection()}
        {activeSection === "permissions" && renderPermissionsSection()}
      </div>

      {activeSection !== "permissions" && (
        <button
          onClick={handleSave}
          style={{
            ...styles.saveBtn,
            ...(saved ? styles.saveBtnSaved : {}),
          }}
        >
          {saved ? "Saved" : "Save Settings"}
        </button>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  title: {
    fontSize: "16px",
    fontWeight: 600,
    marginBottom: "16px",
    color: "#e0e0e0",
  },
  sectionTabs: {
    display: "flex",
    gap: "4px",
    marginBottom: "20px",
    background: "#16213e",
    borderRadius: "8px",
    padding: "3px",
  },
  sectionTab: {
    flex: 1,
    padding: "8px",
    borderRadius: "6px",
    border: "none",
    background: "transparent",
    color: "#888",
    fontSize: "12px",
    fontWeight: 500,
    cursor: "pointer",
  },
  sectionTabActive: {
    background: "#6c63ff",
    color: "#fff",
  },
  sectionContent: {
    minHeight: "200px",
  },
  field: {
    marginBottom: "16px",
  },
  label: {
    display: "block",
    fontSize: "12px",
    color: "#888",
    marginBottom: "4px",
    fontWeight: 500,
  },
  input: {
    width: "100%",
    padding: "8px 12px",
    borderRadius: "6px",
    border: "1px solid #444",
    background: "#16213e",
    color: "#e0e0e0",
    fontSize: "14px",
    outline: "none",
  },
  fieldHint: {
    display: "block",
    fontSize: "11px",
    color: "#555",
    marginTop: "4px",
  },
  row: {
    display: "flex",
    gap: "12px",
  },
  infoBox: {
    background: "#16213e",
    borderRadius: "8px",
    padding: "14px",
    marginTop: "8px",
  },
  infoTitle: {
    fontSize: "12px",
    fontWeight: 600,
    color: "#aaa",
    marginBottom: "10px",
    textTransform: "uppercase" as const,
    letterSpacing: "0.5px",
  },
  infoRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 0",
  },
  infoLabel: {
    fontSize: "13px",
    color: "#888",
  },
  infoValue: {
    fontSize: "13px",
    color: "#e0e0e0",
    fontWeight: 500,
  },
  permBox: {
    background: "#16213e",
    borderRadius: "8px",
    padding: "14px",
    marginBottom: "12px",
  },
  permRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "8px 0",
  },
  permInfo: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  permDot: {
    width: 8,
    height: 8,
    borderRadius: "50%",
    flexShrink: 0,
  },
  permLabel: {
    fontSize: "13px",
    color: "#e0e0e0",
  },
  permStatus: {
    fontSize: "12px",
    fontWeight: 500,
  },
  instructionBox: {
    background: "#16213e",
    borderRadius: "8px",
    padding: "14px",
    marginBottom: "12px",
  },
  instruction: {
    fontSize: "12px",
    color: "#aaa",
    lineHeight: 1.6,
    marginBottom: "4px",
  },
  recheckBtn: {
    marginTop: "12px",
    padding: "8px 16px",
    borderRadius: "6px",
    border: "1px solid #444",
    background: "transparent",
    color: "#aaa",
    fontSize: "12px",
    cursor: "pointer",
  },
  allGoodBox: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "8px",
    padding: "16px",
    background: "rgba(76, 175, 80, 0.1)",
    borderRadius: "8px",
    color: "#4caf50",
    fontSize: "13px",
    fontWeight: 500,
  },
  allGoodCheck: {
    fontSize: "16px",
  },
  saveBtn: {
    width: "100%",
    padding: "12px",
    borderRadius: "8px",
    border: "none",
    background: "#6c63ff",
    color: "#fff",
    fontSize: "14px",
    fontWeight: 600,
    cursor: "pointer",
    marginTop: "16px",
  },
  saveBtnSaved: {
    background: "#4caf50",
  },
};

export default Settings;
