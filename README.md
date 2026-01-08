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
- **Claude hooks** - Built-in slash commands (/done, /log, /interview, /poll-pr, /worktree)
- **Git worktrees** - Isolated parallel agent sessions using native git worktrees

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
- **/worktree** - Create isolated git worktree for the task

Hooks are installed to `~/.claude/commands/` on first run.

## Git Worktrees

Isolated parallel agent sessions using native git worktrees.

### Why Worktrees?

- **Native git** - No extra tooling needed, works with any git repo
- **Parallel agents** - Each agent works in isolation on its own branch
- **Fresh from main** - Always creates worktree from latest `origin/main`
- **Persists on done** - Worktrees are kept after session ends so work can be resumed

### How It Works

When you start a new agent for a code task, Claude will ask:

> "Do you want me to create a git worktree for isolation?"

If you say yes, Claude calls `/worktree` which:

1. Fetches latest `origin/main`
2. Creates a worktree at `~/worktrees/{task-name}`
3. Creates a branch `{branch_prefix}/{task-slug}` from main
4. Changes to the worktree directory

Sessions with worktrees show `[wt]` badge.

### Notes

- Worktrees are created at `~/worktrees/{task-name}` by default
- Branch naming: `{branch_prefix}/{task-slug}` (branch_prefix from config)

### Worktree Workflow for PRs

```bash
# Make changes, then:
git add -A
git commit -m "feat: your changes"
git push -u origin sharkey11/feature-name

# Create PR
gh pr create --title "..." --body "..."
```

### Cleanup

Worktrees are kept after session ends so you can resume work later. To manually clean up:

```bash
# List all worktrees
git worktree list

# Remove a specific worktree
git worktree remove ~/worktrees/task-name

# Clean up stale references
git worktree prune
```

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
