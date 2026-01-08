//! Inbox module for aggregating inbound requests from multiple sources.
//!
//! The inbox provides a unified queue for messages from iMessage, Whop notifications,
//! email, etc. Users can triage items, reply, and convert them into Swarm tasks.

pub mod sources;
pub mod storage;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for pluggable inbox sources (iMessage, Whop, Slack, etc.)
pub trait InboxSource: Send + Sync {
    /// Unique identifier (e.g., "imessage", "whop", "slack")
    fn source_id(&self) -> &'static str;

    /// Display name for UI (e.g., "iMessage", "Whop Support")
    fn display_name(&self) -> &str;

    /// Icon/emoji for list view (e.g., "📱", "⚡", "💬")
    fn icon(&self) -> &str;

    /// Fetch new items since last check
    fn fetch(&self, since: DateTime<Utc>) -> Result<Vec<InboxItem>>;

    /// Fetch conversation history for context
    fn fetch_thread(&self, item: &InboxItem, limit: usize) -> Result<Vec<InboxItem>>;

    /// Send a reply to an inbox item
    fn reply(&self, item: &InboxItem, message: &str) -> Result<()>;

    /// Whether this source supports replies
    fn supports_reply(&self) -> bool;

    /// Render source-specific header lines for expanded view
    fn render_header(&self, item: &InboxItem) -> Vec<String>;
}

/// An item in the inbox (message from any source)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    /// Unique identifier for this item
    pub id: String,

    /// Source identifier (e.g., "imessage", "whop")
    pub source: String,

    /// When the message was sent/received
    pub timestamp: DateTime<Utc>,

    /// The message content
    pub content: String,

    /// Whether this item has been read/viewed
    pub read: bool,

    /// Information about who sent the message
    pub sender: Sender,

    /// Source-specific context needed for replies, thread fetching, etc.
    /// This is opaque to the inbox system - each source interprets its own context.
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,

    /// Additional metadata (attachments, etc.)
    #[serde(default)]
    pub metadata: ItemMetadata,
}

/// Information about the sender of an inbox item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sender {
    /// Display name (e.g., "Steven", "Mom")
    pub display_name: String,

    /// Unique identifier for the sender (phone number, email, user ID, etc.)
    pub identifier: String,

    /// Optional avatar URL
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Additional metadata for an inbox item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemMetadata {
    /// List of attachment descriptions/paths
    #[serde(default)]
    pub attachments: Vec<String>,

    /// Whether this is from a group chat
    #[serde(default)]
    pub is_group: bool,

    /// Group name if applicable
    #[serde(default)]
    pub group_name: Option<String>,
}

impl InboxItem {
    /// Create a new inbox item
    pub fn new(
        id: impl Into<String>,
        source: impl Into<String>,
        content: impl Into<String>,
        sender: Sender,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            timestamp: Utc::now(),
            content: content.into(),
            read: false,
            sender,
            context: HashMap::new(),
            metadata: ItemMetadata::default(),
        }
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Add context data
    pub fn with_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.insert(key.into(), value);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: ItemMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Get a context value as a string
    pub fn context_str(&self, key: &str) -> Option<&str> {
        self.context.get(key).and_then(|v| v.as_str())
    }

    /// Get the sender label (handles group chats)
    pub fn sender_label(&self) -> String {
        if self.metadata.is_group {
            if let Some(ref group_name) = self.metadata.group_name {
                return format!("Group: {}", group_name);
            }
        }
        self.sender.display_name.clone()
    }

    /// Get a preview of the content (truncated)
    pub fn preview(&self, max_len: usize) -> String {
        let content = self.content.replace('\n', " ");
        if content.len() <= max_len {
            content
        } else {
            format!("{}...", &content[..max_len.saturating_sub(3)])
        }
    }
}

impl Sender {
    /// Create a new sender
    pub fn new(display_name: impl Into<String>, identifier: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            identifier: identifier.into(),
            avatar_url: None,
        }
    }
}

/// State for tracking what has been fetched from a source
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceState {
    /// Last time messages were fetched from this source
    pub last_fetch: Option<DateTime<Utc>>,

    /// Any source-specific state data
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Format a timestamp as relative time (e.g., "2m ago", "3h ago")
pub fn format_time_ago(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{}m ago", mins)
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{}h ago", hours)
    } else {
        let days = duration.num_days();
        format!("{}d ago", days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbox_item_creation() {
        let sender = Sender::new("Steven", "+1234567890");
        let item = InboxItem::new("msg_123", "imessage", "Hello world", sender);

        assert_eq!(item.id, "msg_123");
        assert_eq!(item.source, "imessage");
        assert_eq!(item.content, "Hello world");
        assert!(!item.read);
    }

    #[test]
    fn test_sender_label_regular() {
        let sender = Sender::new("Steven", "+1234567890");
        let item = InboxItem::new("msg_123", "imessage", "Hello", sender);

        assert_eq!(item.sender_label(), "Steven");
    }

    #[test]
    fn test_sender_label_group() {
        let sender = Sender::new("Steven", "+1234567890");
        let mut item = InboxItem::new("msg_123", "imessage", "Hello", sender);
        item.metadata.is_group = true;
        item.metadata.group_name = Some("Engineering".to_string());

        assert_eq!(item.sender_label(), "Group: Engineering");
    }

    #[test]
    fn test_preview_truncation() {
        let sender = Sender::new("Steven", "+1234567890");
        let item = InboxItem::new(
            "msg_123",
            "imessage",
            "This is a very long message that should be truncated",
            sender,
        );

        assert_eq!(item.preview(20), "This is a very lo...");
        assert_eq!(
            item.preview(100),
            "This is a very long message that should be truncated"
        );
    }
}
