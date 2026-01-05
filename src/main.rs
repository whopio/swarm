mod config;
mod detection;
mod logs;
mod model;
mod notify;
mod tmux;

use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate, Timelike};
use clap::{Parser, Subcommand};
use config::{Config, session_store_dir, snapshots_dir};
use crossterm::{
	event::{self, Event, KeyCode, KeyEventKind},
	execute,
	terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use detection::{detect_status, detection_for_agent};
use logs::tail_lines;
use model::{AgentSession, AgentStatus, TaskEntry, TaskInfo};
use ratatui::{
	prelude::*,
	text::{Line, Text},
	widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use slug::slugify;
use std::collections::HashSet;
use std::fs;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};
use tmux::{
	SWARM_PREFIX, capture_tail, ensure_pipe, find_tmux, kill_session, list_sessions, pane_last_used,
	send_keys, send_special_key, session_path, start_session, start_session_with_mise,
};

// Embedded hooks - compiled into binary for distribution
const HOOK_DONE: &str = include_str!("../hooks/done.md");
const HOOK_INTERVIEW: &str = include_str!("../hooks/interview.md");
const HOOK_LOG: &str = include_str!("../hooks/log.md");
const HOOK_POLL_PR: &str = include_str!("../hooks/poll-pr.md");
const HOOK_QA_SWARM: &str = include_str!("../hooks/qa-swarm.md");
const HOOK_WORKTREE: &str = include_str!("../hooks/worktree.md");

/// Install Claude hooks to ~/.claude/commands/
fn install_hooks() -> Result<()> {
	let commands_dir = dirs::home_dir()
		.ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
		.join(".claude")
		.join("commands");
	fs::create_dir_all(&commands_dir)?;

	let hooks = [
		("done.md", HOOK_DONE),
		("interview.md", HOOK_INTERVIEW),
		("log.md", HOOK_LOG),
		("poll-pr.md", HOOK_POLL_PR),
		("qa-swarm.md", HOOK_QA_SWARM),
		("worktree.md", HOOK_WORKTREE),
	];

	for (name, content) in hooks {
		let path = commands_dir.join(name);
		fs::write(&path, content)?;
	}

	Ok(())
}

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "whopio/swarm";

#[derive(serde::Deserialize)]
struct GitHubRelease {
	tag_name: String,
	assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
	name: String,
	browser_download_url: String,
}

/// Check for updates and return the latest version if newer
fn check_for_update() -> Result<Option<(String, String)>> {
	let client = reqwest::blocking::Client::builder()
		.user_agent("swarm-updater")
		.timeout(Duration::from_secs(10))
		.build()?;

	let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
	let response = client.get(&url).send()?;

	if !response.status().is_success() {
		return Ok(None);
	}

	let release: GitHubRelease = response.json()?;
	let latest = release.tag_name.trim_start_matches('v');
	let current = CURRENT_VERSION;

	// Simple version comparison
	if latest != current {
		// Find the right asset for this platform
		let target = if cfg!(target_os = "macos") {
			if cfg!(target_arch = "aarch64") {
				"aarch64-apple-darwin"
			} else {
				"x86_64-apple-darwin"
			}
		} else if cfg!(target_os = "linux") {
			"x86_64-unknown-linux-gnu"
		} else {
			return Ok(None);
		};

		for asset in &release.assets {
			if asset.name.contains(target) {
				return Ok(Some((latest.to_string(), asset.browser_download_url.clone())));
			}
		}
	}

	Ok(None)
}

/// Check for updates and install if available
fn check_and_install_update() -> Result<()> {
	println!("Checking for updates...");

	match check_for_update()? {
		Some((version, url)) => {
			println!("New version available: v{} (current: v{})", version, CURRENT_VERSION);
			println!("Downloading update...");

			let client = reqwest::blocking::Client::builder()
				.user_agent("swarm-updater")
				.build()?;

			let response = client.get(&url).send()?;
			if !response.status().is_success() {
				anyhow::bail!("Failed to download update");
			}

			let bytes = response.bytes()?;

			// Create temp file and write binary
			let temp_path = std::env::temp_dir().join("swarm-update");
			fs::write(&temp_path, &bytes)?;

			// Make executable
			#[cfg(unix)]
			{
				use std::os::unix::fs::PermissionsExt;
				fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))?;
			}

			println!("Installing update...");
			self_replace::self_replace(&temp_path)?;
			fs::remove_file(&temp_path)?;

			println!("✓ Updated to v{}! Restart swarm to use the new version.", version);
		}
		None => {
			println!("✓ Already running the latest version (v{})", CURRENT_VERSION);
		}
	}

	Ok(())
}

/// Auto-update on startup (runs in background, once per day)
/// Returns Some(version) if we just updated on a previous run
fn auto_update_on_startup() -> Option<String> {
	let swarm_dir = dirs::home_dir()?.join(".swarm");
	let just_updated_file = swarm_dir.join(".just-updated");
	let last_check_file = swarm_dir.join(".last-update-check");

	// Check if we just updated (on a previous run)
	if let Ok(version) = fs::read_to_string(&just_updated_file) {
		let _ = fs::remove_file(&just_updated_file);
		return Some(version);
	}

	// Only check once per day
	if let Ok(metadata) = fs::metadata(&last_check_file) {
		if let Ok(modified) = metadata.modified() {
			if modified.elapsed().ok()? < Duration::from_secs(86400) {
				return None;
			}
		}
	}

	// Check and auto-update in background thread
	std::thread::spawn(move || {
		let _ = fs::create_dir_all(&swarm_dir);
		let _ = fs::write(&last_check_file, "");

		if let Ok(Some((version, url))) = check_for_update() {
			// Download update
			let client = reqwest::blocking::Client::builder()
				.user_agent("swarm-updater")
				.build();

			if let Ok(client) = client {
				if let Ok(response) = client.get(&url).send() {
					if response.status().is_success() {
						if let Ok(bytes) = response.bytes() {
							let temp_path = std::env::temp_dir().join("swarm-update");
							if fs::write(&temp_path, &bytes).is_ok() {
								#[cfg(unix)]
								{
									use std::os::unix::fs::PermissionsExt;
									let _ = fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755));
								}

								if self_replace::self_replace(&temp_path).is_ok() {
									let _ = fs::remove_file(&temp_path);
									// Mark that we updated - will show on next run
									let _ = fs::write(&just_updated_file, format!("v{}", version));
								}
							}
						}
					}
				}
			}
		}
	});

	None
}

#[derive(Parser)]
#[command(name = "swarm")]
#[command(about = "Terminal dashboard for multiple AI coding agents")]
struct Cli {
	#[command(subcommand)]
	command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
	/// Print JSON status for all swarm-* sessions
	Status,
	/// Check for and install updates
	Update,
	/// Create a new agent session
	New {
		/// Name for the session (without swarm- prefix)
		name: String,
		/// Agent type (defaults to claude)
		#[arg(long, default_value = "claude")]
		agent: String,
		/// Repo path to use
		#[arg(long, default_value = ".")]
		repo: String,
		/// Create a worktree under worktree_dir
		#[arg(long, default_value_t = false)]
		worktree: bool,
		/// Initial prompt to send after launch
		#[arg(long)]
		prompt: Option<String>,
		/// Path to a task file; writes .swarm-task marker in repo/worktree
		#[arg(long)]
		task: Option<String>,
		/// Start Claude in auto-accept mode (sends Shift+Tab after launch)
		#[arg(long, default_value_t = false)]
		auto_accept: bool,
	},
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	let mut cfg = config::load_or_init().context("failed to load config")?;

	match cli.command {
		Some(Commands::Status) => {
			let sessions = collect_sessions(&cfg)?;
			println!("{}", serde_json::to_string_pretty(&sessions)?);
			Ok(())
		}
		Some(Commands::Update) => {
			check_and_install_update()?;
			Ok(())
		}
		Some(Commands::New {
			name,
			agent,
			repo,
			worktree,
			prompt,
			task,
			auto_accept,
		}) => handle_new(&cfg, name, agent, repo, worktree, prompt, task, auto_accept, true),
		None => run_tui(&mut cfg),
	}
}

fn collect_sessions(cfg: &Config) -> Result<Vec<AgentSession>> {
	let sessions = list_sessions()?;
	cleanup_orphans(cfg, &sessions);
	let mut out = Vec::new();
	for session in sessions {
		let log_path = Path::new(&cfg.general.logs_dir).join(format!("{session}.log"));
		let _ = ensure_pipe(&session, &log_path);

		let lines = tail_lines(&log_path, 80).unwrap_or_default();
		let last_output =
			latest_output_time(&log_path).or_else(|| pane_last_used(&session).ok().flatten());
		let age = last_output.and_then(|t| SystemTime::now().duration_since(t).ok());
		let agent = agent_for_session(&session).unwrap_or_else(|_| "claude".to_string());
		let detection = detection_for_agent(&agent);
		let status = detect_status(&lines, &detection, age);
		let task = task_info_for_session(&session)?;

		let preview = tail_lines(&log_path, 12).unwrap_or_default();
		let is_yolo = is_yolo_session(&session);
		out.push(AgentSession {
			name: session.trim_start_matches(SWARM_PREFIX).to_string(),
			session_name: session.clone(),
			agent,
			status,
			last_output,
			log_path,
			preview,
			task,
			is_yolo,
		});
	}
	Ok(out)
}

fn cleanup_orphans(cfg: &Config, active_sessions: &[String]) {
	let active: HashSet<String> = active_sessions.iter().cloned().collect();

	if let Ok(entries) = fs::read_dir(&cfg.general.logs_dir) {
		for entry in entries.flatten() {
			let path = entry.path();
			if !path.is_file() {
				continue;
			}
			let name = entry.file_name().to_string_lossy().to_string();
			if !(name.starts_with(SWARM_PREFIX) && name.ends_with(".log")) {
				continue;
			}
			let session_name = name.trim_end_matches(".log");
			if !active.contains(session_name) {
				let _ = fs::remove_file(&path);
			}
		}
	}

	if let Ok(dir) = session_store_dir() {
		if let Ok(entries) = fs::read_dir(&dir) {
			for entry in entries.flatten() {
				let name = entry.file_name().to_string_lossy().to_string();
				if !active.contains(&name) {
					let _ = fs::remove_dir_all(entry.path());
				}
			}
		}
	}
}

fn latest_output_time(path: &Path) -> Option<SystemTime> {
	fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn handle_new(
	cfg: &Config,
	name: String,
	agent: String,
	repo: String,
	worktree: bool,
	prompt: Option<String>,
	task: Option<String>,
	auto_accept: bool,
	announce: bool,
) -> Result<()> {
	let clean_name = name.trim_start_matches(SWARM_PREFIX).to_string();
	let session = format!("{SWARM_PREFIX}{clean_name}");
	let repo_path = resolve_repo_path(&repo)?;
	let target_dir = if worktree {
		let base = PathBuf::from(&cfg.general.worktree_dir);
		fs::create_dir_all(&base)?;
		let path = base.join(&clean_name);

		// Fetch latest main before creating worktree (matches whop.sh behavior)
		let _ = Command::new("git")
			.arg("-C")
			.arg(&repo_path)
			.arg("fetch")
			.arg("origin")
			.arg("main")
			.status();

		// Create branch with prefix (e.g., sharkey11/task-name)
		let branch_name = format!("{}{}", cfg.general.branch_prefix, clean_name);
		let status = Command::new("git")
			.arg("-C")
			.arg(&repo_path)
			.arg("worktree")
			.arg("add")
			.arg(&path)
			.arg("-b")
			.arg(&branch_name)
			.arg("origin/main")
			.status()
			.context("failed to add worktree")?;
		if !status.success() {
			return Err(anyhow::anyhow!(
				"git worktree add failed with status {}",
				status
			));
		}
		path
	} else {
		repo_path
	};

	if let Some(task_path) = &task {
		let marker = session_task_path(&session)?;
		fs::write(&marker, task_path)?;
	}

	{
		let agent_marker = session_agent_path(&session)?;
		fs::write(&agent_marker, &agent)?;
	}

	// Mark YOLO mode sessions so we can show a warning indicator
	if auto_accept {
		let yolo_marker = session_yolo_path(&session)?;
		fs::write(&yolo_marker, "1")?;
	}

	// Build the command with optional initial prompt (passed as CLI arg, like whop.sh)
	let initial_prompt = if let Some(task_path) = &task {
		Some(format!(
			"Starting task. Read {} for context. Summarize the task before acting.",
			task_path
		))
	} else {
		prompt.clone()
	};

	// Build Claude flags:
	// - YOLO mode: --dangerously-skip-permissions (bypasses everything)
	// - Normal mode: --permission-mode acceptEdits + --allowedTools for safe commands
	let claude_flags = if auto_accept && agent == "claude" {
		" --dangerously-skip-permissions".to_string()
	} else if agent == "claude" {
		let allowed_tools = format_allowed_tools(&cfg.allowed_tools.tools);
		format!(" --permission-mode acceptEdits {}", allowed_tools)
	} else {
		String::new()
	};

	let command = match (agent.as_str(), &initial_prompt) {
		("claude", Some(p)) => {
			format!("claude{} \"{}\"", claude_flags, p.replace('"', "\\\""))
		}
		("claude", None) => format!("claude{}", claude_flags),
		("codex", Some(p)) => format!("codex \"{}\"", p.replace('"', "\\\"")),
		("codex", None) => "codex".to_string(),
		(other, Some(p)) => format!("{} \"{}\"", other, p.replace('"', "\\\"")),
		(other, None) => other.to_string(),
	};

	// Use mise activation for claude/codex to ensure correct environment (node, ruby, etc.)
	let use_mise = matches!(agent.as_str(), "claude" | "codex");
	if use_mise {
		start_session_with_mise(&session, &target_dir, &command)?;
	} else {
		start_session(&session, &target_dir, &command)?;
	}

	// Delay to let tmux session initialize before setting up pipe
	std::thread::sleep(std::time::Duration::from_millis(500));

	let log_path = Path::new(&cfg.general.logs_dir).join(format!("{session}.log"));
	// Pipe setup is best-effort - session is already running
	if let Err(e) = ensure_pipe(&session, &log_path) {
		eprintln!("Warning: pipe setup failed for {}: {}", session, e);
	}

	if announce {
		println!(
			"Started session {} in {} (attach: tmux attach -t {}, detach: Ctrl-b d)",
			session,
			target_dir.display(),
			session
		);
	}
	Ok(())
}

/// Formats the allowed tools list as CLI flags for Claude Code.
fn format_allowed_tools(tools: &[String]) -> String {
	if tools.is_empty() {
		return String::new();
	}
	tools
		.iter()
		.map(|t| format!("--allowedTools \"{}\"", t))
		.collect::<Vec<_>>()
		.join(" ")
}

fn resolve_repo_path(input: &str) -> Result<PathBuf> {
	let path = if input == "." {
		std::env::current_dir()?
	} else {
		PathBuf::from(input)
	};
	if !path.exists() {
		return Err(anyhow::anyhow!(
			"repo path does not exist: {}",
			path.display()
		));
	}
	Ok(path)
}

fn task_info_for_session(session: &str) -> Result<Option<TaskInfo>> {
	if let Some(info) = task_info_from_session_store(session)? {
		return Ok(Some(info));
	}

	let Some(path_str) = session_path(session)? else {
		return Ok(None);
	};
	let marker = PathBuf::from(path_str).join(".swarm-task");
	if !marker.exists() {
		return Ok(None);
	}
	Ok(read_task_info_from_marker(&marker))
}

fn agent_for_session(session: &str) -> Result<String> {
	if let Ok(marker) = session_agent_path(session) {
		if let Ok(val) = fs::read_to_string(&marker) {
			let trimmed = val.trim();
			if !trimmed.is_empty() {
				return Ok(trimmed.to_string());
			}
		}
	}
	Ok("claude".to_string())
}

fn task_info_from_session_store(session: &str) -> Result<Option<TaskInfo>> {
	let marker = session_task_path(session)?;
	if !marker.exists() {
		return Ok(None);
	}
	Ok(read_task_info_from_marker(&marker))
}

fn session_task_path(session: &str) -> Result<PathBuf> {
	let dir = session_store_dir()?.join(session);
	fs::create_dir_all(&dir)?;
	Ok(dir.join("task"))
}

fn session_agent_path(session: &str) -> Result<PathBuf> {
	let dir = session_store_dir()?.join(session);
	fs::create_dir_all(&dir)?;
	Ok(dir.join("agent"))
}

fn session_yolo_path(session: &str) -> Result<PathBuf> {
	let dir = session_store_dir()?.join(session);
	fs::create_dir_all(&dir)?;
	Ok(dir.join("yolo"))
}

fn is_yolo_session(session: &str) -> bool {
	session_yolo_path(session)
		.map(|p| p.exists())
		.unwrap_or(false)
}

fn read_task_info_from_marker(marker: &Path) -> Option<TaskInfo> {
	let target_path = fs::read_to_string(marker)
		.ok()
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty())?;
	Some(build_task_info(PathBuf::from(target_path)))
}

/// Find existing session for a task (by matching task path)
fn find_session_for_task<'a>(
	sessions: &'a [AgentSession],
	task_path: &Path,
) -> Option<&'a AgentSession> {
	sessions.iter().find(|s| {
		s.task
			.as_ref()
			.map(|t| t.path == task_path)
			.unwrap_or(false)
	})
}

fn build_task_info(task_path: PathBuf) -> TaskInfo {
	if task_path.exists() {
		let title = extract_title(&task_path).unwrap_or_else(|| {
			task_path
				.file_stem()
				.unwrap_or_default()
				.to_string_lossy()
				.into_owned()
		});
		TaskInfo {
			path: task_path,
			title,
		}
	} else {
		TaskInfo {
			path: task_path,
			title: "Missing task file".to_string(),
		}
	}
}

fn extract_title(path: &Path) -> Option<String> {
	let content = fs::read_to_string(path).ok()?;
	for line in content.lines() {
		if line.starts_with("# ") {
			return Some(line.trim_start_matches("# ").to_string());
		}
	}
	None
}

fn parse_due(path: &Path) -> Option<NaiveDate> {
	let content = fs::read_to_string(path).ok()?;
	let mut lines = content.lines();
	if lines.next()? != "---" {
		return None;
	}
	for line in lines.by_ref() {
		if line.trim() == "---" {
			break;
		}
		let trimmed = line.trim();
		if let Some(rest) = trimmed.strip_prefix("due:") {
			let val = rest.trim().trim_matches('"').trim();
			if let Ok(date) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
				return Some(date);
			}
		}
	}
	None
}

fn parse_status(path: &Path) -> Option<String> {
	let content = fs::read_to_string(path).ok()?;
	let mut lines = content.lines();
	if lines.next()? != "---" {
		return None;
	}
	for line in lines.by_ref() {
		let trimmed = line.trim();
		if trimmed == "---" {
			break;
		}
		if let Some(rest) = trimmed.strip_prefix("status:") {
			return Some(rest.trim().trim_matches('"').to_lowercase());
		}
	}
	None
}

fn parse_summary(path: &Path) -> Option<String> {
	let content = fs::read_to_string(path).ok()?;
	let mut lines = content.lines();
	if lines.next()? != "---" {
		return None;
	}
	for line in lines.by_ref() {
		let trimmed = line.trim();
		if trimmed == "---" {
			break;
		}
		if let Some(rest) = trimmed.strip_prefix("summary:") {
			return Some(rest.trim().trim_matches('"').to_string());
		}
	}
	None
}

fn format_due(date: NaiveDate) -> String {
	let today = Local::now().date_naive();
	let days = date.signed_duration_since(today).num_days();
	match days {
		0 => "due today".to_string(),
		1 => "due tomorrow".to_string(),
		d if d > 1 && d <= 7 => format!("due in {}d", d),
		-1 => "due yesterday".to_string(),
		d if d < -1 && d >= -7 => format!("due {}d ago", -d),
		_ => format!("due {}", date.format("%b %-d")),
	}
}

fn load_tasks(cfg: &Config) -> Vec<TaskEntry> {
	let dir = PathBuf::from(&cfg.general.tasks_dir);
	let mut tasks = Vec::new();
	if let Ok(entries) = fs::read_dir(&dir) {
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				if path.file_name().map(|n| n == "archive").unwrap_or(false) {
					continue;
				}
				continue;
			}
			if let Some(ext) = path.extension() {
				if ext == "md" {
					if path.file_stem().map(|s| s == "README").unwrap_or(false) {
						continue;
					}
					let status = parse_status(&path);
					if let Some(s) = status.as_deref() {
						if s == "done" || s == "completed" {
							continue;
						}
					}
					// Prefer summary over title for display
					let title = parse_summary(&path)
						.or_else(|| extract_title(&path))
						.unwrap_or_else(|| {
							path.file_stem()
								.unwrap_or_default()
								.to_string_lossy()
								.into_owned()
						});
					let due = parse_due(&path);
					tasks.push(TaskEntry { title, path: path.clone(), due, status });
				}
			}
		}
	}
	tasks.sort_by(|a, b| match (a.due, b.due) {
		(Some(da), Some(db)) => da.cmp(&db),
		(Some(_), None) => std::cmp::Ordering::Less,
		(None, Some(_)) => std::cmp::Ordering::Greater,
		(None, None) => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
	});
	tasks
}

fn task_preview(task: &TaskEntry, max_lines: usize) -> String {
	if let Ok(content) = fs::read_to_string(&task.path) {
		content
			.lines()
			.take(max_lines)
			.map(|s| s.to_string())
			.collect::<Vec<_>>()
			.join("\n")
	} else {
		"Unable to read task".to_string()
	}
}

fn run_tui(cfg: &mut Config) -> Result<()> {
	enable_raw_mode()?;
	let mut stdout_handle = stdout();
	execute!(stdout_handle, EnterAlternateScreen)?;
	let backend = ratatui::backend::CrosstermBackend::new(stdout_handle);
	let mut terminal = ratatui::Terminal::new(backend)?;

	let mut selected: usize = 0;
	let mut list_state = ListState::default();
	list_state.select(Some(0));
	let mut sessions = collect_sessions(cfg)?;
	let mut tasks = load_tasks(cfg);
	let mut tasks_state = ListState::default();
	tasks_state.select(Some(0));
	let mut showing_tasks = false;
	let mut show_help = false;
	// First-run hooks install prompt
	let mut show_hooks_prompt = !cfg.general.hooks_installed;
	// Auto-update on startup (checks once per day, shows "Just updated!" if we updated last run)
	let just_updated = auto_update_on_startup();
	let mut last_refresh = Instant::now();
	let mut status_message: Option<(String, Instant)> = None;
	let mut send_input_mode = false;
	let mut send_input_buf = String::new();
	// Confirmation mode for killing sessions (d key)
	let mut confirm_kill_mode = false;
	let mut pending_kill_session: Option<String> = None;
	// "Name your work" prompt for new agents (n key)
	let mut new_agent_mode = false;
	let mut new_agent_buf = String::new();
	let mut new_agent_due = String::from("tomorrow"); // pre-filled, can be deleted
	let mut new_agent_notify = String::new();
	let mut new_agent_field = 0; // 0 = description, 1 = due, 2 = notify
	let pipe_status: std::collections::HashMap<String, String> =
		std::collections::HashMap::new();
	// Track previous status for each session to detect state changes for notifications
	// Initialize with current session states to avoid notifications on startup
	let mut prev_status: std::collections::HashMap<String, AgentStatus> = sessions
		.iter()
		.map(|s| (s.session_name.clone(), s.status))
		.collect();
	// Cache preview to avoid calling tmux capture-pane on every render frame
	let mut cached_preview: Option<(String, Vec<String>)> = None; // (session_name, lines)
	// Status indicator style - can cycle with 's' key
	let styles = ["unicode", "emoji", "text"];
	let mut style_idx = styles
		.iter()
		.position(|s| *s == cfg.general.status_style)
		.unwrap_or(0);

	loop {
		let active_status = status_message
			.as_ref()
			.and_then(|(msg, ts)| (ts.elapsed() < Duration::from_secs(5)).then(|| msg.clone()));
		if status_message
			.as_ref()
			.map(|(_, ts)| ts.elapsed() >= Duration::from_secs(5))
			.unwrap_or(false)
		{
			status_message = None;
		}

		terminal.draw(|f| {
			let size = f.size();
			let vertical = Layout::default()
				.direction(Direction::Vertical)
				.constraints([Constraint::Min(3), Constraint::Length(2)].as_ref())
				.split(size);

			let (left, right) = if showing_tasks { (45, 55) } else { (35, 65) };
			let chunks = Layout::default()
				.direction(Direction::Horizontal)
				.constraints([Constraint::Percentage(left), Constraint::Percentage(right)].as_ref())
				.split(vertical[0]);

			if showing_tasks {
				// Build a set of task paths that have active sessions
				let active_task_paths: HashSet<PathBuf> = sessions
					.iter()
					.filter_map(|s| s.task.as_ref().map(|t| t.path.clone()))
					.collect();

				let items: Vec<ListItem> = tasks
					.iter()
					.map(|t| {
						let due = t
							.due
							.map(|d| format!(" · {}", format_due(d)))
							.unwrap_or_default();
						let status_tag = t
							.status
							.as_ref()
							.map(|s| format!("[{}] ", s))
							.unwrap_or_default();
						// Show ● indicator if task has an active session
						let active_indicator = if active_task_paths.contains(&t.path) {
							"● "
						} else {
							"• "
						};
						let style = if active_task_paths.contains(&t.path) {
							Style::default().fg(Color::Green)
						} else {
							Style::default()
						};
						ListItem::new(Line::from(Span::styled(
							format!("{}{}{}{}", active_indicator, status_tag, t.title, due),
							style,
						)))
					})
					.collect();
				let list_title = "Tasks (enter=start)".to_string();
				let list = List::new(items)
					.block(Block::default().borders(Borders::ALL).title(list_title))
					.highlight_symbol("▶ ")
					.highlight_style(
						Style::default()
							.add_modifier(Modifier::BOLD | Modifier::REVERSED)
							.fg(Color::White),
					);
				f.render_stateful_widget(list, chunks[0], &mut tasks_state);

				let preview_text = if let Some(sel) = tasks_state
					.selected()
					.and_then(|idx| tasks.get(idx))
				{
					task_preview(sel, 100)
				} else if tasks.is_empty() {
					String::from("No tasks")
				} else {
					String::from("No task selected")
				};
				let preview = Paragraph::new(preview_text)
					.block(Block::default().borders(Borders::ALL).title("Task Preview"))
					.wrap(Wrap { trim: true });
				f.render_widget(preview, chunks[1]);
			} else {
				let current_style = styles[style_idx];
				let items: Vec<ListItem> = sessions
					.iter()
					.enumerate()
					.map(|(idx, s)| {
						let (status_text, status_style) = status_indicator(s.status, current_style);
						let age = s
							.last_output
							.and_then(|t| SystemTime::now().duration_since(t).ok())
							.map(format_human_duration)
							.unwrap_or_else(|| "–".to_string());
						let mut spans: Vec<Span> = Vec::new();
						// Show number for quick access (1-9)
						if idx < 9 {
							spans.push(Span::styled(
								format!("{} ", idx + 1),
								Style::default().fg(Color::DarkGray),
							));
						} else {
							spans.push(Span::raw("  "));
						}
						spans.push(Span::styled(status_text, status_style));
						spans.push(Span::raw(" "));
						if s.is_yolo {
							spans.push(Span::styled(
								"⚠️ ",
								Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
							));
						}
						spans.push(Span::raw(&s.name));
						spans.push(Span::styled(format!(" · {}", age), Style::default().fg(Color::DarkGray)));
						if let Some(task) = &s.task {
							spans.push(Span::raw(" · "));
							spans.push(Span::raw(&task.title));
						}
						if let Some(snippet) = mini_log_preview(&s.preview) {
							spans.push(Span::styled("  · ", Style::default().fg(Color::DarkGray)));
							spans.push(Span::styled(snippet, Style::default().fg(Color::DarkGray)));
						}
						ListItem::new(Line::from(spans))
					})
					.collect();

				// Count agents needing input for header
				let needs_input_count = sessions
					.iter()
					.filter(|s| s.status == AgentStatus::NeedsInput)
					.count();
				let mut agents_title = if needs_input_count > 0 {
					format!("Agents ({} need input)", needs_input_count)
				} else {
					"Agents".to_string()
				};
				// Show "Just updated!" notification in header
				if let Some(ref version) = just_updated {
					agents_title = format!("{} │ ✨ Just updated to {}!", agents_title, version);
				}

				let list = List::new(items)
					.block(Block::default().borders(Borders::ALL).title(agents_title))
					.highlight_symbol("▶ ")
					.highlight_style(
						Style::default()
							.add_modifier(Modifier::BOLD | Modifier::REVERSED)
							.fg(Color::White),
					);
				f.render_stateful_widget(list, chunks[0], &mut list_state);

				let right_panes = Layout::default()
					.direction(Direction::Vertical)
					.constraints([Constraint::Min(10), Constraint::Length(8)].as_ref())
					.split(chunks[1]);

				// Use cached preview instead of calling tmux on every frame
				let (preview_lines_styled, details_text, is_yolo_selected) =
					if let Some(sel) = sessions.get(selected) {
						let preview_lines = cached_preview
							.as_ref()
							.filter(|(name, _)| name == &sel.session_name)
							.map(|(_, lines)| lines.clone())
							.unwrap_or_else(|| sel.preview.clone());
						let cleaned = clean_preview(&preview_lines);

						// Build styled lines, highlighting prompts
						let mut styled_lines: Vec<Line> = Vec::new();

						// Add YOLO warning banner at top if applicable
						if sel.is_yolo {
							styled_lines.push(Line::from("╔══════════════════════════════════════════════════════════╗"));
							styled_lines.push(Line::from("║  ⚠️  YOLO MODE - NO PERMISSION PROMPTS - BE CAREFUL!  ⚠️  ║"));
							styled_lines.push(Line::from("╚══════════════════════════════════════════════════════════╝"));
							styled_lines.push(Line::from(""));
						}

						for line in &cleaned {
							if is_prompt_line(line) {
								// Highlight prompt lines in yellow/bold
								styled_lines.push(Line::from(Span::styled(
									line.clone(),
									Style::default()
										.fg(Color::Yellow)
										.add_modifier(Modifier::BOLD),
								)));
							} else {
								styled_lines.push(Line::from(line.clone()));
							}
						}

						let mut details = agent_details(sel);
						if let Some(pipe_msg) = pipe_status.get(&sel.session_name) {
							details.push_str(&format!("\nPipe: {pipe_msg}"));
						}
						(styled_lines, details, sel.is_yolo)
					} else if sessions.is_empty() {
						// Show helpful hint when no agents exist
						(
							vec![
								Line::from(""),
								Line::from(Span::styled(
									"No agents yet.",
									Style::default().add_modifier(Modifier::BOLD),
								)),
								Line::from(""),
								Line::from("Press n to create a new agent"),
								Line::from("Press t to see saved tasks"),
							],
							String::from("Get started by creating a new agent or selecting an existing task."),
							false,
						)
					} else {
						(
							vec![Line::from("No session selected")],
							String::from("No details available"),
							false,
						)
					};

				let preview_block = if is_yolo_selected {
					Block::default()
						.borders(Borders::ALL)
						.title("⚠️ Preview (YOLO MODE)")
						.border_style(Style::default().fg(Color::Red))
						.title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
				} else {
					Block::default().borders(Borders::ALL).title("Preview")
				};

				// Create paragraph with wrapping to calculate actual visual line count
				let preview = Paragraph::new(Text::from(preview_lines_styled))
					.block(preview_block)
					.wrap(Wrap { trim: true });

				// Calculate scroll offset using actual wrapped line count
				let preview_height = right_panes[0].height.saturating_sub(2) as usize;
				let visual_line_count = preview.line_count(right_panes[0].width.saturating_sub(2));
				let scroll_offset = visual_line_count.saturating_sub(preview_height);

				let preview = preview.scroll((scroll_offset as u16, 0));
				f.render_widget(preview, right_panes[0]);

				let details = Paragraph::new(details_text)
					.block(Block::default().borders(Borders::ALL).title("Details"))
					.wrap(Wrap { trim: true });
				f.render_widget(details, right_panes[1]);
			}

			let footer_height: u16 = if active_status.is_some() || send_input_mode {
				3
			} else {
				2
			};
			let mut footer_lines = vec![if showing_tasks {
				tasks_footer_text(size.width)
			} else if send_input_mode {
				"Input: type message, Enter send, Esc cancel".to_string()
			} else {
				agents_footer_text(size.width)
			}];
			if send_input_mode {
				footer_lines.push(format!("> {}", send_input_buf));
			}
			if let Some(msg) = &active_status {
				footer_lines.push(format!("Status: {msg}"));
			}
			let footer_text = footer_lines.join("  |  ");
			let footer_block = if active_status.is_some() || send_input_mode {
				Block::default().borders(Borders::ALL)
			} else {
				Block::default()
			};
			let footer = Paragraph::new(footer_text)
				.block(footer_block)
				.wrap(Wrap { trim: true });
			let footer_area = Rect {
				x: vertical[1].x,
				y: vertical[1].y,
				width: vertical[1].width,
				height: footer_height,
			};
			f.render_widget(footer, footer_area);

			if show_help {
				let area = centered_rect(70, 80, size);
				let clear = ratatui::widgets::Clear;
				f.render_widget(clear, area);
				let overlay = Paragraph::new(help_text())
					.block(Block::default().borders(Borders::ALL).title("Help"))
					.wrap(Wrap { trim: true });
				f.render_widget(overlay, area);
			}

			if send_input_mode {
				let area = centered_rect(70, 30, size);
				let clear = ratatui::widgets::Clear;
				f.render_widget(clear, area);
				let instructions = "Send input (Enter to send, Esc to cancel)";
				let body = format!("{}\n\n> {}", instructions, send_input_buf);
				let overlay = Paragraph::new(body)
					.block(Block::default().borders(Borders::ALL).title("Send Input"))
					.wrap(Wrap { trim: true });
				f.render_widget(overlay, area);
			}

			if confirm_kill_mode {
				let area = centered_rect(60, 40, size);
				let clear = ratatui::widgets::Clear;
				f.render_widget(clear, area);
				let session_name = pending_kill_session
					.as_deref()
					.unwrap_or("unknown");
				let body = format!(
					r#"⚠️  Are you sure you want to kill this session?

Session: {}

Did you run /done in Claude first?
(Saves learnings, updates daily log, marks task complete)

  [y]   Yes, kill it
  [Esc] No, go back"#,
					session_name
				);
				let overlay = Paragraph::new(body)
					.block(
						Block::default()
							.borders(Borders::ALL)
							.title("⚠️ Confirm Kill Session")
							.border_style(Style::default().fg(Color::Yellow))
							.title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
					)
					.wrap(Wrap { trim: true });
				f.render_widget(overlay, area);
			}

		if new_agent_mode {
				let area = centered_rect(65, 45, size);
				let clear = ratatui::widgets::Clear;
				f.render_widget(clear, area);
				let cursors = [
					if new_agent_field == 0 { "█" } else { "" },
					if new_agent_field == 1 { "█" } else { "" },
					if new_agent_field == 2 { "█" } else { "" },
				];
				let due_display = &new_agent_due;
				let body = format!(
					r#"What are you working on?
> {}{}

Who should be notified when done?
> {}{}

Due date (MM-DD or leave blank for tomorrow)
> {}{}

Tab to switch fields, Enter to start, Esc to cancel"#,
					new_agent_buf, cursors[0],
					new_agent_notify, cursors[1],
					due_display, cursors[2],
				);
				let overlay = Paragraph::new(body)
					.block(
						Block::default()
							.borders(Borders::ALL)
							.title("New Agent")
							.border_style(Style::default().fg(Color::Cyan))
							.title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
					)
					.wrap(Wrap { trim: true });
				f.render_widget(overlay, area);
			}

			// First-run hooks install prompt
			if show_hooks_prompt {
				let area = centered_rect(60, 50, size);
				let clear = ratatui::widgets::Clear;
				f.render_widget(clear, area);
				let body = r#"Welcome to swarm!

swarm comes with Claude commands that help you
work more effectively with AI coding agents:

  /done       - End session, log work
  /interview  - Detailed task planning
  /log        - Save progress to task file
  /worktree   - Move to isolated git worktree
  /poll-pr    - Monitor PR until CI green
  /qa-swarm   - QA test the swarm TUI

Install these commands to ~/.claude/commands/?

  [y] Yes, install (recommended)
  [n] No thanks"#;
				let overlay = Paragraph::new(body)
					.block(
						Block::default()
							.borders(Borders::ALL)
							.title("Setup")
							.border_style(Style::default().fg(Color::Green))
							.title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
					)
					.wrap(Wrap { trim: true });
				f.render_widget(overlay, area);
			}
		})?;

		if event::poll(Duration::from_millis(100))? {
			if let Event::Key(key) = event::read()? {
				if key.kind == KeyEventKind::Press {
					if show_help && key.code != KeyCode::Char('?') && key.code != KeyCode::Esc {
						continue;
					}
					// Handle first-run hooks prompt
					if show_hooks_prompt {
						match key.code {
							KeyCode::Char('y') | KeyCode::Char('Y') => {
								if let Err(e) = install_hooks() {
									status_message = Some((
										format!("Failed to install hooks: {}", e),
										Instant::now(),
									));
								} else {
									status_message = Some((
										"Hooks installed! Press h for list of Claude commands".to_string(),
										Instant::now(),
									));
								}
								cfg.general.hooks_installed = true;
								let _ = config::save_config(cfg);
								show_hooks_prompt = false;
							}
							KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
								cfg.general.hooks_installed = true; // Mark as prompted, don't ask again
								let _ = config::save_config(cfg);
								show_hooks_prompt = false;
							}
							_ => {}
						}
						continue;
					}
					// Handle send-input mode first to capture typing.
					if send_input_mode {
						match key.code {
							KeyCode::Char(c) if !c.is_control() => {
								send_input_buf.push(c);
								status_message =
									Some((format!("Input: {}", send_input_buf), Instant::now()));
							}
							KeyCode::Backspace => {
								send_input_buf.pop();
							}
							KeyCode::Enter => {
								if let Some(sel) = sessions.get(selected) {
									if !send_input_buf.is_empty() {
										let msg = send_input_buf.clone();
										let _ = send_keys(&sel.session_name, &msg);
										status_message = Some((
											format!("Sent to {}: {}", sel.name, msg),
											Instant::now(),
										));
									}
								}
								send_input_mode = false;
								send_input_buf.clear();
							}
							KeyCode::Esc => {
								send_input_mode = false;
								send_input_buf.clear();
							}
							_ => {}
						}
						continue;
					}
					// Handle new agent mode (name your work prompt)
					// Fields: 0 = description, 1 = notify, 2 = due
					if new_agent_mode {
						match key.code {
							KeyCode::Char(c) if !c.is_control() => {
								match new_agent_field {
									0 => new_agent_buf.push(c),
									1 => new_agent_notify.push(c),
									2 => new_agent_due.push(c),
									_ => {}
								}
							}
							KeyCode::Backspace => {
								match new_agent_field {
									0 => { new_agent_buf.pop(); }
									1 => { new_agent_notify.pop(); }
									2 => { new_agent_due.pop(); }
									_ => {}
								}
							}
							KeyCode::Tab => {
								new_agent_field = (new_agent_field + 1) % 3;
							}
							KeyCode::BackTab => {
								new_agent_field = if new_agent_field == 0 { 2 } else { new_agent_field - 1 };
							}
							KeyCode::Enter => {
								if !new_agent_buf.is_empty() {
									// Create task file and start agent
									let notify = if new_agent_notify.trim().is_empty() {
										None
									} else {
										Some(new_agent_notify.clone())
									};
									let due = if new_agent_due.trim().is_empty() || new_agent_due.trim().to_lowercase() == "tomorrow" {
										None // will default to tomorrow
									} else {
										Some(new_agent_due.clone())
									};
									match create_task_and_start_agent(
										cfg,
										&new_agent_buf,
										notify.as_deref(),
										due.as_deref(),
									) {
										Ok(session_name) => {
											status_message = Some((
												format!(
													"Started {} (run /interview in Claude to fill task details)",
													session_name
												),
												Instant::now(),
											));
											// Small delay to let session appear
											std::thread::sleep(std::time::Duration::from_millis(300));
											if let Ok(updated) = collect_sessions(cfg) {
												sessions = updated;
												selected = sessions.len().saturating_sub(1);
												list_state.select(
													sessions.get(selected).map(|_| selected),
												);
											}
											// Refresh tasks list
											tasks = load_tasks(cfg);
										}
										Err(e) => {
											status_message = Some((
												format!("Failed to start agent: {e}"),
												Instant::now(),
											));
										}
									}
								}
								new_agent_mode = false;
								new_agent_buf.clear();
								new_agent_notify.clear();
								new_agent_due = String::from("tomorrow");
								new_agent_field = 0;
							}
							KeyCode::Esc => {
								new_agent_mode = false;
								new_agent_buf.clear();
								new_agent_notify.clear();
								new_agent_due = String::from("tomorrow");
								new_agent_field = 0;
							}
							_ => {}
						}
						continue;
					}
					match key.code {
						KeyCode::Char('q') if !send_input_mode => break,
						KeyCode::Char('t') if !send_input_mode => {
							showing_tasks = !showing_tasks;
							show_help = false;
							if showing_tasks && tasks_state.selected().is_none() && !tasks.is_empty() {
								tasks_state.select(Some(0));
							}
						}
						KeyCode::Char('h') if !send_input_mode => {
							show_help = !show_help;
						}
						KeyCode::Esc => {
							if confirm_kill_mode {
								// Cancel kill confirmation
								confirm_kill_mode = false;
								pending_kill_session = None;
								status_message = Some((
									"Cancelled - session not killed".to_string(),
									Instant::now(),
								));
							} else if new_agent_mode {
								new_agent_mode = false;
								new_agent_buf.clear();
								new_agent_notify.clear();
								new_agent_due = String::from("tomorrow");
								new_agent_field = 0;
							} else if send_input_mode {
								send_input_mode = false;
								send_input_buf.clear();
							} else if showing_tasks {
								// Go back to agents view
								showing_tasks = false;
							}
							show_help = false;
						}
						KeyCode::Char('n')
							if !showing_tasks && !send_input_mode =>
						{
							// Enter "name your work" mode
							new_agent_mode = true;
							new_agent_buf.clear();
						}
						KeyCode::Char('j') | KeyCode::Down => {
							if showing_tasks {
								if let Some(sel) = tasks_state.selected() {
									if sel + 1 < tasks.len() {
										tasks_state.select(Some(sel + 1));
									}
								}
							} else if selected + 1 < sessions.len() {
								selected += 1;
								list_state.select(Some(selected));
								// Update preview cache for newly selected session
								if let Some(sel) = sessions.get(selected) {
									if let Ok(lines) = capture_tail(&sel.session_name, 200) {
										cached_preview = Some((sel.session_name.clone(), lines));
									}
								}
							}
						}
						KeyCode::Char('k') | KeyCode::Up => {
							if showing_tasks {
								if let Some(sel) = tasks_state.selected() {
									if sel > 0 {
										tasks_state.select(Some(sel - 1));
									}
								}
							} else if selected > 0 {
								selected -= 1;
								list_state.select(Some(selected));
								// Update preview cache for newly selected session
								if let Some(sel) = sessions.get(selected) {
									if let Ok(lines) = capture_tail(&sel.session_name, 200) {
										cached_preview = Some((sel.session_name.clone(), lines));
									}
								}
							}
						}
						KeyCode::Char('d')
							if !showing_tasks
								&& !send_input_mode
								&& !confirm_kill_mode =>
						{
							if let Some(sel) = sessions.get(selected) {
								// Show confirmation instead of immediately killing
								confirm_kill_mode = true;
								pending_kill_session = Some(sel.session_name.clone());
							}
						}
						// Handle confirmation mode responses
						KeyCode::Char('y') if confirm_kill_mode => {
							if let Some(session_name) = pending_kill_session.take() {
								if let Some(sel) =
									sessions.iter().find(|s| s.session_name == session_name)
								{
									match mark_done(sel, cfg) {
										Ok(()) => {
											status_message = Some((
												format!("Marked {} done", sel.name),
												Instant::now(),
											));
											if let Ok(updated) = collect_sessions(cfg) {
												sessions = updated;
												if selected >= sessions.len()
													&& !sessions.is_empty()
												{
													selected = sessions.len() - 1;
												}
												list_state.select(
													sessions.get(selected).map(|_| selected),
												);
											}
										}
										Err(e) => {
											eprintln!("Failed to mark done: {e}");
										}
									}
								}
							}
							confirm_kill_mode = false;
						}
						KeyCode::Char('a') if !showing_tasks && !send_input_mode => {
							// Attach to selected agent (full tmux takeover)
							if let Some(sel) = sessions.get(selected) {
								attach_to(&mut terminal, sel)?;
							}
						}
						KeyCode::Char('x')
							if showing_tasks && !send_input_mode =>
						{
							if let Some(idx) = tasks_state.selected() {
								if let Some(task) = tasks.get(idx) {
									match delete_task(task) {
										Ok(()) => {
											status_message = Some((
												format!("Deleted task {}", task.title),
												Instant::now(),
											));
											tasks = load_tasks(cfg);
											if tasks.is_empty() {
												tasks_state.select(None);
											} else if let Some(sel) = tasks_state.selected() {
												if sel >= tasks.len() {
													tasks_state.select(Some(tasks.len() - 1));
												}
											}
										}
										Err(e) => {
											status_message = Some((
												format!("Failed to delete task: {e}"),
												Instant::now(),
											));
										}
									}
								}
							}
						}
						KeyCode::Char('o')
							if showing_tasks && !send_input_mode =>
						{
							// Open task in Cursor
							if let Some(idx) = tasks_state.selected() {
								if let Some(task) = tasks.get(idx) {
									let _ = Command::new("cursor").arg(&task.path).status();
									status_message = Some((
										format!("Opened {} in Cursor", task.title),
										Instant::now(),
									));
								}
							}
						}
						KeyCode::Char('n')
							if showing_tasks && !send_input_mode =>
						{
							// Same "name your work" flow as agents view
							new_agent_mode = true;
							new_agent_buf.clear();
							new_agent_notify.clear();
							new_agent_due = String::from("tomorrow");
							new_agent_field = 0;
						}
						KeyCode::Char('Y') if showing_tasks => {
							// ⚠️ YOLO MODE - Skip permissions (dangerous!)
							if let Some(idx) = tasks_state.selected() {
								if let Some(task) = tasks.get(idx) {
									let task_title = task.title.clone();
									match start_from_task_yolo(cfg, task) {
										Ok(session_name) => {
											status_message = Some((
												format!(
													"⚠️ YOLO MODE: {} for {} (NO PERMISSION PROMPTS!)",
													session_name, task_title
												),
												Instant::now(),
											));
											showing_tasks = false;
											sessions = collect_sessions(cfg)?;
											selected = sessions.len().saturating_sub(1);
											list_state
												.select(sessions.get(selected).map(|_| selected));
										}
										Err(e) => {
											status_message = Some((
												format!("Failed to start YOLO session: {e}"),
												Instant::now(),
											));
										}
									}
								}
							}
						}
						// Force new session (even if one exists for this task)
						KeyCode::Char('N') if showing_tasks => {
							if let Some(idx) = tasks_state.selected() {
								if let Some(task) = tasks.get(idx) {
									let task_title = task.title.clone();
									match start_from_task(cfg, task) {
										Ok(session_name) => {
											status_message = Some((
												format!(
													"Started NEW session {} for {} (attach: tmux attach -t {}, detach: Ctrl-b d)",
													session_name, task_title, session_name
												),
												Instant::now(),
											));
											showing_tasks = false;
											sessions = collect_sessions(cfg)?;
											selected = sessions.len().saturating_sub(1);
											list_state
												.select(sessions.get(selected).map(|_| selected));
										}
										Err(e) => {
											eprintln!("Failed to start session: {e}");
										}
									}
								}
							}
						}
						KeyCode::Enter => {
							if showing_tasks {
								if let Some(idx) = tasks_state.selected() {
									if let Some(task) = tasks.get(idx) {
										// Check if there's already a session for this task
										if let Some(existing) =
											find_session_for_task(&sessions, &task.path)
										{
											// Switch to existing session
											let idx = sessions
												.iter()
												.position(|s| {
													s.session_name == existing.session_name
												})
												.unwrap_or(0);
											selected = idx;
											list_state.select(Some(selected));
											showing_tasks = false;
											status_message = Some((
												format!(
													"Switched to existing session: {}",
													existing.name
												),
												Instant::now(),
											));
										} else {
											// Start new session
											let task_title = task.title.clone();
											match start_from_task(cfg, task) {
												Ok(session_name) => {
													status_message = Some((
														format!(
															"Started {} for {}",
															session_name, task_title
														),
														Instant::now(),
													));
													showing_tasks = false;
													sessions = collect_sessions(cfg)?;
													selected = sessions.len().saturating_sub(1);
													list_state.select(
														sessions.get(selected).map(|_| selected),
													);
												}
												Err(e) => {
													eprintln!("Failed to start session: {e}");
												}
											}
										}
									}
								}
							} else if sessions.get(selected).is_some() {
								// Enter = send input (most common action when monitoring)
								send_input_mode = true;
								send_input_buf.clear();
							}
						}
						KeyCode::Char(c)
							if c.is_ascii_digit()
								&& !showing_tasks
								&& !send_input_mode =>
						{
							let idx = c.to_digit(10).unwrap_or(0);
							if idx > 0 {
								let target = (idx - 1) as usize;
								if sessions.get(target).is_some() {
									selected = target;
									list_state.select(Some(selected));
									// Update preview cache for selected session
									if let Some(sel) = sessions.get(selected) {
										if let Ok(lines) = capture_tail(&sel.session_name, 200) {
											cached_preview = Some((sel.session_name.clone(), lines));
										}
									}
								}
							}
						}
						KeyCode::BackTab
							if !showing_tasks && !send_input_mode =>
						{
							// Send Shift+Tab to cycle Claude Code modes (plan → standard → auto-accept)
							if let Some(sel) = sessions.get(selected) {
								match send_special_key(&sel.session_name, "BTab") {
									Ok(()) => {
										status_message = Some((
											format!("Sent Shift+Tab to {} (cycle mode)", sel.name),
											Instant::now(),
										));
									}
									Err(e) => {
										status_message = Some((
											format!("Failed to send Shift+Tab: {}", e),
											Instant::now(),
										));
									}
								}
							}
						}
						KeyCode::Char('s')
							if !showing_tasks && !send_input_mode =>
						{
							// Cycle through status indicator styles
							style_idx = (style_idx + 1) % styles.len();
							status_message = Some((
								format!("Status style: {}", styles[style_idx]),
								Instant::now(),
							));
						}
						KeyCode::Char('c')
							if !showing_tasks && !send_input_mode =>
						{
							// Open config file in Cursor
							let config_path = config::base_dir()
								.map(|p| p.join("config.toml"))
								.unwrap_or_default();
							let _ = Command::new("cursor").arg(&config_path).status();
							status_message = Some((
								format!("Opened {} in Cursor", config_path.display()),
								Instant::now(),
							));
						}
							_ => {}
					}
				}
			}
		}

		if last_refresh.elapsed() >= Duration::from_millis(cfg.general.poll_interval_ms.min(5_000))
		{
			if let Ok(updated) = collect_sessions(cfg) {
				// Check for state changes and fire notifications
				if cfg.notifications.enabled {
					for session in &updated {
						let old_status = prev_status.get(&session.session_name);
						let new_status = session.status;

						// Notify on transition to NeedsInput
						if new_status == AgentStatus::NeedsInput
							&& old_status != Some(&AgentStatus::NeedsInput)
						{
							notify::notify_needs_input(
								&session.name,
								&cfg.notifications.sound_needs_input,
							);
						}

						// Notify on transition to Done
						if new_status == AgentStatus::Done
							&& old_status != Some(&AgentStatus::Done)
						{
							notify::notify_done(&session.name, &cfg.notifications.sound_done);
						}

						prev_status.insert(session.session_name.clone(), new_status);
					}
				}

				if updated.is_empty() {
					selected = 0;
					list_state.select(None);
				} else if selected >= updated.len() {
					selected = updated.len() - 1;
					list_state.select(Some(selected));
				}
				sessions = updated;
				// Update preview cache for selected session
				if let Some(sel) = sessions.get(selected) {
					if let Ok(lines) = capture_tail(&sel.session_name, 200) {
						cached_preview = Some((sel.session_name.clone(), lines));
					}
				}
			}
			tasks = load_tasks(cfg);
			if tasks.is_empty() {
				tasks_state.select(None);
			} else if tasks_state.selected().is_none() {
				tasks_state.select(Some(0));
			} else if let Some(sel) = tasks_state.selected() {
				if tasks.is_empty() {
					tasks_state.select(None);
				} else if sel >= tasks.len() {
					tasks_state.select(Some(tasks.len() - 1));
				}
			}
			last_refresh = Instant::now();
		}
	}

	teardown_terminal()?;
	Ok(())
}

fn agents_footer_text(width: u16) -> String {
	if width < 100 {
		"A: enter | S-Tab | 1-9 | a | n | d | t | s | c cfg | h | q".to_string()
	} else {
		"Agents: enter | S-Tab mode | 1-9 | a attach | n new | d done | t tasks | s style | c config | h | q".to_string()
	}
}

fn tasks_footer_text(width: u16) -> String {
	if width < 100 {
		"T: enter | N new | n new task | Y⚠️ yolo | Esc back | h | q"
			.to_string()
	} else {
		"Tasks: enter/N start | n new task | Y⚠️ yolo | o Obsidian | x del | Esc back | h help | q"
			.to_string()
	}
}

#[allow(dead_code)] // May be useful if we re-add filtering later
fn task_matches_filter(task: &TaskEntry, filter: &str) -> bool {
	if filter.trim().is_empty() {
		return true;
	}
	let needle = filter.to_lowercase();
	task.title.to_lowercase().contains(&needle)
}

#[allow(dead_code)] // May be useful if we re-add filtering later
fn filtered_tasks<'a>(tasks: &'a [TaskEntry], filter: &str) -> Vec<&'a TaskEntry> {
	tasks
		.iter()
		.filter(|t| task_matches_filter(t, filter))
		.collect()
}

#[allow(dead_code)] // May be useful for future task management features
fn mark_task_done(task: &TaskEntry, cfg: &Config) -> Result<()> {
	let content = fs::read_to_string(&task.path)?;
	if content.starts_with("---") {
		let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
		let mut in_frontmatter = false;
		let mut replaced = false;
		for line in lines.iter_mut() {
			if line.trim() == "---" {
				if !in_frontmatter {
					in_frontmatter = true;
					continue;
				} else {
					break;
				}
			}
			if in_frontmatter && line.trim_start().starts_with("status:") {
				*line = "status: done".to_string();
				replaced = true;
			}
		}
		if in_frontmatter && !replaced {
			// Insert status right after opening ---
			if let Some(pos) = lines.iter().position(|l| l.trim() == "---") {
				lines.insert(pos + 1, "status: done".to_string());
			}
		}
		let updated = lines.join("\n");
		fs::write(&task.path, updated)?;
	}
	let archive_dir = Path::new(&cfg.general.tasks_dir).join("archive");
	fs::create_dir_all(&archive_dir)?;
	let dest = archive_dir.join(
		task.path
			.file_name()
			.unwrap_or_else(|| std::ffi::OsStr::new("task.md")),
	);
	fs::rename(&task.path, dest)?;
	Ok(())
}

fn delete_task(task: &TaskEntry) -> Result<()> {
	fs::remove_file(&task.path)?;
	Ok(())
}

/// Check if a line looks like a prompt asking for user input
fn is_prompt_line(line: &str) -> bool {
	let trimmed = line.trim();
	if trimmed.is_empty() {
		return false;
	}
	// Common prompt patterns
	trimmed.contains("[Y/n]")
		|| trimmed.contains("[y/N]")
		|| trimmed.contains("[yes/no]")
		|| trimmed.ends_with('?')
		|| trimmed.starts_with("> ")
		|| trimmed.starts_with("? ")
		|| trimmed.contains("Do you want")
		|| trimmed.contains("Should I")
		|| trimmed.contains("Press enter")
		|| trimmed.contains("waiting for")
}

fn clean_preview(lines: &[String]) -> Vec<String> {
	let mut out = Vec::with_capacity(lines.len());
	for line in lines {
		let trimmed = line.trim();
		let is_separator = trimmed.chars().all(|c| c == '─' || c == '-' || c == '━');
		if is_separator {
			// Collapse repeated separator lines only.
			if out
				.last()
				.map(|prev: &String| prev.trim() == trimmed)
				.unwrap_or(false)
			{
				continue;
			}
		}
		out.push(line.clone());
	}
	if out.is_empty() {
		vec!["(no recent output; select and attach to see more)".to_string()]
	} else {
		out
	}
}

fn mini_log_preview(lines: &[String]) -> Option<String> {
	let cleaned = clean_preview(lines);
	let snippet = cleaned
		.iter()
		.rev()
		.find(|l| !l.trim().is_empty())
		.cloned()?;
	let max_chars = 80;
	let count = snippet.chars().count();
	if count > max_chars {
		let truncated: String = snippet.chars().take(max_chars).collect();
		Some(format!("{truncated}…"))
	} else {
		Some(snippet)
	}
}

fn status_indicator(status: AgentStatus, style: &str) -> (&'static str, Style) {
	match style {
		"emoji" => match status {
			AgentStatus::NeedsInput => ("🔴", Style::default()),
			AgentStatus::Running => ("🟢", Style::default()),
			AgentStatus::Idle => ("🟡", Style::default()),
			AgentStatus::Done => ("✓ ", Style::default().add_modifier(Modifier::DIM)),
			AgentStatus::Unknown => ("⚪", Style::default()),
		},
		"unicode" => match status {
			AgentStatus::NeedsInput => (
				"●",
				Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
			),
			AgentStatus::Running => (
				"▶",
				Style::default()
					.fg(Color::Green)
					.add_modifier(Modifier::BOLD),
			),
			AgentStatus::Idle => ("○", Style::default().fg(Color::Yellow)),
			AgentStatus::Done => ("✓", Style::default().fg(Color::Cyan)),
			AgentStatus::Unknown => ("·", Style::default().fg(Color::DarkGray)),
		},
		"text" => match status {
			AgentStatus::NeedsInput => (
				"[WAIT]",
				Style::default()
					.fg(Color::White)
					.bg(Color::Red)
					.add_modifier(Modifier::BOLD),
			),
			AgentStatus::Running => (
				"[RUN] ",
				Style::default()
					.fg(Color::Green)
					.add_modifier(Modifier::BOLD),
			),
			AgentStatus::Idle => ("[idle]", Style::default().fg(Color::Yellow)),
			AgentStatus::Done => ("[done]", Style::default().fg(Color::Cyan)),
			AgentStatus::Unknown => ("[ ? ] ", Style::default().fg(Color::DarkGray)),
		},
		// Default to unicode style for unknown values
		_ => match status {
			AgentStatus::NeedsInput => (
				"●",
				Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
			),
			AgentStatus::Running => (
				"▶",
				Style::default()
					.fg(Color::Green)
					.add_modifier(Modifier::BOLD),
			),
			AgentStatus::Idle => ("○", Style::default().fg(Color::Yellow)),
			AgentStatus::Done => ("✓", Style::default().fg(Color::Cyan)),
			AgentStatus::Unknown => ("·", Style::default().fg(Color::DarkGray)),
		},
	}
}

fn format_human_duration(d: Duration) -> String {
	let secs = d.as_secs();
	if secs < 60 {
		format!("{secs}s ago")
	} else if secs < 3600 {
		format!("{}m ago", secs / 60)
	} else if secs < 86_400 {
		format!("{}h ago", secs / 3600)
	} else {
		format!("{}d ago", secs / 86_400)
	}
}

fn agent_details(sel: &AgentSession) -> String {
	let task_path = sel
		.task
		.as_ref()
		.map(|t| t.path.display().to_string())
		.unwrap_or_else(|| "-".to_string());
	let repo_path = session_path(&sel.session_name)
		.ok()
		.flatten()
		.unwrap_or_else(|| "-".to_string());
	let read_cmd = format!("tmux capture-pane -p -S -500 -t {}", sel.session_name);
	format!(
		"Task: {}\nRepo: {}\n\nRead from another Claude:\n{}",
		task_path, repo_path, read_cmd
	)
}

fn help_text() -> String {
	format!(
		r#"╭──────────────────────────────────────╮
│  SWARM - Command your agent fleet   │
│               v{:<22}│
╰──────────────────────────────────────╯

Agents view:
  enter     send input (quick reply)
  S-Tab     cycle Claude mode (plan/std/auto)
  1-9       quick navigate
  a         attach (full tmux)
  n         new agent (creates task)
  d         done (kill session)
  t         tasks view
  s         cycle status style
  c         open config in Cursor
  q         quit

Tasks view:
  enter     start/resume agent
  N         force new session
  Y         ⚠️  YOLO mode (no permissions!)
  n         new task
  o         open in Cursor
  x         delete task
  Esc       back to agents
  q         quit

Claude commands (run inside agent):
  /done       end session, log work
  /log        save progress to task file
  /interview  detailed task planning
  /worktree   move to isolated git worktree
  /poll-pr    monitor PR until CI passes
  /qa-swarm   QA test the swarm TUI

tmux (when attached):
  Ctrl-b d  detach (return to swarm)
  Ctrl-b [  scroll mode (q to exit)

Config: ~/.swarm/config.toml
  [allowed_tools] - auto-accept safe commands

─────────────────────────────────────
Built with 🧡 by Whop
github.com/whopio/swarm"#,
		env!("CARGO_PKG_VERSION")
	)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
	let popup_layout = Layout::default()
		.direction(Direction::Vertical)
		.constraints(
			[
				Constraint::Percentage((100 - percent_y) / 2),
				Constraint::Percentage(percent_y),
				Constraint::Percentage((100 - percent_y) / 2),
			]
			.as_ref(),
		)
		.split(r);

	let horizontal = Layout::default()
		.direction(Direction::Horizontal)
		.constraints(
			[
				Constraint::Percentage((100 - percent_x) / 2),
				Constraint::Percentage(percent_x),
				Constraint::Percentage((100 - percent_x) / 2),
			]
			.as_ref(),
		)
		.split(popup_layout[1]);

	horizontal[1]
}

fn attach_to(
	terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
	sel: &AgentSession,
) -> Result<()> {
	// Leave TUI
	teardown_terminal()?;
	let status = Command::new(find_tmux())
		.arg("attach-session")
		.arg("-t")
		.arg(&sel.session_name)
		.status()
		.context("failed to attach to tmux session")?;
	if !status.success() {
		eprintln!("tmux attach failed: {} (using {})", status, find_tmux());
	}
	// Re-enter TUI
	enable_raw_mode()?;
	let mut stdout_handle = stdout();
	execute!(stdout_handle, EnterAlternateScreen)?;
	*terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout_handle))?;
	Ok(())
}

fn teardown_terminal() -> Result<()> {
	disable_raw_mode()?;
	execute!(stdout(), LeaveAlternateScreen)?;
	Ok(())
}

fn mark_done(session: &AgentSession, _cfg: &Config) -> Result<()> {
	// Just kill the session and clean up session store
	kill_session(&session.session_name)?;
	// Clean up session metadata
	if let Ok(marker) = session_task_path(&session.session_name) {
		let _ = fs::remove_file(&marker);
		if let Some(parent) = marker.parent() {
			let _ = fs::remove_dir_all(parent);
		}
	}
	// Remove log file
	let _ = fs::remove_file(&session.log_path);
	Ok(())
}

#[allow(dead_code)] // May be useful for future daily logging features
fn append_daily(session: &AgentSession, cfg: &Config) -> Result<()> {
	let dir = PathBuf::from(&cfg.general.daily_dir);
	fs::create_dir_all(&dir)?;
	let date = Local::now();
	let file = dir.join(format!(
		"{}-{:02}-{:02}.md",
		date.year(),
		date.month(),
		date.day()
	));
	let mut f = fs::OpenOptions::new()
		.create(true)
		.append(true)
		.open(&file)?;
	let title = session
		.task
		.as_ref()
		.map(|t| t.title.clone())
		.unwrap_or_else(|| session.name.clone());
	use std::io::Write;
	writeln!(
		f,
		"- {:02}:{:02} {} ({}) marked done",
		date.hour(),
		date.minute(),
		session.name,
		title
	)?;
	Ok(())
}

fn start_from_task(cfg: &Config, task: &TaskEntry) -> Result<String> {
	start_from_task_inner(cfg, task, false)
}

/// ⚠️ YOLO MODE - Start task with --dangerously-skip-permissions
fn start_from_task_yolo(cfg: &Config, task: &TaskEntry) -> Result<String> {
	start_from_task_inner(cfg, task, true)
}

fn start_from_task_inner(cfg: &Config, task: &TaskEntry, auto_accept: bool) -> Result<String> {
	let base_name = slugify(task.title.clone());
	let session_name = unique_session_name(&base_name)?;
	let repo = std::env::current_dir()?.to_string_lossy().into_owned();
	let prompt = format!(
		"Starting task. Read {} for context (include any Process Log). Summarize the task file before acting.",
		task.path.display()
	);
	handle_new(
		cfg,
		session_name.clone(),
		cfg.general.default_agent.clone(),
		repo,
		false, // Worktrees disabled by default (slow due to git fetch). Use CLI --worktree flag when needed.
		Some(prompt),
		Some(task.path.to_string_lossy().into_owned()),
		auto_accept,
		false, // announce
	)?;
	Ok(session_name)
}

fn unique_session_name(base: &str) -> Result<String> {
	let mut name = base.to_string();
	let mut counter = 1;
	let existing = list_sessions()?;
	while existing
		.iter()
		.any(|s| s.trim_start_matches(SWARM_PREFIX) == name)
	{
		counter += 1;
		name = format!("{base}-{counter}");
	}
	Ok(name)
}

#[allow(dead_code)] // May be useful for quick untracked agents later
fn quick_new(cfg: &Config, task: Option<String>) -> Result<String> {
	let base = format!("agent-{}", chrono::Local::now().format("%H%M%S"));
	let repo = std::env::current_dir()?.to_string_lossy().into_owned();
	handle_new(
		cfg,
		base.clone(),
		cfg.general.default_agent.clone(),
		repo,
		false,
		None,
		task,
		false, // auto_accept
		false, // announce
	)?;
	Ok(base)
}

/// Create a task file from description and start an agent for it
fn create_task_and_start_agent(
	cfg: &Config,
	description: &str,
	notify: Option<&str>,
	due_input: Option<&str>,
) -> Result<String> {
	// Slugify the description for filename
	let slug = slug::slugify(description);
	let slug = if slug.len() > 50 {
		slug[..50].to_string()
	} else {
		slug
	};

	// Calculate due date
	let today = Local::now().date_naive();
	let due_date = if let Some(input) = due_input {
		// Parse MM-DD format
		let parts: Vec<&str> = input.split('-').collect();
		if parts.len() == 2 {
			if let (Ok(month), Ok(day)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
				// Use current year, bump to next year if date has passed
				let mut year = today.year();
				if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
					if date < today {
						year += 1;
					}
					NaiveDate::from_ymd_opt(year, month, day).unwrap_or(today + chrono::Duration::days(1))
				} else {
					today + chrono::Duration::days(1)
				}
			} else {
				today + chrono::Duration::days(1)
			}
		} else {
			today + chrono::Duration::days(1)
		}
	} else {
		today + chrono::Duration::days(1)
	};

	// Build task file content
	let notify_section = if let Some(who) = notify {
		format!("- {}", who)
	} else {
		"- (fill in who to notify)".to_string()
	};

	let content = format!(
		r#"---
status: todo
due: {}
tags: [work]
summary: {}
---

# {}

{}

## When done
{}

## Process Log
(Claude logs progress here)
"#,
		due_date.format("%Y-%m-%d"),
		description,
		description,
		description,
		notify_section,
	);

	// Write task file
	let tasks_dir = PathBuf::from(&cfg.general.tasks_dir);
	let task_path = tasks_dir.join(format!("{}.md", slug));
	fs::write(&task_path, &content)?;

	// Create agent with this task
	let task_entry = TaskEntry {
		title: description.to_string(),
		path: task_path.clone(),
		due: Some(due_date),
		status: Some("todo".to_string()),
	};

	start_from_task(cfg, &task_entry)
}

#[allow(dead_code)] // Kept for potential Claude-assisted task creation
fn quick_new_with_prompt(cfg: &Config, prompt: &str) -> Result<String> {
	let base = format!("task-creator-{}", chrono::Local::now().format("%H%M%S"));
	let repo = std::env::current_dir()?.to_string_lossy().into_owned();
	handle_new(
		cfg,
		base.clone(),
		cfg.general.default_agent.clone(),
		repo,
		false,
		Some(prompt.to_string()),
		None,
		false, // auto_accept
		false, // announce
	)?;
	Ok(base)
}

#[allow(dead_code)] // May be useful for debugging session issues
fn snapshot_session(session: &AgentSession) -> Result<String> {
	let dir = snapshots_dir()?;
	fs::create_dir_all(&dir)?;
	let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
	let filename = format!("{}-{}.log", session.session_name, ts);
	let path = dir.join(filename);
	let output = Command::new(find_tmux())
		.arg("capture-pane")
		.arg("-p")
		.arg("-J")
		.arg("-t")
		.arg(format!("{}:0.0", session.session_name))
		.output()
		.context("failed to capture pane")?;
	if !output.status.success() {
		return Err(anyhow::anyhow!(
			"tmux capture-pane failed: {}",
			String::from_utf8_lossy(&output.stderr)
		));
	}
	fs::write(&path, output.stdout)?;
	Ok(path.to_string_lossy().to_string())
}
