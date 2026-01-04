# Log Progress

Save current progress to the task file so any Claude can continue later.

**Instructions (if provided):** $ARGUMENTS

## Steps:

1. **Find the task file:**
   - Check for `.swarm-task` in the repo root - it contains the exact path
   - If not found, ask user which task file to use

2. **Summarize current progress:**
   - What was discovered/explored
   - What was implemented (files modified, key changes)
   - Current status (what's working, what's not)
   - Next steps (what remains to be done)
   - Any blockers or decisions needed

3. **Append to the task file's `## Process Log` section:**
   - Use format: `### YYYY-MM-DD HH:MM - [Phase Name]`
   - Include enough detail that a fresh Claude session can pick up where this one left off
   - If user provided specific instructions via $ARGUMENTS, focus on those aspects

4. **Confirm what was saved** - Show user a summary of what was logged

**Important:** The goal is continuity - write as if briefing another Claude who has zero context about this session.
