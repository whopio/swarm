# jj Workspace Guide

Reference guide for working in jj workspaces. Use this for mid-session migration or when you need help with jj commands.

## Creating a Workspace (Mid-Session Migration)

Use this when you realize mid-task you need isolation from other parallel agents.

### Prerequisites

```bash
# Check if jj is initialized
ls -la .jj 2>/dev/null || echo "ERROR: jj not initialized. Run: jj git init --colocate"
```

### Create Workspace

```bash
# Create workspace (instant - no fetch needed!)
jj workspace add ~/workspaces/{workspace-name}

# Move to the workspace
cd ~/workspaces/{workspace-name}

# Start from main
jj new main
```

## Working in a Workspace

**Key differences from git:**
- Changes are auto-tracked (no `git add` needed)
- Use `jj describe` instead of `git commit`
- Use `jj log` instead of `git log`
- Use `jj status` instead of `git status`

### Common Commands

```bash
# Check status
jj status

# View history
jj log

# Describe your changes (like commit message)
jj describe -m "your commit message"

# Update description of current change
jj describe -m "updated message"
```

## Creating a PR from Workspace

When your work is ready, follow these steps to create a PR:

### Step 1: Configure user (first time only)

```bash
jj config set --user user.name "Your Name"
jj config set --user user.email "you@example.com"
```

### Step 2: Describe your changes

```bash
jj describe -m "feat: your commit message"
```

### Step 3: Rebase onto main

Skip the empty parent commit that jj creates:

```bash
jj rebase -r @ -d main
```

### Step 4: Create bookmark and push

```bash
# Create bookmark (like a branch)
jj bookmark create sharkey11/feature-name

# Push to remote
jj git push --bookmark sharkey11/feature-name --allow-new
```

### Step 5: Create PR

From a jj workspace, you must specify `--head` and `--base`:

```bash
gh pr create --repo OWNER/REPO --head sharkey11/feature-name --base main
```

**Note:** Use `--repo` because the workspace isn't a regular git repo.

## After PR is Created

Use `/poll-pr` to monitor CI and respond to AI reviewer feedback.

## Cleanup

Cleanup happens automatically when the agent marks done. Manual cleanup:

```bash
# From the parent repo (not the workspace):
jj workspace forget ~/workspaces/{workspace-name}
rm -rf ~/workspaces/{workspace-name}
```

## Quick Reference

| Git | jj |
|-----|-----|
| `git status` | `jj status` |
| `git add . && git commit -m "msg"` | `jj describe -m "msg"` |
| `git log` | `jj log` |
| `git branch feature` | `jj bookmark create feature` |
| `git push -u origin feature` | `jj git push --bookmark feature --allow-new` |
| `git checkout main` | `jj new main` |
