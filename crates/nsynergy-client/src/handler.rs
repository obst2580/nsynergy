use anyhow::{Context, Result};
use nsynergy_core::event::{InputEvent, TimestampedEvent};
use nsynergy_core::inject::{self, InputInjector};
use nsynergy_core::screen::DisplayInfo;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Statistics tracked by the client handler.
#[derive(Debug, Clone, Default)]
pub struct ClientStats {
    pub events_received: u64,
    pub events_injected: u64,
    pub injection_errors: u64,
    pub last_event_latency_us: u64,
}

/// The client handler receives events from the network and injects them
/// into the local OS via an `InputInjector`.
pub struct ClientHandler {
    injector: Box<dyn InputInjector>,
    local_display: DisplayInfo,
    server_display: Option<DisplayInfo>,
    stats: ClientStats,
}

impl ClientHandler {
    pub fn new(injector: Box<dyn InputInjector>, local_display: DisplayInfo) -> Self {
        Self {
            injector,
            local_display,
            server_display: None,
            stats: ClientStats::default(),
        }
    }

    /// Sets the server's display info for coordinate remapping.
    pub fn set_server_display(&mut self, server_disp: DisplayInfo) {
        info!(
            w = server_disp.width,
            h = server_disp.height,
            "server display info updated"
        );
        self.server_display = Some(server_disp);
    }

    /// Returns current statistics.
    pub fn stats(&self) -> &ClientStats {
        &self.stats
    }

    /// Processes a single received event.
    pub fn handle_event(&mut self, event: &TimestampedEvent) -> Result<()> {
        self.stats.events_received += 1;

        let processed_event = self.remap_if_needed(&event.event);

        match inject::inject_event(self.injector.as_mut(), &processed_event) {
            Ok(()) => {
                self.stats.events_injected += 1;
            }
            Err(e) => {
                self.stats.injection_errors += 1;
                warn!(error = %e, "injection failed");
                return Err(e).context("injecting event");
            }
        }

        Ok(())
    }

    /// Remaps mouse coordinates if server display info is available
    /// and different from local.
    fn remap_if_needed(&self, event: &InputEvent) -> InputEvent {
        match event {
            InputEvent::MouseMove { x, y } => {
                if let Some(server) = &self.server_display {
                    let (new_x, new_y) = inject::remap_coordinates(
                        (server.width, server.height),
                        (self.local_display.width, self.local_display.height),
                        (*x, *y),
                    );
                    InputEvent::MouseMove {
                        x: new_x as f64,
                        y: new_y as f64,
                    }
                } else {
                    event.clone()
                }
            }
            _ => event.clone(),
        }
    }
}

/// Runs the client event loop: receives events from a channel and injects them.
///
/// This is the main async loop for the client side.
pub async fn run_client_loop(
    mut handler: ClientHandler,
    mut event_rx: mpsc::UnboundedReceiver<TimestampedEvent>,
) {
    info!("client event loop started");

    while let Some(event) = event_rx.recv().await {
        if let Err(e) = handler.handle_event(&event) {
            warn!(error = %e, "failed to handle event");
        }
    }

    info!(
        events_received = handler.stats.events_received,
        events_injected = handler.stats.events_injected,
        errors = handler.stats.injection_errors,
        "client event loop ended"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsynergy_core::event::{Button, ClipboardContent, Key, Modifiers};
    use std::sync::{Arc, Mutex};

    struct RecordingInjector {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingInjector {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    calls: calls.clone(),
                },
                calls,
            )
        }
    }

    impl InputInjector for RecordingInjector {
        fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("move({x},{y})"));
            Ok(())
        }

        fn click(&mut self, button: Button, pressed: bool) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("click({button:?},{pressed})"));
            Ok(())
        }

        fn scroll(&mut self, dx: i32, dy: i32) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("scroll({dx},{dy})"));
            Ok(())
        }

        fn key_event(&mut self, key: Key, pressed: bool) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("key({},{})", key.code, pressed));
            Ok(())
        }
    }

    fn make_display(w: u32, h: u32) -> DisplayInfo {
        DisplayInfo {
            id: 0,
            x: 0,
            y: 0,
            width: w,
            height: h,
            scale: 1.0,
        }
    }

    fn ts_event(event: InputEvent) -> TimestampedEvent {
        TimestampedEvent {
            timestamp_us: 1000,
            event,
        }
    }

    #[test]
    fn handle_mouse_move() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        let event = ts_event(InputEvent::MouseMove { x: 100.0, y: 200.0 });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert_eq!(log[0], "move(100,200)");
        assert_eq!(handler.stats().events_received, 1);
        assert_eq!(handler.stats().events_injected, 1);
    }

    #[test]
    fn handle_key_press() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        let event = ts_event(InputEvent::KeyPress {
            key: Key { code: 0x41 },
            pressed: true,
            modifiers: Modifiers::default(),
        });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert_eq!(log[0], "key(65,true)");
    }

    #[test]
    fn handle_button_click() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        let event = ts_event(InputEvent::MouseButton {
            button: Button::Right,
            pressed: true,
        });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert_eq!(log[0], "click(Right,true)");
    }

    #[test]
    fn handle_scroll() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        let event = ts_event(InputEvent::MouseScroll { dx: 0.0, dy: -3.0 });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert_eq!(log[0], "scroll(0,-3)");
    }

    #[test]
    fn remap_coordinates_when_server_display_set() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(2560, 1440));
        handler.set_server_display(make_display(1920, 1080));

        // Server sends (960, 540) which is center of 1920x1080
        // Should remap to center of 2560x1440 = (1280, 720)
        let event = ts_event(InputEvent::MouseMove { x: 960.0, y: 540.0 });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert_eq!(log[0], "move(1280,720)");
    }

    #[test]
    fn no_remap_without_server_display() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(2560, 1440));
        // No server display set -> coordinates pass through

        let event = ts_event(InputEvent::MouseMove { x: 960.0, y: 540.0 });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert_eq!(log[0], "move(960,540)");
    }

    #[test]
    fn clipboard_event_does_not_inject() {
        let (injector, calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        let event = ts_event(InputEvent::ClipboardUpdate {
            content: ClipboardContent::Text("test".to_string()),
        });
        handler.handle_event(&event).unwrap();

        let log = calls.lock().unwrap();
        assert!(log.is_empty());
        assert_eq!(handler.stats().events_received, 1);
        assert_eq!(handler.stats().events_injected, 1); // still counts as success
    }

    #[test]
    fn stats_track_multiple_events() {
        let (injector, _calls) = RecordingInjector::new();
        let mut handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        for i in 0..10 {
            let event = ts_event(InputEvent::MouseMove {
                x: i as f64 * 10.0,
                y: i as f64 * 20.0,
            });
            handler.handle_event(&event).unwrap();
        }

        assert_eq!(handler.stats().events_received, 10);
        assert_eq!(handler.stats().events_injected, 10);
        assert_eq!(handler.stats().injection_errors, 0);
    }

    #[tokio::test]
    async fn client_loop_processes_events() {
        let (injector, calls) = RecordingInjector::new();
        let handler = ClientHandler::new(Box::new(injector), make_display(1920, 1080));

        let (tx, rx) = mpsc::unbounded_channel();

        // Send some events then close
        tx.send(ts_event(InputEvent::MouseMove { x: 10.0, y: 20.0 }))
            .unwrap();
        tx.send(ts_event(InputEvent::KeyPress {
            key: Key { code: 0x41 },
            pressed: true,
            modifiers: Modifiers::default(),
        }))
        .unwrap();
        drop(tx);

        run_client_loop(handler, rx).await;

        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0], "move(10,20)");
        assert_eq!(log[1], "key(65,true)");
    }
}
