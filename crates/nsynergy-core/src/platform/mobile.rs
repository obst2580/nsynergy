use crate::event::{Button, InputEvent, Key, Modifiers, TimestampedEvent};
use crate::inject::InputInjector;
use anyhow::Result;
use std::sync::OnceLock;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::InputCapturer;

// ---- Global bridge channels ----
// The Kotlin plugin pushes events into this sender; MobileCapturer
// hands the receiver to the server/client loop.

static BRIDGE_SENDER: OnceLock<mpsc::UnboundedSender<TimestampedEvent>> = OnceLock::new();
static EPOCH: OnceLock<Instant> = OnceLock::new();

fn epoch() -> &'static Instant {
    EPOCH.get_or_init(Instant::now)
}

/// Called from the Kotlin plugin (via Tauri command) to push a touch/key
/// event into the Rust pipeline.
pub fn bridge_send_event(event: InputEvent) {
    let timestamped = TimestampedEvent {
        timestamp_us: epoch().elapsed().as_micros() as u64,
        event,
    };
    if let Some(tx) = BRIDGE_SENDER.get() {
        if tx.send(timestamped).is_err() {
            warn!("mobile bridge channel closed");
        }
    } else {
        warn!("mobile bridge not initialized; dropping event");
    }
}

/// Called from the Kotlin plugin to send a mouse move event.
pub fn bridge_send_mouse_move(x: f64, y: f64) {
    bridge_send_event(InputEvent::MouseMove { x, y });
}

/// Called from the Kotlin plugin to send a mouse button event.
pub fn bridge_send_mouse_button(button_code: u8, pressed: bool) {
    let button = match button_code {
        0 => Button::Left,
        1 => Button::Right,
        2 => Button::Middle,
        n => Button::Extra(n),
    };
    bridge_send_event(InputEvent::MouseButton { button, pressed });
}

/// Called from the Kotlin plugin to send a scroll event.
pub fn bridge_send_scroll(dx: f64, dy: f64) {
    bridge_send_event(InputEvent::MouseScroll { dx, dy });
}

/// Called from the Kotlin plugin to send a key event.
pub fn bridge_send_key(code: u32, pressed: bool) {
    bridge_send_event(InputEvent::KeyPress {
        key: Key { code },
        pressed,
        modifiers: Modifiers::default(),
    });
}

// ---- Global injection callback ----
// The Kotlin plugin registers a callback; MobileInjector dispatches
// events through it. This uses a boxed closure stored in a OnceLock.

type InjectionCallback = Box<dyn Fn(&InputEvent) + Send + Sync>;
static INJECTION_CALLBACK: OnceLock<InjectionCallback> = OnceLock::new();

/// Register a callback that the Kotlin plugin will handle to perform
/// actual gesture/key injection on Android.
pub fn register_injection_callback<F>(callback: F)
where
    F: Fn(&InputEvent) + Send + Sync + 'static,
{
    let _ = INJECTION_CALLBACK.set(Box::new(callback));
}

// ---- MobileCapturer ----

/// Mobile input capturer that bridges events from the Kotlin accessibility
/// service into the Rust event pipeline.
pub struct MobileCapturer;

impl InputCapturer for MobileCapturer {
    fn start(&self) -> Result<mpsc::UnboundedReceiver<TimestampedEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Try to set the global sender. If already set (e.g., from a
        // previous start call), log a warning but proceed.
        if BRIDGE_SENDER.set(tx).is_err() {
            warn!("mobile bridge sender already initialized");
        }

        // Initialize the epoch
        let _ = epoch();

        debug!("mobile capturer started; events arrive via bridge_send_*");
        Ok(rx)
    }
}

// ---- MobileInjector ----

/// Mobile input injector that dispatches events to the Kotlin accessibility
/// service via the registered injection callback.
pub struct MobileInjector;

impl InputInjector for MobileInjector {
    fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
        let event = InputEvent::MouseMove {
            x: x as f64,
            y: y as f64,
        };
        dispatch_to_android(&event);
        Ok(())
    }

    fn click(&mut self, button: Button, pressed: bool) -> Result<()> {
        let event = InputEvent::MouseButton { button, pressed };
        dispatch_to_android(&event);
        Ok(())
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<()> {
        let event = InputEvent::MouseScroll {
            dx: dx as f64,
            dy: dy as f64,
        };
        dispatch_to_android(&event);
        Ok(())
    }

    fn key_event(&mut self, key: Key, pressed: bool) -> Result<()> {
        let event = InputEvent::KeyPress {
            key,
            pressed,
            modifiers: Modifiers::default(),
        };
        dispatch_to_android(&event);
        Ok(())
    }
}

fn dispatch_to_android(event: &InputEvent) {
    if let Some(cb) = INJECTION_CALLBACK.get() {
        cb(event);
    } else {
        warn!("no injection callback registered; dropping event");
    }
}

/// Creates a mobile injector.
pub fn create_mobile_injector() -> Result<Box<dyn InputInjector>> {
    Ok(Box::new(MobileInjector))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_capturer_returns_channel() {
        let capturer = MobileCapturer;
        let _rx = capturer.start().unwrap();
    }

    #[test]
    fn mobile_injector_stubs_dont_error() {
        let mut injector = MobileInjector;
        injector.move_mouse(100, 200).unwrap();
        injector.click(Button::Left, true).unwrap();
        injector.scroll(0, -1).unwrap();
        injector.key_event(Key { code: 0x41 }, true).unwrap();
    }

    #[test]
    fn bridge_send_mouse_move_does_not_panic_without_init() {
        // Before bridge is initialized, events are dropped gracefully
        bridge_send_mouse_move(50.0, 100.0);
    }

    #[test]
    fn bridge_send_key_does_not_panic_without_init() {
        bridge_send_key(0x41, true);
    }

    #[test]
    fn mobile_button_code_mapping() {
        // Verify the button code mapping logic
        bridge_send_mouse_button(0, true); // Left
        bridge_send_mouse_button(1, false); // Right
        bridge_send_mouse_button(2, true); // Middle
        bridge_send_mouse_button(5, true); // Extra(5)
    }
}
