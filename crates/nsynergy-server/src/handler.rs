use anyhow::Result;
use nsynergy_core::config::ScreenPosition;
use nsynergy_core::event::{InputEvent, TimestampedEvent};
use nsynergy_core::screen::{self, DisplayInfo, ScreenEdge};
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Routing mode: where input events should be directed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    /// Events stay on the local machine (normal operation).
    Local,
    /// Events are forwarded to a remote client.
    Remote,
}

/// Maps a `ScreenEdge` to a `ScreenPosition` for neighbor lookup.
pub fn edge_to_position(edge: ScreenEdge) -> ScreenPosition {
    match edge {
        ScreenEdge::Left => ScreenPosition::Left,
        ScreenEdge::Right => ScreenPosition::Right,
        ScreenEdge::Top => ScreenPosition::Top,
        ScreenEdge::Bottom => ScreenPosition::Bottom,
    }
}

/// Describes a connected client that can receive forwarded events.
#[derive(Debug, Clone)]
pub struct ConnectedClient {
    pub name: String,
    pub position: ScreenPosition,
    pub udp_addr: SocketAddr,
    pub display: DisplayInfo,
}

/// The event router decides whether captured events should be processed
/// locally or forwarded to a remote client.
pub struct EventRouter {
    mode: RoutingMode,
    local_display: DisplayInfo,
    edge_threshold: u32,
    clients: Vec<ConnectedClient>,
    /// Channel for events that should be forwarded to a remote client.
    remote_tx: mpsc::UnboundedSender<(TimestampedEvent, SocketAddr)>,
}

impl EventRouter {
    pub fn new(
        local_display: DisplayInfo,
        edge_threshold: u32,
        remote_tx: mpsc::UnboundedSender<(TimestampedEvent, SocketAddr)>,
    ) -> Self {
        Self {
            mode: RoutingMode::Local,
            local_display,
            edge_threshold,
            clients: Vec::new(),
            remote_tx,
        }
    }

    /// Returns the current routing mode.
    pub fn mode(&self) -> RoutingMode {
        self.mode
    }

    /// Registers a connected client.
    pub fn add_client(&mut self, client: ConnectedClient) {
        info!(name = %client.name, position = ?client.position, "client registered");
        self.clients.push(client);
    }

    /// Removes a client by name.
    pub fn remove_client(&mut self, name: &str) {
        self.clients.retain(|c| c.name != name);
    }

    /// Finds a client at the given screen position.
    fn client_at(&self, pos: ScreenPosition) -> Option<&ConnectedClient> {
        self.clients.iter().find(|c| c.position == pos)
    }

    /// Processes a captured event and decides routing.
    ///
    /// Returns `RoutingMode::Remote` if the event was forwarded,
    /// `RoutingMode::Local` if it should be processed locally.
    pub fn route(&mut self, event: &TimestampedEvent) -> Result<RoutingMode> {
        match self.mode {
            RoutingMode::Local => self.route_local(event),
            RoutingMode::Remote => self.route_remote(event),
        }
    }

    fn route_local(&mut self, event: &TimestampedEvent) -> Result<RoutingMode> {
        // Only mouse moves can trigger edge transitions
        if let InputEvent::MouseMove { x, y } = &event.event {
            let ix = *x as i32;
            let iy = *y as i32;

            if let Some(edge) =
                screen::detect_edge(&self.local_display, ix, iy, self.edge_threshold)
            {
                let pos = edge_to_position(edge);
                if let Some(client) = self.client_at(pos) {
                    let (new_x, new_y) = screen::map_position(
                        &self.local_display,
                        &client.display,
                        edge,
                        ix,
                        iy,
                    );

                    debug!(
                        ?edge,
                        client = %client.name,
                        new_x, new_y,
                        "switching to remote mode"
                    );

                    let addr = client.udp_addr;
                    self.mode = RoutingMode::Remote;

                    // Send the remapped mouse move
                    let remapped = TimestampedEvent {
                        timestamp_us: event.timestamp_us,
                        event: InputEvent::MouseMove {
                            x: new_x as f64,
                            y: new_y as f64,
                        },
                    };
                    let _ = self.remote_tx.send((remapped, addr));
                    return Ok(RoutingMode::Remote);
                }
            }
        }

        Ok(RoutingMode::Local)
    }

    fn route_remote(&mut self, event: &TimestampedEvent) -> Result<RoutingMode> {
        // In remote mode, forward all events to the active client.
        // Check if a mouse move returns to local display edge (switch back).
        if let InputEvent::MouseMove { x, y } = &event.event {
            let ix = *x as i32;
            let iy = *y as i32;

            // Find the current remote client
            if let Some(client) = self.clients.first() {
                if let Some(edge) =
                    screen::detect_edge(&client.display, ix, iy, self.edge_threshold)
                {
                    // If cursor hits an edge that leads back to local
                    let return_pos = edge_to_position(edge);
                    let client_pos = client.position;

                    // Opposite edges mean return to local
                    if is_opposite_edge(client_pos, return_pos) {
                        let (new_x, new_y) = screen::map_position(
                            &client.display,
                            &self.local_display,
                            edge,
                            ix,
                            iy,
                        );
                        debug!(new_x, new_y, "switching back to local mode");
                        self.mode = RoutingMode::Local;
                        return Ok(RoutingMode::Local);
                    }
                }
            }

            // Forward event
            if let Some(client) = self.clients.first() {
                let _ = self.remote_tx.send((event.clone(), client.udp_addr));
            }
        } else {
            // Non-mouse events: forward in remote mode
            if let Some(client) = self.clients.first() {
                let _ = self.remote_tx.send((event.clone(), client.udp_addr));
            }
        }

        Ok(RoutingMode::Remote)
    }
}

fn is_opposite_edge(position: ScreenPosition, edge: ScreenPosition) -> bool {
    matches!(
        (position, edge),
        (ScreenPosition::Right, ScreenPosition::Left)
            | (ScreenPosition::Left, ScreenPosition::Right)
            | (ScreenPosition::Top, ScreenPosition::Bottom)
            | (ScreenPosition::Bottom, ScreenPosition::Top)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsynergy_core::config::ScreenPosition;
    use nsynergy_core::event::{Button, Key, Modifiers};
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn local_display() -> DisplayInfo {
        DisplayInfo {
            id: 0,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
        }
    }

    fn remote_display() -> DisplayInfo {
        DisplayInfo {
            id: 1,
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
            scale: 1.0,
        }
    }

    fn test_addr() -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 100), 24800))
    }

    fn make_router() -> (EventRouter, mpsc::UnboundedReceiver<(TimestampedEvent, SocketAddr)>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let router = EventRouter::new(local_display(), 2, tx);
        (router, rx)
    }

    fn mouse_move(x: f64, y: f64) -> TimestampedEvent {
        TimestampedEvent {
            timestamp_us: 1000,
            event: InputEvent::MouseMove { x, y },
        }
    }

    #[test]
    fn starts_in_local_mode() {
        let (router, _rx) = make_router();
        assert_eq!(router.mode(), RoutingMode::Local);
    }

    #[test]
    fn center_mouse_stays_local() {
        let (mut router, _rx) = make_router();
        let event = mouse_move(960.0, 540.0);
        let result = router.route(&event).unwrap();
        assert_eq!(result, RoutingMode::Local);
    }

    #[test]
    fn edge_without_client_stays_local() {
        let (mut router, _rx) = make_router();
        // Move to right edge but no client registered there
        let event = mouse_move(1919.0, 540.0);
        let result = router.route(&event).unwrap();
        assert_eq!(result, RoutingMode::Local);
    }

    #[test]
    fn edge_with_client_switches_to_remote() {
        let (mut router, mut rx) = make_router();
        router.add_client(ConnectedClient {
            name: "remote-pc".to_string(),
            position: ScreenPosition::Right,
            udp_addr: test_addr(),
            display: remote_display(),
        });

        let event = mouse_move(1919.0, 540.0);
        let result = router.route(&event).unwrap();
        assert_eq!(result, RoutingMode::Remote);
        assert_eq!(router.mode(), RoutingMode::Remote);

        // Check that a remapped event was sent
        let (sent_event, addr) = rx.try_recv().unwrap();
        assert_eq!(addr, test_addr());
        if let InputEvent::MouseMove { x, y } = &sent_event.event {
            // Should be remapped to remote display coords (left side entry)
            assert!(*x > 0.0 && *x < 10.0); // near left edge of remote
            assert!(*y > 0.0);
        } else {
            panic!("expected MouseMove");
        }
    }

    #[test]
    fn key_events_forwarded_in_remote_mode() {
        let (mut router, mut rx) = make_router();
        router.add_client(ConnectedClient {
            name: "remote-pc".to_string(),
            position: ScreenPosition::Right,
            udp_addr: test_addr(),
            display: remote_display(),
        });

        // Switch to remote
        let edge_event = mouse_move(1919.0, 540.0);
        router.route(&edge_event).unwrap();
        rx.try_recv().unwrap(); // consume the mouse move

        // Now send a key event
        let key_event = TimestampedEvent {
            timestamp_us: 2000,
            event: InputEvent::KeyPress {
                key: Key { code: 0x41 },
                pressed: true,
                modifiers: Modifiers::default(),
            },
        };
        let result = router.route(&key_event).unwrap();
        assert_eq!(result, RoutingMode::Remote);

        let (sent, _) = rx.try_recv().unwrap();
        assert_eq!(sent, key_event);
    }

    #[test]
    fn button_events_forwarded_in_remote_mode() {
        let (mut router, mut rx) = make_router();
        router.add_client(ConnectedClient {
            name: "remote-pc".to_string(),
            position: ScreenPosition::Right,
            udp_addr: test_addr(),
            display: remote_display(),
        });

        // Switch to remote
        router.route(&mouse_move(1919.0, 540.0)).unwrap();
        rx.try_recv().unwrap();

        let btn_event = TimestampedEvent {
            timestamp_us: 2000,
            event: InputEvent::MouseButton {
                button: Button::Left,
                pressed: true,
            },
        };
        let result = router.route(&btn_event).unwrap();
        assert_eq!(result, RoutingMode::Remote);

        let (sent, _) = rx.try_recv().unwrap();
        assert_eq!(sent, btn_event);
    }

    #[test]
    fn edge_to_position_mapping() {
        assert_eq!(edge_to_position(ScreenEdge::Left), ScreenPosition::Left);
        assert_eq!(edge_to_position(ScreenEdge::Right), ScreenPosition::Right);
        assert_eq!(edge_to_position(ScreenEdge::Top), ScreenPosition::Top);
        assert_eq!(edge_to_position(ScreenEdge::Bottom), ScreenPosition::Bottom);
    }

    #[test]
    fn opposite_edge_detection() {
        assert!(is_opposite_edge(ScreenPosition::Right, ScreenPosition::Left));
        assert!(is_opposite_edge(ScreenPosition::Left, ScreenPosition::Right));
        assert!(is_opposite_edge(ScreenPosition::Top, ScreenPosition::Bottom));
        assert!(is_opposite_edge(ScreenPosition::Bottom, ScreenPosition::Top));
        assert!(!is_opposite_edge(ScreenPosition::Left, ScreenPosition::Left));
        assert!(!is_opposite_edge(ScreenPosition::Left, ScreenPosition::Top));
    }

    #[test]
    fn add_remove_client() {
        let (mut router, _rx) = make_router();
        assert_eq!(router.clients.len(), 0);

        router.add_client(ConnectedClient {
            name: "pc-a".to_string(),
            position: ScreenPosition::Left,
            udp_addr: test_addr(),
            display: remote_display(),
        });
        assert_eq!(router.clients.len(), 1);

        router.remove_client("pc-a");
        assert_eq!(router.clients.len(), 0);
    }
}
