use crate::event::{Button, InputEvent, Key};
use anyhow::Result;
#[allow(unused_imports)]
use tracing::{debug, warn};

/// Trait abstracting input injection so we can mock it in tests.
pub trait InputInjector {
    fn move_mouse(&mut self, x: i32, y: i32) -> Result<()>;
    fn click(&mut self, button: Button, pressed: bool) -> Result<()>;
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<()>;
    fn key_event(&mut self, key: Key, pressed: bool) -> Result<()>;
}

/// Processes an `InputEvent` by dispatching to the appropriate injector method.
pub fn inject_event(injector: &mut dyn InputInjector, event: &InputEvent) -> Result<()> {
    match event {
        InputEvent::MouseMove { x, y } => {
            injector.move_mouse(*x as i32, *y as i32)?;
        }
        InputEvent::MouseButton { button, pressed } => {
            injector.click(*button, *pressed)?;
        }
        InputEvent::MouseScroll { dx, dy } => {
            injector.scroll(*dx as i32, *dy as i32)?;
        }
        InputEvent::KeyPress {
            key,
            pressed,
            modifiers: _,
        } => {
            injector.key_event(*key, *pressed)?;
        }
        InputEvent::ClipboardUpdate { .. } => {
            debug!("clipboard events handled separately, skipping injection");
        }
    }
    Ok(())
}

/// Remaps coordinates from server resolution to client resolution.
///
/// `server_size`: (width, height) of the server display
/// `client_size`: (width, height) of the client display
/// `pos`: (x, y) in server coordinates
///
/// Returns (x, y) in client coordinates.
pub fn remap_coordinates(
    server_size: (u32, u32),
    client_size: (u32, u32),
    pos: (f64, f64),
) -> (i32, i32) {
    let (sw, sh) = server_size;
    let (cw, ch) = client_size;

    if sw == 0 || sh == 0 {
        warn!("server display has zero dimension, returning origin");
        return (0, 0);
    }

    let x_ratio = cw as f64 / sw as f64;
    let y_ratio = ch as f64 / sh as f64;

    let new_x = (pos.0 * x_ratio).round() as i32;
    let new_y = (pos.1 * y_ratio).round() as i32;

    (
        new_x.clamp(0, cw as i32 - 1),
        new_y.clamp(0, ch as i32 - 1),
    )
}

/// Enigo-based input injector for real OS-level input simulation.
///
/// Only available on desktop platforms (enigo does not compile on Android).
#[cfg(not(target_os = "android"))]
pub struct EnigoInjector {
    enigo: enigo::Enigo,
}

#[cfg(not(target_os = "android"))]
impl EnigoInjector {
    pub fn new() -> Result<Self> {
        let enigo = enigo::Enigo::new(&enigo::Settings::default())
            .map_err(|e| anyhow::anyhow!("failed to create enigo instance: {e}"))?;
        Ok(Self { enigo })
    }
}

#[cfg(not(target_os = "android"))]
impl InputInjector for EnigoInjector {
    fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
        use enigo::{Coordinate, Mouse};
        self.enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| anyhow::anyhow!("mouse move failed: {e}"))?;
        Ok(())
    }

    fn click(&mut self, button: Button, pressed: bool) -> Result<()> {
        use enigo::Mouse;
        let enigo_btn = match button {
            Button::Left => enigo::Button::Left,
            Button::Right => enigo::Button::Right,
            Button::Middle => enigo::Button::Middle,
            Button::Extra(_) => enigo::Button::Left, // fallback
        };
        let direction = if pressed {
            enigo::Direction::Press
        } else {
            enigo::Direction::Release
        };
        self.enigo
            .button(enigo_btn, direction)
            .map_err(|e| anyhow::anyhow!("button click failed: {e}"))?;
        Ok(())
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<()> {
        use enigo::Mouse;
        if dy != 0 {
            self.enigo
                .scroll(dy, enigo::Axis::Vertical)
                .map_err(|e| anyhow::anyhow!("vertical scroll failed: {e}"))?;
        }
        if dx != 0 {
            self.enigo
                .scroll(dx, enigo::Axis::Horizontal)
                .map_err(|e| anyhow::anyhow!("horizontal scroll failed: {e}"))?;
        }
        Ok(())
    }

    fn key_event(&mut self, key: Key, pressed: bool) -> Result<()> {
        use enigo::Keyboard;
        let enigo_key = key_to_enigo(key);
        let direction = if pressed {
            enigo::Direction::Press
        } else {
            enigo::Direction::Release
        };
        self.enigo
            .key(enigo_key, direction)
            .map_err(|e| anyhow::anyhow!("key event failed: {e}"))?;
        Ok(())
    }
}

/// Maps our Key code to an enigo Key.
#[cfg(not(target_os = "android"))]
fn key_to_enigo(key: Key) -> enigo::Key {
    match key.code {
        0x01 => enigo::Key::Alt,
        0x03 => enigo::Key::Backspace,
        0x04 => enigo::Key::CapsLock,
        0x05 | 0x06 => enigo::Key::Control,
        0x07 => enigo::Key::Delete,
        0x08 => enigo::Key::DownArrow,
        0x09 => enigo::Key::End,
        0x0A => enigo::Key::Escape,
        0x0B => enigo::Key::F1,
        0x0C => enigo::Key::F2,
        0x0D => enigo::Key::F3,
        0x0E => enigo::Key::F4,
        0x0F => enigo::Key::F5,
        0x10 => enigo::Key::F6,
        0x11 => enigo::Key::F7,
        0x12 => enigo::Key::F8,
        0x13 => enigo::Key::F9,
        0x14 => enigo::Key::F10,
        0x15 => enigo::Key::F11,
        0x16 => enigo::Key::F12,
        0x17 => enigo::Key::Home,
        0x18 => enigo::Key::LeftArrow,
        0x19 | 0x1A => enigo::Key::Meta,
        0x1B => enigo::Key::PageDown,
        0x1C => enigo::Key::PageUp,
        0x1D => enigo::Key::Return,
        0x1E => enigo::Key::RightArrow,
        0x1F | 0x20 => enigo::Key::Shift,
        0x21 => enigo::Key::Space,
        0x22 => enigo::Key::Tab,
        0x23 => enigo::Key::UpArrow,
        // Letters: 0x41..=0x5A -> Unicode 'a'..'z'
        code @ 0x41..=0x5A => enigo::Key::Unicode((code as u8 + 32) as char),
        // Digits: 0x29..=0x32 -> '1'..'0'
        0x29 => enigo::Key::Unicode('1'),
        0x2A => enigo::Key::Unicode('2'),
        0x2B => enigo::Key::Unicode('3'),
        0x2C => enigo::Key::Unicode('4'),
        0x2D => enigo::Key::Unicode('5'),
        0x2E => enigo::Key::Unicode('6'),
        0x2F => enigo::Key::Unicode('7'),
        0x30 => enigo::Key::Unicode('8'),
        0x31 => enigo::Key::Unicode('9'),
        0x32 => enigo::Key::Unicode('0'),
        0x33 => enigo::Key::Unicode('-'),
        0x34 => enigo::Key::Unicode('='),
        0x28 => enigo::Key::Unicode('`'),
        0x5B => enigo::Key::Unicode('['),
        0x5C => enigo::Key::Unicode('\\'),
        0x5D => enigo::Key::Unicode(']'),
        0x5E => enigo::Key::Unicode(';'),
        0x5F => enigo::Key::Unicode('\''),
        0x60 => enigo::Key::Unicode(','),
        0x61 => enigo::Key::Unicode('.'),
        0x62 => enigo::Key::Unicode('/'),
        _ => enigo::Key::Unicode('?'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{InputEvent, Modifiers};
    use std::sync::Mutex;

    /// Mock injector that records all calls for testing.
    struct MockInjector {
        calls: Mutex<Vec<String>>,
    }

    impl MockInjector {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn call_log(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl InputInjector for MockInjector {
        fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("move_mouse({x},{y})"));
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

    #[test]
    fn inject_mouse_move() {
        let mut mock = MockInjector::new();
        let event = InputEvent::MouseMove { x: 100.0, y: 200.0 };
        inject_event(&mut mock, &event).unwrap();
        assert_eq!(mock.call_log(), vec!["move_mouse(100,200)"]);
    }

    #[test]
    fn inject_mouse_button() {
        let mut mock = MockInjector::new();
        let event = InputEvent::MouseButton {
            button: Button::Left,
            pressed: true,
        };
        inject_event(&mut mock, &event).unwrap();
        assert_eq!(mock.call_log(), vec!["click(Left,true)"]);
    }

    #[test]
    fn inject_mouse_scroll() {
        let mut mock = MockInjector::new();
        let event = InputEvent::MouseScroll { dx: 0.0, dy: -3.0 };
        inject_event(&mut mock, &event).unwrap();
        assert_eq!(mock.call_log(), vec!["scroll(0,-3)"]);
    }

    #[test]
    fn inject_key_press() {
        let mut mock = MockInjector::new();
        let event = InputEvent::KeyPress {
            key: Key { code: 0x41 },
            pressed: true,
            modifiers: Modifiers::default(),
        };
        inject_event(&mut mock, &event).unwrap();
        assert_eq!(mock.call_log(), vec!["key(65,true)"]);
    }

    #[test]
    fn inject_clipboard_is_noop() {
        let mut mock = MockInjector::new();
        let event = InputEvent::ClipboardUpdate {
            content: crate::event::ClipboardContent::Text("test".to_string()),
        };
        inject_event(&mut mock, &event).unwrap();
        assert!(mock.call_log().is_empty());
    }

    #[test]
    fn inject_multiple_events() {
        let mut mock = MockInjector::new();
        let events = vec![
            InputEvent::MouseMove { x: 10.0, y: 20.0 },
            InputEvent::MouseButton {
                button: Button::Left,
                pressed: true,
            },
            InputEvent::MouseButton {
                button: Button::Left,
                pressed: false,
            },
        ];

        for e in &events {
            inject_event(&mut mock, e).unwrap();
        }

        let log = mock.call_log();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0], "move_mouse(10,20)");
        assert_eq!(log[1], "click(Left,true)");
        assert_eq!(log[2], "click(Left,false)");
    }

    #[test]
    fn remap_same_resolution() {
        let (x, y) = remap_coordinates((1920, 1080), (1920, 1080), (960.0, 540.0));
        assert_eq!(x, 960);
        assert_eq!(y, 540);
    }

    #[test]
    fn remap_different_resolution() {
        let (x, y) = remap_coordinates((1920, 1080), (2560, 1440), (960.0, 540.0));
        assert_eq!(x, 1280);
        assert_eq!(y, 720);
    }

    #[test]
    fn remap_clamps_to_bounds() {
        let (x, y) = remap_coordinates((1920, 1080), (800, 600), (1919.0, 1079.0));
        assert!(x < 800);
        assert!(y < 600);
    }

    #[test]
    fn remap_zero_server_size() {
        let (x, y) = remap_coordinates((0, 0), (1920, 1080), (100.0, 200.0));
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }

    #[test]
    fn key_to_enigo_letters() {
        // A=0x41 should map to 'a'
        let e = key_to_enigo(Key { code: 0x41 });
        assert_eq!(e, enigo::Key::Unicode('a'));

        let e = key_to_enigo(Key { code: 0x5A });
        assert_eq!(e, enigo::Key::Unicode('z'));
    }

    #[test]
    fn key_to_enigo_special() {
        assert_eq!(key_to_enigo(Key { code: 0x1D }), enigo::Key::Return);
        assert_eq!(key_to_enigo(Key { code: 0x21 }), enigo::Key::Space);
        assert_eq!(key_to_enigo(Key { code: 0x0A }), enigo::Key::Escape);
    }

    #[test]
    fn key_to_enigo_digits() {
        assert_eq!(key_to_enigo(Key { code: 0x29 }), enigo::Key::Unicode('1'));
        assert_eq!(key_to_enigo(Key { code: 0x32 }), enigo::Key::Unicode('0'));
    }

    #[test]
    fn key_to_enigo_punctuation() {
        assert_eq!(key_to_enigo(Key { code: 0x33 }), enigo::Key::Unicode('-'));
        assert_eq!(key_to_enigo(Key { code: 0x34 }), enigo::Key::Unicode('='));
        assert_eq!(key_to_enigo(Key { code: 0x5B }), enigo::Key::Unicode('['));
        assert_eq!(key_to_enigo(Key { code: 0x5D }), enigo::Key::Unicode(']'));
        assert_eq!(key_to_enigo(Key { code: 0x62 }), enigo::Key::Unicode('/'));
    }

    #[test]
    fn key_to_enigo_modifiers() {
        assert_eq!(key_to_enigo(Key { code: 0x01 }), enigo::Key::Alt);
        assert_eq!(key_to_enigo(Key { code: 0x05 }), enigo::Key::Control);
        assert_eq!(key_to_enigo(Key { code: 0x06 }), enigo::Key::Control);
        assert_eq!(key_to_enigo(Key { code: 0x1F }), enigo::Key::Shift);
        assert_eq!(key_to_enigo(Key { code: 0x19 }), enigo::Key::Meta);
    }

    #[test]
    fn key_to_enigo_unknown_code_returns_question_mark() {
        assert_eq!(key_to_enigo(Key { code: 0xFF }), enigo::Key::Unicode('?'));
    }

    #[test]
    fn remap_corner_cases() {
        // Origin point
        let (x, y) = remap_coordinates((1920, 1080), (2560, 1440), (0.0, 0.0));
        assert_eq!(x, 0);
        assert_eq!(y, 0);

        // Max corner
        let (x, y) = remap_coordinates((1920, 1080), (2560, 1440), (1919.0, 1079.0));
        assert!(x <= 2559);
        assert!(y <= 1439);
    }
}
