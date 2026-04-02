/// Integration tests for the full nsynergy pipeline.
///
/// These tests verify end-to-end behavior: server start, client connect,
/// event forwarding, and graceful shutdown. All ports use 0 (OS-assigned)
/// to avoid conflicts.
use nsynergy_client::client::{ClientConfig, ClientStatus};
use nsynergy_core::config::ScreenPosition;
use nsynergy_core::event::{InputEvent, TimestampedEvent};
use nsynergy_core::screen::DisplayInfo;
use nsynergy_net::reconnect::ReconnectConfig;
use nsynergy_server::server::{ServerConfig, ServerStatus};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

fn test_display(w: u32, h: u32) -> DisplayInfo {
    DisplayInfo {
        id: 0,
        x: 0,
        y: 0,
        width: w,
        height: h,
        scale: 1.0,
    }
}

fn server_config() -> ServerConfig {
    ServerConfig {
        udp_port: 0,
        tcp_port: 0,
        machine_name: "integ-server".to_string(),
        local_display: test_display(1920, 1080),
        edge_threshold: 2,
        enable_mdns: false,
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

/// Extracts the TCP port from the first ServerStatus::Listening message.
async fn wait_for_listening(handle: &mut nsynergy_server::server::ServerHandle) -> SocketAddr {
    let status = handle.status_rx.recv().await.expect("server status channel closed");
    match status {
        ServerStatus::Listening { tcp_addr, .. } => tcp_addr,
        other => panic!("expected ServerStatus::Listening, got {:?}", other),
    }
}

// 1. Server starts and listens on assigned ports.
#[tokio::test]
async fn server_starts_and_listens() {
    let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
    let config = server_config();

    let mut handle = nsynergy_server::server::start_server(config, capture_rx)
        .await
        .expect("server should start");

    let status = handle.status_rx.recv().await.expect("should receive status");
    match status {
        ServerStatus::Listening { tcp_addr, udp_addr } => {
            assert!(tcp_addr.port() > 0, "TCP port should be assigned");
            assert!(udp_addr.port() > 0, "UDP port should be assigned");
        }
        other => panic!("expected Listening, got {:?}", other),
    }

    handle.shutdown();
}

// 2. Client connects to server and both sides report the connection.
#[tokio::test]
async fn client_connects_to_server() {
    let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
    let mut server_handle = nsynergy_server::server::start_server(server_config(), capture_rx)
        .await
        .unwrap();

    let tcp_addr = wait_for_listening(&mut server_handle).await;

    let client_config = ClientConfig {
        server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        client_name: "integ-client".to_string(),
        position: ScreenPosition::Right,
        local_display: test_display(2560, 1440),
        udp_port: 0,
        reconnect: fast_reconnect(),
    };

    let (mut client_handle, _event_rx) =
        nsynergy_client::client::start_client(client_config).await.unwrap();

    // Client should report Connecting then Connected
    let status = timeout(Duration::from_secs(5), client_handle.status_rx.recv())
        .await
        .expect("timeout waiting for client Connecting")
        .expect("channel closed");
    assert!(
        matches!(status, ClientStatus::Connecting { .. }),
        "expected Connecting, got {:?}",
        status
    );

    let status = timeout(Duration::from_secs(5), client_handle.status_rx.recv())
        .await
        .expect("timeout waiting for client Connected")
        .expect("channel closed");
    match status {
        ClientStatus::Connected { server_name, .. } => {
            assert_eq!(server_name, "integ-server");
        }
        other => panic!("expected Connected, got {:?}", other),
    }

    // Server should report ClientConnected
    let server_status = timeout(Duration::from_secs(5), server_handle.status_rx.recv())
        .await
        .expect("timeout waiting for server ClientConnected")
        .expect("channel closed");
    assert!(
        matches!(server_status, ServerStatus::ClientConnected { .. }),
        "expected ClientConnected, got {:?}",
        server_status
    );

    client_handle.shutdown();
    server_handle.shutdown();
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// 3. Full pipeline: mouse event at screen edge is forwarded from server to client.
#[tokio::test]
async fn full_pipeline_event_delivery() {
    let (capture_tx, capture_rx) = mpsc::unbounded_channel();
    let mut server_handle = nsynergy_server::server::start_server(server_config(), capture_rx)
        .await
        .unwrap();

    let tcp_addr = wait_for_listening(&mut server_handle).await;

    let client_config = ClientConfig {
        server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        client_name: "pipeline-client".to_string(),
        position: ScreenPosition::Right,
        local_display: test_display(2560, 1440),
        udp_port: 0,
        reconnect: fast_reconnect(),
    };

    let (mut client_handle, mut event_rx) =
        nsynergy_client::client::start_client(client_config).await.unwrap();

    // Wait for full connection
    client_handle.status_rx.recv().await.unwrap(); // Connecting
    client_handle.status_rx.recv().await.unwrap(); // Connected
    server_handle.status_rx.recv().await.unwrap(); // ClientConnected

    // Allow the EventRouter to fully register the client
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send a mouse move at the right edge of the server display (1920 wide)
    // to trigger the EventRouter to forward to the client at ScreenPosition::Right
    let edge_event = TimestampedEvent {
        timestamp_us: 42000,
        event: InputEvent::MouseMove {
            x: 1919.0,
            y: 540.0,
        },
    };
    capture_tx.send(edge_event).unwrap();

    // Client should receive the forwarded event via UDP
    let received = timeout(Duration::from_secs(5), event_rx.recv())
        .await
        .expect("timeout waiting for event delivery")
        .expect("event channel closed");

    assert!(
        matches!(received.event, InputEvent::MouseMove { .. }),
        "expected MouseMove, got {:?}",
        received.event
    );

    client_handle.shutdown();
    server_handle.shutdown();
}

// 4. Graceful shutdown: client sends Goodbye, server reports disconnect.
#[tokio::test]
async fn graceful_shutdown() {
    let (_capture_tx, capture_rx) = mpsc::unbounded_channel();
    let mut server_handle = nsynergy_server::server::start_server(server_config(), capture_rx)
        .await
        .unwrap();

    let tcp_addr = wait_for_listening(&mut server_handle).await;

    let client_config = ClientConfig {
        server_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), tcp_addr.port()),
        client_name: "shutdown-client".to_string(),
        position: ScreenPosition::Left,
        local_display: test_display(1920, 1080),
        udp_port: 0,
        reconnect: fast_reconnect(),
    };

    let (mut client_handle, _event_rx) =
        nsynergy_client::client::start_client(client_config).await.unwrap();

    // Wait for connection
    client_handle.status_rx.recv().await.unwrap(); // Connecting
    client_handle.status_rx.recv().await.unwrap(); // Connected
    server_handle.status_rx.recv().await.unwrap(); // ClientConnected

    // Initiate graceful shutdown from client side
    client_handle.shutdown();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Server should report ClientDisconnected
    let server_status = timeout(Duration::from_secs(5), server_handle.status_rx.recv())
        .await
        .expect("timeout waiting for server ClientDisconnected")
        .expect("channel closed");

    assert!(
        matches!(server_status, ServerStatus::ClientDisconnected { .. }),
        "expected ClientDisconnected, got {:?}",
        server_status
    );

    // Client should report Disconnected
    let client_status = timeout(Duration::from_secs(5), client_handle.status_rx.recv())
        .await
        .expect("timeout waiting for client Disconnected")
        .expect("channel closed");

    assert!(
        matches!(client_status, ClientStatus::Disconnected { .. }),
        "expected Disconnected, got {:?}",
        client_status
    );

    server_handle.shutdown();
}
