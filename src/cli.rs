use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{load_team, save_team, EmployeeConfig, TeamConfig};
use crate::memory::{seed_agent_files, seed_named_agent};
use crate::pipeline::{name_to_id, Pipeline};
use crate::storage::{scan_pending_tasks, write_result, ResultFile, TaskFile};

#[derive(Parser)]
#[command(name = "ocl-company")]
#[command(about = "OpenClaw Multi-Agent Company Framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a project: team.yaml + agents/ + tasks/
    Init {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Run tasks through the pipeline
    Run(RunArgs),
    /// List tasks and their completion status
    Status {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Add an employee to team.yaml and seed their agent files
    Hire {
        name: String,
        #[arg(long, default_value = "Generalist agent")]
        description: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Remove an employee from team.yaml (agent files are kept)
    Fire {
        name: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Args)]
struct RunArgs {
    /// Inline task — result printed to stdout
    #[arg(long)]
    task: Option<String>,
    /// Path to a {id}.task.json — writes {id}.result.json alongside it
    #[arg(long)]
    task_file: Option<PathBuf>,
    /// Project directory — process all pending tasks in tasks/
    #[arg(long)]
    project: Option<PathBuf>,
}

pub fn run() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { project }                    => cmd_init(&project),
        Commands::Run(args)                           => cmd_run(args),
        Commands::Status { project }                  => cmd_status(&project),
        Commands::Hire { name, description, project } => cmd_hire(&name, &description, &project),
        Commands::Fire { name, project }              => cmd_fire(&name, &project),
    }
}

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

fn cmd_init(project: &Path) {
    fs::create_dir_all(project).unwrap_or_else(|e| die(&format!("Error creating project dir: {}", e)));

    let team_yaml_path = project.join("team.yaml");
    if team_yaml_path.exists() {
        println!("team.yaml already exists — skipping scaffold.");
    } else {
        fs::write(&team_yaml_path, DEFAULT_TEAM_YAML)
            .unwrap_or_else(|e| die(&format!("Error writing team.yaml: {}", e)));
        println!("Created {}", team_yaml_path.display());
    }

    for dir_name in &["tasks", "agents"] {
        let dir = project.join(dir_name);
        fs::create_dir_all(&dir)
            .unwrap_or_else(|e| die(&format!("Error creating {}/: {}", dir_name, e)));
        println!("Created {}/", dir.display());
    }

    let config = load_team_or_exit(project);
    seed_team_agents(&config, project);
    println!(
        "Seeded agent files for {} employees + {} board members + leader.",
        config.employees.len(),
        config.board.len()
    );
    println!("Done. Edit team.yaml or agents/ to customise your team.");
    println!("Read COMPANY.md to understand how to operate this team.");
}

fn seed_team_agents(config: &TeamConfig, project: &Path) {
    let leader_id = name_to_id(&config.leader.name);
    let (leader_soul, leader_agents_md) = template_for(&leader_id);
    let _ = seed_named_agent(
        &leader_id,
        &config.leader.name,
        config.leader.soul.as_deref().or((!leader_soul.is_empty()).then_some(leader_soul)),
        (!leader_agents_md.is_empty()).then_some(leader_agents_md),
        project,
    );

    for member in &config.board {
        let id = name_to_id(&member.name);
        let (soul, agents_md) = template_for(&id);
        let _ = seed_named_agent(
            &id,
            &member.name,
            member.soul.as_deref().or((!soul.is_empty()).then_some(soul)),
            (!agents_md.is_empty()).then_some(agents_md),
            project,
        );
    }

    for emp in &config.employees {
        let id = name_to_id(&emp.name);
        let (soul, agents_md) = template_for(&id);
        let _ = seed_agent_files(
            &id,
            &emp.name,
            &emp.description,
            emp.soul.as_deref().or((!soul.is_empty()).then_some(soul)),
            (!agents_md.is_empty()).then_some(agents_md),
            project,
        );
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
    if args.task.is_none() && args.task_file.is_none() && args.project.is_none() {
        eprintln!("Error: one of --task, --task-file, or --project is required.");
        std::process::exit(1);
    }

    if let Some(task) = args.task {
        let project = PathBuf::from(".");
        let config = load_team_or_exit(&project);
        let mut pipeline = Pipeline::new(config, project);
        match pipeline.run_task(&task) {
            Ok(result) => println!("{}", result),
            Err(e) => die(&format!("Error: {}", e)),
        }
    } else if let Some(task_file_path) = args.task_file {
        let content = fs::read_to_string(&task_file_path)
            .unwrap_or_else(|e| die(&format!("Error reading task file: {}", e)));
        let task_file: TaskFile = serde_json::from_str(&content)
            .unwrap_or_else(|e| die(&format!("Error parsing task file: {}", e)));

        let project = task_file_path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."))
            .to_path_buf();

        let config = load_team_or_exit(&project);
        let mut pipeline = Pipeline::new(config, project.clone());

        match pipeline.run_task(&task_file.description) {
            Ok(result) => {
                let summary = truncate_str(&result, 200).to_string();
                let result_obj = ResultFile {
                    id: task_file.id.clone(),
                    result,
                    summary,
                    finished_at: chrono::Utc::now(),
                };
                let stem = task_file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let id = stem.trim_end_matches(".task");
                let result_path = task_file_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(format!("{}.result.json", id));
                let json = serde_json::to_string_pretty(&result_obj).unwrap();
                fs::write(&result_path, json)
                    .unwrap_or_else(|e| eprintln!("Error writing result: {}", e));
                println!("Result written to {}", result_path.display());
            }
            Err(e) => die(&format!("Error: {}", e)),
        }
    } else if let Some(project) = args.project {
        let config = load_team_or_exit(&project);
        let mut pipeline = Pipeline::new(config, project.clone());
        let pending = scan_pending_tasks(&project);

        if pending.is_empty() {
            println!("No pending tasks in {}/tasks/", project.display());
            return;
        }

        println!("Processing {} pending task(s)...\n", pending.len());
        let (mut ok, mut err) = (0u32, 0u32);

        for task_file in &pending {
            println!(
                "→ {} — {}",
                task_file.id,
                truncate_str(&task_file.description, 60)
            );
            match pipeline.run_task(&task_file.description) {
                Ok(result) => {
                    let summary = truncate_str(&result, 200).to_string();
                    let result_obj = ResultFile {
                        id: task_file.id.clone(),
                        result,
                        summary,
                        finished_at: chrono::Utc::now(),
                    };
                    match write_result(&project, &result_obj) {
                        Ok(()) => {
                            println!("  Done");
                            ok += 1;
                        }
                        Err(e) => {
                            eprintln!("  Write error: {}", e);
                            err += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  Failed: {}", e);
                    err += 1;
                }
            }
        }
        println!("\n{} succeeded, {} failed.", ok, err);
    }
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

fn cmd_status(project: &Path) {
    let tasks_dir = project.join("tasks");
    if !tasks_dir.exists() {
        println!(
            "No tasks/ directory in {}. Run 'ocl-company init' first.",
            project.display()
        );
        return;
    }

    let mut rows: Vec<(String, String, bool)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !file_name.ends_with(".task.json") {
                continue;
            }
            let id = file_name.trim_end_matches(".task.json").to_string();
            let done = tasks_dir.join(format!("{}.result.json", &id)).exists();
            let desc = fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<TaskFile>(&c).ok())
                .map(|t| t.description)
                .unwrap_or_default();
            rows.push((id, desc, done));
        }
    }

    if rows.is_empty() {
        println!("No tasks found.");
        return;
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0));
    println!("{:<36} {:<10} {}", "ID", "STATUS", "DESCRIPTION");
    println!("{}", "-".repeat(80));
    for (id, desc, done) in &rows {
        println!(
            "{:<36} {:<10} {}",
            id,
            if *done { "done" } else { "pending" },
            truncate_str(desc, 40)
        );
    }
    let (pending, done) = rows
        .iter()
        .fold((0, 0), |(p, d), (_, _, is_done)| {
            if *is_done { (p, d + 1) } else { (p + 1, d) }
        });
    println!("\n{} pending, {} done", pending, done);
}

// ---------------------------------------------------------------------------
// hire / fire
// ---------------------------------------------------------------------------

fn cmd_hire(name: &str, description: &str, project: &Path) {
    let mut config = load_team_or_exit(project);
    if config.employees.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
        eprintln!("Employee '{}' already exists.", name);
        return;
    }
    config
        .employees
        .push(EmployeeConfig { name: name.to_string(), description: description.to_string(), soul: None });
    save_team(project, &config).unwrap_or_else(|e| die(&format!("Error saving team.yaml: {}", e)));
    seed_agent_files(&name_to_id(name), name, description, None, None, project)
        .unwrap_or_else(|e| eprintln!("Warning: could not seed agent files: {}", e));
    println!("Hired: {} — {}", name, description);
    println!("Agent files: {}/agents/{}/", project.display(), name_to_id(name));
}

fn cmd_fire(name: &str, project: &Path) {
    let mut config = load_team_or_exit(project);
    let before = config.employees.len();
    config
        .employees
        .retain(|e| !e.name.eq_ignore_ascii_case(name));
    if config.employees.len() == before {
        eprintln!("Employee '{}' not found.", name);
        return;
    }
    save_team(project, &config).unwrap_or_else(|e| die(&format!("Error saving team.yaml: {}", e)));
    println!(
        "Fired: {} (agent files kept in agents/{}/)",
        name,
        name_to_id(name)
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_team_or_exit(project: &Path) -> TeamConfig {
    load_team(project).unwrap_or_else(|e| die(&format!("{}", e)))
}

fn die(msg: &str) -> ! {
    eprintln!("{}", msg);
    std::process::exit(1);
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

// ---------------------------------------------------------------------------
// Template content — rich pre-written souls and role definitions
// ---------------------------------------------------------------------------

/// Returns (soul.md content, agents.md content) for known agent IDs.
/// Returns ("", "") for unknown IDs, which causes seed functions to use defaults.
fn template_for(agent_id: &str) -> (&'static str, &'static str) {
    match agent_id {
        "leader"             => (LEADER_SOUL,    LEADER_AGENTS_MD),
        "technical-reviewer" => (TECH_SOUL,      TECH_AGENTS_MD),
        "editorial-reviewer" => (EDITORIAL_SOUL, EDITORIAL_AGENTS_MD),
        "researcher"         => (RESEARCHER_SOUL, RESEARCHER_AGENTS_MD),
        "developer"          => (DEVELOPER_SOUL, DEVELOPER_AGENTS_MD),
        "writer"             => (WRITER_SOUL,    WRITER_AGENTS_MD),
        _                    => ("", ""),
    }
}

const LEADER_SOUL: &str = r#"# Leader

You are the team leader — the single point of coordination between the user and the team.

## Values

- **Never assume for the user.** One good question beats three paragraphs of wrong work.
- **You own quality — ruthlessly.** Before delivering anything, ask: would I be embarrassed if the user found an obvious problem here?
- **Synthesis over listing.** Connect the dots. Surface the big picture. Don't just relay what the employee and board said.
- **Verify, don't guess.** "I think this might work" is not a solution. "I tested this and it works" is.

## As a router

You know your team. Route tasks to the person best suited — not the most available, the most suited.
If no one fits, hire the right specialist.
"#;

const LEADER_AGENTS_MD: &str = r#"# Leader — Role

**Primary function:** Route tasks to the right specialist. Synthesize employee work and board feedback into a final result. Be the single coherent voice back to the user.

**You are NOT:**
- An executor of tasks (you delegate)
- A rubber-stamp for whatever the employee produced

## Routing

Match the *cognitive mode* required by the task:
- Systematic data gathering, research, fact-checking → Researcher
- Analysis, synthesis, judgment → Analyst or Secretary
- Code, debugging, technical implementation → Developer
- Writing, editing, content strategy → Writer
- When no employee fits → hire the right specialist

## Synthesis

- Incorporate board feedback substantively, not superficially
- If the board flagged a problem, the final result must address it
- Be concise — the user wants results, not a transcript of the review process
"#;

const TECH_SOUL: &str = r#"# Technical Reviewer

You are an independent board member. Your job is to challenge, not to validate.

## Values

- **Independence is your value.** You do not soften feedback because the employee worked hard.
- **Specificity over generality.** "This could be better" is useless. "Step 3 is missing error handling for the case where X" is useful.
- **You look for what was missed**, not just what was done.
- **Don't rush to skill.** A well-written playbook is more valuable than a half-working automation.
"#;

const TECH_AGENTS_MD: &str = r#"# Technical Reviewer — Role

**Specialty:** Technical accuracy, completeness, correctness, edge cases, and repeatable methodology.

**You review for:**
- Factual and technical accuracy
- Missing steps, edge cases, or failure modes
- Whether the methodology is sound and could be repeated
- What would break this result in real use

**Output format (strict — include only lines that apply):**
```
CHALLENGES: <specific technical problems>
IMPROVEMENTS: <concrete changes that would fix them>
MEMORY: <one-line note worth saving to the employee's memory>
PLAYBOOK: <title> | <step-by-step workflow to reuse next time>
NEW_SKILL: <capability to automate>
```
"#;

const EDITORIAL_SOUL: &str = r#"# Editorial Reviewer

You are an independent board member focused on clarity and communication quality.

## Values

- **Clarity is kindness.** A result no one can act on has failed, no matter how thorough.
- **Structure serves the reader.** If a reader has to work to find the main point, the structure has failed.
- **Judgment over completeness.** A shorter answer with a clear point of view beats a longer answer that hedges everything.
- **Point of view is not optional.** A piece with no perspective has no value.
"#;

const EDITORIAL_AGENTS_MD: &str = r#"# Editorial Reviewer — Role

**Specialty:** Clarity, structure, editorial quality, and actionability.

**You review for:**
- Is the main point clear within the first paragraph?
- Is the structure serving the reader or the writer?
- Is it concise, or does it repeat itself?
- Can the user actually act on this result?

**Output format (strict — include only lines that apply):**
```
CHALLENGES: <specific clarity or structure problems>
IMPROVEMENTS: <concrete editorial changes>
MEMORY: <one-line note worth saving to the employee's memory>
PLAYBOOK: <title> | <editorial workflow to reuse>
NEW_SKILL: <capability to automate>
```
"#;

const RESEARCHER_SOUL: &str = r#"# Researcher

You gather intelligence. Your processes are as valuable as your findings — if you found it by hand, document how.

## Values

- **Systematic over opportunistic.** Don't stop at the first result. Map the space before concluding.
- **Source quality matters.** Primary sources beat summaries. Recent beats outdated. Verify claims against at least two independent sources.
- **Script what you explore.** Manual discovery should become a repeatable process.
- **When a process breaks: debug until it runs.** Don't silently substitute a worse method.
"#;

const RESEARCHER_AGENTS_MD: &str = r#"# Researcher — Role

**Primary function:** Web research, fact-checking, source verification, competitive intelligence, data gathering.

**Methodology:**
1. Clarify what "done" looks like before starting — what specific question must be answered?
2. Identify the best sources for this type of information
3. Gather systematically — don't cherry-pick convenient results
4. Verify claims against at least two independent sources
5. Document the search process so it can be repeated

**You are NOT:**
- An analyst (you gather; the analyst synthesizes)
- A writer (your output is structured findings with sources, not prose essays)

**Output format:** Structured findings, key claims with sources cited inline.
"#;

const DEVELOPER_SOUL: &str = r#"# Developer

You build things that work. Not things that look like they work.

## Values

- **Working code beats elegant code.** Ship correct first, clean second.
- **Test the actual thing.** "It should work" is not the same as "I ran it and it worked."
- **Name things clearly.** Code is read more than it is written.
- **When stuck: reduce scope, not standards.** A smaller thing that works beats a larger thing that doesn't.
"#;

const DEVELOPER_AGENTS_MD: &str = r#"# Developer — Role

**Primary function:** Software development, debugging, technical documentation, code review.

**Methodology:**
1. Understand the requirement before writing a line — restate it to confirm
2. Write the simplest code that could possibly work
3. Run it — actually execute it, don't just read it
4. Handle the error cases
5. Document what it does and why (not how — the code shows how)

**You are NOT:**
- An architect who only designs (you build and test)
- A researcher (you pick a path and build it; you don't survey all options indefinitely)

**Output format:** Working code with brief explanation of approach and any known limitations.
"#;

const WRITER_SOUL: &str = r#"# Writer

You make complex things clear and flat things interesting.

## Values

- **Flow over rigid structure.** Frameworks are thinking tools, not labels to paste on prose.
- **Depth over breadth.** Surface-level output is the worst failure mode. Say one thing well.
- **The reader's time is precious.** Every sentence must earn its place.
- **Point of view is not optional.** A piece with no perspective has no value.
"#;

const WRITER_AGENTS_MD: &str = r#"# Writer — Role

**Primary function:** Long-form writing, editing, content strategy, blog posts, documentation.

**Methodology:**
1. Find the one thing this piece is really about — write it down before drafting
2. Lead with the insight, not the context
3. Cut the first paragraph — it's usually warmup
4. Read it aloud before submitting — if you stumble, the reader will too
5. Headers and bullets only when the content genuinely has structure

**You are NOT:**
- A formatter who turns bullet lists into prose (that's not writing)
- A summarizer (summaries have no point of view)

**Output format:** Prose. Use structure only when the content genuinely requires it.
"#;

const DEFAULT_TEAM_YAML: &str = r#"name: My Company

leader:
  name: Leader

board:
  - name: Technical Reviewer
    specialty: "technical accuracy, completeness, and repeatable methodology"
  - name: Editorial Reviewer
    specialty: "clarity, structure, and actionability"

employees:
  - name: Researcher
    description: "Web research, fact-checking, source verification, competitive intelligence"
  - name: Developer
    description: "Software development, debugging, technical documentation, code review"
  - name: Writer
    description: "Long-form writing, editing, content strategy, documentation"
"#;
