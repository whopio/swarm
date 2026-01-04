# Interview Task

Interview user in detail about the current task before implementation.

## Find the task

1. Check for `.swarm-task` file in current directory - it contains the task file path
2. If no `.swarm-task`, ask user which task to interview about

## Interview process

Read the task file, then ask probing questions using AskUserQuestion. Cover:

- **Technical implementation** - architecture, APIs, data flow, dependencies
- **UI/UX** - if applicable, user flows, edge cases, error states
- **Scope** - what's in/out, filtering criteria, edge cases
- **Integration** - how it connects to existing systems, what it replaces
- **Failure modes** - what happens when things go wrong
- **Tradeoffs** - performance vs simplicity, flexibility vs speed

**Question guidelines:**
- Ask non-obvious questions (not things easily answered by reading the task)
- Use AskUserQuestion with 2-4 options per question
- Ask 3-4 questions at a time, continue until all areas are covered
- Dig deeper when answers reveal complexity

## After interview

1. Write the complete spec to the task file under a `## Spec` section
2. Include an `### Implementation Plan` with numbered steps
3. Log the interview completion to `## Process Log`

## Output format

The spec should be detailed enough that Claude (or another developer) could implement it without asking more questions.
