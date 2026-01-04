# swarm

Terminal dashboard for managing multiple AI coding agents in parallel. See who needs attention. Never lose track.

<!-- TODO: Add screenshot here -->

```
╭──────────────────────────────────────────────────────────────────────────────╮
│ Agents (2 need input)                                                        │
├──────────────────────────────────────────────────────────────────────────────┤
│  1 ● auth-bug           [WAIT] Should I proceed with...                      │
│  2   api-refactor       Running tests...                                     │
│  3 ● payment-flow       [WAIT] Which approach do you...                      │
│  4   docs-update        Writing documentation...                             │
╰──────────────────────────────────────────────────────────────────────────────╯
```

## Features

- **Needs input detection** - Instantly see which agents are waiting for you (● red indicator)
- **Desktop notifications** - Get notified when agents need input, even when swarm is in the background
- **Quick reply** - Send input without leaving the dashboard (Enter key)
- **Dashboard view** - See all your AI agents at a glance with live status
- **Task tracking** - Associate agents with task files for context persistence
- **Auto-updates** - Checks daily and updates automatically on startup
- **YOLO mode** - Auto-accept permissions for trusted tasks
- **Shift+Tab** - Cycle Claude Code modes without attaching

## Install

### From source (Cargo)

```bash
cargo install --git https://github.com/whopio/swarm
```

After install, `swarm` is available globally (Cargo adds `~/.cargo/bin` to your PATH).

## Quick Start

```bash
# Launch the dashboard
swarm

# Create a new agent
# Press 'n' in the dashboard, or:
swarm new "Fix the auth bug"

# Check status without opening TUI
swarm status

# Update to latest version
swarm update
```

## Key Bindings

### Agents View

| Key | Action |
|-----|--------|
| **Enter** | Send input to selected agent |
| **Shift+Tab** | Cycle Claude mode (plan/standard/auto) |
| **1-9** | Quick navigate to agent |
| **a** | Attach (full tmux session) |
| **n** | New agent with task |
| **d** | Done (kill session) |
| **t** | Switch to tasks view |
| **s** | Cycle status style |
| **h** | Help |
| **q** | Quit |

### Tasks View

| Key | Action |
|-----|--------|
| **Enter** | Start/resume agent for task |
| **N** | Force new session |
| **Y** | YOLO mode (auto-accept permissions) |
| **n** | New task |
| **o** | Open in editor |
| **x** | Delete task |
| **Esc** | Back to agents |

## Architecture

Swarm has **no database, no server, no daemon** - it's a single binary TUI that derives all state from tmux and the filesystem.

### The Three Sources of Truth

#### 1. tmux (Runtime State)
tmux is the process manager. Each agent runs in a tmux session named `swarm-<name>`.

```bash
# Swarm queries tmux for:
tmux list-sessions              # Discover active agents
tmux capture-pane -p -t sess    # Get terminal output for preview
tmux list-panes -F "#{pane_last_used}"  # Track activity time
```

#### 2. Session Metadata (`~/.swarm/sessions/<session-name>/`)
```
~/.swarm/sessions/swarm-fix-auth-bug/
├── task      # Path to task file
├── agent     # Agent type: "claude" or "codex"
└── yolo      # Marker file if YOLO mode
```

#### 3. Task Files (`~/.swarm/tasks/*.md`)
Markdown with YAML frontmatter - human readable, git-friendly:

```markdown
---
status: in-progress
due: 2026-01-05
summary: Fix authentication bug
---

# Fix authentication bug

## When done
- @jack in slack

## Process Log
### 2026-01-04 14:30 - Investigation
- Found root cause in auth.rs:142
```

### Status Detection

Swarm doesn't hook into Claude - it **parses terminal output** with regex patterns:

```
needs_input = ["[Y/n]", "Do you want", "waiting for"]
running = ["Running", "Thinking", "●"]
done = ["Task completed", "finished"]
```

### Data Flow

```
┌─────────────────────────────────────────────────────────┐
│                     Swarm TUI                           │
│  (ratatui + crossterm)                                  │
└──────────────────────┬──────────────────────────────────┘
                       │
           ┌───────────┴───────────┐
           ▼                       ▼
┌──────────────────┐      ┌───────────────────┐
│   tmux sessions  │      │   File System     │
│  (process state) │      │  (persistent)     │
│                  │      │                   │
│ • swarm-task-1   │      │ ~/.swarm/         │
│ • swarm-task-2   │      │   ├── config.toml │
│ • swarm-task-3   │      │   ├── sessions/   │
└──────────────────┘      │   ├── tasks/      │
                          │   └── logs/       │
                          └───────────────────┘
```

### Design Principles

| Decision | Rationale |
|----------|-----------|
| **No database** | Files are debuggable, backupable, git-friendly |
| **tmux for processes** | Battle-tested, handles all terminal complexity |
| **Polling not events** | Simple, reliable, no IPC complexity |
| **Regex status detection** | Works with any agent without integration |
| **Single binary** | `cargo install` and done, no dependencies |

## Updating

Swarm auto-updates in the background. On startup, it checks for updates once per day and installs them automatically. You'll see "✨ Just updated to vX.X.X!" in the header after an update.

To manually check/update:

```bash
swarm update
```

## Configuration

Config file: `~/.swarm/config.toml`

```toml
[general]
tasks_dir = "~/.swarm/tasks"
poll_interval_ms = 2000
branch_prefix = "yourname/"
default_agent = "claude"

[notifications]
enabled = true
sound_needs_input = "Ping"
sound_done = "Glass"
```

## Requirements

- macOS (Linux support coming)
- [tmux](https://github.com/tmux/tmux)
- [Claude Code](https://claude.ai/code) (or compatible AI coding agent)

## Development

```bash
# Clone
git clone https://github.com/whopio/swarm
cd swarm

# Build
cargo build

# Run
cargo run

# Build release
cargo build --release
```

## Releasing Updates

To release a new version:

1. Update version in `Cargo.toml`
2. Commit and push to main
3. Create a git tag and GitHub release with prebuilt binaries:
   ```bash
   git tag -a v0.2.0 -m "Release v0.2.0"
   git push origin v0.2.0
   ```
4. Attach binaries to the release for each platform:
   - `swarm-aarch64-apple-darwin` (Mac M1/M2)
   - `swarm-x86_64-apple-darwin` (Mac Intel)
   - `swarm-x86_64-unknown-linux-gnu` (Linux)

Users running `swarm update` will automatically get the new version.

## License

MIT

## Credits

Built with 🧡 by [Whop](https://whop.com)

---

*"Don't make me think" - Swarm follows Steve Krug's principles for intuitive UX*
