# ocl-company

**Multi-agent pipelines for OpenClaw.**

`ocl-company` lets you define a team of specialized agents in a single YAML file
and run any task through a structured pipeline — routing, execution, board review,
and synthesis — all powered by OpenClaw agents.

---

## What This Gives OpenClaw

OpenClaw agents are powerful individually. `ocl-company` adds **structured
collaboration**: multiple agents working on the same task in defined roles, with
their knowledge accumulating over time.

| Without ocl-company | With ocl-company |
|---|---|
| One agent handles everything | Tasks are routed to the right specialist |
| No review step | Every result is challenged by a board |
| Context lost after each run | Memory and skills grow with every task |
| Agent identity lives in one place | Team is a repo — shareable, version-controlled |

The core contribution is three things:

**1. Team as code.**
Your team is defined in `team.yaml` and lives in a git repo. Clone the repo,
run `ocl-company init`, and the full team is ready — soul, skills, memory structure
and all. Share a trained team by pushing to git.

**2. A pipeline that improves results.**
Every task goes through leader routing → employee execution → independent board
review → leader synthesis. The board can't approve and move on — it must challenge,
propose improvements, and suggest new skills. The final result incorporates all of that.

**3. Agents that learn.**
After every task, a memory entry is written to `agents/{role}/memory/`.
When the board suggests a new capability, a skill file is added to `agents/{role}/skills/`.
These files are loaded as context the next time that agent is invoked.
The longer you run the team, the better it gets.

---

## Installation

**Requirements:** Rust toolchain, OpenClaw CLI (`openclaw` or `ocl` on PATH).

```bash
cargo install --git https://github.com/openclaw/ocl-company
```

Or build from source:
```bash
git clone https://github.com/openclaw/ocl-company
cd ocl-company
cargo build --release
cp target/release/ocl-company ~/.local/bin/
```

---

## Quick Start

```bash
# Create a new team project
mkdir my-team && cd my-team
ocl-company init

# The default team is ready — edit to customise
$EDITOR team.yaml
$EDITOR agents/researcher/soul.md

# Run a task — result printed to stdout
ocl-company run --task "Compare the top Rust async runtimes"
```

---

## Team Configuration

Everything about your team lives in `team.yaml`:

```yaml
name: My Research Team

leader:
  name: Alex                      # leader agent — routes tasks and synthesizes results
  soul: |                         # optional inline override of agents/alex/soul.md
    You are Alex ...

board:
  - name: Technical Reviewer      # board agents — review every result
    specialty: "technical accuracy and completeness"
  - name: Editorial Reviewer
    specialty: "clarity and editorial quality"

employees:
  - name: Researcher              # worker agents — execute the actual task
    description: "Web research, source verification, fact-checking"
  - name: Developer
    description: "Software development, debugging, technical documentation"
  - name: Writer
    description: "Long-form writing, editing, content strategy"
```

The `name` field is the agent identifier. It maps directly to the agent directory:
`name_to_id("Technical Reviewer")` → `agents/technical-reviewer/`.

---

## Agent Files

Each agent has a directory under `agents/`:

```
agents/researcher/
  soul.md          ← who this agent is (identity, personality, working style)
  skills/
    web-research.md        ← a specific capability
    fact-checking.md
    source-verification.md
    ...                    ← grows as board suggests new skills
  memory/
    2026-03-10-143022.md   ← record of one completed task
    2026-03-11-091455.md
    ...                    ← grows after every task run
```

**soul.md** is static — you write it once and maintain it by hand.
**skills/** grows when the board suggests `NEW_SKILL` in a review.
**memory/** grows automatically after every task.

Because all of these are plain files in the repo, you can:
- Commit them to share a trained team across machines and collaborators
- Edit any file to correct or improve an agent's knowledge
- Delete old memory entries to keep context lean
- Add skill files manually to bootstrap a new employee faster

---

## The Pipeline

When you run a task, seven things happen in sequence:

```
1. Leader   reads the task and the employee roster
            → replies with the name of the best-suited employee

2. Employee receives their soul (from openclaw) + skills (from openclaw)
            + recent memory (injected by ocl-company) + the task
            → produces a result

3. Board    each member independently receives the task and the employee result
            → replies with CHALLENGES, IMPROVEMENTS, and optionally NEW_SKILL

4. Leader   receives the original task, the employee result, and all board feedback
            → produces the final synthesized result

5. Memory   a dated summary is written to agents/{employee}/memory/

6. Skills   each NEW_SKILL suggestion from the board becomes a new file
            in agents/{employee}/skills/

7. Output   the final result is returned to the caller
```

The soul and skills of each named agent are loaded by OpenClaw when it invokes
`openclaw --agent <name> --agents-dir ./agents`. OpenClaw manages agent context.
`ocl-company` manages orchestration, memory, and skill accumulation.

---

## Integration with OpenClaw

### As a tool call (simplest)

Add `ocl-company` as a tool in your OpenClaw agent config. When your agent
needs to delegate a complex task to a team, it calls:

```bash
ocl-company run --task "{input}"
```

The final result arrives on stdout.

### File-based handoff (recommended)

Your OpenClaw agent writes a task file, triggers the pipeline, reads the result:

```bash
# Agent writes:
echo '{"id":"abc","description":"...","created_at":"..."}' > tasks/abc.task.json

# Agent triggers:
ocl-company run --task-file tasks/abc.task.json

# Agent reads:
cat tasks/abc.result.json
```

This keeps large payloads out of argv and produces a clean audit trail.

### Batch / background worker

Drop task files into `tasks/` from any source, then process them all:

```bash
ocl-company run --project /path/to/my-team
```

Run this on a schedule or in a loop to process tasks as they arrive.

---

## Team Management

```bash
# Add a new employee — creates agents/{slug}/ with default files
ocl-company hire "Data Analyst" --description "Statistical analysis, data visualisation"

# Remove from team.yaml (agent files are kept — knowledge is preserved)
ocl-company fire "Data Analyst"

# Show task status
ocl-company status
ocl-company status --project /path/to/my-team
```

---

## Project Layout

```
my-team/
  team.yaml          ← team definition — the only file you must edit
  openclaw.toml      ← tool manifest for openclaw installer
  agents/            ← agent files — commit these to share your trained team
    alex/
    technical-reviewer/
    editorial-reviewer/
    researcher/
    developer/
    writer/
  tasks/             ← task handoff directory
    {id}.task.json   ←   written by openclaw
    {id}.result.json ←   written by ocl-company
```

---

## License

MIT
