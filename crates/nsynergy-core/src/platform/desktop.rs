use crate::capture;
use crate::event::TimestampedEvent;
use crate::inject::{EnigoInjector, InputInjector};
use anyhow::Result;
use tokio::sync::mpsc;

use super::InputCapturer;

/// Desktop input capturer using rdev.
pub struct DesktopCapturer;

impl InputCapturer for DesktopCapturer {
    fn start(&self) -> Result<mpsc::UnboundedReceiver<TimestampedEvent>> {
        let handle = capture::start_capture()?;
        Ok(handle.event_rx)
    }
}

/// Creates an enigo-based input injector for desktop platforms.
pub fn create_desktop_injector() -> Result<Box<dyn InputInjector>> {
    let injector = EnigoInjector::new()?;
    Ok(Box::new(injector))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_capturer_implements_trait() {
        let capturer = DesktopCapturer;
        // Verify the type satisfies InputCapturer
        let _: &dyn InputCapturer = &capturer;
    }
}
