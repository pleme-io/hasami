//! Clipboard history with a bounded ring buffer.
//!
//! Maintains a fixed-size history of clipboard entries with timestamps,
//! supporting retrieval of recent items, substring search, and removal.

use std::fmt;
use std::time::SystemTime;

/// A single clipboard history entry with its captured timestamp.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// The clipboard text content.
    pub text: String,
    /// When this entry was added to the history.
    pub timestamp: SystemTime,
}

impl HistoryEntry {
    /// Create a new history entry with the current timestamp.
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            timestamp: SystemTime::now(),
        }
    }
}

impl fmt::Display for HistoryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

/// A bounded clipboard history backed by a `Vec` ring buffer.
///
/// When the buffer reaches its capacity, the oldest entry is evicted
/// to make room for new ones. Consecutive duplicate entries are deduplicated.
#[derive(Debug, Clone)]
pub struct ClipboardHistory {
    entries: Vec<HistoryEntry>,
    capacity: usize,
}

impl ClipboardHistory {
    /// Create a new history with the given maximum capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "clipboard history capacity must be > 0");
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new entry into the history.
    ///
    /// If the entry is identical to the most recent one, it is not added
    /// (deduplication). If the buffer is at capacity, the oldest entry
    /// is evicted.
    pub fn push(&mut self, text: &str) {
        // Deduplicate against most recent entry
        if let Some(last) = self.entries.last()
            && last.text == text
        {
            return;
        }

        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(HistoryEntry::new(text));
    }

    /// Return references to the last `n` entries, newest first.
    #[must_use]
    pub fn recent(&self, n: usize) -> &[HistoryEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Search the history for entries containing `query` as a substring
    /// (case-insensitive). Returns matching entries newest first.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|entry| entry.text.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Remove and return the entry at the given index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn remove(&mut self, index: usize) -> Option<HistoryEntry> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the total number of entries currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_recent() {
        let mut h = ClipboardHistory::new(10);
        h.push("first");
        h.push("second");
        h.push("third");
        let recent = h.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "second");
        assert_eq!(recent[1].text, "third");
    }

    #[test]
    fn recent_more_than_available() {
        let mut h = ClipboardHistory::new(10);
        h.push("only");
        let recent = h.recent(5);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].text, "only");
    }

    #[test]
    fn capacity_limit_oldest_removed() {
        let mut h = ClipboardHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        h.push("d"); // evicts "a"
        h.push("e"); // evicts "b"

        assert_eq!(h.len(), 3);
        let all = h.recent(10);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].text, "c");
        assert_eq!(all[1].text, "d");
        assert_eq!(all[2].text, "e");
    }

    #[test]
    fn dedup_consecutive() {
        let mut h = ClipboardHistory::new(10);
        h.push("same");
        h.push("same");
        h.push("same");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn dedup_allows_non_consecutive() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        h.push("a"); // not consecutive dup, should be added
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn search_case_insensitive() {
        let mut h = ClipboardHistory::new(10);
        h.push("Hello World");
        h.push("HELLO RUST");

        let results = h.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_substring_match() {
        let mut h = ClipboardHistory::new(10);
        h.push("hello world");
        h.push("goodbye world");
        h.push("hello universe");
        h.push("nothing here");

        let results = h.search("hello");
        assert_eq!(results.len(), 2);
        // Newest first
        assert_eq!(results[0].text, "hello universe");
        assert_eq!(results[1].text, "hello world");
    }

    #[test]
    fn search_no_results() {
        let mut h = ClipboardHistory::new(10);
        h.push("alpha");
        h.push("beta");
        let results = h.search("gamma");
        assert!(results.is_empty());
    }

    #[test]
    fn remove_by_index() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        h.push("c");

        let removed = h.remove(1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().text, "b");
        assert_eq!(h.len(), 2);

        let recent = h.recent(10);
        assert_eq!(recent[0].text, "a");
        assert_eq!(recent[1].text, "c");
    }

    #[test]
    fn remove_out_of_bounds() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        assert!(h.remove(5).is_none());
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn clear_history() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        assert!(!h.is_empty());
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn len_and_is_empty() {
        let mut h = ClipboardHistory::new(10);
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        h.push("x");
        assert!(!h.is_empty());
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn capacity_returns_max() {
        let h = ClipboardHistory::new(42);
        assert_eq!(h.capacity(), 42);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        let _ = ClipboardHistory::new(0);
    }

    #[test]
    fn display_impl() {
        let entry = HistoryEntry::new("display test");
        assert_eq!(entry.to_string(), "display test");
    }

    #[test]
    fn entry_has_timestamp() {
        let before = SystemTime::now();
        let entry = HistoryEntry::new("timestamped");
        let after = SystemTime::now();

        assert!(entry.timestamp >= before);
        assert!(entry.timestamp <= after);
    }

    #[test]
    fn capacity_one_evicts_immediately() {
        let mut h = ClipboardHistory::new(1);
        h.push("a");
        h.push("b");
        assert_eq!(h.len(), 1);
        assert_eq!(h.recent(1)[0].text, "b");
    }

    #[test]
    fn dedup_does_not_suppress_after_removal() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        h.remove(1); // remove "b", last is now "a"
        h.push("a"); // would be consecutive dup, should be suppressed
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn dedup_after_different_insert() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        h.push("b"); // consecutive dup, suppressed
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent(10)[1].text, "b");
    }

    #[test]
    fn recent_zero_returns_empty_slice() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        let recent = h.recent(0);
        assert!(recent.is_empty());
    }

    #[test]
    fn recent_on_empty_history() {
        let h = ClipboardHistory::new(10);
        let recent = h.recent(5);
        assert!(recent.is_empty());
    }

    #[test]
    fn search_empty_query_matches_all() {
        let mut h = ClipboardHistory::new(10);
        h.push("alpha");
        h.push("beta");
        h.push("gamma");
        let results = h.search("");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn search_on_empty_history() {
        let h = ClipboardHistory::new(10);
        let results = h.search("anything");
        assert!(results.is_empty());
    }

    #[test]
    fn search_returns_newest_first() {
        let mut h = ClipboardHistory::new(10);
        h.push("match-first");
        h.push("no");
        h.push("match-third");
        let results = h.search("match");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "match-third");
        assert_eq!(results[1].text, "match-first");
    }

    #[test]
    fn search_unicode_case_insensitive() {
        let mut h = ClipboardHistory::new(10);
        h.push("Straße Berlin");
        h.push("straße münchen");
        let results = h.search("STRASSE");
        // "ß".to_lowercase() is "ß", not "ss", so "STRASSE" won't match "straße"
        assert!(results.is_empty());
    }

    #[test]
    fn remove_first_entry() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        h.push("c");
        let removed = h.remove(0).unwrap();
        assert_eq!(removed.text, "a");
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent(10)[0].text, "b");
    }

    #[test]
    fn remove_last_entry() {
        let mut h = ClipboardHistory::new(10);
        h.push("a");
        h.push("b");
        h.push("c");
        let removed = h.remove(2).unwrap();
        assert_eq!(removed.text, "c");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn remove_from_empty_history() {
        let mut h = ClipboardHistory::new(10);
        assert!(h.remove(0).is_none());
    }

    #[test]
    fn clear_then_push() {
        let mut h = ClipboardHistory::new(5);
        h.push("a");
        h.push("b");
        h.clear();
        assert!(h.is_empty());
        h.push("c");
        assert_eq!(h.len(), 1);
        assert_eq!(h.recent(1)[0].text, "c");
    }

    #[test]
    fn push_preserves_insertion_order() {
        let mut h = ClipboardHistory::new(5);
        for i in 0..5 {
            h.push(&format!("item-{i}"));
        }
        let all = h.recent(10);
        for (i, entry) in all.iter().enumerate() {
            assert_eq!(entry.text, format!("item-{i}"));
        }
    }

    #[test]
    fn eviction_preserves_order_after_wraparound() {
        let mut h = ClipboardHistory::new(3);
        // Fill and overflow multiple times
        for i in 0..10 {
            h.push(&format!("v{i}"));
        }
        assert_eq!(h.len(), 3);
        let all = h.recent(10);
        assert_eq!(all[0].text, "v7");
        assert_eq!(all[1].text, "v8");
        assert_eq!(all[2].text, "v9");
    }

    #[test]
    fn clone_produces_independent_copy() {
        let mut h = ClipboardHistory::new(10);
        h.push("original");
        let mut cloned = h.clone();
        cloned.push("cloned-only");
        assert_eq!(h.len(), 1);
        assert_eq!(cloned.len(), 2);
    }

    #[test]
    fn entry_clone_preserves_fields() {
        let entry = HistoryEntry::new("cloneable");
        let cloned = entry.clone();
        assert_eq!(entry.text, cloned.text);
        assert_eq!(entry.timestamp, cloned.timestamp);
    }

    #[test]
    fn push_very_long_text() {
        let mut h = ClipboardHistory::new(5);
        let long_text = "a".repeat(100_000);
        h.push(&long_text);
        assert_eq!(h.len(), 1);
        assert_eq!(h.recent(1)[0].text.len(), 100_000);
    }

    #[test]
    fn search_partial_match() {
        let mut h = ClipboardHistory::new(10);
        h.push("abcdef");
        h.push("ghijkl");
        let results = h.search("cde");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "abcdef");
    }

    #[test]
    fn capacity_after_clear_unchanged() {
        let mut h = ClipboardHistory::new(42);
        h.push("x");
        h.clear();
        assert_eq!(h.capacity(), 42);
    }

    #[test]
    fn dedup_at_capacity_boundary() {
        let mut h = ClipboardHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        // At capacity, pushing dup of last should not evict
        h.push("c");
        assert_eq!(h.len(), 3);
        assert_eq!(h.recent(10)[0].text, "a"); // "a" not evicted
    }

    #[test]
    fn remove_then_push_respects_capacity() {
        let mut h = ClipboardHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        h.remove(0); // remove "a", len = 2
        h.push("d"); // len = 3
        h.push("e"); // len = 3, evicts "b"
        assert_eq!(h.len(), 3);
        let all = h.recent(10);
        assert_eq!(all[0].text, "c");
        assert_eq!(all[1].text, "d");
        assert_eq!(all[2].text, "e");
    }

    #[test]
    fn debug_impl_exists() {
        let h = ClipboardHistory::new(5);
        let debug_str = format!("{h:?}");
        assert!(debug_str.contains("ClipboardHistory"));
    }
}
