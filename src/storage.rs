use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// {project}/agents/{agent-id}/
pub fn get_project_agents_dir(project_dir: &Path) -> PathBuf {
    project_dir.join("agents")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFile {
    pub id: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultFile {
    pub id: String,
    pub result: String,
    pub summary: String,
    pub finished_at: DateTime<Utc>,
}

/// Return all pending tasks in `{project}/tasks/` that have no matching result file.
pub fn scan_pending_tasks(project_dir: &Path) -> Vec<TaskFile> {
    let tasks_dir = project_dir.join("tasks");
    let mut tasks = Vec::new();
    let Ok(entries) = fs::read_dir(&tasks_dir) else {
        return tasks;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !file_name.ends_with(".task.json") {
            continue;
        }
        let id = file_name.trim_end_matches(".task.json");
        if tasks_dir.join(format!("{}.result.json", id)).exists() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(task) = serde_json::from_str::<TaskFile>(&content) {
                tasks.push(task);
            }
        }
    }
    tasks
}

pub fn write_result(project_dir: &Path, result: &ResultFile) -> Result<(), String> {
    let tasks_dir = project_dir.join("tasks");
    fs::create_dir_all(&tasks_dir).map_err(|e| e.to_string())?;
    let path = tasks_dir.join(format!("{}.result.json", result.id));
    let content = serde_json::to_string_pretty(result).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn write_task(project_dir: &Path, task: &TaskFile) -> Result<(), String> {
    let tasks_dir = project_dir.join("tasks");
    fs::create_dir_all(&tasks_dir).map_err(|e| e.to_string())?;
    let path = tasks_dir.join(format!("{}.task.json", task.id));
    let content = serde_json::to_string_pretty(task).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}
