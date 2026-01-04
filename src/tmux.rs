use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SWARM_PREFIX: &str = "swarm-";

pub fn list_sessions() -> Result<Vec<String>> {
	let output = Command::new("tmux")
		.arg("list-sessions")
		.arg("-F")
		.arg("#{session_name}")
		.output();

	let output = match output {
		Ok(out) => out,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
			return Err(anyhow::anyhow!("tmux not found on PATH"));
		}
		Err(e) => return Err(e.into()),
	};

	if !output.status.success() {
		return Ok(vec![]);
	}

	let stdout = String::from_utf8_lossy(&output.stdout);
	let sessions = stdout
		.lines()
		.filter(|line| line.starts_with(SWARM_PREFIX))
		.map(|s| s.trim().to_string())
		.collect();
	Ok(sessions)
}

pub fn ensure_pipe(session: &str, log_path: &Path) -> Result<()> {
	if let Some(parent) = log_path.parent() {
		fs::create_dir_all(parent)?;
	}

	let cmd = format!("cat >> {}", log_path.to_string_lossy());
	let target = format!("{session}:0.0");
	let status = Command::new("tmux")
		.arg("pipe-pane")
		.arg("-t")
		.arg(&target)
		.arg(cmd)
		.status()
		.with_context(|| "failed to set tmux pipe-pane")?;

	if !status.success() {
		return Err(anyhow::anyhow!(
			"tmux pipe-pane failed for session {}",
			session
		));
	}
	Ok(())
}

pub fn capture_tail(session: &str, lines: usize) -> Result<Vec<String>> {
	let output = Command::new("tmux")
		.arg("capture-pane")
		.arg("-p")
		.arg("-J")
		.arg("-t")
		.arg(format!("{session}:0.0"))
		.arg("-S")
		.arg(format!("-{}", lines as isize))
		.output()
		.context("failed to capture pane")?;

	if !output.status.success() {
		return Err(anyhow::anyhow!(
			"tmux capture-pane failed: {}",
			String::from_utf8_lossy(&output.stderr)
		));
	}

	let stdout = String::from_utf8_lossy(&output.stdout);
	Ok(stdout.lines().map(|s| s.to_string()).collect())
}

pub fn pane_last_used(session: &str) -> Result<Option<SystemTime>> {
	let output = Command::new("tmux")
		.arg("list-panes")
		.arg("-t")
		.arg(session)
		.arg("-F")
		.arg("#{pane_last_used}")
		.output()?;

	if !output.status.success() {
		return Ok(None);
	}

	let stdout = String::from_utf8_lossy(&output.stdout);
	let max_epoch = stdout
		.lines()
		.filter_map(|l| l.trim().parse::<u64>().ok())
		.max();

	Ok(max_epoch.map(|secs| UNIX_EPOCH + Duration::from_secs(secs)))
}

pub fn start_session(session: &str, dir: &Path, command: &str) -> Result<()> {
	start_session_with_options(session, dir, command, false)
}

/// Start a session with optional mise activation (for Claude/Codex in monorepo)
pub fn start_session_with_mise(session: &str, dir: &Path, command: &str) -> Result<()> {
	start_session_with_options(session, dir, command, true)
}

fn start_session_with_options(
	session: &str,
	dir: &Path,
	command: &str,
	use_mise: bool,
) -> Result<()> {
	// Wrap command with mise activation if requested
	// This ensures the agent runs with the correct environment (node, ruby, etc.)
	let final_command = if use_mise {
		format!(
			"zsh -c 'mise trust 2>/dev/null; eval \"$(mise activate zsh)\"; exec {}'",
			command
		)
	} else {
		command.to_string()
	};

	let status = Command::new("tmux")
		.arg("new-session")
		.arg("-d")
		.arg("-s")
		.arg(session)
		.arg("-c")
		.arg(dir)
		.arg(&final_command)
		.status()
		.with_context(|| format!("failed to start tmux session {}", session))?;

	if !status.success() {
		return Err(anyhow::anyhow!(
			"tmux new-session failed for {} (status {})",
			session,
			status
		));
	}
	Ok(())
}

pub fn send_keys(session: &str, text: &str) -> Result<()> {
	// Send the text literally first
	let status = Command::new("tmux")
		.arg("send-keys")
		.arg("-l") // literal mode - don't interpret special chars in text
		.arg("-t")
		.arg(session)
		.arg(text)
		.status()
		.with_context(|| format!("failed to send keys to {}", session))?;
	if !status.success() {
		return Err(anyhow::anyhow!("tmux send-keys failed for {}", session));
	}

	// Then send Enter separately
	let status = Command::new("tmux")
		.arg("send-keys")
		.arg("-t")
		.arg(session)
		.arg("Enter")
		.status()
		.with_context(|| format!("failed to send Enter to {}", session))?;
	if !status.success() {
		return Err(anyhow::anyhow!(
			"tmux send-keys Enter failed for {}",
			session
		));
	}
	Ok(())
}

/// Send a special key like "BTab" (Shift+Tab), "C-c" (Ctrl+C), etc.
pub fn send_special_key(session: &str, key: &str) -> Result<()> {
	let status = Command::new("tmux")
		.arg("send-keys")
		.arg("-t")
		.arg(session)
		.arg(key)
		.status()
		.with_context(|| format!("failed to send {} to {}", key, session))?;
	if !status.success() {
		return Err(anyhow::anyhow!(
			"tmux send-keys {} failed for {}",
			key,
			session
		));
	}
	Ok(())
}

pub fn kill_session(session: &str) -> Result<()> {
	let status = Command::new("tmux")
		.arg("kill-session")
		.arg("-t")
		.arg(session)
		.status()
		.with_context(|| format!("failed to kill session {}", session))?;
	if !status.success() {
		return Err(anyhow::anyhow!(
			"tmux kill-session failed for {} (status {})",
			session,
			status
		));
	}
	Ok(())
}

pub fn session_path(session: &str) -> Result<Option<String>> {
	let output = Command::new("tmux")
		.arg("display-message")
		.arg("-p")
		.arg("-t")
		.arg(session)
		.arg("#{pane_current_path}")
		.output()?;
	if !output.status.success() {
		return Ok(None);
	}
	let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
	if stdout.is_empty() {
		Ok(None)
	} else {
		Ok(Some(stdout))
	}
}
