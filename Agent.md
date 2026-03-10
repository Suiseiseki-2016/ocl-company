# Using ocl-company

When you face a task that is complex, multi-faceted, or benefits from specialist
knowledge and independent review — delegate it to your team via `ocl-company`.

---

## When to Use This

Use `ocl-company` instead of handling a task yourself when:

- The task requires deep specialist knowledge (research, coding, writing)
- The result needs to be challenged and improved, not just produced
- The task is long-running and benefits from structured review
- You want the result and the reasoning behind it to be saved for future use

---

## Run a Task

```bash
# Delegate a task — your team handles it end-to-end, result on stdout
ocl-company run --task "your task description here"
```

The team will:
1. Route the task to the best specialist
2. Have that specialist execute it
3. Have the board challenge and improve the result
4. Synthesize everything into a final answer

You receive the final result. Use it as your own output or build on it.

---

## Enhance the Team

If a task exposed a gap in your team's capabilities, fix it immediately.

**Add a new specialist:**
```bash
ocl-company hire "Data Analyst" --description "Statistical analysis, data visualisation, quantitative reasoning"
```

**Edit a specialist's identity or skills directly** — these are plain files:
```
agents/{role}/soul.md        ← rewrite to sharpen identity or focus
agents/{role}/skills/{name}.md  ← edit to deepen a specific capability
```

For example, if the Researcher produced shallow results, open
`agents/researcher/soul.md` and make its standards more demanding.
Or add a new skill file:
```
agents/researcher/skills/academic-search.md
```

**Remove a role that is no longer needed:**
```bash
ocl-company fire "Data Analyst"
```

---

## Review Past Work

```bash
ocl-company status          # see all tasks and their completion state
```

Each completed task leaves a memory entry at:
```
agents/{role}/memory/{date}.md
```

Read these to understand how the team has handled similar problems before.

---

## Check Team Composition

```bash
cat team.yaml               # who is on the team and what they do
ls agents/                  # all agent directories
```

If the current team cannot handle a task well, hire the right specialist first,
then delegate.

---

## Key Principle

You are the orchestrator. `ocl-company` is your team.
For routine or simple tasks, act directly.
For complex tasks that require expertise, review, and lasting knowledge — delegate.
