import React, { useState } from "react";
import type { Device, ConnectionStatus } from "../types";
import PairingDialog from "./PairingDialog";

interface DeviceListProps {
  devices: Device[];
  onRefresh: () => void;
}

interface DeviceConnectionState {
  [deviceName: string]: ConnectionStatus;
}

function DeviceList({ devices, onRefresh }: DeviceListProps) {
  const [connectionStates, setConnectionStates] = useState<DeviceConnectionState>({});
  const [pairingTarget, setPairingTarget] = useState<{
    device: Device;
    mode: "host" | "join";
  } | null>(null);
  const [scanning, setScanning] = useState(false);

  function getStatus(device: Device): ConnectionStatus {
    return connectionStates[device.name] ?? (device.connected ? "connected" : "disconnected");
  }

  function setDeviceStatus(deviceName: string, status: ConnectionStatus) {
    setConnectionStates((prev) => ({ ...prev, [deviceName]: status }));
  }

  async function handleConnect(device: Device) {
    setDeviceStatus(device.name, "connecting");

    // Simulate connection attempt - in production this would call
    // invoke("connect_device", { address: device.address })
    await new Promise((resolve) => setTimeout(resolve, 1000));

    // After connection attempt, start pairing flow
    setDeviceStatus(device.name, "pairing");
    setPairingTarget({ device, mode: "join" });
  }

  function handleDisconnect(device: Device) {
    setDeviceStatus(device.name, "disconnected");
  }

  function handlePairDevice(device: Device) {
    setPairingTarget({ device, mode: "host" });
  }

  function handlePairingComplete() {
    if (pairingTarget) {
      setDeviceStatus(pairingTarget.device.name, "connected");
    }
    setPairingTarget(null);
  }

  function handlePairingClose() {
    if (pairingTarget) {
      const current = getStatus(pairingTarget.device);
      if (current !== "connected") {
        setDeviceStatus(pairingTarget.device.name, "disconnected");
      }
    }
    setPairingTarget(null);
  }

  async function handleScan() {
    setScanning(true);
    onRefresh();
    // Give the refresh a moment to show the scanning state
    await new Promise((resolve) => setTimeout(resolve, 1500));
    setScanning(false);
  }

  function renderStatusBadge(status: ConnectionStatus) {
    const config = {
      disconnected: { color: "#666", bg: "transparent", text: "Offline" },
      connecting: { color: "#ff9800", bg: "rgba(255, 152, 0, 0.1)", text: "Connecting..." },
      pairing: { color: "#6c63ff", bg: "rgba(108, 99, 255, 0.1)", text: "Pairing..." },
      connected: { color: "#4caf50", bg: "rgba(76, 175, 80, 0.1)", text: "Connected" },
    }[status];

    return (
      <span
        style={{
          fontSize: "11px",
          color: config.color,
          background: config.bg,
          padding: "2px 8px",
          borderRadius: "10px",
          fontWeight: 500,
        }}
      >
        {config.text}
      </span>
    );
  }

  function renderActions(device: Device) {
    const status = getStatus(device);

    if (status === "connecting" || status === "pairing") {
      return null;
    }

    if (status === "connected") {
      return (
        <button
          onClick={() => handleDisconnect(device)}
          style={styles.actionBtn}
        >
          Disconnect
        </button>
      );
    }

    return (
      <div style={styles.actions}>
        <button
          onClick={() => handleConnect(device)}
          style={styles.connectBtn}
        >
          Connect
        </button>
        <button
          onClick={() => handlePairDevice(device)}
          style={styles.actionBtn}
        >
          Pair
        </button>
      </div>
    );
  }

  return (
    <div>
      <div style={styles.header}>
        <h3 style={styles.title}>Discovered Devices</h3>
        <button
          onClick={handleScan}
          disabled={scanning}
          style={{
            ...styles.refreshBtn,
            ...(scanning ? styles.refreshBtnDisabled : {}),
          }}
        >
          {scanning ? "Scanning..." : "Scan"}
        </button>
      </div>

      {devices.length === 0 ? (
        <div style={styles.empty}>
          <div style={styles.emptyIcon}>&#9881;</div>
          <p style={styles.emptyTitle}>No devices found</p>
          <p style={styles.emptyHint}>
            Make sure nsynergy is running on other machines on the same network.
          </p>
          <button onClick={handleScan} style={styles.scanBtn}>
            Scan Network
          </button>
        </div>
      ) : (
        <div style={styles.list}>
          {devices.map((device) => {
            const status = getStatus(device);
            return (
              <div
                key={device.name}
                style={{
                  ...styles.card,
                  ...(status === "connected" ? styles.cardConnected : {}),
                }}
              >
                <div style={styles.cardTop}>
                  <div style={styles.cardInfo}>
                    <span style={statusDotStyle(status)} />
                    <div>
                      <div style={styles.deviceName}>{device.name}</div>
                      <div style={styles.deviceAddr}>{device.address}</div>
                    </div>
                  </div>
                  {renderStatusBadge(status)}
                </div>
                <div style={styles.cardBottom}>
                  <span style={styles.position}>{device.position}</span>
                  {renderActions(device)}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {pairingTarget && (
        <PairingDialog
          mode={pairingTarget.mode}
          deviceName={pairingTarget.device.name}
          onClose={handlePairingClose}
          onPaired={handlePairingComplete}
        />
      )}
    </div>
  );
}

function statusDotStyle(status: ConnectionStatus): React.CSSProperties {
  const colorMap: Record<ConnectionStatus, string> = {
    disconnected: "#666",
    connecting: "#ff9800",
    pairing: "#6c63ff",
    connected: "#4caf50",
  };
  return {
    width: 10,
    height: 10,
    borderRadius: "50%",
    background: colorMap[status],
    flexShrink: 0,
  };
}

const styles: Record<string, React.CSSProperties> = {
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "16px",
  },
  title: {
    fontSize: "16px",
    fontWeight: 600,
    color: "#e0e0e0",
    margin: 0,
  },
  refreshBtn: {
    padding: "6px 14px",
    borderRadius: "6px",
    border: "1px solid #444",
    background: "transparent",
    color: "#aaa",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 500,
  },
  refreshBtnDisabled: {
    opacity: 0.5,
    cursor: "not-allowed",
  },
  empty: {
    textAlign: "center" as const,
    padding: "48px 20px",
  },
  emptyIcon: {
    fontSize: "32px",
    color: "#444",
    marginBottom: "12px",
  },
  emptyTitle: {
    fontSize: "15px",
    color: "#888",
    fontWeight: 500,
    marginBottom: "8px",
  },
  emptyHint: {
    fontSize: "12px",
    color: "#555",
    marginBottom: "20px",
    lineHeight: 1.5,
  },
  scanBtn: {
    padding: "10px 24px",
    borderRadius: "8px",
    border: "none",
    background: "#6c63ff",
    color: "#fff",
    fontSize: "13px",
    fontWeight: 600,
    cursor: "pointer",
  },
  list: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "10px",
  },
  card: {
    padding: "14px",
    background: "#16213e",
    borderRadius: "10px",
    border: "1px solid #333",
  },
  cardConnected: {
    borderColor: "rgba(76, 175, 80, 0.3)",
  },
  cardTop: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "10px",
  },
  cardInfo: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  },
  deviceName: {
    fontSize: "14px",
    fontWeight: 500,
    color: "#e0e0e0",
  },
  deviceAddr: {
    fontSize: "11px",
    color: "#888",
    marginTop: "2px",
  },
  cardBottom: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
  },
  position: {
    fontSize: "12px",
    color: "#6c63ff",
    fontWeight: 500,
  },
  actions: {
    display: "flex",
    gap: "6px",
  },
  connectBtn: {
    padding: "5px 12px",
    borderRadius: "6px",
    border: "none",
    background: "#6c63ff",
    color: "#fff",
    fontSize: "12px",
    fontWeight: 500,
    cursor: "pointer",
  },
  actionBtn: {
    padding: "5px 12px",
    borderRadius: "6px",
    border: "1px solid #444",
    background: "transparent",
    color: "#aaa",
    fontSize: "12px",
    fontWeight: 500,
    cursor: "pointer",
  },
};

export default DeviceList;
