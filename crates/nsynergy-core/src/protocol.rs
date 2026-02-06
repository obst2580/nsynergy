use crate::event::TimestampedEvent;
use thiserror::Error;

/// Errors that can occur during protocol serialization/deserialization.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("serialization failed: {0}")]
    Serialize(#[from] bincode::Error),

    #[error("message too large: {size} bytes (max {max} bytes)")]
    MessageTooLarge { size: usize, max: usize },
}

/// Maximum UDP payload size.
/// Standard MTU (1500) minus IP header (20) minus UDP header (8).
pub const MAX_UDP_PAYLOAD: usize = 1472;

/// Serializes a `TimestampedEvent` into a byte vector using bincode.
pub fn serialize_event(event: &TimestampedEvent) -> Result<Vec<u8>, ProtocolError> {
    let bytes = bincode::serialize(event)?;
    if bytes.len() > MAX_UDP_PAYLOAD {
        return Err(ProtocolError::MessageTooLarge {
            size: bytes.len(),
            max: MAX_UDP_PAYLOAD,
        });
    }
    Ok(bytes)
}

/// Deserializes a `TimestampedEvent` from a byte slice.
pub fn deserialize_event(data: &[u8]) -> Result<TimestampedEvent, ProtocolError> {
    let event = bincode::deserialize(data)?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Button, InputEvent, Key, Modifiers};

    fn make_mouse_move(x: f64, y: f64) -> TimestampedEvent {
        TimestampedEvent {
            timestamp_us: 1000,
            event: InputEvent::MouseMove { x, y },
        }
    }

    #[test]
    fn roundtrip_mouse_move() {
        let original = make_mouse_move(42.5, 99.1);
        let bytes = serialize_event(&original).unwrap();
        let decoded = deserialize_event(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn roundtrip_mouse_button() {
        let original = TimestampedEvent {
            timestamp_us: 2000,
            event: InputEvent::MouseButton {
                button: Button::Right,
                pressed: true,
            },
        };
        let bytes = serialize_event(&original).unwrap();
        let decoded = deserialize_event(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn roundtrip_mouse_scroll() {
        let original = TimestampedEvent {
            timestamp_us: 3000,
            event: InputEvent::MouseScroll { dx: -1.0, dy: 3.0 },
        };
        let bytes = serialize_event(&original).unwrap();
        let decoded = deserialize_event(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn roundtrip_key_press() {
        let original = TimestampedEvent {
            timestamp_us: 4000,
            event: InputEvent::KeyPress {
                key: Key { code: 13 },
                pressed: false,
                modifiers: Modifiers {
                    shift: false,
                    ctrl: true,
                    alt: false,
                    meta: true,
                },
            },
        };
        let bytes = serialize_event(&original).unwrap();
        let decoded = deserialize_event(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn roundtrip_clipboard_text() {
        let original = TimestampedEvent {
            timestamp_us: 5000,
            event: InputEvent::ClipboardUpdate {
                content: crate::event::ClipboardContent::Text("Hello, world!".to_string()),
            },
        };
        let bytes = serialize_event(&original).unwrap();
        let decoded = deserialize_event(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn serialized_mouse_move_fits_udp() {
        let event = make_mouse_move(1920.0, 1080.0);
        let bytes = serialize_event(&event).unwrap();
        assert!(
            bytes.len() <= MAX_UDP_PAYLOAD,
            "mouse move serialized to {} bytes, exceeds {}",
            bytes.len(),
            MAX_UDP_PAYLOAD
        );
    }

    #[test]
    fn large_clipboard_rejected_for_udp() {
        let big_text = "x".repeat(MAX_UDP_PAYLOAD + 100);
        let event = TimestampedEvent {
            timestamp_us: 6000,
            event: InputEvent::ClipboardUpdate {
                content: crate::event::ClipboardContent::Text(big_text),
            },
        };
        let result = serialize_event(&event);
        assert!(result.is_err());
        if let Err(ProtocolError::MessageTooLarge { size, max }) = result {
            assert!(size > max);
        } else {
            panic!("expected MessageTooLarge error");
        }
    }

    #[test]
    fn deserialize_garbage_fails() {
        let garbage = vec![0xFF, 0xFE, 0xFD];
        let result = deserialize_event(&garbage);
        assert!(result.is_err());
    }
}
