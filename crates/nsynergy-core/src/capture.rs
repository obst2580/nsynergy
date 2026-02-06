use crate::event::{Button, InputEvent, Key, Modifiers, TimestampedEvent};
use anyhow::{Context, Result};
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Converts an `rdev::Event` to our `InputEvent`.
/// Returns `None` for events we don't care about.
pub fn convert_rdev_event(rdev_event: &rdev::Event) -> Option<InputEvent> {
    match &rdev_event.event_type {
        rdev::EventType::MouseMove { x, y } => Some(InputEvent::MouseMove { x: *x, y: *y }),
        rdev::EventType::ButtonPress(btn) => Some(InputEvent::MouseButton {
            button: convert_button(btn),
            pressed: true,
        }),
        rdev::EventType::ButtonRelease(btn) => Some(InputEvent::MouseButton {
            button: convert_button(btn),
            pressed: false,
        }),
        rdev::EventType::Wheel { delta_x, delta_y } => Some(InputEvent::MouseScroll {
            dx: *delta_x as f64,
            dy: *delta_y as f64,
        }),
        rdev::EventType::KeyPress(key) => Some(InputEvent::KeyPress {
            key: convert_key(key),
            pressed: true,
            modifiers: Modifiers::default(), // TODO: track modifier state
        }),
        rdev::EventType::KeyRelease(key) => Some(InputEvent::KeyPress {
            key: convert_key(key),
            pressed: false,
            modifiers: Modifiers::default(),
        }),
    }
}

fn convert_button(btn: &rdev::Button) -> Button {
    match btn {
        rdev::Button::Left => Button::Left,
        rdev::Button::Right => Button::Right,
        rdev::Button::Middle => Button::Middle,
        rdev::Button::Unknown(n) => Button::Extra(*n),
    }
}

fn convert_key(key: &rdev::Key) -> Key {
    // Map rdev::Key to a u32 code. We use the Debug representation hash
    // as a simple mapping for now. A full keycode table will be needed
    // for cross-platform correctness.
    let code = match key {
        rdev::Key::Alt => 0x01,
        rdev::Key::AltGr => 0x02,
        rdev::Key::Backspace => 0x03,
        rdev::Key::CapsLock => 0x04,
        rdev::Key::ControlLeft => 0x05,
        rdev::Key::ControlRight => 0x06,
        rdev::Key::Delete => 0x07,
        rdev::Key::DownArrow => 0x08,
        rdev::Key::End => 0x09,
        rdev::Key::Escape => 0x0A,
        rdev::Key::F1 => 0x0B,
        rdev::Key::F2 => 0x0C,
        rdev::Key::F3 => 0x0D,
        rdev::Key::F4 => 0x0E,
        rdev::Key::F5 => 0x0F,
        rdev::Key::F6 => 0x10,
        rdev::Key::F7 => 0x11,
        rdev::Key::F8 => 0x12,
        rdev::Key::F9 => 0x13,
        rdev::Key::F10 => 0x14,
        rdev::Key::F11 => 0x15,
        rdev::Key::F12 => 0x16,
        rdev::Key::Home => 0x17,
        rdev::Key::LeftArrow => 0x18,
        rdev::Key::MetaLeft => 0x19,
        rdev::Key::MetaRight => 0x1A,
        rdev::Key::PageDown => 0x1B,
        rdev::Key::PageUp => 0x1C,
        rdev::Key::Return => 0x1D,
        rdev::Key::RightArrow => 0x1E,
        rdev::Key::ShiftLeft => 0x1F,
        rdev::Key::ShiftRight => 0x20,
        rdev::Key::Space => 0x21,
        rdev::Key::Tab => 0x22,
        rdev::Key::UpArrow => 0x23,
        rdev::Key::PrintScreen => 0x24,
        rdev::Key::ScrollLock => 0x25,
        rdev::Key::Pause => 0x26,
        rdev::Key::NumLock => 0x27,
        rdev::Key::BackQuote => 0x28,
        rdev::Key::Num1 => 0x29,
        rdev::Key::Num2 => 0x2A,
        rdev::Key::Num3 => 0x2B,
        rdev::Key::Num4 => 0x2C,
        rdev::Key::Num5 => 0x2D,
        rdev::Key::Num6 => 0x2E,
        rdev::Key::Num7 => 0x2F,
        rdev::Key::Num8 => 0x30,
        rdev::Key::Num9 => 0x31,
        rdev::Key::Num0 => 0x32,
        rdev::Key::Minus => 0x33,
        rdev::Key::Equal => 0x34,
        rdev::Key::KeyA => 0x41,
        rdev::Key::KeyB => 0x42,
        rdev::Key::KeyC => 0x43,
        rdev::Key::KeyD => 0x44,
        rdev::Key::KeyE => 0x45,
        rdev::Key::KeyF => 0x46,
        rdev::Key::KeyG => 0x47,
        rdev::Key::KeyH => 0x48,
        rdev::Key::KeyI => 0x49,
        rdev::Key::KeyJ => 0x4A,
        rdev::Key::KeyK => 0x4B,
        rdev::Key::KeyL => 0x4C,
        rdev::Key::KeyM => 0x4D,
        rdev::Key::KeyN => 0x4E,
        rdev::Key::KeyO => 0x4F,
        rdev::Key::KeyP => 0x50,
        rdev::Key::KeyQ => 0x51,
        rdev::Key::KeyR => 0x52,
        rdev::Key::KeyS => 0x53,
        rdev::Key::KeyT => 0x54,
        rdev::Key::KeyU => 0x55,
        rdev::Key::KeyV => 0x56,
        rdev::Key::KeyW => 0x57,
        rdev::Key::KeyX => 0x58,
        rdev::Key::KeyY => 0x59,
        rdev::Key::KeyZ => 0x5A,
        rdev::Key::LeftBracket => 0x5B,
        rdev::Key::RightBracket => 0x5D,
        rdev::Key::BackSlash => 0x5C,
        rdev::Key::SemiColon => 0x5E,
        rdev::Key::Quote => 0x5F,
        rdev::Key::Comma => 0x60,
        rdev::Key::Dot => 0x61,
        rdev::Key::Slash => 0x62,
        rdev::Key::Insert => 0x63,
        rdev::Key::KpReturn => 0x64,
        rdev::Key::KpMinus => 0x65,
        rdev::Key::KpPlus => 0x66,
        rdev::Key::KpMultiply => 0x67,
        rdev::Key::KpDivide => 0x68,
        rdev::Key::Kp0 => 0x70,
        rdev::Key::Kp1 => 0x71,
        rdev::Key::Kp2 => 0x72,
        rdev::Key::Kp3 => 0x73,
        rdev::Key::Kp4 => 0x74,
        rdev::Key::Kp5 => 0x75,
        rdev::Key::Kp6 => 0x76,
        rdev::Key::Kp7 => 0x77,
        rdev::Key::Kp8 => 0x78,
        rdev::Key::Kp9 => 0x79,
        rdev::Key::KpDelete => 0x7A,
        rdev::Key::Function => 0x7B,
        rdev::Key::IntlBackslash => 0x7C,
        rdev::Key::Unknown(code) => *code as u32,
    };
    Key { code }
}

/// Starts a global input listener on a background thread and sends
/// converted events to the returned channel.
///
/// This function spawns a native thread because `rdev::listen` blocks.
/// Events are bridged to tokio via an `mpsc::unbounded_channel`.
///
/// **Requires Accessibility permissions on macOS.**
pub fn start_capture() -> Result<mpsc::UnboundedReceiver<TimestampedEvent>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let epoch = Instant::now();

    std::thread::Builder::new()
        .name("input-capture".to_string())
        .spawn(move || {
            debug!("input capture thread started");
            let callback = move |event: rdev::Event| {
                if let Some(input_event) = convert_rdev_event(&event) {
                    let timestamped = TimestampedEvent {
                        timestamp_us: epoch.elapsed().as_micros() as u64,
                        event: input_event,
                    };
                    if tx.send(timestamped).is_err() {
                        warn!("capture channel closed, stopping listener");
                    }
                }
            };
            if let Err(e) = rdev::listen(callback) {
                error!(?e, "rdev::listen failed (check Accessibility permissions)");
            }
        })
        .context("spawning input capture thread")?;

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_mouse_move() {
        let rdev_event = rdev::Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type: rdev::EventType::MouseMove { x: 100.0, y: 200.0 },
        };
        let result = convert_rdev_event(&rdev_event);
        assert_eq!(
            result,
            Some(InputEvent::MouseMove { x: 100.0, y: 200.0 })
        );
    }

    #[test]
    fn convert_button_press() {
        let rdev_event = rdev::Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type: rdev::EventType::ButtonPress(rdev::Button::Left),
        };
        let result = convert_rdev_event(&rdev_event);
        assert_eq!(
            result,
            Some(InputEvent::MouseButton {
                button: Button::Left,
                pressed: true,
            })
        );
    }

    #[test]
    fn convert_button_release() {
        let rdev_event = rdev::Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type: rdev::EventType::ButtonRelease(rdev::Button::Right),
        };
        let result = convert_rdev_event(&rdev_event);
        assert_eq!(
            result,
            Some(InputEvent::MouseButton {
                button: Button::Right,
                pressed: false,
            })
        );
    }

    #[test]
    fn convert_scroll() {
        let rdev_event = rdev::Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type: rdev::EventType::Wheel {
                delta_x: 0,
                delta_y: -3,
            },
        };
        let result = convert_rdev_event(&rdev_event);
        assert_eq!(
            result,
            Some(InputEvent::MouseScroll {
                dx: 0.0,
                dy: -3.0,
            })
        );
    }

    #[test]
    fn convert_key_press_release() {
        let press = rdev::Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type: rdev::EventType::KeyPress(rdev::Key::KeyA),
        };
        let release = rdev::Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type: rdev::EventType::KeyRelease(rdev::Key::KeyA),
        };

        let p = convert_rdev_event(&press).unwrap();
        let r = convert_rdev_event(&release).unwrap();

        if let InputEvent::KeyPress { pressed, key, .. } = &p {
            assert!(pressed);
            assert_eq!(key.code, 0x41);
        } else {
            panic!("expected KeyPress");
        }

        if let InputEvent::KeyPress { pressed, key, .. } = &r {
            assert!(!pressed);
            assert_eq!(key.code, 0x41);
        } else {
            panic!("expected KeyPress (release)");
        }
    }

    #[test]
    fn convert_button_mapping() {
        assert_eq!(convert_button(&rdev::Button::Left), Button::Left);
        assert_eq!(convert_button(&rdev::Button::Right), Button::Right);
        assert_eq!(convert_button(&rdev::Button::Middle), Button::Middle);
        assert_eq!(convert_button(&rdev::Button::Unknown(5)), Button::Extra(5));
    }

    #[test]
    fn convert_key_mapping_letters() {
        assert_eq!(convert_key(&rdev::Key::KeyA).code, 0x41);
        assert_eq!(convert_key(&rdev::Key::KeyZ).code, 0x5A);
    }

    #[test]
    fn convert_key_mapping_special() {
        assert_eq!(convert_key(&rdev::Key::Return).code, 0x1D);
        assert_eq!(convert_key(&rdev::Key::Space).code, 0x21);
        assert_eq!(convert_key(&rdev::Key::Escape).code, 0x0A);
        assert_eq!(convert_key(&rdev::Key::Tab).code, 0x22);
    }
}
