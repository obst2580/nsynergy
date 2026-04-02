use anyhow::{Context, Result};
use nsynergy_core::config::{AppConfig, ScreenPosition};
use nsynergy_core::discovery;
use nsynergy_core::event::TimestampedEvent;
use nsynergy_core::protocol;
use nsynergy_core::screen::{self, DisplayInfo};
use nsynergy_net::tcp::{self, TcpTransport};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, Notify};
use tracing::{debug, error, info, warn};

use crate::handler::{ConnectedClient, EventRouter};

/// Message types exchanged between server and clients over TCP.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ServerMessage {
    /// Client announces itself when connecting.
    Hello {
        name: String,
        position: ScreenPosition,
        display: DisplayInfo,
        udp_port: u16,
    },
    /// Server acknowledges the client connection.
    Welcome {
        server_name: String,
        server_display: DisplayInfo,
    },
    /// Server notifies the client it is being disconnected.
    Goodbye,
    /// Heartbeat ping.
    Ping,
    /// Heartbeat pong.
    Pong,
}

/// Tracks a TCP-connected client session.
#[derive(Debug)]
#[allow(dead_code)]
struct ClientSession {
    name: String,
    position: ScreenPosition,
    display_info: DisplayInfo,
    udp_addr: SocketAddr,
    tcp_addr: SocketAddr,
}

/// Configuration for the server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub udp_port: u16,
    pub tcp_port: u16,
    pub machine_name: String,
    pub local_display: DisplayInfo,
    pub edge_threshold: u32,
    /// Whether to register on mDNS for auto-discovery.
    pub enable_mdns: bool,
}

impl From<&AppConfig> for ServerConfig {
    fn from(app: &AppConfig) -> Self {
        Self {
            udp_port: app.udp_port,
            tcp_port: app.tcp_port,
            machine_name: app.machine_name.clone(),
            local_display: screen::primary_display(),
            edge_threshold: app.edge_threshold,
            enable_mdns: true,
        }
    }
}

/// Handle returned by `start_server` to control the running server.
pub struct ServerHandle {
    shutdown: Arc<Notify>,
    /// Receives status updates from the server.
    pub status_rx: mpsc::UnboundedReceiver<ServerStatus>,
}

impl ServerHandle {
    /// Signals the server to shut down gracefully.
    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }
}

/// Status updates emitted by the server.
#[derive(Debug, Clone)]
pub enum ServerStatus {
    Listening { tcp_addr: SocketAddr, udp_addr: SocketAddr },
    ClientConnected { name: String, addr: SocketAddr },
    ClientDisconnected { name: String },
    Error(String),
}

/// Starts the server in the background.
///
/// Returns a handle for controlling the server and receiving status updates.
/// The server listens for TCP client connections and routes captured
/// input events to connected clients via UDP.
pub async fn start_server(
    config: ServerConfig,
    mut capture_rx: mpsc::UnboundedReceiver<TimestampedEvent>,
) -> Result<ServerHandle> {
    let shutdown = Arc::new(Notify::new());
    let (status_tx, status_rx) = mpsc::unbounded_channel();

    // Bind TCP listener
    let tcp_bind = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, config.tcp_port));
    let tcp_transport = TcpTransport::bind(tcp_bind)
        .await
        .context("binding TCP transport")?;
    let tcp_addr = tcp_transport.local_addr()?;

    // Bind UDP socket for forwarding events
    let udp_bind = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, config.udp_port));
    let udp_socket = tokio::net::UdpSocket::bind(udp_bind)
        .await
        .context("binding UDP socket")?;
    let udp_addr = udp_socket.local_addr()?;
    let udp_socket = Arc::new(udp_socket);

    info!(%tcp_addr, %udp_addr, "server listening");
    let _ = status_tx.send(ServerStatus::Listening { tcp_addr, udp_addr });

    // Register mDNS service for auto-discovery
    let _mdns_registration = if config.enable_mdns {
        match discovery::ServiceRegistration::register(
            &config.machine_name,
            udp_addr.port(),
            tcp_addr.port(),
        ) {
            Ok(reg) => {
                info!("mDNS service registered");
                Some(reg)
            }
            Err(e) => {
                warn!(error = %e, "mDNS registration failed, clients must connect manually");
                None
            }
        }
    } else {
        None
    };

    // Shared client sessions
    let sessions: Arc<Mutex<HashMap<String, ClientSession>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Channel for the EventRouter to send events to remote clients
    let (remote_tx, mut remote_rx) =
        mpsc::unbounded_channel::<(TimestampedEvent, SocketAddr)>();

    // EventRouter
    let router = Arc::new(Mutex::new(EventRouter::new(
        config.local_display.clone(),
        config.edge_threshold,
        remote_tx,
    )));

    let shutdown_clone = shutdown.clone();

    // Task 1: Accept TCP connections
    let sessions_tcp = sessions.clone();
    let router_tcp = router.clone();
    let status_tx_tcp = status_tx.clone();
    let server_name = config.machine_name.clone();
    let server_display = config.local_display.clone();
    let shutdown_accept = shutdown.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = tcp_transport.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            info!(%addr, "new TCP connection");
                            let sessions = sessions_tcp.clone();
                            let router = router_tcp.clone();
                            let status_tx = status_tx_tcp.clone();
                            let server_name = server_name.clone();
                            let server_display = server_display.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_client_connection(
                                    stream,
                                    addr,
                                    sessions,
                                    router,
                                    status_tx,
                                    &server_name,
                                    &server_display,
                                )
                                .await
                                {
                                    warn!(%addr, error = %e, "client connection error");
                                }
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "TCP accept failed");
                            let _ = status_tx_tcp.send(ServerStatus::Error(e.to_string()));
                        }
                    }
                }
                _ = shutdown_accept.notified() => {
                    info!("TCP accept loop shutting down");
                    break;
                }
            }
        }
    });

    // Task 2: Forward routed events via UDP
    let udp_fwd = udp_socket.clone();
    let shutdown_udp = shutdown.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some((event, target_addr)) = remote_rx.recv() => {
                    match protocol::serialize_event(&event) {
                        Ok(bytes) => {
                            if let Err(e) = udp_fwd.send_to(&bytes, target_addr).await {
                                warn!(%target_addr, error = %e, "UDP forward failed");
                            }
                        }
                        Err(e) => {
                            // For large events (clipboard), serialize for TCP instead
                            debug!(error = %e, "event too large for UDP, skipping");
                        }
                    }
                }
                _ = shutdown_udp.notified() => {
                    info!("UDP forward loop shutting down");
                    break;
                }
            }
        }
    });

    // Task 3: Route captured events
    let router_capture = router.clone();
    let shutdown_capture = shutdown.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = capture_rx.recv() => {
                    match event {
                        Some(ev) => {
                            let mut r = router_capture.lock().await;
                            if let Err(e) = r.route(&ev) {
                                warn!(error = %e, "routing error");
                            }
                        }
                        None => {
                            info!("capture channel closed");
                            break;
                        }
                    }
                }
                _ = shutdown_capture.notified() => {
                    info!("capture routing loop shutting down");
                    break;
                }
            }
        }
    });

    Ok(ServerHandle {
        shutdown: shutdown_clone,
        status_rx,
    })
}

/// Handles a single client TCP connection.
///
/// Reads the Hello message, registers the client, and keeps the
/// connection alive for heartbeats and control messages.
async fn handle_client_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    sessions: Arc<Mutex<HashMap<String, ClientSession>>>,
    router: Arc<Mutex<EventRouter>>,
    status_tx: mpsc::UnboundedSender<ServerStatus>,
    server_name: &str,
    server_display: &DisplayInfo,
) -> Result<()> {
    // Read Hello message from client
    let hello_data = tcp::recv_message(&mut stream)
        .await?
        .context("client disconnected before Hello")?;

    let hello: ServerMessage =
        bincode::deserialize(&hello_data).context("deserializing Hello message")?;

    let (client_name, position, client_display, client_udp_port) = match hello {
        ServerMessage::Hello {
            name,
            position,
            display,
            udp_port,
        } => (name, position, display, udp_port),
        _ => anyhow::bail!("expected Hello message, got {:?}", hello),
    };

    info!(
        name = %client_name,
        ?position,
        %addr,
        "client identified"
    );

    // Construct the client's UDP address (same IP, client's UDP port)
    let client_udp_addr = SocketAddr::new(addr.ip(), client_udp_port);

    // Send Welcome
    let welcome = ServerMessage::Welcome {
        server_name: server_name.to_string(),
        server_display: server_display.clone(),
    };
    let welcome_bytes = bincode::serialize(&welcome).context("serializing Welcome")?;
    tcp::send_message(&mut stream, &welcome_bytes).await?;

    // Register the session
    let session = ClientSession {
        name: client_name.clone(),
        position,
        display_info: client_display.clone(),
        udp_addr: client_udp_addr,
        tcp_addr: addr,
    };

    {
        let mut s = sessions.lock().await;
        s.insert(client_name.clone(), session);
    }

    // Register in the EventRouter
    {
        let mut r = router.lock().await;
        r.add_client(ConnectedClient {
            name: client_name.clone(),
            position,
            udp_addr: client_udp_addr,
            display: client_display,
        });
    }

    let _ = status_tx.send(ServerStatus::ClientConnected {
        name: client_name.clone(),
        addr,
    });

    // Keep connection alive for control messages / heartbeats
    loop {
        match tcp::recv_message(&mut stream).await {
            Ok(Some(data)) => {
                match bincode::deserialize::<ServerMessage>(&data) {
                    Ok(ServerMessage::Ping) => {
                        let pong = bincode::serialize(&ServerMessage::Pong)?;
                        tcp::send_message(&mut stream, &pong).await?;
                    }
                    Ok(ServerMessage::Goodbye) => {
                        info!(name = %client_name, "client sent Goodbye");
                        break;
                    }
                    Ok(msg) => {
                        debug!(name = %client_name, ?msg, "unexpected message");
                    }
                    Err(e) => {
                        warn!(name = %client_name, error = %e, "bad message from client");
                    }
                }
            }
            Ok(None) => {
                info!(name = %client_name, "client TCP connection closed");
                break;
            }
            Err(e) => {
                warn!(name = %client_name, error = %e, "client TCP read error");
                break;
            }
        }
    }

    // Cleanup
    {
        let mut s = sessions.lock().await;
        s.remove(&client_name);
    }
    {
        let mut r = router.lock().await;
        r.remove_client(&client_name);
    }

    let _ = status_tx.send(ServerStatus::ClientDisconnected {
        name: client_name,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsynergy_core::event::InputEvent;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::time::{sleep, Duration};

    fn test_config(tcp_port: u16, udp_port: u16) -> ServerConfig {
        ServerConfig {
            udp_port,
            tcp_port,
            machine_name: "test-server".to_string(),
            local_display: DisplayInfo {
                id: 0,
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            edge_threshold: 2,
            enable_mdns: false,
        }
    }

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[tokio::test]
    async fn server_starts_and_reports_listening() {
        let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
        let config = test_config(0, 0);

        let mut handle = start_server(config, capture_rx).await.unwrap();

        // Should receive a Listening status
        let status = handle.status_rx.recv().await.unwrap();
        match status {
            ServerStatus::Listening { tcp_addr, udp_addr } => {
                assert!(tcp_addr.port() > 0);
                assert!(udp_addr.port() > 0);
            }
            _ => panic!("expected Listening status"),
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn client_connects_and_receives_welcome() {
        let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
        let config = test_config(0, 0);

        let mut handle = start_server(config, capture_rx).await.unwrap();

        let status = handle.status_rx.recv().await.unwrap();
        let tcp_addr = match status {
            ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
            _ => panic!("expected Listening"),
        };

        // Connect as a client
        let mut client_stream = tcp::connect(
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        )
        .await
        .unwrap();

        // Send Hello
        let hello = ServerMessage::Hello {
            name: "test-client".to_string(),
            position: ScreenPosition::Right,
            display: DisplayInfo {
                id: 1,
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
                scale: 1.0,
            },
            udp_port: 12345,
        };
        let hello_bytes = bincode::serialize(&hello).unwrap();
        tcp::send_message(&mut client_stream, &hello_bytes)
            .await
            .unwrap();

        // Read Welcome
        let welcome_data = tcp::recv_message(&mut client_stream)
            .await
            .unwrap()
            .unwrap();
        let welcome: ServerMessage = bincode::deserialize(&welcome_data).unwrap();
        match welcome {
            ServerMessage::Welcome {
                server_name,
                server_display,
            } => {
                assert_eq!(server_name, "test-server");
                assert_eq!(server_display.width, 1920);
            }
            _ => panic!("expected Welcome"),
        }

        // Server should report ClientConnected
        let status = handle.status_rx.recv().await.unwrap();
        match status {
            ServerStatus::ClientConnected { name, .. } => {
                assert_eq!(name, "test-client");
            }
            _ => panic!("expected ClientConnected"),
        }

        // Disconnect
        drop(client_stream);
        sleep(Duration::from_millis(100)).await;

        // Server should report ClientDisconnected
        let status = handle.status_rx.recv().await.unwrap();
        match status {
            ServerStatus::ClientDisconnected { name } => {
                assert_eq!(name, "test-client");
            }
            _ => panic!("expected ClientDisconnected"),
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn ping_pong_heartbeat() {
        let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
        let config = test_config(0, 0);

        let mut handle = start_server(config, capture_rx).await.unwrap();
        let status = handle.status_rx.recv().await.unwrap();
        let tcp_addr = match status {
            ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
            _ => panic!("expected Listening"),
        };

        let mut stream = tcp::connect(
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        )
        .await
        .unwrap();

        // Hello
        let hello = ServerMessage::Hello {
            name: "ping-client".to_string(),
            position: ScreenPosition::Left,
            display: DisplayInfo {
                id: 1,
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            udp_port: 0,
        };
        let bytes = bincode::serialize(&hello).unwrap();
        tcp::send_message(&mut stream, &bytes).await.unwrap();

        // Read Welcome
        tcp::recv_message(&mut stream).await.unwrap().unwrap();

        // Consume ClientConnected
        handle.status_rx.recv().await.unwrap();

        // Send Ping
        let ping = bincode::serialize(&ServerMessage::Ping).unwrap();
        tcp::send_message(&mut stream, &ping).await.unwrap();

        // Expect Pong
        let pong_data = tcp::recv_message(&mut stream).await.unwrap().unwrap();
        let pong: ServerMessage = bincode::deserialize(&pong_data).unwrap();
        assert!(matches!(pong, ServerMessage::Pong));

        handle.shutdown();
    }

    #[tokio::test]
    async fn events_routed_to_connected_client() {
        let (capture_tx, capture_rx) = mpsc::unbounded_channel();
        let config = test_config(0, 0);

        let mut handle = start_server(config, capture_rx).await.unwrap();
        let status = handle.status_rx.recv().await.unwrap();
        let (tcp_addr, _udp_addr) = match status {
            ServerStatus::Listening { tcp_addr, udp_addr } => (tcp_addr, udp_addr),
            _ => panic!("expected Listening"),
        };

        // Bind a UDP receiver for the client
        let client_udp = tokio::net::UdpSocket::bind(localhost(0)).await.unwrap();
        let client_udp_port = client_udp.local_addr().unwrap().port();

        // Connect client via TCP
        let mut stream = tcp::connect(
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        )
        .await
        .unwrap();

        let hello = ServerMessage::Hello {
            name: "route-client".to_string(),
            position: ScreenPosition::Right,
            display: DisplayInfo {
                id: 1,
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
                scale: 1.0,
            },
            udp_port: client_udp_port,
        };
        let bytes = bincode::serialize(&hello).unwrap();
        tcp::send_message(&mut stream, &bytes).await.unwrap();
        tcp::recv_message(&mut stream).await.unwrap().unwrap(); // Welcome

        // Wait for ClientConnected
        handle.status_rx.recv().await.unwrap();
        // Give the router a moment to register the client
        sleep(Duration::from_millis(50)).await;

        // Send a mouse move event at the right edge to trigger routing
        let event = TimestampedEvent {
            timestamp_us: 1000,
            event: InputEvent::MouseMove {
                x: 1919.0,
                y: 540.0,
            },
        };
        capture_tx.send(event).unwrap();

        // The client should receive a UDP packet
        let mut buf = vec![0u8; 2048];
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            client_udp.recv_from(&mut buf),
        )
        .await;

        assert!(result.is_ok(), "should receive UDP event within timeout");
        let (len, _from) = result.unwrap().unwrap();
        let received: TimestampedEvent =
            protocol::deserialize_event(&buf[..len]).unwrap();

        // Should be a MouseMove (remapped to the client's display)
        assert!(matches!(received.event, InputEvent::MouseMove { .. }));

        handle.shutdown();
    }

    #[tokio::test]
    async fn graceful_goodbye() {
        let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
        let config = test_config(0, 0);

        let mut handle = start_server(config, capture_rx).await.unwrap();
        let status = handle.status_rx.recv().await.unwrap();
        let tcp_addr = match status {
            ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
            _ => panic!("expected Listening"),
        };

        let mut stream = tcp::connect(
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        )
        .await
        .unwrap();

        let hello = ServerMessage::Hello {
            name: "bye-client".to_string(),
            position: ScreenPosition::Left,
            display: DisplayInfo {
                id: 1,
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            udp_port: 0,
        };
        tcp::send_message(&mut stream, &bincode::serialize(&hello).unwrap())
            .await
            .unwrap();
        tcp::recv_message(&mut stream).await.unwrap(); // Welcome
        handle.status_rx.recv().await.unwrap(); // ClientConnected

        // Send Goodbye
        let goodbye = bincode::serialize(&ServerMessage::Goodbye).unwrap();
        tcp::send_message(&mut stream, &goodbye).await.unwrap();

        // Should get ClientDisconnected
        let status = handle.status_rx.recv().await.unwrap();
        assert!(matches!(status, ServerStatus::ClientDisconnected { .. }));

        handle.shutdown();
    }
}
