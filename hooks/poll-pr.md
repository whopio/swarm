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

7. **Update PR title & description** - After all fixes are pushed:
   - Read current title/body: `gh pr view <number> --json title,body`
   - Update title if scope changed
   - Update body to reflect ALL changes made during this session

8. **Report status** - Tell user:
   - What checks passed/failed
   - What fixes were made (if any)
   - Whether the PR is ready for human review

## Example Commands

```bash
# Check CI status
gh pr checks 123

# View ALL comments (with pagination!)
gh api repos/OWNER/REPO/pulls/123/comments --paginate --jq '.[] | "\(.user.login): \(.body[0:100])..."'

# Count total comments
gh api repos/OWNER/REPO/pulls/123/comments --paginate --jq 'length'

# View general PR comments
gh pr view 123 --comments
```
