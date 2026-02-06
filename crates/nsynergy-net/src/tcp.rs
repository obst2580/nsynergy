use anyhow::{Context, Result};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::debug;

/// Framing: each message is prefixed with a 4-byte big-endian length.
const LENGTH_PREFIX_SIZE: usize = 4;
/// Maximum allowed message size (16 MiB) to prevent unbounded allocation.
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Sends a length-prefixed message over a TCP stream.
pub async fn send_message(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    let len = data.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .await
        .context("writing TCP length prefix")?;
    stream
        .write_all(data)
        .await
        .context("writing TCP payload")?;
    stream.flush().await.context("flushing TCP stream")?;
    Ok(())
}

/// Reads a length-prefixed message from a TCP stream.
/// Returns `None` on clean EOF (connection closed).
pub async fn recv_message(stream: &mut TcpStream) -> Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e).context("reading TCP length prefix"),
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        anyhow::bail!(
            "TCP message too large: {len} bytes (max {MAX_MESSAGE_SIZE})"
        );
    }

    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .await
        .context("reading TCP payload")?;
    Ok(Some(buf))
}

/// A simple TCP listener that accepts connections and provides
/// length-framed message exchange.
pub struct TcpTransport {
    listener: TcpListener,
}

impl TcpTransport {
    /// Binds a TCP listener on `bind_addr`.
    pub async fn bind(bind_addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("binding TCP listener to {bind_addr}"))?;
        debug!(%bind_addr, "TCP listener created");
        Ok(Self { listener })
    }

    /// Accepts the next incoming connection.
    pub async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (stream, addr) = self.listener.accept().await.context("accepting TCP")?;
        debug!(%addr, "TCP connection accepted");
        Ok((stream, addr))
    }

    /// Returns the local address the listener is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }
}

/// Connects to a TCP endpoint.
pub async fn connect(addr: SocketAddr) -> Result<TcpStream> {
    let stream = TcpStream::connect(addr)
        .await
        .with_context(|| format!("connecting TCP to {addr}"))?;
    debug!(%addr, "TCP connection established");
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[tokio::test]
    async fn send_recv_roundtrip() {
        let transport = TcpTransport::bind(localhost(0)).await.unwrap();
        let server_addr = transport.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _) = transport.accept().await.unwrap();
            let msg = recv_message(&mut stream).await.unwrap().unwrap();
            send_message(&mut stream, &msg).await.unwrap();
            msg
        });

        let mut client = connect(server_addr).await.unwrap();
        let payload = b"hello clipboard data";
        send_message(&mut client, payload).await.unwrap();
        let echoed = recv_message(&mut client).await.unwrap().unwrap();

        assert_eq!(echoed, payload);
        let server_received = server.await.unwrap();
        assert_eq!(server_received, payload);
    }

    #[tokio::test]
    async fn send_recv_large_message() {
        let transport = TcpTransport::bind(localhost(0)).await.unwrap();
        let server_addr = transport.local_addr().unwrap();

        let large_data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let expected = large_data.clone();

        let server = tokio::spawn(async move {
            let (mut stream, _) = transport.accept().await.unwrap();
            recv_message(&mut stream).await.unwrap().unwrap()
        });

        let mut client = connect(server_addr).await.unwrap();
        send_message(&mut client, &large_data).await.unwrap();

        let received = server.await.unwrap();
        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn multiple_messages_in_sequence() {
        let transport = TcpTransport::bind(localhost(0)).await.unwrap();
        let server_addr = transport.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _) = transport.accept().await.unwrap();
            let mut messages = Vec::new();
            for _ in 0..3 {
                if let Some(msg) = recv_message(&mut stream).await.unwrap() {
                    messages.push(msg);
                }
            }
            messages
        });

        let mut client = connect(server_addr).await.unwrap();
        send_message(&mut client, b"msg1").await.unwrap();
        send_message(&mut client, b"msg2").await.unwrap();
        send_message(&mut client, b"msg3").await.unwrap();

        let received = server.await.unwrap();
        assert_eq!(received.len(), 3);
        assert_eq!(received[0], b"msg1");
        assert_eq!(received[1], b"msg2");
        assert_eq!(received[2], b"msg3");
    }

    #[tokio::test]
    async fn eof_returns_none() {
        let transport = TcpTransport::bind(localhost(0)).await.unwrap();
        let server_addr = transport.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _) = transport.accept().await.unwrap();
            let result = recv_message(&mut stream).await.unwrap();
            result
        });

        let client = connect(server_addr).await.unwrap();
        drop(client); // close immediately

        let result = server.await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn clipboard_event_over_tcp() {
        use nsynergy_core::event::{ClipboardContent, InputEvent, TimestampedEvent};
        use nsynergy_core::protocol;

        let transport = TcpTransport::bind(localhost(0)).await.unwrap();
        let server_addr = transport.local_addr().unwrap();

        let event = TimestampedEvent {
            timestamp_us: 999,
            event: InputEvent::ClipboardUpdate {
                content: ClipboardContent::Text("shared clipboard text".to_string()),
            },
        };
        let event_clone = event.clone();

        let server = tokio::spawn(async move {
            let (mut stream, _) = transport.accept().await.unwrap();
            let data = recv_message(&mut stream).await.unwrap().unwrap();
            let decoded: TimestampedEvent = bincode::deserialize(&data).unwrap();
            decoded
        });

        let mut client = connect(server_addr).await.unwrap();
        let serialized = protocol::serialize_event(&event_clone).unwrap();
        send_message(&mut client, &serialized).await.unwrap();

        let received = server.await.unwrap();
        assert_eq!(received, event);
    }
}
