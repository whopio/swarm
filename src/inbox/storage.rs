//! Storage layer for inbox items.
//!
//! Handles reading/writing inbox items to `~/.swarm/inbox/`.
//! Items are stored as JSON files (encryption will be added in crypto.rs).

use crate::inbox::{InboxItem, SourceState};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

/// Manages inbox item storage
pub struct InboxStorage {
    /// Base directory for inbox storage (~/.swarm/inbox)
    inbox_dir: PathBuf,
    /// Directory for source state files (~/.swarm/inbox/.state)
    state_dir: PathBuf,
}

impl InboxStorage {
    /// Create a new inbox storage manager
    pub fn new(base_dir: &Path) -> Result<Self> {
        let inbox_dir = base_dir.join("inbox");
        let state_dir = inbox_dir.join(".state");

        // Ensure directories exist
        fs::create_dir_all(&inbox_dir).context("Failed to create inbox directory")?;
        fs::create_dir_all(&state_dir).context("Failed to create inbox state directory")?;

        Ok(Self {
            inbox_dir,
            state_dir,
        })
    }

    /// Get the path for an inbox item
    fn item_path(&self, id: &str) -> PathBuf {
        // Sanitize the ID to prevent path traversal
        let safe_id = id.replace(['/', '\\'], "_").replace("..", "_");
        self.inbox_dir.join(format!("{}.json", safe_id))
    }

    /// Get the path for source state
    fn state_path(&self, source_id: &str) -> PathBuf {
        let safe_id = source_id.replace(['/', '\\'], "_").replace("..", "_");
        self.state_dir.join(format!("{}.json", safe_id))
    }

    /// Save an inbox item to disk
    pub fn save_item(&self, item: &InboxItem) -> Result<()> {
        let path = self.item_path(&item.id);
        let json = serde_json::to_string_pretty(item).context("Failed to serialize inbox item")?;
        fs::write(&path, json).with_context(|| format!("Failed to write inbox item to {:?}", path))
    }

    /// Load an inbox item from disk
    pub fn load_item(&self, id: &str) -> Result<InboxItem> {
        let path = self.item_path(id);
        let json =
            fs::read_to_string(&path).with_context(|| format!("Failed to read inbox item {:?}", path))?;
        serde_json::from_str(&json).context("Failed to parse inbox item JSON")
    }

    /// Delete an inbox item from disk
    pub fn delete_item(&self, id: &str) -> Result<()> {
        let path = self.item_path(id);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete inbox item {:?}", path))?;
        }
        Ok(())
    }

    /// List all inbox items, sorted by timestamp (newest first)
    pub fn list_items(&self) -> Result<Vec<InboxItem>> {
        let mut items = Vec::new();

        for entry in fs::read_dir(&self.inbox_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip directories and non-JSON files
            if path.is_dir() || path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            // Skip state directory entries
            if path.starts_with(&self.state_dir) {
                continue;
            }

            match fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<InboxItem>(&json) {
                    Ok(item) => items.push(item),
                    Err(e) => {
                        eprintln!("Warning: Failed to parse inbox item {:?}: {}", path, e);
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Failed to read inbox item {:?}: {}", path, e);
                }
            }
        }

        // Sort by timestamp, newest first
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(items)
    }

    /// Count unread items
    pub fn count_unread(&self) -> Result<usize> {
        let items = self.list_items()?;
        Ok(items.iter().filter(|i| !i.read).count())
    }

    /// Mark an item as read
    pub fn mark_read(&self, id: &str) -> Result<()> {
        let mut item = self.load_item(id)?;
        item.read = true;
        self.save_item(&item)
    }

    /// Save source state (last fetch time, etc.)
    pub fn save_source_state(&self, source_id: &str, state: &SourceState) -> Result<()> {
        let path = self.state_path(source_id);
        let json = serde_json::to_string_pretty(state).context("Failed to serialize source state")?;
        fs::write(&path, json).with_context(|| format!("Failed to write source state to {:?}", path))
    }

    /// Load source state
    pub fn load_source_state(&self, source_id: &str) -> Result<SourceState> {
        let path = self.state_path(source_id);
        if !path.exists() {
            return Ok(SourceState::default());
        }
        let json =
            fs::read_to_string(&path).with_context(|| format!("Failed to read source state {:?}", path))?;
        serde_json::from_str(&json).context("Failed to parse source state JSON")
    }

    /// Get the last fetch time for a source
    pub fn get_last_fetch(&self, source_id: &str) -> Result<Option<DateTime<Utc>>> {
        let state = self.load_source_state(source_id)?;
        Ok(state.last_fetch)
    }

    /// Update the last fetch time for a source
    pub fn update_last_fetch(&self, source_id: &str, time: DateTime<Utc>) -> Result<()> {
        let mut state = self.load_source_state(source_id)?;
        state.last_fetch = Some(time);
        self.save_source_state(source_id, &state)
    }

    /// Save multiple items at once (from a fetch)
    pub fn save_items(&self, items: &[InboxItem]) -> Result<()> {
        for item in items {
            self.save_item(item)?;
        }
        Ok(())
    }

    /// Check if an item exists
    pub fn item_exists(&self, id: &str) -> bool {
        self.item_path(id).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::Sender;
    use tempfile::TempDir;

    fn setup_test_storage() -> (TempDir, InboxStorage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = InboxStorage::new(temp_dir.path()).unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_save_and_load_item() {
        let (_temp, storage) = setup_test_storage();

        let sender = Sender::new("Steven", "+1234567890");
        let item = InboxItem::new("msg_123", "imessage", "Hello world", sender);

        storage.save_item(&item).unwrap();
        let loaded = storage.load_item("msg_123").unwrap();

        assert_eq!(loaded.id, "msg_123");
        assert_eq!(loaded.content, "Hello world");
    }

    #[test]
    fn test_delete_item() {
        let (_temp, storage) = setup_test_storage();

        let sender = Sender::new("Steven", "+1234567890");
        let item = InboxItem::new("msg_456", "imessage", "Test", sender);

        storage.save_item(&item).unwrap();
        assert!(storage.item_exists("msg_456"));

        storage.delete_item("msg_456").unwrap();
        assert!(!storage.item_exists("msg_456"));
    }

    #[test]
    fn test_list_items_sorted() {
        let (_temp, storage) = setup_test_storage();

        let sender = Sender::new("Steven", "+1234567890");

        let older = InboxItem::new("msg_1", "imessage", "Older", sender.clone())
            .with_timestamp(DateTime::from_timestamp(1000, 0).unwrap());
        let newer = InboxItem::new("msg_2", "imessage", "Newer", sender)
            .with_timestamp(DateTime::from_timestamp(2000, 0).unwrap());

        storage.save_item(&older).unwrap();
        storage.save_item(&newer).unwrap();

        let items = storage.list_items().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "msg_2"); // Newer first
        assert_eq!(items[1].id, "msg_1");
    }

    #[test]
    fn test_source_state() {
        let (_temp, storage) = setup_test_storage();

        let now = Utc::now();
        storage.update_last_fetch("imessage", now).unwrap();

        let last_fetch = storage.get_last_fetch("imessage").unwrap();
        assert!(last_fetch.is_some());
    }

    #[test]
    fn test_count_unread() {
        let (_temp, storage) = setup_test_storage();

        let sender = Sender::new("Steven", "+1234567890");
        let item1 = InboxItem::new("msg_1", "imessage", "Unread", sender.clone());
        let mut item2 = InboxItem::new("msg_2", "imessage", "Read", sender);
        item2.read = true;

        storage.save_item(&item1).unwrap();
        storage.save_item(&item2).unwrap();

        assert_eq!(storage.count_unread().unwrap(), 1);
    }
}
