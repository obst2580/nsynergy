import React from "react";

interface Device {
  name: string;
  address: string;
  position: string;
  connected: boolean;
}

interface DeviceListProps {
  devices: Device[];
  onRefresh: () => void;
}

function DeviceList({ devices, onRefresh }: DeviceListProps) {
  return (
    <div>
      <div style={styles.header}>
        <h3 style={styles.title}>Discovered Devices</h3>
        <button onClick={onRefresh} style={styles.refreshBtn}>
          Refresh
        </button>
      </div>

      {devices.length === 0 ? (
        <div style={styles.empty}>
          <p>No devices found on the network.</p>
          <p style={styles.hint}>
            Make sure nsynergy is running on other machines.
          </p>
        </div>
      ) : (
        <div style={styles.list}>
          {devices.map((device) => (
            <div key={device.name} style={styles.card}>
              <div style={styles.cardLeft}>
                <span style={styles.deviceDot(device.connected)} />
                <div>
                  <div style={styles.deviceName}>{device.name}</div>
                  <div style={styles.deviceAddr}>{device.address}</div>
                </div>
              </div>
              <span style={styles.position}>{device.position}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

const styles = {
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "16px",
  } as React.CSSProperties,
  title: {
    fontSize: "16px",
    fontWeight: 600,
    color: "#e0e0e0",
  } as React.CSSProperties,
  refreshBtn: {
    padding: "6px 12px",
    borderRadius: "4px",
    border: "1px solid #444",
    background: "transparent",
    color: "#aaa",
    cursor: "pointer",
    fontSize: "12px",
  } as React.CSSProperties,
  empty: {
    textAlign: "center" as const,
    padding: "40px 0",
    color: "#666",
  } as React.CSSProperties,
  hint: {
    fontSize: "12px",
    marginTop: "8px",
    color: "#555",
  } as React.CSSProperties,
  list: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "8px",
  } as React.CSSProperties,
  card: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px",
    background: "#16213e",
    borderRadius: "8px",
    border: "1px solid #333",
  } as React.CSSProperties,
  cardLeft: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  } as React.CSSProperties,
  deviceDot: (connected: boolean): React.CSSProperties => ({
    width: 10,
    height: 10,
    borderRadius: "50%",
    background: connected ? "#4caf50" : "#f44336",
    flexShrink: 0,
  }),
  deviceName: {
    fontSize: "14px",
    fontWeight: 500,
  } as React.CSSProperties,
  deviceAddr: {
    fontSize: "11px",
    color: "#888",
    marginTop: "2px",
  } as React.CSSProperties,
  position: {
    fontSize: "12px",
    color: "#6c63ff",
    fontWeight: 500,
  } as React.CSSProperties,
};

export default DeviceList;
