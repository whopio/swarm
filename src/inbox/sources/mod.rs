//! Inbox source implementations.
//!
//! Each source (iMessage, Whop, Slack, etc.) gets its own module here.

pub mod imessage;

pub use imessage::IMessageSource;
