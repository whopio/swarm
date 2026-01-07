# swarm

Command your agent fleet.

## Install

```bash
cargo install --git https://github.com/whopio/swarm
```

After install, `swarm` is available globally (Cargo adds `~/.cargo/bin` to your PATH).

**Requirements:** macOS, [tmux](https://github.com/tmux/tmux), [Claude Code](https://claude.ai/code)

### Install tmux

tmux is required for swarm to manage agent sessions.

```bash
# macOS
brew install tmux

# Ubuntu/Debian
sudo apt install tmux

# Fedora
sudo dnf install tmux
```

After installing, restart your terminal or run `source ~/.zshrc` (or `~/.bashrc`).

## Quick Start

```bash
# Launch the dashboard
swarm

# Create a new agent (press 'n' in dashboard, or):
swarm new "Fix the auth bug"

# Check status without opening TUI
swarm status

# Update to latest version
swarm update
```

## Screenshot

```
╭─ Agents (2 need input) ─────────────╮╭─ Preview ──────────────────────────╮
│  1 ● auth-bug         [WAIT] Sho...││ I've analyzed the authentication   │
│  2 ▶ api-refactor     Running te...││ code and found the issue. The      │
│  3 ● payment-flow     [WAIT] Whi...││ session token wasn't being...      │
│  4 ▶ docs-update      Writing do...││                                    │
│                                     ││ Should I proceed with the fix?     │
│                                     │├─ Details ──────────────────────────┤
│                                     ││ Task: ~/.swarm/tasks/auth-bug.md   │
│                                     ││ Repo: ~/code/myproject             │
╰─────────────────────────────────────╯╰─────────────────────────────────────╯
 Agents: enter | S-Tab mode | 1-9 | a attach | n new | d done | t tasks | h | q
```

## Features

- **Needs input detection** - Instantly see which agents are waiting for you (● red indicator)
- **Desktop notifications** - Get notified when agents need input, even when swarm is in the background
- **Quick reply** - Send input without leaving the dashboard (Enter key)
- **Dashboard view** - See all your AI agents at a glance with live status
- **Task tracking** - Associate agents with task files for context persistence
- **Auto-updates** - Checks daily and updates automatically on startup
- **Mode cycling** - Press Shift+Tab to cycle Claude between plan/standard/auto modes
- **YOLO mode** - Auto-accept permissions for trusted tasks
- **Allowed tools** - Configure safe commands to auto-accept in `[allowed_tools]` config
- **Daily logs** - Browse your daily log files with preview (press `l`)
- **Claude hooks** - Built-in slash commands (/done, /log, /interview, /poll-pr, /workspace)
- **jj workspaces** - Instant parallel agent sessions with jj (no git fetch needed)

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
| **c** | Open config in Cursor |
| **l** | Daily logs view |
| **h** | Help |
| **q** | Quit |

### Daily Logs View

| Key | Action |
|-----|--------|
| **↑/↓** | Navigate |
| **o** | Open in editor |
| **Esc** | Back to agents |

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

## tmux Keybindings

When attached to a session (press `a`), swarm uses custom tmux keybindings:

| Key | Action |
|-----|--------|
| **Alt+d** | Detach (return to swarm) |
| **Alt+↑/↓** | Scroll up/down |
| **Mouse** | Scroll enabled |

Config: `~/.swarm/tmux.conf`

## How It Works

Swarm manages AI coding agents by:

1. Creating tmux sessions prefixed with `swarm-*`
2. Monitoring session output for patterns like `[Y/n]`, `Should I`, etc.
3. Displaying status indicators (● red = needs input, ● green = running)
4. Allowing quick input without full terminal attachment

Sessions are linked to task files in `~/.swarm/tasks/` for context persistence across sessions.

## Updating

Swarm auto-updates in the background. On startup, it checks for updates once per day and installs them automatically. After an update, you'll see a changelog modal showing what's new - press any key to continue.

To manually check/update:

```bash
swarm update
```

## Configuration

Config file: `~/.swarm/config.toml`

```toml
[general]
tasks_dir = "~/.swarm/tasks"
daily_dir = "~/.swarm/daily"
hooks_installed = true
status_style = "unicode"  # unicode, emoji, or text

[notifications]
enabled = true

# Auto-accept these commands without prompting (uses sensible defaults)
# Customize by adding your own patterns:
[allowed_tools]
tools = [
  "Bash(git status:*)",
  "Bash(cargo build:*)",
  "Bash(npm run:*)",
  # Add more patterns here...
]
```

**Editing allowed_tools:** Open `~/.swarm/config.toml` and add/remove patterns in the `[allowed_tools]` section. Patterns use Claude Code's tool format: `Bash(command:*)` where `*` matches any arguments.

## Claude Hooks

Swarm includes Claude Code slash commands that work inside your agents:

- **/done** - End session and log completed work
- **/log** - Save progress to the linked task file
- **/interview** - Detailed task planning before starting
- **/poll-pr** - Monitor PR until CI passes
- **/workspace** - Move to isolated jj workspace

Hooks are installed to `~/.claude/commands/` on first run.

## jj Workspaces

Instant parallel agent sessions using [jj](https://github.com/martinvonz/jj) workspaces (faster than git worktrees).

### Why jj?

- **Instant workspace creation** - No fetch/clone needed, workspaces are instant
- **Parallel agents** - Each agent works in isolation, no merge conflicts
- **Auto-tracked changes** - No `git add` needed, jj tracks everything
- **Clean history** - Easy to rebase and squash before PRs

### Setup (One-Time)

```bash
# 1. Install jj
brew install jj

# 2. Init in your repo (works alongside git)
cd your-repo
jj git init --colocate

# 3. Configure user (for commits)
jj config set --user user.name "Your Name"
jj config set --user user.email "you@example.com"
```

### Opting In

**Per-agent:** When creating a new agent, press Tab to the `[Workspace: ○]` field, then Space to enable it. The indicator changes to `●` when enabled.

```
╭─ New Agent ─────────────────────────────────╮
│ Task: fix auth bug                          │
│ Repo: ~/code/myproject                      │
│ [Workspace: ●]  (Space to toggle)           │  ← Toggle here
╰─────────────────────────────────────────────╯
```

**Make it the default:** Set `workspace_default = true` in your config:

```toml
# ~/.swarm/config.toml
[general]
workspace_dir = "~/workspaces"
workspace_default = true  # New agents use jj workspaces by default
```

**Mid-session migration:** If you started without a workspace and need isolation, use the `/workspace` command inside your agent session. This creates a workspace and moves your work there.

Sessions with workspaces show `[jj]` badge. Auto-cleans up on done.

### jj Workflow for PRs

```bash
# Configure user (one-time)
jj config set --user user.name "Your Name"
jj config set --user user.email "you@example.com"

# Make changes, then:
jj describe -m "your commit message"
jj rebase -r @ -d main              # Skip empty parent commit
jj bookmark create sharkey11/feature-name
jj git push --bookmark sharkey11/feature-name --allow-new

# Create PR (must use --head since not a git repo)
gh pr create --repo whopio/swarm --head sharkey11/feature-name --base main
```

### git → jj Cheat Sheet

| What you want | git | jj |
|---------------|-----|-----|
| Check status | `git status` | `jj status` |
| Commit changes | `git add . && git commit -m "msg"` | `jj describe -m "msg"` |
| View history | `git log` | `jj log` |
| Create branch | `git checkout -b feature` | `jj bookmark create feature` |
| Push branch | `git push -u origin feature` | `jj git push --bookmark feature --allow-new` |
| Switch to main | `git checkout main` | `jj new main` |

**Key difference:** jj auto-tracks all changes. No `git add` needed.

### Learn jj

- [15 min video intro](https://www.youtube.com/watch?v=bx_LGilOuE4)
- [Official docs](https://jj-vcs.github.io/jj/)

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
