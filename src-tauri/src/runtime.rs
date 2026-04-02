use anyhow::{Context, Result};
use nsynergy_client::client::{ClientConfig, ClientHandle, ClientStatus};
use nsynergy_core::capture::{self, CaptureHandle};
use nsynergy_core::config::{AppConfig, ScreenPosition};
use nsynergy_core::discovery::DiscoveryEvent;
use nsynergy_core::inject::EnigoInjector;
use nsynergy_core::screen;
use nsynergy_net::reconnect::ReconnectConfig;
use nsynergy_server::server::{ServerConfig, ServerHandle, ServerStatus};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{error, info, warn};

/// Info about a device connected through the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedDeviceInfo {
    pub name: String,
    pub address: String,
}

/// Info about a discovered peer on the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub name: String,
    pub address: String,
    pub tcp_port: u16,
    pub udp_port: u16,
}

/// The application runtime that orchestrates all backend services.
///
/// This struct manages the server, client, input capture, and discovery
/// subsystems. It is owned by the Tauri AppState behind a Mutex.
pub struct AppRuntime {
    server_handle: Option<ServerHandle>,
    client_handle: Option<ClientHandle>,
    capture_handle: Option<CaptureHandle>,
    is_connected: bool,
    connected_devices: Vec<ConnectedDeviceInfo>,
    server_name: Option<String>,
}

impl AppRuntime {
    pub fn new() -> Self {
        Self {
            server_handle: None,
            client_handle: None,
            capture_handle: None,
            is_connected: false,
            connected_devices: Vec::new(),
            server_name: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn connected_devices(&self) -> &[ConnectedDeviceInfo] {
        &self.connected_devices
    }

    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    /// Starts the app as a server: begins input capture and starts
    /// the TCP/UDP server that clients connect to.
    pub async fn start_as_server(&mut self, config: &AppConfig) -> Result<()> {
        self.stop().await;

        // Start input capture (rdev::listen on a native thread)
        let capture = capture::start_capture()
            .context("starting input capture")?;
        let capture_rx = capture.into_receiver();

        let server_config = ServerConfig::from(config);
        let server_handle = nsynergy_server::server::start_server(server_config, capture_rx)
            .await
            .context("starting server")?;

        // We don't store capture_handle here because into_receiver() consumed it.
        // The capture thread will stop when the server drops the receiver.
        self.server_handle = Some(server_handle);
        self.is_connected = true;

        info!("server started");
        Ok(())
    }

    /// Starts the app as a client: connects to the given server address,
    /// creates an input injector, and spawns the client event loop.
    pub async fn start_as_client(
        &mut self,
        server_addr: SocketAddr,
        config: &AppConfig,
    ) -> Result<()> {
        self.stop().await;

        let local_display = screen::primary_display();

        let client_config = ClientConfig {
            server_addr,
            client_name: config.machine_name.clone(),
            position: ScreenPosition::Right,
            local_display: local_display.clone(),
            udp_port: config.udp_port,
            reconnect: ReconnectConfig::default(),
        };

        let (client_handle, event_rx) = nsynergy_client::client::start_client(client_config)
            .await
            .context("starting client")?;

        // Spawn the injection loop on a std::thread because
        // enigo::Enigo is !Send on macOS (CGEventSource).
        std::thread::Builder::new()
            .name("input-injector".to_string())
            .spawn(move || {
                let injector = match EnigoInjector::new() {
                    Ok(inj) => inj,
                    Err(e) => {
                        error!(error = %e, "failed to create input injector");
                        return;
                    }
                };
                let handler = nsynergy_client::handler::ClientHandler::new(
                    Box::new(injector),
                    local_display,
                );

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio runtime for injector");

                rt.block_on(nsynergy_client::handler::run_client_loop(handler, event_rx));
            })
            .context("spawning injector thread")?;

        self.client_handle = Some(client_handle);
        self.is_connected = true;

        info!(%server_addr, "client started");
        Ok(())
    }

    /// Scans the local network for nsynergy peers via mDNS.
    /// Collects results for a short duration and returns discovered peers.
    pub async fn scan_network(&self) -> Result<Vec<DiscoveredPeer>> {
        let mut discovery_rx = nsynergy_client::client::start_discovery()
            .context("starting mDNS discovery")?;

        let mut peers = Vec::new();

        // Collect peers for 3 seconds
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            tokio::select! {
                event = discovery_rx.recv() => {
                    match event {
                        Some(DiscoveryEvent::PeerFound(peer)) => {
                            peers.push(DiscoveredPeer {
                                name: peer.name.clone(),
                                address: peer.address.to_string(),
                                tcp_port: peer.tcp_port,
                                udp_port: peer.udp_port,
                            });
                        }
                        Some(DiscoveryEvent::PeerLost(_)) => {}
                        None => break,
                    }
                }
                _ = tokio::time::sleep_until(deadline) => {
                    break;
                }
            }
        }

        info!(count = peers.len(), "network scan complete");
        Ok(peers)
    }

    /// Polls the server handle for status updates and updates internal state.
    /// Call this periodically or after operations to sync connected_devices.
    pub fn poll_server_status(&mut self) {
        if let Some(ref mut handle) = self.server_handle {
            while let Ok(status) = handle.status_rx.try_recv() {
                match status {
                    ServerStatus::ClientConnected { name, addr } => {
                        info!(%name, %addr, "client connected to server");
                        self.connected_devices.push(ConnectedDeviceInfo {
                            name,
                            address: addr.to_string(),
                        });
                    }
                    ServerStatus::ClientDisconnected { name } => {
                        info!(%name, "client disconnected from server");
                        self.connected_devices.retain(|d| d.name != name);
                    }
                    ServerStatus::Error(e) => {
                        warn!(error = %e, "server error");
                    }
                    ServerStatus::Listening { tcp_addr, udp_addr } => {
                        info!(%tcp_addr, %udp_addr, "server listening");
                    }
                }
            }
        }
    }

    /// Polls the client handle for status updates.
    pub fn poll_client_status(&mut self) {
        if let Some(ref mut handle) = self.client_handle {
            while let Ok(status) = handle.status_rx.try_recv() {
                match status {
                    ClientStatus::Connected { server_name, .. } => {
                        info!(%server_name, "connected to server");
                        self.is_connected = true;
                        self.server_name = Some(server_name);
                    }
                    ClientStatus::Disconnected { reason } => {
                        info!(%reason, "disconnected from server");
                        self.is_connected = false;
                        self.server_name = None;
                    }
                    ClientStatus::Reconnecting { attempt } => {
                        info!(attempt, "reconnecting to server");
                        self.is_connected = false;
                    }
                    ClientStatus::Error(e) => {
                        warn!(error = %e, "client error");
                        self.is_connected = false;
                    }
                    ClientStatus::Connecting { server_addr } => {
                        info!(%server_addr, "connecting to server");
                    }
                }
            }
        }
    }

    /// Stops all running services.
    pub async fn stop(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            handle.shutdown();
            info!("server shut down");
        }

        if let Some(handle) = self.client_handle.take() {
            handle.shutdown();
            info!("client shut down");
        }

        // CaptureHandle is consumed by into_receiver(), so the capture
        // thread stops when the server drops the receiver channel.
        self.capture_handle = None;
        self.is_connected = false;
        self.connected_devices.clear();
        self.server_name = None;
    }
}
