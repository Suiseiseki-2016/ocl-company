use serde::{Deserialize, Serialize};

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
