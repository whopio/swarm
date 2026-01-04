# QA Swarm TUI

Comprehensive QA testing for the Swarm TUI.

## Product Philosophy: "Don't Make Me Think"

Swarm follows Steve Krug's "Don't Make Me Think" principles:

1. **Obvious > Clever** - Every action should be self-evident
2. **Optimize for common case** - Enter = send input (most frequent action)
3. **Minimize choices** - Removed features that weren't used (filter, snapshots, re-pipe)
4. **Progressive disclosure** - Simple footer, detailed help on `h`
5. **Spatial stability** - Lists don't jump around based on status

**Core Use Case:** Monitor 5-10 AI coding agents, send quick replies, never lose track of who needs attention.

---

## Setup

1. Build swarm:
   ```bash
   cargo build -p swarm
   ```

2. Run swarm:
   ```bash
   cargo run -p swarm
   ```

---

## Key Bindings Reference

### Agents View (default)
| Key | Action |
|-----|--------|
| **Enter** | Send input (opens modal for quick reply) |
| **Shift+Tab** | Cycle Claude mode (plan → standard → auto-accept) |
| **1-9** | Navigate to agent (select, not attach) |
| **a** | Attach (full tmux takeover) |
| **n** | New agent (creates task file) |
| **d** | Done (kill session with confirmation) |
| **t** | Switch to tasks view |
| **s** | Cycle status style (emoji/unicode/text) |
| **c** | Open config in Cursor |
| **h** | Help modal |
| **q** | Quit |
| **Esc** | Close any modal |

### Tasks View
| Key | Action |
|-----|--------|
| **Enter** | Start/resume agent for task |
| **N** | Force new session (even if one exists) |
| **Y** | YOLO mode (--dangerously-skip-permissions) |
| **n** | New task (same flow as agents view) |
| **o** | Open in Cursor |
| **x** | Delete task |
| **Esc** | Back to agents view |
| **h** | Help modal |
| **q** | Quit |

---

## Test Flows

### Flow 1: Quick Reply (Most Common)
**Scenario:** Agent shows needs input, send "yes"

1. [ ] See agent with needs-input status (● red or [WAIT])
2. [ ] Press `1-9` or arrows to navigate to it
3. [ ] Header shows "Agents (N need input)" count
4. [ ] Preview highlights prompt line in yellow/bold
5. [ ] Press `Enter` → send input modal opens
6. [ ] Type "yes" or "y"
7. [ ] Press `Enter` → input sent, modal closes
8. [ ] Status shows confirmation message

**UX Check:** This should feel instant and natural.

### Flow 2: Start New Work
**Scenario:** Start a new coding task

1. [ ] Press `n` from agents view
2. [ ] "Name your work" modal appears with 3 fields
3. [ ] Type description: "Fix auth bug"
4. [ ] Press `Tab` → moves to "Who to notify" field
5. [ ] Type: "@someone in slack"
6. [ ] Press `Tab` → moves to due date (shows "tomorrow")
7. [ ] Leave blank or enter "01-15" for Jan 15
8. [ ] Press `Enter` → creates task + starts agent
9. [ ] Agent appears in list with task title
10. [ ] Task file created in `~/.swarm/tasks/`

**Verify task file has:**
- `status: in-progress`
- `due: YYYY-MM-DD`
- `## When done` section with notify info

### Flow 3: Resume Existing Work
**Scenario:** Pick up a task that already has a session

1. [ ] Press `t` to go to tasks view
2. [ ] Tasks with active sessions show `●` prefix in green
3. [ ] Navigate to a task with active session
4. [ ] Press `Enter`
5. [ ] Shows "Switched to existing session: <name>"
6. [ ] Returns to agents view with that session selected

### Flow 4: Empty State
**Scenario:** No agents running

1. [ ] Kill all swarm sessions
2. [ ] Preview shows "No agents yet."
3. [ ] Shows hints: "Press n to create a new agent" and "Press t to see saved tasks"

### Flow 5: Full Attach (When Needed)
**Scenario:** Need to scroll history or run /done

1. [ ] Select agent with `1-9` or arrows
2. [ ] Press `a` to attach
3. [ ] Full tmux session takes over
4. [ ] Can scroll with `Ctrl-b [`, run commands, etc.
5. [ ] Press `Ctrl-b d` to detach
6. [ ] Returns to swarm dashboard

### Flow 6: End Session Properly
**Scenario:** Finish work on an agent

1. [ ] Select the agent to close
2. [ ] Press `d`
3. [ ] Confirmation modal appears: "Did you run /done in Claude first?"
4. [ ] Press `y` to confirm OR `Esc` to cancel
5. [ ] If confirmed: session killed, removed from list

### Flow 7: YOLO Mode (Dangerous)
**Scenario:** Start task with auto-accept permissions

1. [ ] Press `t` for tasks view
2. [ ] Select a task
3. [ ] Press `Y` (capital Y)
4. [ ] Session starts with `--dangerously-skip-permissions`
5. [ ] YOLO session shows: ⚠️ in agent list, red border on preview, warning banner

### Flow 8: First-Run Onboarding
**Scenario:** Test the hooks install prompt

1. [ ] Edit ~/.swarm/config.toml, set `hooks_installed = false`
2. [ ] Run swarm
3. [ ] Welcome modal appears with hooks list
4. [ ] Press `y` to install
5. [ ] Hooks copied to ~/.claude/commands/
6. [ ] Status message confirms installation
7. [ ] Modal closes, normal TUI shows
8. [ ] Run swarm again - no modal (hooks_installed = true now)

### Flow 9: Mode Cycling (Shift+Tab)
**Scenario:** Cycle Claude Code between plan/standard/auto-accept modes

1. [ ] Select an agent
2. [ ] Press `Shift+Tab`
3. [ ] Status message shows "Sent Shift+Tab to <agent> (cycle mode)"
4. [ ] If attached, Claude Code cycles through modes

**Note:** This sends the actual Shift+Tab keystroke to Claude Code inside the tmux session.

### Flow 10: Open Config (c key)
**Scenario:** Quickly edit config from within swarm

1. [ ] Press `c` from agents view
2. [ ] Cursor opens `~/.swarm/config.toml`
3. [ ] Status message shows "Opened ~/.swarm/config.toml in Cursor"
4. [ ] Can edit allowed_tools, notifications, etc.

---

## Test Checklist

### Agents View
- [ ] Numbers 1-9 shown next to agents for quick nav
- [ ] Status indicators cycle with `s` key (3 styles: unicode/emoji/text)
- [ ] Shift+Tab sends mode cycle keystroke to selected agent
- [ ] `c` key opens config file in Cursor
- [ ] Header shows "(N need input)" when any agents waiting
- [ ] Preview shows live output (bottom-anchored)
- [ ] Prompt lines highlighted in yellow/bold
- [ ] Details panel shows: task path, repo path, tmux read command
- [ ] Mini-log snippet shown in agent list row
- [ ] YOLO sessions show ⚠️ indicator

### Tasks View
- [ ] Tasks sorted by due date
- [ ] Due dates formatted nicely ("due tomorrow", "due in 6d")
- [ ] Active sessions show `●` prefix in green
- [ ] `status: done/completed` tasks filtered out
- [ ] `archive/` directory skipped
- [ ] Esc returns to agents view
- [ ] `o` opens task in Cursor

### Empty State
- [ ] Shows "No agents yet." message
- [ ] Shows hints for n and t keys

---

## Performance Checklist

```bash
# Get swarm PID and CPU usage
ps aux | grep swarm | grep -v grep | head -1

# Expected: <5% CPU at idle, <15% during refresh
```

- [ ] Idle CPU < 5% with no agents
- [ ] Idle CPU < 10% with 5+ agents
- [ ] Navigation is instant (no lag on 1-9 or arrows)
- [ ] Preview updates within 1 second of selection change

---

## Report Format

### Critical Issues
- **Issue name** (file:line if known)
- Repro steps
- Expected vs actual

### UX Friction
| Area | Issue | Suggestion |
|------|-------|------------|

### Working Features
Bulleted list of verified features.

### "Don't Make Me Think" Violations
List any places where the UI requires thought or is non-obvious.
