//! Integration tests for hasami — clipboard, history, and timed clearing
//! working together across module boundaries.

use std::sync::Arc;
use std::time::Duration;

use hasami::{
    ClipboardHistory, ClipboardProvider, HasamiError, MockClipboard, TimedClipboard,
};

// ---------------------------------------------------------------------------
// ClipboardProvider + ClipboardHistory integration
// ---------------------------------------------------------------------------

#[test]
fn mock_clipboard_feeds_history() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(50);

    let texts = ["first copy", "second copy", "third copy"];
    for text in &texts {
        mock.copy_text(text).unwrap();
        let pasted = mock.paste_text().unwrap();
        history.push(&pasted);
    }

    assert_eq!(history.len(), 3);
    let recent = history.recent(3);
    assert_eq!(recent[0].text, "first copy");
    assert_eq!(recent[1].text, "second copy");
    assert_eq!(recent[2].text, "third copy");
}

#[test]
fn history_search_finds_clipboard_content() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(100);

    let snippets = [
        "https://example.com/page1",
        "ssh-rsa AAAAB3...",
        "https://example.com/page2",
        "SELECT * FROM users",
        "https://docs.rs/hasami",
    ];

    for s in &snippets {
        mock.copy_text(s).unwrap();
        history.push(&mock.paste_text().unwrap());
    }

    let urls = history.search("https://");
    assert_eq!(urls.len(), 3);

    let sql = history.search("SELECT");
    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0].text, "SELECT * FROM users");
}

#[test]
fn clipboard_clear_does_not_affect_history() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(10);

    mock.copy_text("preserved in history").unwrap();
    history.push(&mock.paste_text().unwrap());

    mock.clear().unwrap();

    // Clipboard is empty now
    assert!(mock.paste_text().is_err());

    // But history retains the entry
    assert_eq!(history.len(), 1);
    assert_eq!(history.recent(1)[0].text, "preserved in history");
}

// ---------------------------------------------------------------------------
// TimedClipboard + ClipboardHistory integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timed_clipboard_with_history_tracking() {
    let mock = Arc::new(MockClipboard::new());
    let timed = TimedClipboard::new(Arc::clone(&mock));
    let mut history = ClipboardHistory::new(50);

    // Copy sensitive data and track in history
    timed
        .copy_sensitive("password123", Duration::from_millis(80))
        .unwrap();
    history.push(&mock.paste_text().unwrap());

    // Copy normal data
    timed.copy_text("public note").unwrap();
    history.push(&mock.paste_text().unwrap());

    // History has both entries
    assert_eq!(history.len(), 2);

    // Wait for timed clear (the sensitive timer will fire and clear)
    tokio::time::sleep(Duration::from_millis(150)).await;

    // History still has both entries even after clipboard clear
    assert_eq!(history.len(), 2);
    let results = history.search("password");
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn sequential_sensitive_copies_only_last_timer_fires() {
    let mock = Arc::new(MockClipboard::new());
    let timed = TimedClipboard::new(Arc::clone(&mock));
    let mut history = ClipboardHistory::new(10);

    for i in 0..5 {
        let text = format!("secret-{i}");
        timed
            .copy_sensitive(&text, Duration::from_millis(100))
            .unwrap();
        history.push(&mock.paste_text().unwrap());
    }

    // All 5 entries in history
    assert_eq!(history.len(), 5);

    // Clipboard currently has last value
    assert_eq!(mock.paste_text().unwrap(), "secret-4");

    // After timeout, clipboard is cleared by the last timer
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(mock.paste_text().is_err());
}

// ---------------------------------------------------------------------------
// Trait object usage
// ---------------------------------------------------------------------------

#[test]
fn clipboard_provider_as_trait_object() {
    let provider: Box<dyn ClipboardProvider> = Box::new(MockClipboard::new());

    provider.copy_text("trait object test").unwrap();
    assert_eq!(provider.paste_text().unwrap(), "trait object test");

    provider.clear().unwrap();
    assert!(provider.paste_text().is_err());
}

#[test]
fn arc_clipboard_provider_shared_across_scopes() {
    let provider: Arc<dyn ClipboardProvider> = Arc::new(MockClipboard::new());

    let writer = Arc::clone(&provider);
    let reader = Arc::clone(&provider);

    writer.copy_text("shared access").unwrap();
    assert_eq!(reader.paste_text().unwrap(), "shared access");
}

// ---------------------------------------------------------------------------
// History deduplication + clipboard interaction
// ---------------------------------------------------------------------------

#[test]
fn dedup_prevents_repeated_paste_tracking() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(100);

    mock.copy_text("same content").unwrap();

    // Simulate multiple paste-and-track cycles with the same content
    for _ in 0..10 {
        let text = mock.paste_text().unwrap();
        history.push(&text);
    }

    // Only one entry due to dedup
    assert_eq!(history.len(), 1);
}

#[test]
fn history_eviction_under_high_throughput() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(5);

    for i in 0..1000 {
        mock.copy_text(&format!("item-{i}")).unwrap();
        history.push(&mock.paste_text().unwrap());
    }

    assert_eq!(history.len(), 5);
    let recent = history.recent(5);
    assert_eq!(recent[0].text, "item-995");
    assert_eq!(recent[4].text, "item-999");
}

// ---------------------------------------------------------------------------
// Unicode and special content across modules
// ---------------------------------------------------------------------------

#[test]
fn unicode_through_full_pipeline() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(10);

    let unicode_texts = [
        "鋏 はさみ",
        "Clipboard: 📋",
        "Emoji chain: 🎉🎊🎈",
        "Mixed: hello世界",
        "CJK: 你好世界こんにちは",
        "Arabic: مرحبا",
        "Zalgo: H̷e̸l̵l̶o̴",
    ];

    for text in &unicode_texts {
        mock.copy_text(text).unwrap();
        let pasted = mock.paste_text().unwrap();
        assert_eq!(&pasted, text);
        history.push(&pasted);
    }

    assert_eq!(history.len(), unicode_texts.len());

    // Search works with unicode
    let results = history.search("世界");
    assert_eq!(results.len(), 2);
}

#[test]
fn empty_string_handling_across_modules() {
    let mock = MockClipboard::new();
    let mut history = ClipboardHistory::new(10);

    // MockClipboard stores empty string as Some("")
    mock.copy_text("").unwrap();
    let pasted = mock.paste_text().unwrap();
    assert_eq!(pasted, "");

    // History should accept empty strings
    history.push(&pasted);
    assert_eq!(history.len(), 1);
    assert_eq!(history.recent(1)[0].text, "");
}

// ---------------------------------------------------------------------------
// Error handling integration
// ---------------------------------------------------------------------------

#[test]
fn error_type_is_consistent_across_operations() {
    let mock = MockClipboard::new();

    let err = mock.paste_text().unwrap_err();
    assert!(matches!(err, HasamiError::Empty));

    // Error display is meaningful
    let msg = err.to_string();
    assert!(!msg.is_empty());
    assert!(msg.contains("empty"));
}

// ---------------------------------------------------------------------------
// Thread-safety integration
// ---------------------------------------------------------------------------

#[test]
fn concurrent_clipboard_and_history() {
    use std::sync::Mutex;
    use std::thread;

    let mock = Arc::new(MockClipboard::new());
    let history = Arc::new(Mutex::new(ClipboardHistory::new(1000)));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let mock = Arc::clone(&mock);
            let history = Arc::clone(&history);
            thread::spawn(move || {
                for j in 0..10 {
                    let text = format!("thread-{i}-item-{j}");
                    mock.copy_text(&text).unwrap();
                    // Note: paste might get a different thread's value due to races
                    if let Ok(pasted) = mock.paste_text() {
                        history.lock().unwrap().push(&pasted);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap();
    // History should have entries (exact count depends on race conditions and dedup)
    assert!(history.len() > 0);
    assert!(history.len() <= 1000);
}

// ---------------------------------------------------------------------------
// TimedClipboard edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timed_clipboard_provider_accessor_is_usable() {
    let mock = Arc::new(MockClipboard::new());
    let timed = TimedClipboard::new(Arc::clone(&mock));

    // Use provider() to do operations
    timed.provider().copy_text("via accessor").unwrap();
    assert_eq!(timed.provider().paste_text().unwrap(), "via accessor");
    timed.provider().clear().unwrap();
    assert!(timed.provider().paste_text().is_err());
}

#[tokio::test]
async fn timed_clipboard_sensitive_with_very_long_text() {
    let mock = Arc::new(MockClipboard::new());
    let timed = TimedClipboard::new(Arc::clone(&mock));

    let long_password = "x".repeat(100_000);
    timed
        .copy_sensitive(&long_password, Duration::from_millis(50))
        .unwrap();
    assert_eq!(mock.paste_text().unwrap().len(), 100_000);

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(mock.paste_text().is_err());
}

// ---------------------------------------------------------------------------
// History boundary conditions
// ---------------------------------------------------------------------------

#[test]
fn history_capacity_one_with_search() {
    let mut h = ClipboardHistory::new(1);
    h.push("searchable content");
    h.push("other content"); // evicts first

    let results = h.search("searchable");
    assert!(results.is_empty());

    let results = h.search("other");
    assert_eq!(results.len(), 1);
}

#[test]
fn history_entry_display_trait() {
    let mut h = ClipboardHistory::new(10);
    h.push("display me");

    let entry = &h.recent(1)[0];
    assert_eq!(format!("{entry}"), "display me");
}

#[test]
fn history_remove_then_search() {
    let mut h = ClipboardHistory::new(10);
    h.push("alpha");
    h.push("beta");
    h.push("alpha-gamma");

    h.remove(0); // remove "alpha"

    let results = h.search("alpha");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].text, "alpha-gamma");
}
