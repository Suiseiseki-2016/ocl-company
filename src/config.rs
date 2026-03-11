use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TeamConfig {
    pub name: String,
    pub leader: LeaderConfig,
    pub board: Vec<BoardConfig>,
    pub employees: Vec<EmployeeConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeaderConfig {
    pub name: String,
    pub soul: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoardConfig {
    pub name: String,
    pub specialty: String,
    pub soul: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmployeeConfig {
    pub name: String,
    pub description: String,
    pub soul: Option<String>,
}

pub fn load_team(project: &Path) -> Result<TeamConfig, String> {
    let path = project.join("team.yaml");
    if !path.exists() {
        return Err(format!(
            "team.yaml not found in {}. Run 'ocl-company init' first.",
            project.display()
        ));
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let config: TeamConfig = serde_yaml::from_str(&content).map_err(|e| e.to_string())?;
    validate_team(&config)?;
    Ok(config)
}

/// Validate that all names are safe to use as filesystem path components.
fn validate_team(config: &TeamConfig) -> Result<(), String> {
    validate_name(&config.leader.name, "leader")?;
    for member in &config.board {
        validate_name(&member.name, "board member")?;
    }
    for emp in &config.employees {
        validate_name(&emp.name, "employee")?;
    }
    Ok(())
}

/// Reject names that are empty, contain path separators, or attempt directory traversal.
pub fn validate_name(name: &str, role: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(format!("{} name must not be empty", role));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(format!("{} name '{}' must not contain path separators", role, name));
    }
    if name == ".." || name.starts_with("../") || name.contains("/../") {
        return Err(format!("{} name '{}' must not traverse directories", role, name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_ok() {
        assert!(validate_name("Alice", "employee").is_ok());
        assert!(validate_name("Technical Reviewer", "board").is_ok());
    }

    #[test]
    fn validate_name_empty() {
        assert!(validate_name("", "employee").is_err());
        assert!(validate_name("   ", "employee").is_err());
    }

    #[test]
    fn validate_name_path_traversal() {
        assert!(validate_name("../../etc/passwd", "employee").is_err());
        assert!(validate_name("foo/bar", "employee").is_err());
        assert!(validate_name("foo\\bar", "employee").is_err());
        assert!(validate_name("..", "employee").is_err());
    }
}

pub fn save_team(project: &Path, config: &TeamConfig) -> Result<(), String> {
    let path = project.join("team.yaml");
    let content = serde_yaml::to_string(config).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}
