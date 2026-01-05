# End of Session

Wrap up the session. Keep it brief.

## 1. Quick summary
1-3 bullet points of what shipped. High-level only.

## 2. Log to daily file
Append to `~/.swarm/daily/YYYY-MM-DD.md`:

```markdown
## [Task name]
- What shipped (PR #XX if applicable)
```

**Keep it ultra-brief** - 1-2 lines per task. Include PR links. Someone skimming for standup should get it in 3 seconds.

## 3. Learnings (only obvious wins)
Only suggest saving a learning if it:
- Applies across multiple projects, AND
- Will help in 6+ months, OR
- User explicitly asks to save it

**Don't ask about learnings unless something clearly saved 30+ min of debugging.**

When suggesting, categorize by type:
- **Workflow/meta**: How to run Claude/swarm better
- **Framework**: CI, testing, code patterns that generalize
- **Gotcha**: Technical traps that caused pain

Format for CLAUDE.md: `Rule + 1-line context`
```markdown
- GITHUB_TOKEN commits don't trigger workflows (use same-run builds for release automation)
```

## 4. Notifications
Check task file's `## When done` section. Remind user who to notify.

## 5. Workspace cleanup
If in jj workspace, note the path. Swarm handles cleanup on done.
