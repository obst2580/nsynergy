import React from "react";
import type { ConnectionStatus } from "../types";

interface ConnectionIndicatorProps {
  status: ConnectionStatus;
  label?: string;
  size?: "small" | "medium";
}

function ConnectionIndicator({
  status,
  label,
  size = "small",
}: ConnectionIndicatorProps) {
  const config = STATUS_CONFIG[status];
  const dotSize = size === "small" ? 8 : 10;

  return (
    <div style={styles.container}>
      <div
        style={{
          width: dotSize,
          height: dotSize,
          borderRadius: "50%",
          background: config.color,
          flexShrink: 0,
          boxShadow:
            status === "connected"
              ? `0 0 6px ${config.color}`
              : "none",
        }}
      />
      {label && (
        <span
          style={{
            fontSize: size === "small" ? "11px" : "12px",
            color: config.color,
            fontWeight: 500,
          }}
        >
          {label}
        </span>
      )}
    </div>
  );
}

const STATUS_CONFIG: Record<
  ConnectionStatus,
  { color: string; label: string }
> = {
  disconnected: { color: "#666", label: "Offline" },
  connecting: { color: "#ff9800", label: "Connecting" },
  pairing: { color: "#6c63ff", label: "Pairing" },
  connected: { color: "#4caf50", label: "Connected" },
};

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
};

export default ConnectionIndicator;
