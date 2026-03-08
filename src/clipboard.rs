//! Core clipboard abstraction with trait-based provider pattern.
//!
//! Provides a `ClipboardProvider` trait for clipboard operations, a real
//! implementation wrapping `arboard::Clipboard`, and a `MockClipboard`
//! for testing without a display server.

use std::sync::{Arc, Mutex};

use thiserror::Error;

/// Errors that can occur during clipboard operations.
#[derive(Debug, Error)]
pub enum HasamiError {
    /// The system clipboard could not be accessed.
    #[error("clipboard access error: {0}")]
    ClipboardAccess(String),

    /// A timed clear operation timed out.
    #[error("clipboard clear timed out")]
    Timeout,

    /// The clipboard is empty (no text content).
    #[error("clipboard is empty")]
    Empty,
}

impl From<arboard::Error> for HasamiError {
    fn from(e: arboard::Error) -> Self {
        Self::ClipboardAccess(e.to_string())
    }
}

/// Trait abstracting clipboard read/write/clear operations.
///
/// Implementations must be thread-safe (`Send + Sync`) so they can be
/// shared across async tasks and threads.
pub trait ClipboardProvider: Send + Sync {
    /// Copy text to the clipboard.
    fn copy_text(&self, text: &str) -> Result<(), HasamiError>;

    /// Read the current text from the clipboard.
    fn paste_text(&self) -> Result<String, HasamiError>;

    /// Clear the clipboard contents.
    fn clear(&self) -> Result<(), HasamiError>;
}

/// Thread-safe clipboard handle wrapping `arboard::Clipboard`.
///
/// All operations lock briefly on an internal mutex and return immediately.
pub struct Clipboard {
    inner: Arc<Mutex<arboard::Clipboard>>,
}

impl Clipboard {
    /// Create a new clipboard handle.
    ///
    /// # Errors
    ///
    /// Returns `HasamiError::ClipboardAccess` if the system clipboard cannot
    /// be opened (e.g., no display server on Linux).
    pub fn new() -> Result<Self, HasamiError> {
        let inner = arboard::Clipboard::new()?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

impl ClipboardProvider for Clipboard {
    fn copy_text(&self, text: &str) -> Result<(), HasamiError> {
        let mut cb = self.inner.lock().expect("clipboard mutex poisoned");
        cb.set_text(text)?;
        tracing::debug!(len = text.len(), "copied text to clipboard");
        Ok(())
    }

    fn paste_text(&self) -> Result<String, HasamiError> {
        let mut cb = self.inner.lock().expect("clipboard mutex poisoned");
        let text = cb.get_text()?;
        if text.is_empty() {
            return Err(HasamiError::Empty);
        }
        Ok(text)
    }

    fn clear(&self) -> Result<(), HasamiError> {
        let mut cb = self.inner.lock().expect("clipboard mutex poisoned");
        cb.clear()?;
        tracing::debug!("clipboard cleared");
        Ok(())
    }
}

/// Mock clipboard for testing without a real display server.
///
/// Stores clipboard contents in an `Arc<Mutex<Option<String>>>` so it
/// can be cloned and shared across threads.
#[derive(Debug, Clone)]
pub struct MockClipboard {
    contents: Arc<Mutex<Option<String>>>,
}

impl MockClipboard {
    /// Create a new mock clipboard (initially empty).
    #[must_use]
    pub fn new() -> Self {
        Self {
            contents: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for MockClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for MockClipboard {
    fn copy_text(&self, text: &str) -> Result<(), HasamiError> {
        let mut guard = self.contents.lock().expect("mock mutex poisoned");
        *guard = Some(text.to_owned());
        Ok(())
    }

    fn paste_text(&self) -> Result<String, HasamiError> {
        let guard = self.contents.lock().expect("mock mutex poisoned");
        guard.clone().ok_or(HasamiError::Empty)
    }

    fn clear(&self) -> Result<(), HasamiError> {
        let mut guard = self.contents.lock().expect("mock mutex poisoned");
        *guard = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_copy_paste_roundtrip() {
        let mock = MockClipboard::new();
        mock.copy_text("hello hasami").unwrap();
        let text = mock.paste_text().unwrap();
        assert_eq!(text, "hello hasami");
    }

    #[test]
    fn mock_overwrite() {
        let mock = MockClipboard::new();
        mock.copy_text("first").unwrap();
        mock.copy_text("second").unwrap();
        assert_eq!(mock.paste_text().unwrap(), "second");
    }

    #[test]
    fn mock_clear() {
        let mock = MockClipboard::new();
        mock.copy_text("data").unwrap();
        mock.clear().unwrap();
        assert!(mock.paste_text().is_err());
    }

    #[test]
    fn mock_paste_empty_returns_error() {
        let mock = MockClipboard::new();
        let result = mock.paste_text();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HasamiError::Empty));
    }

    #[test]
    fn mock_clear_when_empty_is_ok() {
        let mock = MockClipboard::new();
        assert!(mock.clear().is_ok());
    }

    #[test]
    fn mock_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockClipboard>();
    }

    #[test]
    fn error_display() {
        let err = HasamiError::ClipboardAccess("test error".into());
        assert_eq!(err.to_string(), "clipboard access error: test error");

        let err = HasamiError::Timeout;
        assert_eq!(err.to_string(), "clipboard clear timed out");

        let err = HasamiError::Empty;
        assert_eq!(err.to_string(), "clipboard is empty");
    }
}
