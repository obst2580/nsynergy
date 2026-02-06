import React from "react";

interface Device {
  name: string;
  address: string;
  position: string;
  connected: boolean;
}

interface ScreenLayoutProps {
  devices: Device[];
}

function ScreenLayout({ devices }: ScreenLayoutProps) {
  const deviceAt = (pos: string) =>
    devices.find((d) => d.position === pos);

  return (
    <div>
      <h3 style={styles.title}>Screen Layout</h3>
      <p style={styles.desc}>
        Configure which device is on each side of your screen.
      </p>

      <div style={styles.grid}>
        {/* Top */}
        <div style={{ ...styles.cell, gridColumn: "2" }}>
          <SlotBox position="Top" device={deviceAt("Top")} />
        </div>

        {/* Left */}
        <div style={{ ...styles.cell, gridColumn: "1", gridRow: "2" }}>
          <SlotBox position="Left" device={deviceAt("Left")} />
        </div>

        {/* Center (this machine) */}
        <div style={{ ...styles.cell, gridColumn: "2", gridRow: "2" }}>
          <div style={styles.center}>This Machine</div>
        </div>

        {/* Right */}
        <div style={{ ...styles.cell, gridColumn: "3", gridRow: "2" }}>
          <SlotBox position="Right" device={deviceAt("Right")} />
        </div>

        {/* Bottom */}
        <div style={{ ...styles.cell, gridColumn: "2", gridRow: "3" }}>
          <SlotBox position="Bottom" device={deviceAt("Bottom")} />
        </div>
      </div>
    </div>
  );
}

function SlotBox({
  position,
  device,
}: {
  position: string;
  device?: Device;
}) {
  return (
    <div style={styles.slot}>
      <div style={styles.slotLabel}>{position}</div>
      {device ? (
        <div style={styles.slotDevice}>{device.name}</div>
      ) : (
        <div style={styles.slotEmpty}>Empty</div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  title: {
    fontSize: "16px",
    fontWeight: 600,
    marginBottom: "8px",
  },
  desc: {
    fontSize: "12px",
    color: "#888",
    marginBottom: "24px",
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "1fr 1fr 1fr",
    gridTemplateRows: "1fr 1fr 1fr",
    gap: "8px",
    maxWidth: "360px",
    margin: "0 auto",
  },
  cell: {},
  center: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    height: "80px",
    background: "#6c63ff",
    borderRadius: "8px",
    fontSize: "13px",
    fontWeight: 600,
    color: "#fff",
  },
  slot: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    height: "80px",
    background: "#16213e",
    borderRadius: "8px",
    border: "1px dashed #444",
  },
  slotLabel: {
    fontSize: "10px",
    color: "#666",
    marginBottom: "4px",
    textTransform: "uppercase",
    letterSpacing: "1px",
  },
  slotDevice: {
    fontSize: "12px",
    color: "#e0e0e0",
    fontWeight: 500,
  },
  slotEmpty: {
    fontSize: "12px",
    color: "#444",
  },
};

export default ScreenLayout;
