# Git Worktree Setup

This skill creates an isolated git worktree for the current task, allowing parallel work without conflicts.

## What This Does

1. Fetches latest main from remote
2. Creates a new worktree at `~/worktrees/{session-name}`
3. Creates a new branch `{branch_prefix}/{task-slug}` from main
4. Changes working directory to the worktree
5. Saves worktree path for the TUI to display

## Instructions for Claude

When the user confirms they want a worktree, run the following commands:

### Step 1: Get session info

```bash
# Get the tmux session name (needed for saving worktree path)
tmux display-message -p '#S'
```

### Step 2: Create the worktree

```bash
# Fetch latest main
git fetch origin main

# Create worktree with new branch from main
# Replace {worktree-name} with a slugified version of the task
# Replace {branch-name} with your branch_prefix + task slug
git worktree add ~/worktrees/{worktree-name} -b {branch-name} origin/main

# Change to the worktree
cd ~/worktrees/{worktree-name}
```

### Step 3: Save worktree path for TUI

This is **required** for the `[wt]` badge to appear in swarm:

```bash
# Replace {session-name} with the tmux session name from Step 1
mkdir -p ~/.swarm/sessions/{session-name}
echo ~/worktrees/{worktree-name} > ~/.swarm/sessions/{session-name}/worktree
```

### Step 4: Confirm setup

After creating the worktree, tell the user:
- The worktree path: `~/worktrees/{worktree-name}`
- The branch name: `{branch-name}`
- Remind them that changes need `git add` before commit

## Working in a Worktree

Unlike the main repo, you're now on an isolated branch. Key commands:

```bash
# Check status
git status

# Stage and commit changes
git add -A
git commit -m "your commit message"

# Push your branch (first time)
git push -u origin {branch-name}

# Create PR
gh pr create --title "Your PR title" --body "Description"
```

## Cleanup

When the session ends, the worktree is kept so you can resume later.

To manually clean up old worktrees:
```bash
git worktree remove ~/worktrees/{worktree-name}
git worktree prune  # removes stale references
```

## Quick Reference

| Action | Command |
|--------|---------|
| Check status | `git status` |
| Stage all changes | `git add -A` |
| Commit | `git commit -m "msg"` |
| View log | `git log --oneline` |
| Current branch | `git branch --show-current` |
| Push new branch | `git push -u origin branch-name` |
| List worktrees | `git worktree list` |
