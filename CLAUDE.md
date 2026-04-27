# Hasami (鋏) — Clipboard Manager

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


## Build & Test

```bash
cargo build          # compile
cargo test           # 75 unit tests + 1 doc-test
```

## Architecture

Clipboard management library providing:
- Trait-based clipboard abstraction (`ClipboardProvider`) with real and mock implementations
- Timed auto-clearing for sensitive data (passwords) with generation-counter cancellation
- Bounded history with timestamps, deduplication, substring search, and index removal

### Module Map

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Re-exports all public types |
| `src/clipboard.rs` | `ClipboardProvider` trait, `Clipboard` (arboard), `MockClipboard`, `HasamiError` (21 tests) |
| `src/timed.rs` | `TimedClipboard<C>` — generic auto-clear via tokio with generation counter (16 tests) |
| `src/history.rs` | `ClipboardHistory` — timestamped `Vec` ring buffer with search/remove (38 tests) |

### Key Types

- **`ClipboardProvider`** — trait: `copy_text()`, `paste_text()`, `clear()`
- **`Clipboard`** — real `arboard::Clipboard` wrapper (thread-safe via `Arc<Mutex>`)
- **`MockClipboard`** — in-memory mock for testing without a display server
- **`HasamiError`** — `ClipboardAccess(String)`, `Timeout`, `Empty`
- **`TimedClipboard<C>`** — generic over provider, spawns tokio clear task with generation counter
- **`ClipboardHistory`** — `Vec<HistoryEntry>` with dedup, `recent(n)`, `search(query)`, `remove(idx)`
- **`HistoryEntry`** — `{ text: String, timestamp: SystemTime }`

### Usage Pattern

```rust
use hasami::{MockClipboard, ClipboardProvider, ClipboardHistory, TimedClipboard};
use std::sync::Arc;
use std::time::Duration;

// Mock clipboard for testing
let cb = Arc::new(MockClipboard::new());
cb.copy_text("hello").unwrap();
assert_eq!(cb.paste_text().unwrap(), "hello");

// Password-safe copy (clears after 30s, cancelled by new copy)
let timed = TimedClipboard::new(cb);
timed.copy_sensitive("p@ssw0rd", Duration::from_secs(30)).unwrap();

// History tracking with timestamps
let mut history = ClipboardHistory::new(100);
history.push("copied text");
let matches = history.search("text");
```

## Consumers

- **tobira** — app launcher clipboard integration
- **hikyaku** — email copy-to-clipboard actions
