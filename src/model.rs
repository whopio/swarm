use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
	NeedsInput,
	Running,
	Idle,
	Done,
	Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSession {
	pub name: String,
	pub session_name: String,
	pub agent: String,
	pub status: AgentStatus,
	pub last_output: Option<SystemTime>,
	pub log_path: PathBuf,
	pub preview: Vec<String>,
	pub task: Option<TaskInfo>,
	pub is_yolo: bool, // ⚠️ Started with --dangerously-skip-permissions
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
	pub path: PathBuf,
	pub title: String,
}

#[derive(Debug, Clone)]
pub struct TaskEntry {
	pub title: String,
	pub path: PathBuf,
	pub due: Option<chrono::NaiveDate>,
	pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DailyEntry {
	pub date: chrono::NaiveDate,
	pub path: PathBuf,
	pub preview: String, // First non-empty line for list display
}
