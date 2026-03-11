use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::storage::get_project_agents_dir;

pub fn get_agent_dir(agent_id: &str, project_dir: &Path) -> PathBuf {
    get_project_agents_dir(project_dir).join(agent_id)
}

pub fn get_memory_dir(agent_id: &str, project_dir: &Path) -> PathBuf {
    get_agent_dir(agent_id, project_dir).join("memory")
}

pub fn get_skills_dir(agent_id: &str, project_dir: &Path) -> PathBuf {
    get_agent_dir(agent_id, project_dir).join("skills")
}

pub fn get_playbooks_dir(agent_id: &str, project_dir: &Path) -> PathBuf {
    get_agent_dir(agent_id, project_dir).join("playbooks")
}

/// Seed a regular employee's agent directory with all subdirs and default files.
/// soul_content and agents_md_content override defaults if provided.
pub fn seed_agent_files(
    agent_id: &str,
    name: &str,
    description: &str,
    soul_content: Option<&str>,
    agents_md_content: Option<&str>,
    project_dir: &Path,
) -> Result<(), String> {
    let dir = get_agent_dir(agent_id, project_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_skills_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_memory_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_playbooks_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;

    let default_soul = format!("# {name}\n\n{description}\n");
    write_if_missing(
        &dir.join("soul.md"),
        soul_content.unwrap_or(&default_soul),
    )?;

    let default_agents_md = format!(
        "# {name} — Role\n\n**Description:** {description}\n\n\
        ## Methodology\n\nApproach tasks systematically. Deliver clear, actionable results.\n"
    );
    write_if_missing(
        &dir.join("agents.md"),
        agents_md_content.unwrap_or(&default_agents_md),
    )?;

    Ok(())
}

/// Seed a leader or board member agent directory.
pub fn seed_named_agent(
    agent_id: &str,
    name: &str,
    soul_override: Option<&str>,
    agents_md_override: Option<&str>,
    project_dir: &Path,
) -> Result<(), String> {
    let dir = get_agent_dir(agent_id, project_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_skills_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_memory_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_playbooks_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;

    let default_soul = format!(
        "# {name}\n\nYou are {name}, an effective leader and synthesizer.\n"
    );
    write_if_missing(
        &dir.join("soul.md"),
        soul_override.unwrap_or(&default_soul),
    )?;

    let default_agents_md = format!(
        "# {name} — Role\n\nYou coordinate and synthesize. You own quality.\n"
    );
    write_if_missing(
        &dir.join("agents.md"),
        agents_md_override.unwrap_or(&default_agents_md),
    )?;

    Ok(())
}

/// Append a timestamped entry to the agent's memory/.
pub fn append_memory(agent_id: &str, entry: &str, project_dir: &Path) -> Result<(), String> {
    let memory_dir = get_memory_dir(agent_id, project_dir);
    fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    let filename = format!("{}.md", Utc::now().format("%Y-%m-%d-%H%M%S"));
    fs::write(memory_dir.join(filename), entry).map_err(|e| e.to_string())
}

/// Write (or overwrite) a playbook file. Playbooks improve over time, so overwrite is correct.
pub fn append_playbook(
    agent_id: &str,
    title: &str,
    content: &str,
    project_dir: &Path,
) -> Result<(), String> {
    let playbooks_dir = get_playbooks_dir(agent_id, project_dir);
    fs::create_dir_all(&playbooks_dir).map_err(|e| e.to_string())?;
    let path = playbooks_dir.join(format!("{}.md", slugify(title)));
    fs::write(&path, format!("# {}\n\n_CURRENT BEST PRACTICE — Subject to change_\n\n{}\n", title, content))
        .map_err(|e| e.to_string())
}

/// Append a new skill file. No-op if already exists (skills don't overwrite).
pub fn append_skill(agent_id: &str, skill_text: &str, project_dir: &Path) -> Result<(), String> {
    let skills_dir = get_skills_dir(agent_id, project_dir);
    fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;
    let path = skills_dir.join(format!("{}.md", slugify(skill_text)));
    if !path.exists() {
        fs::write(
            &path,
            format!("# {}\n\n{}\n", title_case(skill_text), skill_text),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Read agents.md (the JD) for an agent.
pub fn read_agents_md(agent_id: &str, project_dir: &Path) -> String {
    let path = get_agent_dir(agent_id, project_dir).join("agents.md");
    fs::read_to_string(&path).unwrap_or_default()
}

/// Read the most recent N memory entries for an agent (project-specific context).
pub fn read_recent_memory(agent_id: &str, project_dir: &Path) -> String {
    read_recent_md_files(&get_memory_dir(agent_id, project_dir), 5)
}

/// Read all playbooks for an agent.
pub fn read_playbooks(agent_id: &str, project_dir: &Path) -> String {
    read_recent_md_files(&get_playbooks_dir(agent_id, project_dir), 20)
}

fn read_recent_md_files(dir: &Path, limit: usize) -> String {
    if !dir.exists() {
        return String::new();
    }
    let mut entries: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries
        .iter()
        .rev()
        .take(limit)
        .rev()
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[allow(dead_code)]
pub fn write_skill_file(
    agent_id: &str,
    filename: &str,
    content: &str,
    project_dir: &Path,
) -> Result<(), String> {
    let skills_dir = get_skills_dir(agent_id, project_dir);
    fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;
    write_if_missing(&skills_dir.join(filename), content)
}

fn write_if_missing(path: &PathBuf, content: &str) -> Result<(), String> {
    if !path.exists() {
        fs::write(path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(60)
        .collect()
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
