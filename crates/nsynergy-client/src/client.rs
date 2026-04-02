use anyhow::{Context, Result};
use nsynergy_core::config::ScreenPosition;
use nsynergy_core::discovery::{self, DiscoveryEvent, PeerInfo};
use nsynergy_core::event::TimestampedEvent;
use nsynergy_core::protocol;
use nsynergy_core::screen::DisplayInfo;
use nsynergy_net::reconnect::{ReconnectConfig, ReconnectState};
use nsynergy_net::tcp;
use nsynergy_server::server::ServerMessage;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};

/// Configuration for the client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub server_addr: SocketAddr,
    pub client_name: String,
    pub position: ScreenPosition,
    pub local_display: DisplayInfo,
    pub udp_port: u16,
    pub reconnect: ReconnectConfig,
}

/// Handle returned by `start_client` to control the running client.
pub struct ClientHandle {
    shutdown_tx: watch::Sender<bool>,
    /// Receives status updates from the client.
    pub status_rx: mpsc::UnboundedReceiver<ClientStatus>,
}

impl ClientHandle {
    /// Signals the client to shut down gracefully.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Status updates emitted by the client.
#[derive(Debug, Clone)]
pub enum ClientStatus {
    Connecting { server_addr: SocketAddr },
    Connected { server_name: String, server_display: DisplayInfo },
    Disconnected { reason: String },
    Reconnecting { attempt: u32 },
    Error(String),
}

/// Discovers nsynergy servers on the local network via mDNS.
///
/// Returns a channel that emits `DiscoveryEvent`s as servers are
/// found or lost. The browser runs until the returned receiver is dropped.
pub fn start_discovery() -> Result<mpsc::UnboundedReceiver<DiscoveryEvent>> {
    let (_browser, rx) = discovery::ServiceBrowser::start()?;
    Ok(rx)
}

/// Resolves a server address from either a direct address or
/// a discovered peer.
pub fn peer_to_server_addr(peer: &PeerInfo) -> SocketAddr {
    SocketAddr::new(peer.address.into(), peer.tcp_port)
}

/// Starts the client in the background.
///
/// The client connects to the server via TCP, exchanges Hello/Welcome,
/// then listens for UDP events from the server and forwards them to
/// the returned event channel for injection.
pub async fn start_client(
    config: ClientConfig,
) -> Result<(ClientHandle, mpsc::UnboundedReceiver<TimestampedEvent>)> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (status_tx, status_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        run_client_loop(config, shutdown_rx, status_tx, event_tx).await;
    });

    Ok((
        ClientHandle {
            shutdown_tx,
            status_rx,
        },
        event_rx,
    ))
}

/// Internal client loop with reconnection support.
async fn run_client_loop(
    config: ClientConfig,
    mut shutdown_rx: watch::Receiver<bool>,
    status_tx: mpsc::UnboundedSender<ClientStatus>,
    event_tx: mpsc::UnboundedSender<TimestampedEvent>,
) {
    let mut reconnect_state = ReconnectState::new(config.reconnect.clone());

    loop {
        let _ = status_tx.send(ClientStatus::Connecting {
            server_addr: config.server_addr,
        });

        match connect_and_run(&config, &mut shutdown_rx, &status_tx, &event_tx).await {
            Ok(()) => {
                info!("client disconnected gracefully");
                let _ = status_tx.send(ClientStatus::Disconnected {
                    reason: "graceful shutdown".to_string(),
                });
                break;
            }
            Err(e) => {
                warn!(error = %e, "client connection failed");
                let _ = status_tx.send(ClientStatus::Disconnected {
                    reason: e.to_string(),
                });
            }
        }

        // Check if shutdown was requested
        if *shutdown_rx.borrow() {
            info!("client shutdown requested, not reconnecting");
            break;
        }

        // Attempt reconnection
        let _ = status_tx.send(ClientStatus::Reconnecting {
            attempt: reconnect_state.attempt() + 1,
        });

        tokio::select! {
            should_continue = reconnect_state.wait_and_advance() => {
                if !should_continue {
                    let _ = status_tx.send(ClientStatus::Error(
                        "max reconnection attempts reached".to_string(),
                    ));
                    break;
                }
            }
            _ = shutdown_rx.changed() => {
                info!("shutdown during reconnection wait");
                break;
            }
        }
    }
}

/// Connects to the server, does the handshake, and runs the event receive loop.
/// Returns Ok(()) on graceful shutdown, Err on connection failure.
async fn connect_and_run(
    config: &ClientConfig,
    shutdown_rx: &mut watch::Receiver<bool>,
    status_tx: &mpsc::UnboundedSender<ClientStatus>,
    event_tx: &mpsc::UnboundedSender<TimestampedEvent>,
) -> Result<()> {
    // Bind UDP receiver first so we know the actual port
    let udp_bind = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, config.udp_port));
    let udp_socket = tokio::net::UdpSocket::bind(udp_bind)
        .await
        .context("binding client UDP socket")?;
    let actual_udp_port = udp_socket.local_addr()?.port();

    // Connect TCP
    let mut tcp_stream = tcp::connect(config.server_addr).await?;

    // Send Hello with the actual bound UDP port
    let hello = ServerMessage::Hello {
        name: config.client_name.clone(),
        position: config.position,
        display: config.local_display.clone(),
        udp_port: actual_udp_port,
    };
    let hello_bytes = bincode::serialize(&hello).context("serializing Hello")?;
    tcp::send_message(&mut tcp_stream, &hello_bytes).await?;

    // Read Welcome
    let welcome_data = tcp::recv_message(&mut tcp_stream)
        .await?
        .context("server closed before Welcome")?;
    let welcome: ServerMessage =
        bincode::deserialize(&welcome_data).context("deserializing Welcome")?;

    let (server_name, server_display) = match welcome {
        ServerMessage::Welcome {
            server_name,
            server_display,
        } => (server_name, server_display),
        _ => anyhow::bail!("expected Welcome, got {:?}", welcome),
    };

    info!(
        server = %server_name,
        display_w = server_display.width,
        display_h = server_display.height,
        "connected to server"
    );

    let _ = status_tx.send(ClientStatus::Connected {
        server_name,
        server_display,
    });

    let mut udp_buf = vec![0u8; protocol::MAX_UDP_PAYLOAD];

    // Main event loop
    loop {
        tokio::select! {
            // Receive UDP events from server
            result = udp_socket.recv_from(&mut udp_buf) => {
                match result {
                    Ok((len, _from)) => {
                        match protocol::deserialize_event(&udp_buf[..len]) {
                            Ok(event) => {
                                if event_tx.send(event).is_err() {
                                    info!("event channel closed");
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "malformed UDP event");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "UDP recv error");
                    }
                }
            }
            // Check for TCP control messages (heartbeat, disconnect)
            result = tcp::recv_message(&mut tcp_stream) => {
                match result {
                    Ok(Some(data)) => {
                        match bincode::deserialize::<ServerMessage>(&data) {
                            Ok(ServerMessage::Ping) => {
                                let pong = bincode::serialize(&ServerMessage::Pong)?;
                                tcp::send_message(&mut tcp_stream, &pong).await?;
                            }
                            Ok(ServerMessage::Goodbye) => {
                                info!("server sent Goodbye");
                                return Ok(());
                            }
                            Ok(msg) => {
                                debug!(?msg, "unexpected server message");
                            }
                            Err(e) => {
                                warn!(error = %e, "bad TCP message from server");
                            }
                        }
                    }
                    Ok(None) => {
                        info!("server TCP connection closed");
                        return Err(anyhow::anyhow!("server disconnected"));
                    }
                    Err(e) => {
                        return Err(e).context("TCP read from server");
                    }
                }
            }
            // Shutdown signal
            _ = shutdown_rx.changed() => {
                info!("client shutdown signal received");
                // Send Goodbye to server
                let goodbye = bincode::serialize(&ServerMessage::Goodbye)
                    .context("serializing Goodbye")?;
                let _ = tcp::send_message(&mut tcp_stream, &goodbye).await;
                return Ok(());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsynergy_core::config::ScreenPosition;
    use nsynergy_core::event::{InputEvent, TimestampedEvent};
    use nsynergy_core::screen::DisplayInfo;
    use nsynergy_server::server::{self, ServerConfig, ServerStatus};
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::time::{sleep, Duration};

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    fn test_display() -> DisplayInfo {
        DisplayInfo {
            id: 0,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
        }
    }

    fn fast_reconnect() -> ReconnectConfig {
        ReconnectConfig {
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_factor: 2.0,
            max_attempts: 3,
        }
    }

    #[tokio::test]
    async fn client_connects_to_server() {
        let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
        let server_config = ServerConfig {
            udp_port: 0,
            tcp_port: 0,
            machine_name: "test-server".to_string(),
            local_display: test_display(),
            edge_threshold: 2,
            enable_mdns: false,
        };
        let mut server_handle = server::start_server(server_config, capture_rx)
            .await
            .unwrap();

        let status = server_handle.status_rx.recv().await.unwrap();
        let tcp_addr = match status {
            ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
            _ => panic!("expected Listening"),
        };

        let client_config = ClientConfig {
            server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
            client_name: "test-client".to_string(),
            position: ScreenPosition::Right,
            local_display: test_display(),
            udp_port: 0,
            reconnect: fast_reconnect(),
        };

        let (mut client_handle, _event_rx) = start_client(client_config).await.unwrap();

        // Check client receives Connecting then Connected
        let status = client_handle.status_rx.recv().await.unwrap();
        assert!(matches!(status, ClientStatus::Connecting { .. }));

        let status = client_handle.status_rx.recv().await.unwrap();
        match status {
            ClientStatus::Connected { server_name, .. } => {
                assert_eq!(server_name, "test-server");
            }
            other => panic!("expected Connected, got {:?}", other),
        }

        // Server should see ClientConnected
        let server_status = server_handle.status_rx.recv().await.unwrap();
        assert!(matches!(
            server_status,
            ServerStatus::ClientConnected { .. }
        ));

        client_handle.shutdown();
        server_handle.shutdown();
        sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn client_receives_events_from_server() {
        let (capture_tx, capture_rx) = mpsc::unbounded_channel();
        let server_config = ServerConfig {
            udp_port: 0,
            tcp_port: 0,
            machine_name: "event-server".to_string(),
            local_display: test_display(),
            edge_threshold: 2,
            enable_mdns: false,
        };
        let mut server_handle = server::start_server(server_config, capture_rx)
            .await
            .unwrap();

        let status = server_handle.status_rx.recv().await.unwrap();
        let tcp_addr = match status {
            ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
            _ => panic!("expected Listening"),
        };

        let client_config = ClientConfig {
            server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
            client_name: "event-client".to_string(),
            position: ScreenPosition::Right,
            local_display: DisplayInfo {
                id: 1,
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
                scale: 1.0,
            },
            udp_port: 0,
            reconnect: fast_reconnect(),
        };

        let (mut client_handle, mut event_rx) = start_client(client_config).await.unwrap();

        // Wait for connected
        client_handle.status_rx.recv().await.unwrap(); // Connecting
        client_handle.status_rx.recv().await.unwrap(); // Connected
        server_handle.status_rx.recv().await.unwrap(); // ClientConnected

        // Allow registration to settle
        sleep(Duration::from_millis(100)).await;

        // Send edge mouse move to trigger routing to client
        let event = TimestampedEvent {
            timestamp_us: 5000,
            event: InputEvent::MouseMove {
                x: 1919.0,
                y: 540.0,
            },
        };
        capture_tx.send(event).unwrap();

        // Client should receive the event via UDP
        let received = tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await;
        assert!(received.is_ok(), "should receive event within timeout");
        let received = received.unwrap().unwrap();
        assert!(matches!(received.event, InputEvent::MouseMove { .. }));

        client_handle.shutdown();
        server_handle.shutdown();
    }

    #[tokio::test]
    async fn client_graceful_shutdown_sends_goodbye() {
        let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
        let server_config = ServerConfig {
            udp_port: 0,
            tcp_port: 0,
            machine_name: "bye-server".to_string(),
            local_display: test_display(),
            edge_threshold: 2,
            enable_mdns: false,
        };
        let mut server_handle = server::start_server(server_config, capture_rx)
            .await
            .unwrap();

        let status = server_handle.status_rx.recv().await.unwrap();
        let tcp_addr = match status {
            ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
            _ => panic!("expected Listening"),
        };

        let client_config = ClientConfig {
            server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
            client_name: "goodbye-client".to_string(),
            position: ScreenPosition::Left,
            local_display: test_display(),
            udp_port: 0,
            reconnect: fast_reconnect(),
        };

        let (mut client_handle, _event_rx) = start_client(client_config).await.unwrap();

        // Wait for connection
        client_handle.status_rx.recv().await.unwrap(); // Connecting
        client_handle.status_rx.recv().await.unwrap(); // Connected
        server_handle.status_rx.recv().await.unwrap(); // ClientConnected

        // Shutdown client gracefully
        client_handle.shutdown();
        sleep(Duration::from_millis(200)).await;

        // Server should receive ClientDisconnected (via Goodbye)
        let server_status = server_handle.status_rx.recv().await.unwrap();
        assert!(matches!(
            server_status,
            ServerStatus::ClientDisconnected { .. }
        ));

        server_handle.shutdown();
    }

    #[tokio::test]
    async fn client_reconnects_on_server_disconnect() {
        use tokio::net::TcpListener;

        // Use a raw TCP listener that we can fully control
        let listener = TcpListener::bind(localhost(0)).await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Spawn a fake server that accepts one connection, does the handshake, then closes
        let fake_server = tokio::spawn(async move {
            let (mut stream, _addr) = listener.accept().await.unwrap();

            // Read Hello
            let hello_data = tcp::recv_message(&mut stream).await.unwrap().unwrap();
            let _hello: ServerMessage = bincode::deserialize(&hello_data).unwrap();

            // Send Welcome
            let welcome = ServerMessage::Welcome {
                server_name: "fake-server".to_string(),
                server_display: test_display(),
            };
            let bytes = bincode::serialize(&welcome).unwrap();
            tcp::send_message(&mut stream, &bytes).await.unwrap();

            // Wait a moment, then close connection to simulate server going down
            sleep(Duration::from_millis(100)).await;
            drop(stream);
            // Drop listener too, so reconnection attempts will fail
            drop(listener);
        });

        let client_config = ClientConfig {
            server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), server_addr.port()),
            client_name: "reconnect-client".to_string(),
            position: ScreenPosition::Right,
            local_display: test_display(),
            udp_port: 0,
            reconnect: fast_reconnect(),
        };

        let (mut client_handle, _event_rx) = start_client(client_config).await.unwrap();

        // Wait for initial connection
        client_handle.status_rx.recv().await.unwrap(); // Connecting
        client_handle.status_rx.recv().await.unwrap(); // Connected

        // Wait for fake server to close
        fake_server.await.unwrap();

        // Client should report Disconnected then Reconnecting
        let status = client_handle.status_rx.recv().await.unwrap();
        assert!(matches!(status, ClientStatus::Disconnected { .. }));

        let status = client_handle.status_rx.recv().await.unwrap();
        assert!(matches!(status, ClientStatus::Reconnecting { .. }));

        // Wait for the reconnect loop to exhaust (3 attempts with short delays)
        let mut found_error = false;
        for _ in 0..30 {
            match tokio::time::timeout(Duration::from_secs(2), client_handle.status_rx.recv()).await
            {
                Ok(Some(ClientStatus::Error(msg))) => {
                    assert!(msg.contains("max reconnection attempts"));
                    found_error = true;
                    break;
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }
        assert!(found_error, "should eventually hit max reconnection attempts");

        client_handle.shutdown();
    }
}
