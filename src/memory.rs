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

/// Seed agent directory: soul.md, skills/, memory/.
pub fn seed_agent_files(
    agent_id: &str,
    name: &str,
    description: &str,
    project_dir: &Path,
) -> Result<(), String> {
    let dir = get_agent_dir(agent_id, project_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_skills_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_memory_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    write_if_missing(&dir.join("soul.md"), &default_soul(name, description))?;
    Ok(())
}

/// Seed minimal agent directory for a leader or board member.
pub fn seed_named_agent(
    agent_id: &str,
    name: &str,
    soul_override: Option<&str>,
    project_dir: &Path,
) -> Result<(), String> {
    let dir = get_agent_dir(agent_id, project_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_skills_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(get_memory_dir(agent_id, project_dir)).map_err(|e| e.to_string())?;
    let default = format!("# {name}\n\nYou are {name}, an effective leader and synthesizer.\n");
    write_if_missing(&dir.join("soul.md"), soul_override.unwrap_or(&default))?;
    Ok(())
}

/// Write a new dated entry into the agent's memory/ directory.
pub fn append_memory(agent_id: &str, entry: &str, project_dir: &Path) -> Result<(), String> {
    let memory_dir = get_memory_dir(agent_id, project_dir);
    fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    let filename = format!("{}.md", Utc::now().format("%Y-%m-%d-%H%M%S"));
    fs::write(memory_dir.join(filename), entry).map_err(|e| e.to_string())
}

/// Write a new skill file into the agent's skills/ directory.
/// Filename is derived from the skill text (slugified). No-op if already exists.
pub fn append_skill(agent_id: &str, skill_text: &str, project_dir: &Path) -> Result<(), String> {
    let skills_dir = get_skills_dir(agent_id, project_dir);
    fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;
    let path = skills_dir.join(format!("{}.md", slugify(skill_text)));
    if !path.exists() {
        fs::write(&path, format!("# {}\n\n{}\n", title_case(skill_text), skill_text))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[allow(dead_code)]
/// Write a skill file with an explicit filename and content (used for seeding).
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

fn default_soul(name: &str, description: &str) -> String {
    format!("# {name}\n\n{description}\n")
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
