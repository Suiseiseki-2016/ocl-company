# ocl-company

A multi-agent company framework for OpenClaw. Clone this repo and your openclaw gains a full team: a leader who routes tasks, specialists who execute them, a board that reviews the results, and an experience system that makes everyone better over time.

---

## Quick Start

```bash
cargo build --release
cp target/release/ocl-company ~/.local/bin/   # or any directory on PATH

ocl-company init                              # scaffold team.yaml + agents/ + tasks/
ocl-company run --task "your task here"       # run a task through the full pipeline
```

---

## How the Team Works

Every task goes through a 7-step pipeline:

1. **Leader routes** — picks the best specialist, or hires a new one if no one fits
2. **Employee executes** — runs the task with their role definition, memory, and playbooks as context
3. **Board reviews** — each board member independently challenges the result
4. **Leader synthesizes** — produces the final result incorporating board feedback
5. **Memory written** — notable observations saved to the employee's memory
6. **Playbooks updated** — repeatable workflows captured or refined
7. **Skills noted** — capabilities worth automating flagged for openclaw

---

## Agent Files

Each agent lives in `agents/{id}/`:

```
agents/{id}/
├── soul.md        — values and character (who they are)
├── agents.md      — role definition, methodology, what they're NOT for
├── memory/        — timestamped experience notes (grows automatically)
├── playbooks/     — validated repeatable workflows (grows automatically)
└── skills/        — openclaw-native skills (flagged by board, built by you)
```

**soul.md** is injected into every invocation. **agents.md**, **memory/**, and **playbooks/** are loaded by the pipeline as task context. **skills/** are openclaw's domain — the agent calls them at runtime; we never inject them as text.

---

## Experience Ladder

The board upgrades employee knowledge after each task:

| Level | What | Where | Trigger |
|---|---|---|---|
| 1 | Observation | (ephemeral) | Every task |
| 2 | Memory | `memory/` | Board: `MEMORY: <note>` |
| 3 | Playbook | `playbooks/` | Board: `PLAYBOOK: <title> | <steps>` |
| 4 | Skill | `skills/` | Board: `NEW_SKILL: <description>` |

> Don't rush promotion. A working memory note beats a premature playbook. A working playbook beats a buggy skill.

---

## Autonomous Hiring

The leader can hire new employees mid-task. If no current employee fits, the leader responds:

```
HIRE: <name> | <description>
```

The pipeline creates the employee, seeds their files, updates `team.yaml`, and routes the task to them. You can also hire manually:

```bash
ocl-company hire "Data Analyst" --description "Statistical analysis, data visualisation"
ocl-company fire "Data Analyst"
```

---

## Running Tasks

```bash
# Inline task — result to stdout
ocl-company run --task "Summarise the key trends in LLM research this month"

# Task file — reads {id}.task.json, writes {id}.result.json
ocl-company run --task-file tasks/abc123.task.json

# Process all pending tasks in a project directory
ocl-company run --project /path/to/project
```

---

## Sharing Your Team

The `agents/` directory is your trained team. Commit it to share your employees with others. Clone a shared repo and run `ocl-company init` — you get their team.

Agent files you've accumulated:
- `soul.md` — their character (editable, ships with defaults)
- `agents.md` — their role definition (editable, ships with defaults)
- `memory/*.md` — what they've learned from past tasks
- `playbooks/*.md` — proven workflows they've built up
- `skills/*.md` — capabilities flagged for automation

---

## Commands

```bash
ocl-company init [--project <path>]
ocl-company run --task "..." | --task-file <path> | --project <path>
ocl-company status [--project <path>]
ocl-company hire "<name>" [--description "<desc>"] [--project <path>]
ocl-company fire "<name>" [--project <path>]
```

---

## Customising Your Team

Edit agent files directly — they're plain markdown:

```bash
# Sharpen an employee's standards
$EDITOR agents/researcher/soul.md

# Update their role definition or methodology
$EDITOR agents/researcher/agents.md

# Add a skill they should be able to invoke
$EDITOR agents/researcher/skills/academic-search.md
```

To add a new specialist the leader can route to:
```bash
ocl-company hire "Scout" --description "Data collection, platform scraping, systematic gathering"
$EDITOR agents/scout/soul.md      # write their character
$EDITOR agents/scout/agents.md    # write their methodology
```
