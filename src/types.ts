export interface Device {
  name: string;
  address: string;
  position: string;
  connected: boolean;
}

export interface AppState {
  role: "Server" | "Client";
  machine_name: string;
  connected: boolean;
  devices: Device[];
}

export interface SettingsData {
  machine_name: string;
  udp_port: number;
  tcp_port: number;
  edge_threshold: number;
}

export interface PermissionCheck {
  accessibility: "Granted" | "Denied" | "NotApplicable";
  input_monitoring: "Granted" | "Denied" | "NotApplicable";
}

export type ConnectionStatus = "disconnected" | "connecting" | "pairing" | "connected";

export interface DiscoveredPeer {
  name: string;
  address: string;
  tcp_port: number;
  udp_port: number;
}
