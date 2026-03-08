//! Hasami (鋏) — clipboard manager with timed clearing and history.
//!
//! Provides a trait-based clipboard abstraction, automatic timed clearing
//! for sensitive data (passwords), and a bounded clipboard history with
//! substring search.
//!
//! # Quick Start
//!
//! ```no_run
//! use hasami::{Clipboard, ClipboardProvider, ClipboardHistory, TimedClipboard};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! // Basic clipboard
//! let cb = Arc::new(Clipboard::new().unwrap());
//! cb.copy_text("hello").unwrap();
//! assert_eq!(cb.paste_text().unwrap(), "hello");
//!
//! // Timed clipboard for passwords
//! let timed = TimedClipboard::new(cb);
//! timed.copy_sensitive("s3cret", Duration::from_secs(30)).unwrap();
//!
//! // History
//! let mut history = ClipboardHistory::new(100);
//! history.push("first copy");
//! history.push("second copy");
//! let recent = history.recent(5);
//! ```

pub mod clipboard;
pub mod history;
pub mod timed;

pub use clipboard::{Clipboard, ClipboardProvider, HasamiError, MockClipboard};
pub use history::{ClipboardHistory, HistoryEntry};
pub use timed::TimedClipboard;
