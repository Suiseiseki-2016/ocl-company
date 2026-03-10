use std::path::PathBuf;

use chrono::Utc;

use crate::agent::invoke_agent;
use crate::config::{EmployeeConfig, TeamConfig};
use crate::memory::{append_memory, append_skill, get_memory_dir};
use crate::storage::get_project_agents_dir;

pub struct Pipeline {
    pub config: TeamConfig,
    pub project_dir: PathBuf,
}

impl Pipeline {
    pub fn new(config: TeamConfig, project_dir: PathBuf) -> Self {
        Pipeline { config, project_dir }
    }

    /// Run the full 7-step pipeline for a task. Returns the final synthesized result.
    pub fn run_task(&self, task: &str) -> Result<String, String> {
        let agents_dir = get_project_agents_dir(&self.project_dir);

        // Step 1: Leader routes task to best-suited employee
        let employee = self.route_task(task, &agents_dir)?;
        eprintln!("→ Routing to: {}", employee.name);

        // Step 2: Employee executes the task
        // openclaw loads soul + skills from agents/{id}/
        // We inject recent memory ourselves as it is project-specific context
        let employee_id = name_to_id(&employee.name);
        let recent_memory = read_recent_memory(&employee_id, &self.project_dir);
        let employee_message = build_employee_message(task, &recent_memory);
        let emp_result = invoke_agent(&employee_id, &employee_message, &agents_dir)
            .unwrap_or_else(|e| {
                eprintln!("Warning: employee agent failed: {}", e);
                format!("[Agent unavailable: {}]", e)
            });
        eprintln!("→ Employee result: {} chars", emp_result.len());

        // Step 3: Each board member independently reviews the result
        // openclaw loads each reviewer's soul from agents/{id}/
        let board_feedbacks = self.run_board_review(task, &employee.name, &emp_result, &agents_dir);

        // Step 4: Leader synthesizes board feedback into final result
        // openclaw loads alex's soul from agents/{leader_id}/
        let final_result = self.leader_synthesize(
            task, &employee.name, &emp_result, &board_feedbacks, &agents_dir,
        )?;

        // Step 5: Write task summary to employee memory/
        let memory_entry = format!(
            "## Task ({})\n{}\n\nSummary: {}\n",
            Utc::now().format("%Y-%m-%d"),
            task,
            truncate(&final_result, 200),
        );
        let _ = append_memory(&employee_id, &memory_entry, &self.project_dir);

        // Step 6: Apply NEW_SKILL suggestions from board — each becomes a skills/ file
        for (member_name, feedback) in &board_feedbacks {
            if let Some(skill) = extract_new_skill(feedback) {
                eprintln!("→ New skill for {} (from {}): {}", employee.name, member_name, skill);
                let _ = append_skill(&employee_id, &skill, &self.project_dir);
            }
        }

        // Step 7: Return final result
        Ok(final_result)
    }

    // -----------------------------------------------------------------------
    // Step 1: routing
    // -----------------------------------------------------------------------

    fn route_task(
        &self,
        task: &str,
        agents_dir: &std::path::Path,
    ) -> Result<EmployeeConfig, String> {
        if self.config.employees.is_empty() {
            return Err("No employees defined in team.yaml".to_string());
        }

        let employee_list = self.config.employees.iter()
            .map(|e| format!("  - {}: {}", e.name, e.description))
            .collect::<Vec<_>>()
            .join("\n");

        // Only the routing decision — soul ("You are Alex...") is loaded by openclaw
        let message = format!(
            "Team:\n{}\n\nTask: {}\n\nReply with only the employee name best suited for this task.",
            employee_list, task
        );

        let leader_id = name_to_id(&self.config.leader.name);
        let response = invoke_agent(&leader_id, &message, agents_dir).unwrap_or_default();
        let chosen = response.trim().to_lowercase();

        Ok(self.config.employees.iter()
            .find(|e| chosen.contains(&e.name.to_lowercase()))
            .or_else(|| self.config.employees.first())
            .cloned()
            .expect("employees is non-empty"))
    }

    // -----------------------------------------------------------------------
    // Step 3: board review
    // -----------------------------------------------------------------------

    fn run_board_review(
        &self,
        task: &str,
        employee_name: &str,
        result: &str,
        agents_dir: &std::path::Path,
    ) -> Vec<(String, String)> {
        self.config.board.iter().map(|member| {
            // Soul ("You are the Technical Reviewer...") is loaded by openclaw
            let message = format!(
                "Task: {}\n\
                 Employee result ({}):\n{}\n\n\
                 Respond in exactly this format:\n\
                 CHALLENGES: ...\n\
                 IMPROVEMENTS: ...\n\
                 NEW_SKILL: ... (omit line if none)",
                task, employee_name, result
            );
            let member_id = name_to_id(&member.name);
            let feedback = invoke_agent(&member_id, &message, agents_dir)
                .unwrap_or_else(|e| {
                    eprintln!("Warning: board member {} unavailable: {}", member.name, e);
                    "CHALLENGES: N/A\nIMPROVEMENTS: N/A".to_string()
                });
            (member.name.clone(), feedback)
        }).collect()
    }

    // -----------------------------------------------------------------------
    // Step 4: leader synthesis
    // -----------------------------------------------------------------------

    fn leader_synthesize(
        &self,
        task: &str,
        employee_name: &str,
        employee_result: &str,
        board_feedbacks: &[(String, String)],
        agents_dir: &std::path::Path,
    ) -> Result<String, String> {
        let feedback_section = board_feedbacks.iter()
            .zip(self.config.board.iter())
            .map(|((name, feedback), member)| {
                format!("### {} ({})\n{}", name, member.specialty, feedback)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Soul ("You are Alex, leader...") is loaded by openclaw
        let message = format!(
            "Original task: {}\n\
             Employee result ({}):\n{}\n\n\
             Board feedback:\n{}\n\n\
             Produce the best possible final result incorporating all board feedback.",
            task, employee_name, employee_result, feedback_section,
        );

        let leader_id = name_to_id(&self.config.leader.name);
        match invoke_agent(&leader_id, &message, agents_dir) {
            Ok(r) => Ok(r),
            Err(_) => Ok(format!("{}\n\n---\nBoard feedback:\n{}", employee_result, feedback_section)),
        }
    }
}

// ---------------------------------------------------------------------------
// Message builders — task-specific content only, no soul/skills injected
// ---------------------------------------------------------------------------

/// Build the message sent to the employee agent.
/// Soul + skills are loaded by openclaw. We only add recent memory as context.
fn build_employee_message(task: &str, recent_memory: &str) -> String {
    let mut msg = String::new();
    if !recent_memory.trim().is_empty() {
        msg.push_str("# Recent Memory\n");
        msg.push_str(recent_memory.trim());
        msg.push_str("\n---\n");
    }
    msg.push_str("# Current Task\n");
    msg.push_str(task);
    msg.push_str("\n---\nDeliver thorough work. Your result will be reviewed by the board.");
    msg
}

/// Read the last N memory entries for an agent (project-specific context,
/// not managed by openclaw).
fn read_recent_memory(agent_id: &str, project_dir: &std::path::Path) -> String {
    let memory_dir = get_memory_dir(agent_id, project_dir);
    if !memory_dir.exists() {
        return String::new();
    }
    let mut entries: Vec<_> = std::fs::read_dir(&memory_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    // Take only the most recent 5 entries to keep context lean
    let recent = entries.iter().rev().take(5).rev();
    recent
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a display name to a filesystem-safe agent ID.
pub fn name_to_id(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

fn extract_new_skill(feedback: &str) -> Option<String> {
    for line in feedback.lines() {
        if line.trim().to_uppercase().starts_with("NEW_SKILL:") {
            let skill = line[line.find(':').unwrap() + 1..].trim().to_string();
            if !skill.is_empty() {
                return Some(skill);
            }
        }
    }
    None
}

