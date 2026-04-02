import React from "react";

interface StatusBarProps {
  role: "Server" | "Client";
  machineName: string;
  connected: boolean;
  connectedCount: number;
  totalCount: number;
  onToggleRole: () => void;
}

function StatusBar({
  role,
  machineName,
  connected,
  connectedCount,
  totalCount,
  onToggleRole,
}: StatusBarProps) {
  const networkLabel =
    connectedCount > 0
      ? `${connectedCount}/${totalCount} devices`
      : totalCount > 0
        ? `${totalCount} discovered`
        : "No devices";

  return (
    <div style={styles.bar}>
      <div style={styles.left}>
        <div style={styles.statusGroup}>
          <div
            style={{
              ...styles.dot,
              background: connected ? "#4caf50" : "#666",
              boxShadow: connected ? "0 0 6px #4caf50" : "none",
            }}
          />
          <div>
            <div style={styles.name}>{machineName}</div>
            <div style={styles.networkLabel}>{networkLabel}</div>
          </div>
        </div>
      </div>
      <div style={styles.right}>
        <button onClick={onToggleRole} style={styles.roleBtn}>
          {role}
        </button>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  bar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px 16px",
    background: "#16213e",
    borderBottom: "1px solid #333",
  },
  left: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  statusGroup: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  },
  dot: {
    width: 10,
    height: 10,
    borderRadius: "50%",
    flexShrink: 0,
  },
  name: {
    fontSize: "14px",
    fontWeight: 600,
    color: "#e0e0e0",
  },
  networkLabel: {
    fontSize: "11px",
    color: "#888",
    marginTop: "1px",
  },
  right: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  roleBtn: {
    padding: "4px 12px",
    borderRadius: "4px",
    border: "1px solid #6c63ff",
    background: "transparent",
    color: "#6c63ff",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 600,
  },
};

export default StatusBar;
