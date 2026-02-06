use anyhow::{Context, Result};
use nsynergy_core::security::TlsIdentity;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tracing::debug;

/// Framing constants (same as plain TCP).
const LENGTH_PREFIX_SIZE: usize = 4;
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Parses PEM certificate and key from a `TlsIdentity`.
fn parse_identity(
    identity: &TlsIdentity,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut BufReader::new(identity.cert_pem.as_bytes()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("parsing PEM certificates")?;

    let key = rustls_pemfile::private_key(&mut BufReader::new(identity.key_pem.as_bytes()))
        .context("parsing PEM private key")?
        .context("no private key found in PEM")?;

    Ok((certs, key))
}

/// A TLS-encrypted TCP server.
pub struct TlsServer {
    listener: TcpListener,
    acceptor: TlsAcceptor,
}

impl TlsServer {
    /// Creates a TLS server bound to `bind_addr` using the given identity.
    pub async fn bind(bind_addr: SocketAddr, identity: &TlsIdentity) -> Result<Self> {
        let (certs, key) = parse_identity(identity)?;

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("building TLS server config")?;

        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("binding TLS server to {bind_addr}"))?;

        debug!(%bind_addr, "TLS server listening");
        Ok(Self { listener, acceptor })
    }

    /// Accepts an incoming TLS connection.
    pub async fn accept(
        &self,
    ) -> Result<(TlsStream<TcpStream>, SocketAddr)> {
        let (tcp_stream, addr) = self.listener.accept().await.context("accepting TCP")?;
        let tls_stream = self
            .acceptor
            .accept(tcp_stream)
            .await
            .context("TLS handshake failed")?;
        debug!(%addr, "TLS connection accepted");
        Ok((TlsStream::Server(tls_stream), addr))
    }

    /// Returns the local address.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }
}

/// Connects to a TLS server, trusting the given server certificate.
pub async fn tls_connect(
    addr: SocketAddr,
    server_cert_pem: &str,
    server_name: &str,
) -> Result<TlsStream<TcpStream>> {
    let mut root_store = RootCertStore::empty();

    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut BufReader::new(server_cert_pem.as_bytes()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("parsing server certificate")?;

    for cert in &certs {
        root_store.add(cert.clone()).context("adding root cert")?;
    }

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let tcp_stream = TcpStream::connect(addr)
        .await
        .with_context(|| format!("connecting TCP to {addr}"))?;

    let server_name = rustls::pki_types::ServerName::try_from(server_name.to_string())
        .context("invalid server name")?;

    let tls_stream = connector
        .connect(server_name, tcp_stream)
        .await
        .context("TLS handshake failed")?;

    debug!(%addr, "TLS connection established");
    Ok(TlsStream::Client(tls_stream))
}

/// Sends a length-prefixed message over a TLS stream.
pub async fn tls_send_message(
    stream: &mut TlsStream<TcpStream>,
    data: &[u8],
) -> Result<()> {
    let len = data.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .await
        .context("writing TLS length prefix")?;
    stream
        .write_all(data)
        .await
        .context("writing TLS payload")?;
    stream.flush().await.context("flushing TLS stream")?;
    Ok(())
}

/// Reads a length-prefixed message from a TLS stream.
/// Returns `None` on clean EOF.
pub async fn tls_recv_message(
    stream: &mut TlsStream<TcpStream>,
) -> Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e).context("reading TLS length prefix"),
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        anyhow::bail!("TLS message too large: {len} bytes (max {MAX_MESSAGE_SIZE})");
    }

    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .await
        .context("reading TLS payload")?;
    Ok(Some(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsynergy_core::security::generate_self_signed_cert;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[tokio::test]
    async fn tls_send_recv_roundtrip() {
        let identity = generate_self_signed_cert("localhost").unwrap();

        let server = TlsServer::bind(localhost(0), &identity).await.unwrap();
        let server_addr = server.local_addr().unwrap();
        let cert_pem = identity.cert_pem.clone();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let msg = tls_recv_message(&mut stream).await.unwrap().unwrap();
            tls_send_message(&mut stream, &msg).await.unwrap();
            msg
        });

        let mut client = tls_connect(server_addr, &cert_pem, "localhost")
            .await
            .unwrap();

        let payload = b"hello encrypted";
        tls_send_message(&mut client, payload).await.unwrap();
        let echoed = tls_recv_message(&mut client).await.unwrap().unwrap();

        assert_eq!(echoed, payload);
        let server_received = server_task.await.unwrap();
        assert_eq!(server_received, payload);
    }

    #[tokio::test]
    async fn tls_multiple_messages() {
        let identity = generate_self_signed_cert("localhost").unwrap();

        let server = TlsServer::bind(localhost(0), &identity).await.unwrap();
        let server_addr = server.local_addr().unwrap();
        let cert_pem = identity.cert_pem.clone();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let mut messages = Vec::new();
            for _ in 0..3 {
                if let Some(msg) = tls_recv_message(&mut stream).await.unwrap() {
                    messages.push(msg);
                }
            }
            messages
        });

        let mut client = tls_connect(server_addr, &cert_pem, "localhost")
            .await
            .unwrap();

        tls_send_message(&mut client, b"msg1").await.unwrap();
        tls_send_message(&mut client, b"msg2").await.unwrap();
        tls_send_message(&mut client, b"msg3").await.unwrap();

        let received = server_task.await.unwrap();
        assert_eq!(received.len(), 3);
        assert_eq!(received[0], b"msg1");
        assert_eq!(received[1], b"msg2");
        assert_eq!(received[2], b"msg3");
    }

    #[tokio::test]
    async fn tls_eof_returns_none() {
        let identity = generate_self_signed_cert("localhost").unwrap();

        let server = TlsServer::bind(localhost(0), &identity).await.unwrap();
        let server_addr = server.local_addr().unwrap();
        let cert_pem = identity.cert_pem.clone();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            tls_recv_message(&mut stream).await.unwrap()
        });

        let client = tls_connect(server_addr, &cert_pem, "localhost")
            .await
            .unwrap();
        drop(client); // close immediately

        let result = server_task.await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn tls_large_message() {
        let identity = generate_self_signed_cert("localhost").unwrap();

        let server = TlsServer::bind(localhost(0), &identity).await.unwrap();
        let server_addr = server.local_addr().unwrap();
        let cert_pem = identity.cert_pem.clone();

        let large_data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let expected = large_data.clone();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            tls_recv_message(&mut stream).await.unwrap().unwrap()
        });

        let mut client = tls_connect(server_addr, &cert_pem, "localhost")
            .await
            .unwrap();
        tls_send_message(&mut client, &large_data).await.unwrap();

        let received = server_task.await.unwrap();
        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn tls_clipboard_event_roundtrip() {
        use nsynergy_core::event::{ClipboardContent, InputEvent, TimestampedEvent};
        use nsynergy_core::protocol;

        let identity = generate_self_signed_cert("localhost").unwrap();
        let server = TlsServer::bind(localhost(0), &identity).await.unwrap();
        let server_addr = server.local_addr().unwrap();
        let cert_pem = identity.cert_pem.clone();

        let event = TimestampedEvent {
            timestamp_us: 42,
            event: InputEvent::ClipboardUpdate {
                content: ClipboardContent::Text("encrypted clipboard".to_string()),
            },
        };
        let event_clone = event.clone();

        let server_task = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let data = tls_recv_message(&mut stream).await.unwrap().unwrap();
            let decoded: TimestampedEvent = bincode::deserialize(&data).unwrap();
            decoded
        });

        let mut client = tls_connect(server_addr, &cert_pem, "localhost")
            .await
            .unwrap();
        let serialized = protocol::serialize_event(&event_clone).unwrap();
        tls_send_message(&mut client, &serialized).await.unwrap();

        let received = server_task.await.unwrap();
        assert_eq!(received, event);
    }
}
