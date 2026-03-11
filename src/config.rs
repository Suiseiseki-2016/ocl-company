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
    serde_yaml::from_str(&content).map_err(|e| e.to_string())
}

pub fn save_team(project: &Path, config: &TeamConfig) -> Result<(), String> {
    let path = project.join("team.yaml");
    let content = serde_yaml::to_string(config).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}
