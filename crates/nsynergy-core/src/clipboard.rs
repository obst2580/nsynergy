use crate::event::ClipboardContent;
use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Computes a hash of clipboard content for change detection.
fn content_hash(content: &ClipboardContent) -> u64 {
    let mut hasher = DefaultHasher::new();
    match content {
        ClipboardContent::Text(s) => {
            0u8.hash(&mut hasher);
            s.hash(&mut hasher);
        }
        ClipboardContent::Image {
            width,
            height,
            data,
        } => {
            1u8.hash(&mut hasher);
            width.hash(&mut hasher);
            height.hash(&mut hasher);
            data.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Serializes clipboard content to bytes for TCP transmission.
pub fn serialize_clipboard(content: &ClipboardContent) -> Result<Vec<u8>> {
    bincode::serialize(content).context("serializing clipboard content")
}

/// Deserializes clipboard content from bytes received over TCP.
pub fn deserialize_clipboard(data: &[u8]) -> Result<ClipboardContent> {
    bincode::deserialize(data).context("deserializing clipboard content")
}

/// Trait abstracting clipboard access for testability.
pub trait ClipboardProvider {
    fn get_text(&mut self) -> Result<Option<String>>;
    fn set_text(&mut self, text: &str) -> Result<()>;
}

/// Real clipboard provider using arboard.
pub struct ArboardProvider {
    clipboard: arboard::Clipboard,
}

impl ArboardProvider {
    pub fn new() -> Result<Self> {
        let clipboard =
            arboard::Clipboard::new().context("initializing clipboard")?;
        Ok(Self { clipboard })
    }
}

impl ClipboardProvider for ArboardProvider {
    fn get_text(&mut self) -> Result<Option<String>> {
        match self.clipboard.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("clipboard get_text: {e}")),
        }
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        self.clipboard
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("clipboard set_text: {e}"))
    }
}

/// Monitors the local clipboard for changes by polling at a fixed interval.
///
/// Sends `ClipboardContent` through the channel whenever a change is detected.
/// The monitor runs on a dedicated thread since arboard is not async.
pub fn start_clipboard_monitor(
    mut provider: Box<dyn ClipboardProvider + Send>,
    poll_interval_ms: u64,
) -> Result<mpsc::UnboundedReceiver<ClipboardContent>> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::Builder::new()
        .name("clipboard-monitor".to_string())
        .spawn(move || {
            info!(poll_interval_ms, "clipboard monitor started");
            let mut last_hash: u64 = 0;

            loop {
                std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));

                match provider.get_text() {
                    Ok(Some(text)) if !text.is_empty() => {
                        let content = ClipboardContent::Text(text);
                        let hash = content_hash(&content);

                        if hash != last_hash {
                            last_hash = hash;
                            debug!("clipboard changed, sending update");
                            if tx.send(content).is_err() {
                                debug!("clipboard channel closed, stopping monitor");
                                break;
                            }
                        }
                    }
                    Ok(_) => {} // empty or no text
                    Err(e) => {
                        warn!(error = %e, "clipboard read failed");
                    }
                }
            }
        })
        .context("spawning clipboard monitor thread")?;

    Ok(rx)
}

/// Applies received clipboard content to the local clipboard.
pub fn apply_clipboard(
    provider: &mut dyn ClipboardProvider,
    content: &ClipboardContent,
) -> Result<()> {
    match content {
        ClipboardContent::Text(text) => {
            provider.set_text(text)?;
            debug!(len = text.len(), "clipboard text applied");
        }
        ClipboardContent::Image { .. } => {
            warn!("image clipboard not yet supported for apply");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockClipboard {
        text: Arc<Mutex<Option<String>>>,
    }

    impl MockClipboard {
        fn new(initial: Option<&str>) -> Self {
            Self {
                text: Arc::new(Mutex::new(initial.map(|s| s.to_string()))),
            }
        }

        fn get_current(&self) -> Option<String> {
            self.text.lock().unwrap().clone()
        }
    }

    impl ClipboardProvider for MockClipboard {
        fn get_text(&mut self) -> Result<Option<String>> {
            Ok(self.text.lock().unwrap().clone())
        }

        fn set_text(&mut self, text: &str) -> Result<()> {
            *self.text.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
    }

    /// Shared-state mock that can be cloned for monitor tests.
    struct SharedMockClipboard {
        text: Arc<Mutex<Option<String>>>,
    }

    impl SharedMockClipboard {
        fn new() -> Self {
            Self {
                text: Arc::new(Mutex::new(None)),
            }
        }

        fn with_text(text: &str) -> Self {
            Self {
                text: Arc::new(Mutex::new(Some(text.to_string()))),
            }
        }

        fn handle(&self) -> Arc<Mutex<Option<String>>> {
            self.text.clone()
        }
    }

    impl ClipboardProvider for SharedMockClipboard {
        fn get_text(&mut self) -> Result<Option<String>> {
            Ok(self.text.lock().unwrap().clone())
        }

        fn set_text(&mut self, text: &str) -> Result<()> {
            *self.text.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
    }

    #[test]
    fn serialize_deserialize_text() {
        let content = ClipboardContent::Text("hello world".to_string());
        let bytes = serialize_clipboard(&content).unwrap();
        let decoded = deserialize_clipboard(&bytes).unwrap();
        assert_eq!(content, decoded);
    }

    #[test]
    fn serialize_deserialize_image() {
        let content = ClipboardContent::Image {
            width: 2,
            height: 2,
            data: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255],
        };
        let bytes = serialize_clipboard(&content).unwrap();
        let decoded = deserialize_clipboard(&bytes).unwrap();
        assert_eq!(content, decoded);
    }

    #[test]
    fn serialize_deserialize_empty_text() {
        let content = ClipboardContent::Text(String::new());
        let bytes = serialize_clipboard(&content).unwrap();
        let decoded = deserialize_clipboard(&bytes).unwrap();
        assert_eq!(content, decoded);
    }

    #[test]
    fn serialize_deserialize_large_text() {
        let content = ClipboardContent::Text("x".repeat(100_000));
        let bytes = serialize_clipboard(&content).unwrap();
        let decoded = deserialize_clipboard(&bytes).unwrap();
        assert_eq!(content, decoded);
    }

    #[test]
    fn content_hash_differs_for_different_text() {
        let a = ClipboardContent::Text("hello".to_string());
        let b = ClipboardContent::Text("world".to_string());
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn content_hash_same_for_same_text() {
        let a = ClipboardContent::Text("hello".to_string());
        let b = ClipboardContent::Text("hello".to_string());
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn content_hash_differs_text_vs_image() {
        let text = ClipboardContent::Text("hello".to_string());
        let image = ClipboardContent::Image {
            width: 1,
            height: 1,
            data: vec![0, 0, 0, 0],
        };
        assert_ne!(content_hash(&text), content_hash(&image));
    }

    #[test]
    fn apply_clipboard_text() {
        let mut mock = MockClipboard::new(None);
        let content = ClipboardContent::Text("applied text".to_string());
        apply_clipboard(&mut mock, &content).unwrap();
        assert_eq!(mock.get_current(), Some("applied text".to_string()));
    }

    #[test]
    fn apply_clipboard_image_is_noop() {
        let mut mock = MockClipboard::new(Some("original"));
        let content = ClipboardContent::Image {
            width: 1,
            height: 1,
            data: vec![0, 0, 0, 0],
        };
        apply_clipboard(&mut mock, &content).unwrap();
        // Image apply is a no-op, original text should remain
        assert_eq!(mock.get_current(), Some("original".to_string()));
    }

    #[test]
    fn monitor_detects_change() {
        let mock = SharedMockClipboard::with_text("initial");

        let mut rx = start_clipboard_monitor(Box::new(mock), 50).unwrap();

        // Wait for initial detection
        let content = std::thread::spawn(move || {
            // Block on the receiver in a new runtime
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
                    .await
                    .ok()
                    .flatten()
            })
        })
        .join()
        .unwrap();

        assert!(content.is_some());
        if let Some(ClipboardContent::Text(text)) = content {
            assert_eq!(text, "initial");
        } else {
            panic!("expected Text content");
        }
    }

    #[test]
    fn monitor_detects_subsequent_change() {
        let mock = SharedMockClipboard::new();
        let handle = mock.handle();

        let mut rx = start_clipboard_monitor(Box::new(mock), 30).unwrap();

        // Set text after a small delay
        let handle_clone = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            *handle_clone.lock().unwrap() = Some("new text".to_string());
        });

        let content = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
                    .await
                    .ok()
                    .flatten()
            })
        })
        .join()
        .unwrap();

        assert!(content.is_some());
        if let Some(ClipboardContent::Text(text)) = content {
            assert_eq!(text, "new text");
        } else {
            panic!("expected Text content");
        }
    }
}
