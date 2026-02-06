import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import StatusBar from "./components/StatusBar";
import DeviceList from "./components/DeviceList";
import ScreenLayout from "./components/ScreenLayout";
import Settings from "./components/Settings";

interface AppState {
  role: "Server" | "Client";
  machine_name: string;
  connected: boolean;
  devices: Device[];
}

interface Device {
  name: string;
  address: string;
  position: string;
  connected: boolean;
}

type Tab = "devices" | "layout" | "settings";

function App() {
  const [state, setState] = useState<AppState>({
    role: "Server",
    machine_name: "loading...",
    connected: false,
    devices: [],
  });
  const [tab, setTab] = useState<Tab>("devices");

  useEffect(() => {
    loadState();
  }, []);

  async function loadState() {
    try {
      const result = await invoke<AppState>("get_app_state");
      setState(result);
    } catch (e) {
      console.error("Failed to load state:", e);
    }
  }

  async function toggleRole() {
    try {
      const newRole = state.role === "Server" ? "Client" : "Server";
      await invoke("set_role", { role: newRole });
      setState({ ...state, role: newRole });
    } catch (e) {
      console.error("Failed to toggle role:", e);
    }
  }

  return (
    <div style={styles.container}>
      <StatusBar
        role={state.role}
        machineName={state.machine_name}
        connected={state.connected}
        onToggleRole={toggleRole}
      />

      <nav style={styles.nav}>
        {(["devices", "layout", "settings"] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            style={{
              ...styles.navBtn,
              ...(tab === t ? styles.navBtnActive : {}),
            }}
          >
            {t === "devices" ? "Devices" : t === "layout" ? "Layout" : "Settings"}
          </button>
        ))}
      </nav>

      <main style={styles.main}>
        {tab === "devices" && (
          <DeviceList devices={state.devices} onRefresh={loadState} />
        )}
        {tab === "layout" && <ScreenLayout devices={state.devices} />}
        {tab === "settings" && <Settings onSave={loadState} />}
      </main>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    background: "#1a1a2e",
  },
  nav: {
    display: "flex",
    gap: 0,
    borderBottom: "1px solid #333",
  },
  navBtn: {
    flex: 1,
    padding: "10px",
    background: "transparent",
    color: "#888",
    border: "none",
    borderBottom: "2px solid transparent",
    cursor: "pointer",
    fontSize: "13px",
    fontWeight: 500,
  },
  navBtnActive: {
    color: "#e0e0e0",
    borderBottomColor: "#6c63ff",
  },
  main: {
    flex: 1,
    overflow: "auto",
    padding: "16px",
  },
};

export default App;
