# End of Session

User is wrapping up this session. Do the following:

1. **Summarize what got done** - List the main accomplishments from this session

2. **Ask about learnings** - "What did we learn that should be added to context files?"

3. **Log to daily file** - Append to `~/.swarm/daily/YYYY-MM-DD.md`:
   - What got done
   - Any learnings (after user confirms)
   - Create the file if it doesn't exist

4. **Check for pending notifications** - If any tasks were completed, remind user who needs to be notified (check `## When done` section in task file)

5. **Suggest context updates** - If anything learned should be added to CLAUDE.md or context files, propose where it should go and wait for approval

6. **Clean up worktrees** - Check if there are stale worktrees from completed tasks:
   - Run `git worktree list` to see active worktrees
   - For each worktree, check if its PR was merged/closed
   - Offer to clean up worktrees for merged/closed PRs
