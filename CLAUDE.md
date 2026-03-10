# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build              # Debug build
cargo build --release    # Optimized release build (size-optimized with LTO)
cargo run -- <command>   # Run with arguments
cargo test               # Run tests
```

### CLI commands (after building):
```bash
./ocl-company init
./ocl-company assign "Search for AI news"
./ocl-company org
./ocl-company tasks
./ocl-company review <task_id> <true|false> "<feedback>"
./ocl-company hire "<name>" <role>
./ocl-company fire "<name>"
./ocl-company meeting
./ocl-company stats
```

## Architecture

Single-file Rust application (`src/main.rs`, ~756 lines) with no submodules. All logic lives in one file organized into these conceptual sections:

### Data Model
- `EmployeeRole` enum — roles from CEO down to BoardMember, each with name, department, and salary level
- `Employee` struct — ID, role, tasks completed, performance score (1–10, starts at 5.0)
- `Task` struct — ID, description, assigned employee, status, result, timestamp
- `Company` struct — employees list, tasks list, board meeting minutes
- `TaskStatus` progression: `Pending → InProgress → Review → Completed/Rejected`

### Task Assignment (`assign_task`)
Keywords in the task description (English and Chinese) determine which role handles it:
- Scout → research/search keywords
- Analyst → analysis/report keywords
- Developer → code/programming keywords
- Writer → content/writing keywords
- CEO → fallback default

Within the matched role, the highest-performing available employee is selected.

### OpenClaw Integration (`spawn_employee`)
After assignment, the system builds a role-specific prompt (`build_employee_prompt`) and attempts to invoke:
1. `openclaw agent --agent main --message <prompt> --deliver`
2. Falls back to `ocl agent --message <prompt> --deliver`

This is the integration point with the external OpenClaw agent execution system.

### Persistence
Company state is serialized as JSON to a platform-specific directory:
- Uses `directories` crate (`ProjectDirs`) → `~/.local/share/com/openclaw/ocl-company/company.json` on Linux
- Falls back to `.ocl-company/company.json` if `ProjectDirs` fails
- State is loaded at startup and saved after every mutating command

### Default Company (`create_default`)
Running `init` creates 13 employees: Alice (CEO), Bob (CTO), Carol (CFO), David (Director Research), Eve (Director Analysis), Frank (Director Engineering), Grace (Scout), Henry (Analyst), Ivy (Developer), Jack (Writer), plus 3 Board Members.
