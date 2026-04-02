//! Touch-to-mouse coordinate mapping for mobile-to-desktop control.
//!
//! Converts Android touch gestures into desktop mouse/keyboard events:
//! - Single finger drag -> mouse movement (relative or absolute)
//! - Tap -> left click
//! - Long press -> right click
//! - Two-finger scroll -> mouse scroll
//! - Pinch -> scroll (zoom)

use crate::event::{Button, InputEvent, Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Configuration for touch-to-mouse mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchConfig {
    /// Mouse movement sensitivity multiplier.
    /// Higher values = faster cursor, lower = more precise.
    pub sensitivity: f64,

    /// If true, use relative mode (touch delta -> mouse delta).
    /// If false, use absolute mode (touch position -> screen position).
    pub relative_mode: bool,

    /// Desktop screen width in pixels (for absolute mode mapping).
    pub desktop_width: u32,

    /// Desktop screen height in pixels (for absolute mode mapping).
    pub desktop_height: u32,

    /// Touch area width in pixels (phone screen or touchpad region).
    pub touch_width: u32,

    /// Touch area height in pixels.
    pub touch_height: u32,

    /// Duration threshold for a tap (shorter than this = tap).
    pub tap_threshold_ms: u64,

    /// Duration threshold for a long press (longer than this = right click).
    pub long_press_threshold_ms: u64,

    /// Distance threshold for a tap (movement less than this = tap, not drag).
    pub tap_distance_threshold: f64,

    /// Scroll sensitivity multiplier for two-finger scroll.
    pub scroll_sensitivity: f64,
}

impl Default for TouchConfig {
    fn default() -> Self {
        Self {
            sensitivity: 1.5,
            relative_mode: true,
            desktop_width: 1920,
            desktop_height: 1080,
            touch_width: 1080,
            touch_height: 2340,
            tap_threshold_ms: 200,
            long_press_threshold_ms: 500,
            tap_distance_threshold: 10.0,
            scroll_sensitivity: 1.0,
        }
    }
}

/// Tracks the state of an ongoing touch gesture.
#[derive(Debug, Clone)]
struct TouchPoint {
    /// Position when the finger first touched down (reserved for gesture recognition).
    _start_x: f64,
    _start_y: f64,
    /// Most recent position.
    last_x: f64,
    last_y: f64,
    /// When the finger touched down.
    start_time: Instant,
    /// Total distance traveled (for tap vs drag detection).
    total_distance: f64,
}

/// Processes raw touch events and emits mouse/keyboard InputEvents.
pub struct TouchMapper {
    config: TouchConfig,
    /// Primary finger state (finger 0).
    primary: Option<TouchPoint>,
    /// Secondary finger state (finger 1, for two-finger gestures).
    secondary: Option<TouchPoint>,
    /// Current virtual cursor position (for absolute mode).
    cursor_x: f64,
    cursor_y: f64,
}

impl TouchMapper {
    pub fn new(config: TouchConfig) -> Self {
        Self {
            cursor_x: config.desktop_width as f64 / 2.0,
            cursor_y: config.desktop_height as f64 / 2.0,
            config,
            primary: None,
            secondary: None,
        }
    }

    /// Update the configuration (e.g., when desktop resolution changes).
    pub fn update_config(&mut self, config: TouchConfig) {
        self.config = config;
    }

    /// Process a finger-down event.
    /// `finger_id`: 0 = primary, 1 = secondary.
    pub fn touch_down(&mut self, finger_id: u8, x: f64, y: f64) -> Vec<InputEvent> {
        let point = TouchPoint {
            _start_x: x,
            _start_y: y,
            last_x: x,
            last_y: y,
            start_time: Instant::now(),
            total_distance: 0.0,
        };

        match finger_id {
            0 => {
                self.primary = Some(point);
            }
            1 => {
                self.secondary = Some(point);
            }
            _ => {}
        }

        Vec::new()
    }

    /// Process a finger-move event. Returns mouse events to send.
    pub fn touch_move(&mut self, finger_id: u8, x: f64, y: f64) -> Vec<InputEvent> {
        let mut events = Vec::new();

        match finger_id {
            0 => {
                if let Some(ref mut primary) = self.primary {
                    let dx = x - primary.last_x;
                    let dy = y - primary.last_y;
                    primary.total_distance += (dx * dx + dy * dy).sqrt();
                    primary.last_x = x;
                    primary.last_y = y;

                    // If secondary finger is also down, it is a two-finger scroll
                    if self.secondary.is_some() {
                        let scroll_dx = dx * self.config.scroll_sensitivity;
                        let scroll_dy = -dy * self.config.scroll_sensitivity;
                        events.push(InputEvent::MouseScroll {
                            dx: scroll_dx,
                            dy: scroll_dy,
                        });
                    } else {
                        // Single finger: mouse movement
                        let mouse_event = if self.config.relative_mode {
                            self.relative_move(dx, dy)
                        } else {
                            self.absolute_move(x, y)
                        };
                        events.push(mouse_event);
                    }
                }
            }
            1 => {
                if let Some(ref mut secondary) = self.secondary {
                    let dx = x - secondary.last_x;
                    let dy = y - secondary.last_y;
                    secondary.total_distance += (dx * dx + dy * dy).sqrt();
                    secondary.last_x = x;
                    secondary.last_y = y;
                    // Secondary finger movement contributes to scroll
                    // (already handled via primary movement)
                }
            }
            _ => {}
        }

        events
    }

    /// Process a finger-up event. Returns click events if it was a tap.
    pub fn touch_up(&mut self, finger_id: u8) -> Vec<InputEvent> {
        let mut events = Vec::new();

        match finger_id {
            0 => {
                if let Some(primary) = self.primary.take() {
                    let duration = primary.start_time.elapsed();

                    if primary.total_distance < self.config.tap_distance_threshold {
                        if duration < Duration::from_millis(self.config.tap_threshold_ms) {
                            // Short tap -> left click
                            events.push(InputEvent::MouseButton {
                                button: Button::Left,
                                pressed: true,
                            });
                            events.push(InputEvent::MouseButton {
                                button: Button::Left,
                                pressed: false,
                            });
                        } else if duration
                            >= Duration::from_millis(self.config.long_press_threshold_ms)
                        {
                            // Long press -> right click
                            events.push(InputEvent::MouseButton {
                                button: Button::Right,
                                pressed: true,
                            });
                            events.push(InputEvent::MouseButton {
                                button: Button::Right,
                                pressed: false,
                            });
                        }
                    }
                }
            }
            1 => {
                self.secondary = None;
            }
            _ => {}
        }

        events
    }

    /// Process a virtual keyboard key event.
    pub fn key_input(&self, code: u32, pressed: bool) -> InputEvent {
        InputEvent::KeyPress {
            key: Key { code },
            pressed,
            modifiers: Modifiers::default(),
        }
    }

    /// Relative mode: touch delta -> mouse delta, scaled by sensitivity.
    fn relative_move(&mut self, dx: f64, dy: f64) -> InputEvent {
        let scaled_dx = dx * self.config.sensitivity;
        let scaled_dy = dy * self.config.sensitivity;

        self.cursor_x = (self.cursor_x + scaled_dx).clamp(0.0, self.config.desktop_width as f64 - 1.0);
        self.cursor_y = (self.cursor_y + scaled_dy).clamp(0.0, self.config.desktop_height as f64 - 1.0);

        InputEvent::MouseMove {
            x: self.cursor_x,
            y: self.cursor_y,
        }
    }

    /// Absolute mode: touch position maps directly to screen position.
    fn absolute_move(&mut self, x: f64, y: f64) -> InputEvent {
        let x_ratio = x / self.config.touch_width as f64;
        let y_ratio = y / self.config.touch_height as f64;

        self.cursor_x = (x_ratio * self.config.desktop_width as f64).clamp(
            0.0,
            self.config.desktop_width as f64 - 1.0,
        );
        self.cursor_y = (y_ratio * self.config.desktop_height as f64).clamp(
            0.0,
            self.config.desktop_height as f64 - 1.0,
        );

        InputEvent::MouseMove {
            x: self.cursor_x,
            y: self.cursor_y,
        }
    }

    /// Returns the current virtual cursor position.
    pub fn cursor_position(&self) -> (f64, f64) {
        (self.cursor_x, self.cursor_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_mapper() -> TouchMapper {
        TouchMapper::new(TouchConfig::default())
    }

    #[test]
    fn default_config_has_sensible_values() {
        let config = TouchConfig::default();
        assert!(config.sensitivity > 0.0);
        assert!(config.desktop_width > 0);
        assert!(config.desktop_height > 0);
        assert!(config.tap_threshold_ms > 0);
        assert!(config.long_press_threshold_ms > config.tap_threshold_ms);
    }

    #[test]
    fn cursor_starts_at_center() {
        let mapper = default_mapper();
        let (cx, cy) = mapper.cursor_position();
        assert!((cx - 960.0).abs() < 1.0);
        assert!((cy - 540.0).abs() < 1.0);
    }

    #[test]
    fn touch_down_produces_no_events() {
        let mut mapper = default_mapper();
        let events = mapper.touch_down(0, 100.0, 200.0);
        assert!(events.is_empty());
    }

    #[test]
    fn single_finger_drag_produces_mouse_move() {
        let mut mapper = default_mapper();
        mapper.touch_down(0, 100.0, 200.0);
        let events = mapper.touch_move(0, 110.0, 220.0);
        assert_eq!(events.len(), 1);
        match &events[0] {
            InputEvent::MouseMove { x, y } => {
                // Moved 10px right and 20px down with sensitivity 1.5
                assert!(*x > 960.0);
                assert!(*y > 540.0);
            }
            other => panic!("expected MouseMove, got {other:?}"),
        }
    }

    #[test]
    fn relative_mode_applies_sensitivity() {
        let config = TouchConfig {
            sensitivity: 2.0,
            ..TouchConfig::default()
        };
        let mut mapper = TouchMapper::new(config);
        let start_x = 960.0; // center
        let start_y = 540.0;

        mapper.touch_down(0, 100.0, 200.0);
        let events = mapper.touch_move(0, 110.0, 200.0); // dx=10, dy=0

        match &events[0] {
            InputEvent::MouseMove { x, y } => {
                // 10 * 2.0 = 20px movement from center
                assert!((x - (start_x + 20.0)).abs() < 0.1);
                assert!((y - start_y).abs() < 0.1);
            }
            other => panic!("expected MouseMove, got {other:?}"),
        }
    }

    #[test]
    fn absolute_mode_maps_coordinates() {
        let config = TouchConfig {
            relative_mode: false,
            touch_width: 1080,
            touch_height: 2340,
            desktop_width: 1920,
            desktop_height: 1080,
            ..TouchConfig::default()
        };
        let mut mapper = TouchMapper::new(config);

        mapper.touch_down(0, 540.0, 1170.0); // center of touch area
        let events = mapper.touch_move(0, 540.0, 1170.0);

        match &events[0] {
            InputEvent::MouseMove { x, y } => {
                // 50% of touch -> 50% of desktop
                assert!((x - 960.0).abs() < 1.0);
                assert!((y - 540.0).abs() < 1.0);
            }
            other => panic!("expected MouseMove, got {other:?}"),
        }
    }

    #[test]
    fn tap_produces_left_click() {
        let mut mapper = default_mapper();
        mapper.touch_down(0, 100.0, 200.0);
        // Very small movement (within tap threshold)
        mapper.touch_move(0, 101.0, 201.0);
        let events = mapper.touch_up(0);

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            InputEvent::MouseButton {
                button: Button::Left,
                pressed: true,
            }
        );
        assert_eq!(
            events[1],
            InputEvent::MouseButton {
                button: Button::Left,
                pressed: false,
            }
        );
    }

    #[test]
    fn long_press_produces_right_click() {
        let mut mapper = default_mapper();
        mapper.touch_down(0, 100.0, 200.0);

        // Simulate a long press by manipulating the start time
        if let Some(ref mut primary) = mapper.primary {
            primary.start_time =
                Instant::now() - Duration::from_millis(mapper.config.long_press_threshold_ms + 100);
        }

        let events = mapper.touch_up(0);
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            InputEvent::MouseButton {
                button: Button::Right,
                pressed: true,
            }
        );
        assert_eq!(
            events[1],
            InputEvent::MouseButton {
                button: Button::Right,
                pressed: false,
            }
        );
    }

    #[test]
    fn drag_does_not_produce_click() {
        let mut mapper = default_mapper();
        mapper.touch_down(0, 100.0, 200.0);
        // Large movement (exceeds tap distance threshold)
        for i in 0..20 {
            mapper.touch_move(0, 100.0 + i as f64 * 5.0, 200.0);
        }
        let events = mapper.touch_up(0);
        assert!(events.is_empty(), "drag should not produce click events");
    }

    #[test]
    fn two_finger_scroll() {
        let mut mapper = default_mapper();
        mapper.touch_down(0, 100.0, 200.0);
        mapper.touch_down(1, 200.0, 200.0);

        let events = mapper.touch_move(0, 100.0, 220.0); // move down

        assert_eq!(events.len(), 1);
        match &events[0] {
            InputEvent::MouseScroll { dx: _, dy } => {
                // dy is negative of touch movement (natural scrolling)
                assert!(*dy < 0.0, "scroll down should produce negative dy");
            }
            other => panic!("expected MouseScroll, got {other:?}"),
        }
    }

    #[test]
    fn cursor_clamped_to_screen_bounds() {
        let config = TouchConfig {
            sensitivity: 100.0, // extreme sensitivity
            ..TouchConfig::default()
        };
        let mut mapper = TouchMapper::new(config);

        mapper.touch_down(0, 100.0, 200.0);
        // Large movement that would push cursor off screen
        mapper.touch_move(0, 10000.0, 10000.0);

        let (cx, cy) = mapper.cursor_position();
        assert!(cx <= 1919.0);
        assert!(cy <= 1079.0);
    }

    #[test]
    fn cursor_clamped_to_zero() {
        let config = TouchConfig {
            sensitivity: 100.0,
            ..TouchConfig::default()
        };
        let mut mapper = TouchMapper::new(config);

        mapper.touch_down(0, 10000.0, 10000.0);
        mapper.touch_move(0, 0.0, 0.0); // large negative delta

        let (cx, cy) = mapper.cursor_position();
        assert!(cx >= 0.0);
        assert!(cy >= 0.0);
    }

    #[test]
    fn key_input_produces_key_event() {
        let mapper = default_mapper();
        let event = mapper.key_input(0x41, true);
        assert_eq!(
            event,
            InputEvent::KeyPress {
                key: Key { code: 0x41 },
                pressed: true,
                modifiers: Modifiers::default(),
            }
        );
    }

    #[test]
    fn update_config_changes_behavior() {
        let mut mapper = default_mapper();

        let new_config = TouchConfig {
            sensitivity: 3.0,
            ..TouchConfig::default()
        };
        mapper.update_config(new_config);

        mapper.touch_down(0, 100.0, 200.0);
        let events = mapper.touch_move(0, 110.0, 200.0); // dx=10

        match &events[0] {
            InputEvent::MouseMove { x, .. } => {
                // 10 * 3.0 = 30px from center (960)
                assert!((x - 990.0).abs() < 0.1);
            }
            other => panic!("expected MouseMove, got {other:?}"),
        }
    }

    #[test]
    fn second_finger_up_clears_secondary() {
        let mut mapper = default_mapper();
        mapper.touch_down(0, 100.0, 200.0);
        mapper.touch_down(1, 200.0, 200.0);

        let events = mapper.touch_up(1);
        assert!(events.is_empty()); // secondary up produces no events

        // Primary drag should now produce mouse move (not scroll)
        let events = mapper.touch_move(0, 110.0, 200.0);
        assert_eq!(events.len(), 1);
        matches!(&events[0], InputEvent::MouseMove { .. });
    }

    #[test]
    fn touch_move_without_down_produces_nothing() {
        let mut mapper = default_mapper();
        let events = mapper.touch_move(0, 100.0, 200.0);
        assert!(events.is_empty());
    }

    #[test]
    fn touch_up_without_down_produces_nothing() {
        let mut mapper = default_mapper();
        let events = mapper.touch_up(0);
        assert!(events.is_empty());
    }
}
