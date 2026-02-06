use serde::{Deserialize, Serialize};

/// Mouse button identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Button {
    Left,
    Right,
    Middle,
    /// Extra buttons (e.g., side buttons on gaming mice)
    Extra(u8),
}

/// Keyboard modifier flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Command on macOS, Win on Windows
    pub meta: bool,
}

/// Platform-independent key representation.
///
/// Uses u32 keycodes for now; a full keycode mapping will be added
/// when the input capture layer (rdev) is integrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    /// Platform-independent key code
    pub code: u32,
}

/// Clipboard content that can be shared between machines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    Image {
        width: u32,
        height: u32,
        /// RGBA pixel data
        data: Vec<u8>,
    },
}

/// A single input event that can be sent across the network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove {
        x: f64,
        y: f64,
    },
    MouseButton {
        button: Button,
        pressed: bool,
    },
    MouseScroll {
        dx: f64,
        dy: f64,
    },
    KeyPress {
        key: Key,
        pressed: bool,
        modifiers: Modifiers,
    },
    ClipboardUpdate {
        content: ClipboardContent,
    },
}

/// Wraps an InputEvent with a monotonic timestamp (microseconds)
/// for latency measurement and ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimestampedEvent {
    /// Monotonic timestamp in microseconds since an arbitrary epoch
    pub timestamp_us: u64,
    pub event: InputEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_equality() {
        assert_eq!(Button::Left, Button::Left);
        assert_ne!(Button::Left, Button::Right);
        assert_eq!(Button::Extra(1), Button::Extra(1));
        assert_ne!(Button::Extra(1), Button::Extra(2));
    }

    #[test]
    fn modifiers_default_is_all_false() {
        let m = Modifiers::default();
        assert!(!m.shift);
        assert!(!m.ctrl);
        assert!(!m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn clipboard_content_text() {
        let content = ClipboardContent::Text("hello".to_string());
        if let ClipboardContent::Text(s) = &content {
            assert_eq!(s, "hello");
        } else {
            panic!("expected Text variant");
        }
    }

    #[test]
    fn clipboard_content_image() {
        let data = vec![255, 0, 0, 255]; // one red pixel RGBA
        let content = ClipboardContent::Image {
            width: 1,
            height: 1,
            data: data.clone(),
        };
        if let ClipboardContent::Image {
            width,
            height,
            data: d,
        } = &content
        {
            assert_eq!(*width, 1);
            assert_eq!(*height, 1);
            assert_eq!(d, &data);
        } else {
            panic!("expected Image variant");
        }
    }

    #[test]
    fn input_event_mouse_move() {
        let event = InputEvent::MouseMove { x: 100.0, y: 200.0 };
        if let InputEvent::MouseMove { x, y } = &event {
            assert!((x - 100.0).abs() < f64::EPSILON);
            assert!((y - 200.0).abs() < f64::EPSILON);
        } else {
            panic!("expected MouseMove variant");
        }
    }

    #[test]
    fn timestamped_event_preserves_data() {
        let event = TimestampedEvent {
            timestamp_us: 12345,
            event: InputEvent::KeyPress {
                key: Key { code: 65 },
                pressed: true,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            },
        };
        assert_eq!(event.timestamp_us, 12345);
        if let InputEvent::KeyPress {
            key,
            pressed,
            modifiers,
        } = &event.event
        {
            assert_eq!(key.code, 65);
            assert!(pressed);
            assert!(modifiers.shift);
            assert!(!modifiers.ctrl);
        } else {
            panic!("expected KeyPress variant");
        }
    }
}
