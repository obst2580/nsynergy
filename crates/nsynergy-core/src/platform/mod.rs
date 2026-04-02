/// Platform abstraction layer for input capture and injection.
///
/// On desktop (macOS, Windows, Linux): uses rdev for capture, enigo for injection.
/// On mobile (Android, iOS): provides stubs; actual implementation lives in
/// platform-native code (Kotlin/Swift) bridged via Tauri plugins.

#[cfg(not(target_os = "android"))]
mod desktop;

#[cfg(target_os = "android")]
mod mobile;

#[cfg(not(target_os = "android"))]
pub use desktop::*;

#[cfg(target_os = "android")]
pub use mobile::*;

use crate::event::TimestampedEvent;
use anyhow::Result;
use tokio::sync::mpsc;

/// Trait for platform-specific input capture.
///
/// Implementors capture global keyboard/mouse events and send them
/// through a channel for processing.
pub trait InputCapturer {
    /// Starts capturing input events.
    /// Returns a receiver for captured events.
    fn start(&self) -> Result<mpsc::UnboundedReceiver<TimestampedEvent>>;
}

/// Trait for platform-specific input injection.
///
/// This re-exports the existing `InputInjector` trait from `inject.rs`.
/// Note: On macOS, `enigo::Enigo` is `!Send`, so this trait
/// must NOT have a `Send` bound.
pub use crate::inject::InputInjector;

/// Creates the platform-appropriate input capturer.
pub fn create_capturer() -> Box<dyn InputCapturer> {
    #[cfg(not(target_os = "android"))]
    {
        Box::new(desktop::DesktopCapturer)
    }
    #[cfg(target_os = "android")]
    {
        Box::new(mobile::MobileCapturer)
    }
}

/// Creates the platform-appropriate input injector.
pub fn create_injector() -> Result<Box<dyn InputInjector>> {
    #[cfg(not(target_os = "android"))]
    {
        desktop::create_desktop_injector()
    }
    #[cfg(target_os = "android")]
    {
        mobile::create_mobile_injector()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_capturer_returns_valid_instance() {
        let capturer = create_capturer();
        // Just verify it doesn't panic to create
        let _ = capturer;
    }
}
