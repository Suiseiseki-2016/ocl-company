use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{EmployeeConfig, TeamConfig};
use crate::memory::{seed_agent_files, seed_named_agent};
use crate::pipeline::{name_to_id, Pipeline};
use crate::storage::{scan_pending_tasks, write_result, ResultFile, TaskFile};

#[derive(Parser)]
#[command(name = "ocl-company")]
#[command(about = "OpenClaw Multi-Agent Framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a team.yaml + agents/ + tasks/ directory
    Init {
        /// Project directory (default: current dir)
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Run one or more tasks through the pipeline
    Run(RunArgs),
    /// List tasks and their status
    Status {
        /// Project directory
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Add an employee to team.yaml and seed their agent files
    Hire {
        /// Employee name
        name: String,
        /// Employee description
        #[arg(long, default_value = "Generalist agent")]
        description: String,
        /// Project directory
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Remove an employee from team.yaml
    Fire {
        /// Employee name
        name: String,
        /// Project directory
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Args)]
struct RunArgs {
    /// Inline task description; result printed to stdout
    #[arg(long)]
    task: Option<String>,
    /// Path to a {id}.task.json file; writes {id}.result.json alongside it
    #[arg(long)]
    task_file: Option<PathBuf>,
    /// Project directory; process all pending tasks in tasks/
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
    fs::create_dir_all(project).unwrap_or_else(|e| {
        eprintln!("Error creating project dir: {}", e);
        std::process::exit(1);
    });

    let team_yaml_path = project.join("team.yaml");
    if team_yaml_path.exists() {
        println!("team.yaml already exists — skipping scaffold.");
    } else {
        fs::write(&team_yaml_path, default_team_yaml()).unwrap_or_else(|e| {
            eprintln!("Error writing team.yaml: {}", e);
            std::process::exit(1);
        });
        println!("Created {}", team_yaml_path.display());
    }

    for dir_name in &["tasks", "agents"] {
        let dir = project.join(dir_name);
        fs::create_dir_all(&dir).unwrap_or_else(|e| {
            eprintln!("Error creating {}/: {}", dir_name, e);
            std::process::exit(1);
        });
        println!("Created {}/", dir.display());
    }

    let config = load_team(project);
    seed_team_agents(&config, project);
    println!(
        "Seeded agent files for {} employees + {} board members + leader.",
        config.employees.len(),
        config.board.len()
    );
    println!("Done. Edit team.yaml or agents/ to customise your team.");
}

fn seed_team_agents(config: &TeamConfig, project: &Path) {
    let leader_id = name_to_id(&config.leader.name);
    let _ = seed_named_agent(&leader_id, &config.leader.name, config.leader.soul.as_deref(), project);

    for member in &config.board {
        let id = name_to_id(&member.name);
        let _ = seed_named_agent(&id, &member.name, member.soul.as_deref(), project);
    }

    for emp in &config.employees {
        let id = name_to_id(&emp.name);
        let _ = seed_agent_files(&id, &emp.name, &emp.description, project);
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
        // Inline mode: load team.yaml from current dir, result → stdout
        let project = PathBuf::from(".");
        let config = load_team(&project);
        let pipeline = Pipeline::new(config, project);
        match pipeline.run_task(&task) {
            Ok(result) => println!("{}", result),
            Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
        }
    } else if let Some(task_file_path) = args.task_file {
        // Task-file mode: read {id}.task.json, write {id}.result.json alongside
        let content = fs::read_to_string(&task_file_path).unwrap_or_else(|e| {
            eprintln!("Error reading task file: {}", e);
            std::process::exit(1);
        });
        let task_file: TaskFile = serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Error parsing task file: {}", e);
            std::process::exit(1);
        });

        // team.yaml is two levels up: {project}/tasks/{id}.task.json → {project}/
        let project = task_file_path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."))
            .to_path_buf();

        let config = load_team(&project);
        let pipeline = Pipeline::new(config, project.clone());

        match pipeline.run_task(&task_file.description) {
            Ok(result) => {
                let summary = truncate_str(&result, 200).to_string();
                let result_obj = ResultFile {
                    id: task_file.id.clone(),
                    result,
                    summary,
                    finished_at: chrono::Utc::now(),
                };
                // Strip ".task" from stem of {id}.task.json to get {id}
                let stem = task_file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                let id = stem.trim_end_matches(".task");
                let result_path = task_file_path.parent().unwrap_or(Path::new("."))
                    .join(format!("{}.result.json", id));
                let json = serde_json::to_string_pretty(&result_obj).unwrap();
                fs::write(&result_path, json).unwrap_or_else(|e| eprintln!("Error writing result: {}", e));
                println!("Result written to {}", result_path.display());
            }
            Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
        }
    } else if let Some(project) = args.project {
        // Project mode: process all pending tasks in tasks/
        let config = load_team(&project);
        let pipeline = Pipeline::new(config, project.clone());
        let pending = scan_pending_tasks(&project);

        if pending.is_empty() {
            println!("No pending tasks in {}/tasks/", project.display());
            return;
        }

        println!("Processing {} pending task(s)...\n", pending.len());
        let (mut ok, mut err) = (0u32, 0u32);

        for task_file in &pending {
            println!("→ {} — {}", task_file.id, truncate_str(&task_file.description, 60));
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
                        Ok(()) => { println!("  ✓ Done"); ok += 1; }
                        Err(e) => { eprintln!("  ✗ Write error: {}", e); err += 1; }
                    }
                }
                Err(e) => { eprintln!("  ✗ Failed: {}", e); err += 1; }
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
        println!("No tasks/ directory in {}. Run 'ocl-company init' first.", project.display());
        return;
    }

    let mut rows: Vec<(String, String, bool)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !file_name.ends_with(".task.json") { continue; }
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
        println!("{:<36} {:<10} {}", id, if *done { "done" } else { "pending" }, truncate_str(desc, 40));
    }
    let (pending, done) = rows.iter().fold((0, 0), |(p, d), (_, _, is_done)| {
        if *is_done { (p, d + 1) } else { (p + 1, d) }
    });
    println!("\n{} pending, {} done", pending, done);
}

// ---------------------------------------------------------------------------
// hire / fire
// ---------------------------------------------------------------------------

fn cmd_hire(name: &str, description: &str, project: &Path) {
    let mut config = load_team(project);
    if config.employees.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
        eprintln!("Employee '{}' already exists.", name);
        return;
    }
    config.employees.push(EmployeeConfig { name: name.to_string(), description: description.to_string(), soul: None });
    save_team(project, &config);
    let _ = seed_agent_files(&name_to_id(name), name, description, project);
    println!("Hired: {} — {}", name, description);
    println!("Agent files: {}/agents/{}/", project.display(), name_to_id(name));
}

fn cmd_fire(name: &str, project: &Path) {
    let mut config = load_team(project);
    let before = config.employees.len();
    config.employees.retain(|e| !e.name.eq_ignore_ascii_case(name));
    if config.employees.len() == before {
        eprintln!("Employee '{}' not found.", name);
        return;
    }
    save_team(project, &config);
    println!("Fired: {} (agent files kept in agents/{}/)", name, name_to_id(name));
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn load_team(project: &Path) -> TeamConfig {
    let path = project.join("team.yaml");
    if !path.exists() {
        eprintln!("team.yaml not found in {}. Run 'ocl-company init' first.", project.display());
        std::process::exit(1);
    }
    let content = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Error reading team.yaml: {}", e);
        std::process::exit(1);
    });
    serde_yaml::from_str(&content).unwrap_or_else(|e| {
        eprintln!("Error parsing team.yaml: {}", e);
        std::process::exit(1);
    })
}

fn save_team(project: &Path, config: &TeamConfig) {
    let path = project.join("team.yaml");
    let content = serde_yaml::to_string(config).unwrap_or_else(|e| {
        eprintln!("Error serialising team.yaml: {}", e);
        std::process::exit(1);
    });
    fs::write(&path, content).unwrap_or_else(|e| {
        eprintln!("Error writing team.yaml: {}", e);
        std::process::exit(1);
    });
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

fn default_team_yaml() -> &'static str {
    r#"name: My Research Team

leader:
  name: Leader        # soul → agents/leader/soul.md

board:
  - name: Technical Reviewer
    specialty: "technical accuracy and completeness"   # soul → agents/technical-reviewer/soul.md
  - name: Editorial Reviewer
    specialty: "clarity and editorial quality"         # soul → agents/editorial-reviewer/soul.md

employees:
  - name: Researcher
    description: "Web research, source verification, fact-checking"
  - name: Developer
    description: "Software development, debugging, technical documentation"
  - name: Writer
    description: "Long-form writing, editing, content strategy"
"#
}
