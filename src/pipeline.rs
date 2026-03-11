use std::path::PathBuf;

use chrono::Utc;

use crate::agent::invoke_agent;
use crate::config::{save_team, EmployeeConfig, TeamConfig};
use crate::memory::{
    append_memory, append_playbook, append_skill, read_agents_md, read_playbooks,
    read_recent_memory, seed_agent_files,
};
use crate::storage::get_project_agents_dir;

pub struct Pipeline {
    pub config: TeamConfig,
    pub project_dir: PathBuf,
}

impl Pipeline {
    pub fn new(config: TeamConfig, project_dir: PathBuf) -> Self {
        Pipeline { config, project_dir }
    }

    /// Run the full pipeline for a task. Returns the final synthesized result.
    pub fn run_task(&mut self, task: &str) -> Result<String, String> {
        let agents_dir = get_project_agents_dir(&self.project_dir);

        // Step 1: Leader routes task — may autonomously hire
        let employee = self.route_task(task, &agents_dir)?;
        eprintln!("→ Routing to: {}", employee.name);

        // Step 2: Employee executes with agents.md + memory + playbooks as context
        // Soul is loaded by openclaw. Skills are NOT injected — openclaw handles them.
        let employee_id = name_to_id(&employee.name);
        let agents_md = read_agents_md(&employee_id, &self.project_dir);
        let memory    = read_recent_memory(&employee_id, &self.project_dir);
        let playbooks = read_playbooks(&employee_id, &self.project_dir);
        let message   = build_employee_message(task, &agents_md, &memory, &playbooks);

        let emp_result = invoke_agent(&employee_id, &message, &agents_dir).unwrap_or_else(|e| {
            eprintln!("Warning: employee agent failed: {}", e);
            format!("[Agent unavailable: {}]", e)
        });
        eprintln!("→ Employee result: {} chars", emp_result.len());

        // Step 3: Each board member independently reviews the result
        let board_feedbacks =
            self.run_board_review(task, &employee.name, &emp_result, &agents_dir);

        // Step 4: Leader synthesizes board feedback into final result
        let final_result =
            self.leader_synthesize(task, &employee.name, &emp_result, &board_feedbacks, &agents_dir)?;

        // Steps 5–7: Apply experience from board feedback (experience ladder)
        let date = Utc::now().format("%Y-%m-%d").to_string();
        for (member_name, feedback) in &board_feedbacks {
            let ex = parse_board_feedback(feedback);

            if let Some(note) = ex.memory {
                eprintln!("→ Memory (from {}): {}", member_name, truncate(&note, 80));
                let entry = format!("## {} (reviewed by {})\n{}\n", date, member_name, note);
                let _ = append_memory(&employee_id, &entry, &self.project_dir);
            }
            if let Some((title, content)) = ex.playbook {
                eprintln!("→ Playbook (from {}): {}", member_name, title);
                let _ = append_playbook(&employee_id, &title, &content, &self.project_dir);
            }
            if let Some(skill) = ex.skill {
                eprintln!("→ New skill (from {}): {}", member_name, skill);
                let _ = append_skill(&employee_id, &skill, &self.project_dir);
            }
        }

        Ok(final_result)
    }

    // -----------------------------------------------------------------------
    // Step 1: routing — leader may autonomously hire
    // -----------------------------------------------------------------------

    fn route_task(
        &mut self,
        task: &str,
        agents_dir: &std::path::Path,
    ) -> Result<EmployeeConfig, String> {
        if self.config.employees.is_empty() {
            return Err("No employees defined in team.yaml".to_string());
        }

        let employee_list = self
            .config
            .employees
            .iter()
            .map(|e| format!("  - {}: {}", e.name, e.description))
            .collect::<Vec<_>>()
            .join("\n");

        let leader_id = name_to_id(&self.config.leader.name);
        let message = format!(
            "Current team:\n{}\n\nIncoming task: {}\n\n\
            Reply with EXACTLY ONE of:\n\
            1. The name of the best-suited employee (must match the list exactly)\n\
            2. HIRE: <name> | <one-line description>  — only if no current employee fits",
            employee_list, task
        );

        let response = invoke_agent(&leader_id, &message, agents_dir).unwrap_or_default();

        // Check for autonomous hire directive
        for line in response.lines() {
            if line.trim_start().to_uppercase().starts_with("HIRE:") {
                let colon = line.find(':').unwrap();
                let rest = line[colon + 1..].trim();
                if let Some((name, desc)) = rest.split_once('|') {
                    let name = name.trim().to_string();
                    let desc = desc.trim().to_string();
                    if !name.is_empty() {
                        eprintln!("→ Leader hiring: {} — {}", name, desc);
                        return self.do_hire(&name, &desc);
                    }
                }
            }
        }

        let chosen = response.trim().to_lowercase();
        Ok(self
            .config
            .employees
            .iter()
            .find(|e| chosen.contains(&e.name.to_lowercase()))
            .or_else(|| self.config.employees.first())
            .cloned()
            .expect("employees is non-empty"))
    }

    /// Hire an employee: update in-memory config, seed agent files, persist team.yaml.
    /// Public so cli.rs can also delegate to this (avoids duplicating hire logic).
    pub fn do_hire(&mut self, name: &str, description: &str) -> Result<EmployeeConfig, String> {
        // Idempotent: return existing if already hired
        if let Some(existing) = self
            .config
            .employees
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
        {
            return Ok(existing.clone());
        }
        let emp = EmployeeConfig {
            name: name.to_string(),
            description: description.to_string(),
            soul: None,
        };
        self.config.employees.push(emp.clone());
        save_team(&self.project_dir, &self.config)?;
        seed_agent_files(
            &name_to_id(name),
            name,
            description,
            None,
            None,
            &self.project_dir,
        )?;
        Ok(emp)
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
        self.config
            .board
            .iter()
            .map(|member| {
                let message = format!(
                    "Task: {}\n\
                     Employee result ({}):\n{}\n\n\
                     Review this result. Respond in exactly this format:\n\
                     CHALLENGES: <specific problems or risks>\n\
                     IMPROVEMENTS: <concrete changes that would fix them>\n\
                     MEMORY: <one-line note worth saving to employee memory> (omit if nothing notable)\n\
                     PLAYBOOK: <title> | <step-by-step workflow to reuse> (omit if no repeatable process)\n\
                     NEW_SKILL: <capability to automate> (omit if none)",
                    task, employee_name, result
                );
                let member_id = name_to_id(&member.name);
                let feedback = invoke_agent(&member_id, &message, agents_dir).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: board member {} unavailable: {}",
                        member.name, e
                    );
                    "CHALLENGES: N/A\nIMPROVEMENTS: N/A".to_string()
                });
                (member.name.clone(), feedback)
            })
            .collect()
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
        let feedback_section = board_feedbacks
            .iter()
            .zip(self.config.board.iter())
            .map(|((name, feedback), member)| {
                format!("### {} ({})\n{}", name, member.specialty, feedback)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let leader_id = name_to_id(&self.config.leader.name);
        let message = format!(
            "Original task: {}\n\
             Employee result ({}):\n{}\n\n\
             Board feedback:\n{}\n\n\
             Produce the best possible final result incorporating the board's improvements. \
             Be concise — the user wants results, not a transcript of the review.",
            task, employee_name, employee_result, feedback_section,
        );

        match invoke_agent(&leader_id, &message, agents_dir) {
            Ok(r) => Ok(r),
            Err(_) => Ok(format!(
                "{}\n\n---\nBoard feedback:\n{}",
                employee_result, feedback_section
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Message builder — soul is loaded by openclaw, we inject role context only
// ---------------------------------------------------------------------------

fn build_employee_message(
    task: &str,
    agents_md: &str,
    memory: &str,
    playbooks: &str,
) -> String {
    let mut msg = String::new();

    if !agents_md.trim().is_empty() {
        msg.push_str("# Your Role\n");
        msg.push_str(agents_md.trim());
        msg.push_str("\n---\n");
    }
    if !memory.trim().is_empty() {
        msg.push_str("# Memory\n");
        msg.push_str(memory.trim());
        msg.push_str("\n---\n");
    }
    if !playbooks.trim().is_empty() {
        msg.push_str("# Playbooks\n");
        msg.push_str(playbooks.trim());
        msg.push_str("\n---\n");
    }

    msg.push_str("# Task\n");
    msg.push_str(task);
    msg.push_str("\n\nDeliver thorough work. Your result will be reviewed by the board.");
    msg
}

// ---------------------------------------------------------------------------
// Board feedback parser — extracts MEMORY: / PLAYBOOK: / NEW_SKILL: lines
// ---------------------------------------------------------------------------

struct BoardExtract {
    memory:   Option<String>,
    playbook: Option<(String, String)>, // (title, content)
    skill:    Option<String>,
}

fn parse_board_feedback(feedback: &str) -> BoardExtract {
    let mut memory   = None;
    let mut playbook = None;
    let mut skill    = None;

    for line in feedback.lines() {
        let upper = line.trim_start().to_uppercase();
        if upper.starts_with("MEMORY:") {
            let v = line[line.find(':').unwrap() + 1..].trim().to_string();
            if !v.is_empty() {
                memory = Some(v);
            }
        } else if upper.starts_with("PLAYBOOK:") {
            let v = line[line.find(':').unwrap() + 1..].trim().to_string();
            if let Some((title, content)) = v.split_once('|') {
                let t = title.trim().to_string();
                let c = content.trim().to_string();
                if !t.is_empty() {
                    playbook = Some((t, c));
                }
            }
        } else if upper.starts_with("NEW_SKILL:") {
            let v = line[line.find(':').unwrap() + 1..].trim().to_string();
            if !v.is_empty() {
                skill = Some(v);
            }
        }
    }

    BoardExtract { memory, playbook, skill }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn name_to_id(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
