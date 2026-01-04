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
- **Claude hooks** - Built-in slash commands (/done, /log, /interview)

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

## How It Works

Swarm manages AI coding agents by:

1. Creating tmux sessions prefixed with `swarm-*`
2. Monitoring session output for patterns like `[Y/n]`, `Should I`, etc.
3. Displaying status indicators (● red = needs input, ● green = running)
4. Allowing quick input without full terminal attachment

Sessions are linked to task files in `~/.swarm/tasks/` for context persistence across sessions.

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
daily_dir = "~/.swarm/daily"
hooks_installed = true

[notifications]
enabled = true

[status_style]
style = "unicode"  # unicode, emoji, or text
```

## Claude Hooks

Swarm includes Claude Code slash commands that work inside your agents:

- **/done** - End session and log completed work
- **/log** - Save progress to the linked task file
- **/interview** - Detailed task planning before starting

Hooks are installed to `~/.claude/commands/` on first run.

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
