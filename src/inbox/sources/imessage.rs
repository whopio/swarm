//! iMessage source implementation using the `imsg` CLI tool.
//!
//! This source fetches messages from iMessage via the steipete/imsg CLI,
//! which reads from the local Messages database (requires Full Disk Access).

use crate::inbox::{InboxItem, InboxSource, ItemMetadata, Sender};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

/// iMessage source using the `imsg` CLI tool
pub struct IMessageSource {
    /// Path to the imsg binary
    imsg_path: PathBuf,
    /// Contacts to exclude from the inbox
    blocked_contacts: Vec<String>,
}

/// Raw message from imsg JSON output
#[derive(Debug, Deserialize)]
struct ImsgMessage {
    /// Message GUID
    guid: String,
    /// Message text
    #[serde(default)]
    text: Option<String>,
    /// Sender handle (phone/email)
    #[serde(default)]
    sender: Option<String>,
    /// Whether the message is from me
    is_from_me: bool,
    /// ISO8601 timestamp
    created_at: String,
    /// Chat identifier (numeric)
    chat_id: i64,
}

/// Raw chat from imsg JSON output
#[derive(Debug, Deserialize)]
struct ImsgChat {
    /// Chat row ID
    id: i64,
    /// Chat identifier string
    identifier: String,
    /// Display name (for group chats)
    #[serde(default)]
    name: Option<String>,
    /// Service type
    #[serde(default)]
    service: Option<String>,
    /// Last message timestamp
    #[serde(default)]
    last_message_at: Option<String>,
}

impl IMessageSource {
    /// Create a new iMessage source
    pub fn new(imsg_path: PathBuf) -> Self {
        Self {
            imsg_path,
            blocked_contacts: Vec::new(),
        }
    }

    /// Set blocked contacts
    pub fn with_blocked_contacts(mut self, blocked: Vec<String>) -> Self {
        self.blocked_contacts = blocked;
        self
    }

    /// Check if the imsg binary exists and is executable
    pub fn check_binary(&self) -> Result<()> {
        if !self.imsg_path.exists() {
            anyhow::bail!(
                "imsg not found at {}. It should be bundled with Swarm - please reinstall or build from source.",
                self.imsg_path.display()
            );
        }
        Ok(())
    }

    /// Run imsg command with given arguments
    /// SECURITY: Always use Command::args(), never shell interpolation
    fn run_imsg(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.imsg_path)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute imsg with args: {:?}", args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("permission") || stderr.contains("Full Disk Access") {
                anyhow::bail!(
                    "Cannot read Messages database. Grant Full Disk Access to your terminal app in System Settings → Privacy & Security → Full Disk Access"
                );
            }
            anyhow::bail!("imsg command failed: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Convert an imsg message to an InboxItem
    fn message_to_item(&self, msg: ImsgMessage, chat_name: Option<&str>) -> Option<InboxItem> {
        // Skip messages from self
        if msg.is_from_me {
            return None;
        }

        // Skip empty messages
        let content = msg.text.as_ref().filter(|t| !t.is_empty())?;

        // Skip blocked contacts
        if let Some(ref sender) = msg.sender {
            if self.blocked_contacts.iter().any(|b| b == sender) {
                return None;
            }
        }

        let sender_handle = msg.sender.clone().unwrap_or_else(|| "Unknown".to_string());
        // Use chat name for groups, otherwise use sender handle
        let sender_name = chat_name
            .filter(|n| !n.is_empty())
            .map(|n| n.to_string())
            .unwrap_or_else(|| sender_handle.clone());

        let sender = Sender::new(&sender_name, &sender_handle);

        // Parse ISO8601 timestamp
        let timestamp = DateTime::parse_from_rfc3339(&msg.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let is_group = chat_name.map(|n| !n.is_empty()).unwrap_or(false);
        let metadata = ItemMetadata {
            attachments: Vec::new(),
            is_group,
            group_name: chat_name.filter(|n| !n.is_empty()).map(|s| s.to_string()),
        };

        let mut item = InboxItem::new(&msg.guid, "imessage", content, sender)
            .with_timestamp(timestamp)
            .with_metadata(metadata);

        // Store context needed for replies
        item = item.with_context("chat_id", serde_json::json!(msg.chat_id));
        item = item.with_context("handle", serde_json::json!(sender_handle));

        Some(item)
    }

    /// Fetch recent messages from all chats (simplified method for inbox)
    pub fn fetch_recent(&self, max_chats: usize, messages_per_chat: usize) -> Result<Vec<InboxItem>> {
        self.check_binary()?;

        // Get recent chats
        let chats_output = self.run_imsg(&["chats", "--limit", &max_chats.to_string(), "--json"])?;

        let mut all_items = Vec::new();

        // Parse each line as a separate JSON object (NDJSON format)
        for line in chats_output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let chat: ImsgChat = match serde_json::from_str(line) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Get messages from this chat
            let history_output = self.run_imsg(&[
                "history",
                "--chat-id", &chat.id.to_string(),
                "--limit", &messages_per_chat.to_string(),
                "--json"
            ]);

            if let Ok(output) = history_output {
                for msg_line in output.lines() {
                    if msg_line.trim().is_empty() {
                        continue;
                    }

                    if let Ok(msg) = serde_json::from_str::<ImsgMessage>(msg_line) {
                        if let Some(item) = self.message_to_item(msg, chat.name.as_deref()) {
                            all_items.push(item);
                        }
                    }
                }
            }
        }

        // Sort by timestamp, newest first
        all_items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(all_items)
    }
}

impl InboxSource for IMessageSource {
    fn source_id(&self) -> &'static str {
        "imessage"
    }

    fn display_name(&self) -> &str {
        "iMessage"
    }

    fn icon(&self) -> &str {
        "📱"
    }

    fn fetch(&self, since: DateTime<Utc>) -> Result<Vec<InboxItem>> {
        // Use fetch_recent for simplicity - the since parameter would require
        // fetching from each chat with --start flag
        let _ = since; // unused for now
        self.fetch_recent(10, 3)
    }

    fn fetch_thread(&self, item: &InboxItem, limit: usize) -> Result<Vec<InboxItem>> {
        self.check_binary()?;

        let chat_id = item
            .context
            .get("chat_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("No chat_id in item context"))?;

        let output = self.run_imsg(&[
            "history",
            "--chat-id",
            &chat_id.to_string(),
            "--limit",
            &limit.to_string(),
            "--json",
        ])?;

        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Parse NDJSON format
        let items: Vec<InboxItem> = output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<ImsgMessage>(line).ok())
            .map(|msg| {
                let sender_handle = msg.sender.clone().unwrap_or_else(|| "Unknown".to_string());
                let sender_name = if msg.is_from_me {
                    "You".to_string()
                } else {
                    sender_handle.clone()
                };

                let sender = Sender::new(&sender_name, &sender_handle);
                let content = msg.text.unwrap_or_default();
                let timestamp = DateTime::parse_from_rfc3339(&msg.created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                InboxItem::new(&msg.guid, "imessage", content, sender).with_timestamp(timestamp)
            })
            .collect();

        Ok(items)
    }

    fn reply(&self, item: &InboxItem, message: &str) -> Result<()> {
        self.check_binary()?;

        let handle = item
            .context_str("handle")
            .ok_or_else(|| anyhow::anyhow!("No handle in item context for reply"))?;

        // SECURITY: Use Command::args() to prevent injection
        // Never use shell string interpolation with user data
        let output = Command::new(&self.imsg_path)
            .args(["send", "--to", handle, "--text", message])
            .output()
            .context("Failed to send iMessage")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to send message: {}", stderr);
        }

        Ok(())
    }

    fn supports_reply(&self) -> bool {
        true
    }

    fn render_header(&self, item: &InboxItem) -> Vec<String> {
        let mut lines = Vec::new();

        let handle = item.context_str("handle").unwrap_or("Unknown");
        lines.push(format!(
            "From: {} ({})",
            item.sender.display_name, handle
        ));

        let time_ago = format_time_ago(item.timestamp);
        let formatted_time = item.timestamp.format("%b %d, %l:%M %p").to_string();
        lines.push(format!("Time: {} ({})", time_ago, formatted_time));

        lines
    }
}

/// Format a timestamp as relative time (e.g., "2 minutes ago", "3 hours ago")
fn format_time_ago(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = duration.num_days();
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time_ago() {
        let now = Utc::now();

        assert_eq!(format_time_ago(now), "just now");

        let five_mins_ago = now - chrono::Duration::minutes(5);
        assert_eq!(format_time_ago(five_mins_ago), "5 minutes ago");

        let one_hour_ago = now - chrono::Duration::hours(1);
        assert_eq!(format_time_ago(one_hour_ago), "1 hour ago");

        let two_days_ago = now - chrono::Duration::days(2);
        assert_eq!(format_time_ago(two_days_ago), "2 days ago");
    }
}
