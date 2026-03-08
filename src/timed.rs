//! Timed clipboard that automatically clears after a duration.
//!
//! Useful for password managers and other security-sensitive applications
//! where clipboard contents should not persist indefinitely.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::clipboard::{ClipboardProvider, HasamiError};

/// A clipboard wrapper that can automatically clear its contents after
/// a specified duration.
///
/// Uses tokio tasks for the delayed clear, so a tokio runtime must be
/// available when calling [`copy_sensitive`](Self::copy_sensitive).
///
/// Each new `copy_sensitive` call cancels the previous timer by bumping
/// a generation counter -- only the latest timer will actually clear.
pub struct TimedClipboard<C: ClipboardProvider> {
    provider: Arc<C>,
    generation: Arc<AtomicU64>,
}

impl<C: ClipboardProvider + 'static> TimedClipboard<C> {
    /// Create a new `TimedClipboard` wrapping the given provider.
    #[must_use]
    pub fn new(provider: Arc<C>) -> Self {
        Self {
            provider,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Copy text to the clipboard and schedule automatic clearing.
    ///
    /// The clipboard will be cleared after `clear_after` elapses, but only
    /// if no newer `copy_sensitive` call has been made (previous timers are
    /// implicitly cancelled via a generation counter).
    pub fn copy_sensitive(
        &self,
        text: &str,
        clear_after: Duration,
    ) -> Result<(), HasamiError> {
        self.provider.copy_text(text)?;

        // Bump generation to cancel any pending timer
        let current_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gen_ref = Arc::clone(&self.generation);
        let cb = Arc::clone(&self.provider);

        tokio::spawn(async move {
            tokio::time::sleep(clear_after).await;

            // Only clear if this is still the latest generation
            if gen_ref.load(Ordering::SeqCst) != current_gen {
                tracing::debug!("timer cancelled by newer copy_sensitive call");
                return;
            }

            if let Err(e) = cb.clear() {
                tracing::warn!(error = %e, "failed to clear clipboard after timeout");
            } else {
                tracing::debug!(
                    secs = clear_after.as_secs(),
                    "clipboard auto-cleared after timeout"
                );
            }
        });

        Ok(())
    }

    /// Copy text to the clipboard without scheduling a clear.
    pub fn copy_text(&self, text: &str) -> Result<(), HasamiError> {
        self.provider.copy_text(text)
    }

    /// Access the underlying clipboard provider.
    #[must_use]
    pub fn provider(&self) -> &C {
        &self.provider
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::MockClipboard;

    #[test]
    fn timed_clipboard_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TimedClipboard<MockClipboard>>();
    }

    #[tokio::test]
    async fn copy_text_delegates() {
        let mock = Arc::new(MockClipboard::new());
        let timed = TimedClipboard::new(Arc::clone(&mock));

        timed.copy_text("delegated").unwrap();
        assert_eq!(mock.paste_text().unwrap(), "delegated");
    }

    #[tokio::test]
    async fn copy_sensitive_clears_after_duration() {
        let mock = Arc::new(MockClipboard::new());
        let timed = TimedClipboard::new(Arc::clone(&mock));

        timed
            .copy_sensitive("secret", Duration::from_millis(50))
            .unwrap();
        assert_eq!(mock.paste_text().unwrap(), "secret");

        // Wait for the auto-clear to fire
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(mock.paste_text().is_err(), "clipboard should be cleared");
    }

    #[tokio::test]
    async fn copy_sensitive_cancels_previous_timer() {
        let mock = Arc::new(MockClipboard::new());
        let timed = TimedClipboard::new(Arc::clone(&mock));

        // First sensitive copy with 100ms timer
        timed
            .copy_sensitive("first", Duration::from_millis(100))
            .unwrap();

        // After 30ms, copy again -- cancels first timer
        tokio::time::sleep(Duration::from_millis(30)).await;
        timed
            .copy_sensitive("second", Duration::from_millis(100))
            .unwrap();

        // At 80ms total: first timer would have fired at 100ms but is cancelled
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should still contain "second"
        assert_eq!(mock.paste_text().unwrap(), "second");

        // Wait past second timer (130ms total > 30+100=130)
        tokio::time::sleep(Duration::from_millis(80)).await;

        assert!(
            mock.paste_text().is_err(),
            "clipboard should be cleared by second timer"
        );
    }

    #[tokio::test]
    async fn copy_sensitive_does_not_clear_before_duration() {
        let mock = Arc::new(MockClipboard::new());
        let timed = TimedClipboard::new(Arc::clone(&mock));

        timed
            .copy_sensitive("still here", Duration::from_millis(200))
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(mock.paste_text().unwrap(), "still here");
    }

    #[tokio::test]
    async fn provider_accessor() {
        let mock = Arc::new(MockClipboard::new());
        let timed = TimedClipboard::new(Arc::clone(&mock));

        timed.provider().copy_text("via provider").unwrap();
        assert_eq!(mock.paste_text().unwrap(), "via provider");
    }
}
