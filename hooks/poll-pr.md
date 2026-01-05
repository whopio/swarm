# Poll PR Until Ready

Monitor the current PR until CI is green and AI reviewers are satisfied.

## Instructions

1. **Detect PR number** - Use `gh pr view --json number` or ask for the PR number

2. **Poll PR checks** - Run `gh pr checks <number>` to see CI status

3. **Check for AI reviewer comments** - Look for feedback from:
   - **Cursor (Bug Bot)** - finds bugs and issues
   - **Sentry** - code review feedback

   **IMPORTANT:** Use `--paginate` to get ALL comments (GitHub defaults to 30):
   ```bash
   gh api repos/<owner>/<repo>/pulls/<number>/comments --paginate
   ```

4. **Review suggestions** - For each AI comment:
   - Read the suggestion carefully
   - If it's accurate and actionable, implement the fix
   - If it's a false positive, ignore it
   - Track status: ✅ Fixed, ⚪ Design choice, ❌ False positive

5. **Push fixes** - If changes were made:
   - Commit with a descriptive message like `fix: address cursor/sentry feedback`
   - Push to the branch

6. **Repeat** - Continue polling until:
   - All CI checks pass (green)
   - No new actionable AI feedback
   - **IMPORTANT:** Check for NEW AI comments after each push - they review new commits!

7. **Update PR title & description** - After all fixes are pushed:
   - Read current title/body: `gh pr view <number> --json title,body`
   - Update title if scope changed
   - Update body to reflect ALL changes made during this session

8. **Report final status** - ALWAYS show:

   **AI Reviewer Feedback Summary:**
   | Issue | Reviewer | Status | Action |
   |-------|----------|--------|--------|
   | Error handling missing | Sentry | ✅ Fixed | Added set -e |
   | Unused variable | Cursor | ❌ False positive | N/A |
   | Design concern | Cursor | ⚪ Design choice | Intentional |

   **CI Status:**
   - List all checks and their status
   - Highlight any that are still pending/failing

   **Ready for review:** Yes/No

## Example Commands

```bash
# Check CI status
gh pr checks 123

# View ALL comments (with pagination!)
gh api repos/OWNER/REPO/pulls/123/comments --paginate --jq '.[] | "\(.user.login): \(.body[0:100])..."'

# Filter to just AI reviewers
gh api repos/OWNER/REPO/pulls/123/comments --paginate --jq '.[] | select(.user.login | test("cursor|sentry")) | "\(.user.login): \(.body[0:200])..."'

# Count total comments
gh api repos/OWNER/REPO/pulls/123/comments --paginate --jq 'length'

# Check for comments after a specific time (to find new ones after pushing)
gh api repos/OWNER/REPO/pulls/123/comments --paginate --jq '.[] | select(.created_at > "2024-01-01T00:00:00Z") | "\(.user.login): \(.body[0:100])..."'

# View general PR comments
gh pr view 123 --comments
```
