use anyhow::{Context, Result};
use nsynergy_core::event::TimestampedEvent;
use nsynergy_core::protocol::{self, MAX_UDP_PAYLOAD};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::{debug, warn};

/// A UDP sender that serializes and transmits input events.
pub struct UdpEventSender {
    socket: UdpSocket,
    target: SocketAddr,
}

impl UdpEventSender {
    /// Creates a new sender bound to `bind_addr`, targeting `target`.
    pub async fn new(bind_addr: SocketAddr, target: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .with_context(|| format!("binding UDP socket to {bind_addr}"))?;
        debug!(%bind_addr, %target, "UDP sender created");
        Ok(Self { socket, target })
    }

    /// Sends a timestamped event to the configured target.
    pub async fn send(&self, event: &TimestampedEvent) -> Result<()> {
        let bytes = protocol::serialize_event(event)?;
        self.socket
            .send_to(&bytes, self.target)
            .await
            .with_context(|| format!("sending UDP to {}", self.target))?;
        Ok(())
    }

    /// Returns the local address the socket is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }
}

/// A UDP receiver that listens for incoming events.
pub struct UdpEventReceiver {
    socket: UdpSocket,
    buf: Vec<u8>,
}

impl UdpEventReceiver {
    /// Creates a new receiver bound to `bind_addr`.
    pub async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .with_context(|| format!("binding UDP receiver to {bind_addr}"))?;
        debug!(%bind_addr, "UDP receiver created");
        Ok(Self {
            socket,
            buf: vec![0u8; MAX_UDP_PAYLOAD],
        })
    }

    /// Waits for the next event. Returns the event and the sender address.
    pub async fn recv(&mut self) -> Result<(TimestampedEvent, SocketAddr)> {
        loop {
            let (len, addr) = self
                .socket
                .recv_from(&mut self.buf)
                .await
                .context("receiving UDP datagram")?;

            match protocol::deserialize_event(&self.buf[..len]) {
                Ok(event) => return Ok((event, addr)),
                Err(e) => {
                    warn!(%addr, error = %e, "dropping malformed UDP packet");
                    continue;
                }
            }
        }
    }

    /// Returns the local address the socket is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsynergy_core::event::{Button, InputEvent, Key, Modifiers};
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[tokio::test]
    async fn send_recv_mouse_move() {
        let mut receiver = UdpEventReceiver::new(localhost(0)).await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let sender = UdpEventSender::new(localhost(0), recv_addr).await.unwrap();

        let event = TimestampedEvent {
            timestamp_us: 100,
            event: InputEvent::MouseMove { x: 10.0, y: 20.0 },
        };

        sender.send(&event).await.unwrap();
        let (received, _from) = receiver.recv().await.unwrap();
        assert_eq!(event, received);
    }

    #[tokio::test]
    async fn send_recv_key_press() {
        let mut receiver = UdpEventReceiver::new(localhost(0)).await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let sender = UdpEventSender::new(localhost(0), recv_addr).await.unwrap();

        let event = TimestampedEvent {
            timestamp_us: 200,
            event: InputEvent::KeyPress {
                key: Key { code: 65 },
                pressed: true,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            },
        };

        sender.send(&event).await.unwrap();
        let (received, _from) = receiver.recv().await.unwrap();
        assert_eq!(event, received);
    }

    #[tokio::test]
    async fn send_recv_mouse_button() {
        let mut receiver = UdpEventReceiver::new(localhost(0)).await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let sender = UdpEventSender::new(localhost(0), recv_addr).await.unwrap();

        let event = TimestampedEvent {
            timestamp_us: 300,
            event: InputEvent::MouseButton {
                button: Button::Left,
                pressed: true,
            },
        };

        sender.send(&event).await.unwrap();
        let (received, _from) = receiver.recv().await.unwrap();
        assert_eq!(event, received);
    }

    #[tokio::test]
    async fn send_recv_multiple_events() {
        let mut receiver = UdpEventReceiver::new(localhost(0)).await.unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let sender = UdpEventSender::new(localhost(0), recv_addr).await.unwrap();

        let events: Vec<TimestampedEvent> = (0..5)
            .map(|i| TimestampedEvent {
                timestamp_us: i as u64 * 100,
                event: InputEvent::MouseMove {
                    x: i as f64,
                    y: i as f64 * 2.0,
                },
            })
            .collect();

        for e in &events {
            sender.send(e).await.unwrap();
        }

        for expected in &events {
            let (received, _) = receiver.recv().await.unwrap();
            assert_eq!(*expected, received);
        }
    }

    #[tokio::test]
    async fn sender_local_addr() {
        let sender = UdpEventSender::new(localhost(0), localhost(9999))
            .await
            .unwrap();
        let addr = sender.local_addr().unwrap();
        assert!(addr.port() > 0);
    }
}
