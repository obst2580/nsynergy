import React from "react";

interface StatusBarProps {
  role: "Server" | "Client";
  machineName: string;
  connected: boolean;
  onToggleRole: () => void;
}

function StatusBar({ role, machineName, connected, onToggleRole }: StatusBarProps) {
  return (
    <div style={styles.bar}>
      <div style={styles.left}>
        <span style={styles.dot(connected)} />
        <span style={styles.name}>{machineName}</span>
      </div>
      <button onClick={onToggleRole} style={styles.roleBtn}>
        {role}
      </button>
    </div>
  );
}

const styles = {
  bar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px 16px",
    background: "#16213e",
    borderBottom: "1px solid #333",
  } as React.CSSProperties,
  left: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  } as React.CSSProperties,
  dot: (connected: boolean): React.CSSProperties => ({
    width: 8,
    height: 8,
    borderRadius: "50%",
    background: connected ? "#4caf50" : "#666",
  }),
  name: {
    fontSize: "14px",
    fontWeight: 600,
  } as React.CSSProperties,
  roleBtn: {
    padding: "4px 12px",
    borderRadius: "4px",
    border: "1px solid #6c63ff",
    background: "transparent",
    color: "#6c63ff",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 600,
  } as React.CSSProperties,
};

export default StatusBar;
