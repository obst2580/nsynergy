import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SettingsData {
  machine_name: string;
  udp_port: number;
  tcp_port: number;
  edge_threshold: number;
}

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

  useEffect(() => {
    loadSettings();
  }, []);

  async function loadSettings() {
    try {
      const result = await invoke<SettingsData>("get_settings");
      setSettings(result);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function handleSave() {
    try {
      await invoke("save_settings", { settings });
      setSaved(true);
      onSave();
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  return (
    <div>
      <h3 style={styles.title}>Settings</h3>

      <div style={styles.field}>
        <label style={styles.label}>Machine Name</label>
        <input
          style={styles.input}
          value={settings.machine_name}
          onChange={(e) =>
            setSettings({ ...settings, machine_name: e.target.value })
          }
        />
      </div>

      <div style={styles.row}>
        <div style={styles.field}>
          <label style={styles.label}>UDP Port</label>
          <input
            style={styles.input}
            type="number"
            value={settings.udp_port}
            onChange={(e) =>
              setSettings({ ...settings, udp_port: parseInt(e.target.value) || 0 })
            }
          />
        </div>
        <div style={styles.field}>
          <label style={styles.label}>TCP Port</label>
          <input
            style={styles.input}
            type="number"
            value={settings.tcp_port}
            onChange={(e) =>
              setSettings({ ...settings, tcp_port: parseInt(e.target.value) || 0 })
            }
          />
        </div>
      </div>

      <div style={styles.field}>
        <label style={styles.label}>Edge Threshold (px)</label>
        <input
          style={styles.input}
          type="number"
          value={settings.edge_threshold}
          onChange={(e) =>
            setSettings({
              ...settings,
              edge_threshold: parseInt(e.target.value) || 0,
            })
          }
        />
      </div>

      <button onClick={handleSave} style={styles.saveBtn}>
        {saved ? "Saved" : "Save Settings"}
      </button>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  title: {
    fontSize: "16px",
    fontWeight: 600,
    marginBottom: "16px",
  },
  field: {
    marginBottom: "16px",
    flex: 1,
  },
  label: {
    display: "block",
    fontSize: "12px",
    color: "#888",
    marginBottom: "4px",
  },
  input: {
    width: "100%",
    padding: "8px 12px",
    borderRadius: "4px",
    border: "1px solid #444",
    background: "#16213e",
    color: "#e0e0e0",
    fontSize: "14px",
    outline: "none",
  },
  row: {
    display: "flex",
    gap: "12px",
  },
  saveBtn: {
    width: "100%",
    padding: "10px",
    borderRadius: "6px",
    border: "none",
    background: "#6c63ff",
    color: "#fff",
    fontSize: "14px",
    fontWeight: 600,
    cursor: "pointer",
    marginTop: "8px",
  },
};

export default Settings;
