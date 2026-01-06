use crate::model::AgentStatus;
use regex::Regex;
use std::time::Duration;

pub struct DetectionConfig {
	pub needs_input_patterns: Vec<Regex>,
	pub running_threshold: Duration,
	pub idle_threshold: Duration,
}

pub fn detection_for_agent(agent: &str) -> DetectionConfig {
	// Defaults are tuned for Claude Code; other agents fall back to same set.
	let patterns = vec![
		// Permission prompts (high confidence)
		Regex::new(r"\[Y/n\]").unwrap(),
		Regex::new(r"\[y/N\]").unwrap(),
		Regex::new(r"\(y/N\)").unwrap(),
		Regex::new(r"\(Y/n\)").unwrap(),
		// Question patterns (high confidence)
		Regex::new(r"Do you want to proceed").unwrap(),
		Regex::new(r"Should I proceed").unwrap(),
		Regex::new(r"Would you like me to").unwrap(),
		Regex::new(r"Press enter to continue").unwrap(),
		Regex::new(r"waiting for.*input").unwrap(),
		// fzf-style prompt
		Regex::new(r"^\? ").unwrap(),
		// AskUserQuestion multi-select prompt
		Regex::new(r"Enter to select.*Tab/Arrow").unwrap(),
		// AskUserQuestion text input prompt
		Regex::new(r"Type your answer").unwrap(),
	];

	let running_threshold = Duration::from_secs(5);
	let idle_threshold = Duration::from_secs(30);

	match agent {
		_ => DetectionConfig {
			needs_input_patterns: patterns,
			running_threshold,
			idle_threshold,
		},
	}
}

pub fn detect_status(
	lines: &[String],
	detection: &DetectionConfig,
	age: Option<Duration>,
) -> AgentStatus {
	// Explicit markers first.
	if lines.iter().any(|l| l.contains("/swarm:needs_input")) {
		return AgentStatus::NeedsInput;
	}
	if lines.iter().any(|l| l.contains("/swarm:done")) {
		return AgentStatus::Done;
	}

	// Regex prompts.
	if lines.iter().any(|l| {
		detection
			.needs_input_patterns
			.iter()
			.any(|re| re.is_match(l))
	}) {
		return AgentStatus::NeedsInput;
	}

	if let Some(age) = age {
		if age <= detection.running_threshold {
			return AgentStatus::Running;
		}
		if age <= detection.idle_threshold {
			return AgentStatus::Idle;
		}
		return AgentStatus::Idle;
	}

	AgentStatus::Unknown
}
