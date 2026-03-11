use std::path::PathBuf;

use chrono::Utc;

use crate::agent::invoke_agent;
use crate::config::{save_team, BoardConfig, EmployeeConfig, TeamConfig};
use crate::memory::{
    append_memory, append_playbook, append_skill, read_agents_md, read_playbooks,
    read_recent_memory, seed_agent_files,
};
use crate::storage::get_project_agents_dir;

struct RouteDecision {
    employee:  EmployeeConfig,
    reviewers: Vec<BoardConfig>,
}

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

        // Step 1: Leader routes task — may autonomously hire; also selects relevant reviewers
        let decision = self.route_task(task, &agents_dir)?;
        let employee = decision.employee;
        let reviewers = decision.reviewers;
        eprintln!("→ Routing to: {}", employee.name);
        eprintln!(
            "→ Reviewers: {}",
            if reviewers.is_empty() {
                "none".to_string()
            } else {
                reviewers.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
            }
        );

        // Step 2: Employee executes with agents.md + memory + playbooks as context
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

        // Step 3: Selected board members independently review the result
        if reviewers.is_empty() {
            return Ok(emp_result);
        }
        let board_feedbacks =
            self.run_board_review(task, &employee.name, &emp_result, &reviewers, &agents_dir);

        // Early exit: if ALL reviewers approved, skip synthesis (#5: exact match only)
        let all_approved = board_feedbacks
            .iter()
            .all(|(_, f)| f.trim().to_uppercase() == "APPROVED");
        if all_approved {
            eprintln!("→ All reviewers approved — skipping synthesis");
            return Ok(emp_result);
        }

        // Step 4: Leader synthesizes board feedback into final result
        let final_result =
            self.leader_synthesize(task, &employee.name, &emp_result, &board_feedbacks)?;

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
    // Step 1: routing — leader may autonomously hire + selects relevant reviewers
    // -----------------------------------------------------------------------

    fn route_task(
        &mut self,
        task: &str,
        agents_dir: &std::path::Path,
    ) -> Result<RouteDecision, String> {
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

        let reviewer_list = self
            .config
            .board
            .iter()
            .map(|b| format!("  - {}: {}", b.name, b.specialty))
            .collect::<Vec<_>>()
            .join("\n");

        let leader_id = name_to_id(&self.config.leader.name);
        let message = format!(
            "Current team:\n{}\n\nBoard reviewers:\n{}\n\nIncoming task: {}\n\n\
            Reply with EXACTLY:\n\
            EMPLOYEE: <name>  (or: HIRE: <name> | <one-line description> if no one fits)\n\
            REVIEWERS: <comma-separated reviewer names relevant to this task, or NONE>",
            employee_list, reviewer_list, task
        );

        let response = invoke_agent(&leader_id, &message, agents_dir).unwrap_or_default();

        // Parse EMPLOYEE / HIRE line — use split_once to avoid unsafe indexing (#1)
        let mut hired_employee: Option<EmployeeConfig> = None;
        let mut chosen_name: Option<String> = None;
        for line in response.lines() {
            let upper = line.trim_start().to_uppercase();
            if upper.starts_with("HIRE:") {
                if let Some((_, rest)) = line.split_once(':') {
                    if let Some((name, desc)) = rest.split_once('|') {
                        let name = name.trim().to_string();
                        let desc = desc.trim().to_string();
                        if !name.is_empty() {
                            eprintln!("→ Leader hiring: {} — {}", name, desc);
                            hired_employee = Some(self.do_hire(&name, &desc)?);
                        }
                    }
                }
            } else if upper.starts_with("EMPLOYEE:") {
                if let Some((_, name)) = line.split_once(':') {
                    chosen_name = Some(name.trim().to_string());
                }
            }
        }

        // Employee resolution: exact match first, then substring, then first with warning (#3, #4)
        let employee = if let Some(emp) = hired_employee {
            emp
        } else {
            let name_lc = chosen_name.unwrap_or_default().to_lowercase();
            let found = self
                .config
                .employees
                .iter()
                .find(|e| e.name.to_lowercase() == name_lc)
                .or_else(|| {
                    self.config
                        .employees
                        .iter()
                        .find(|e| name_lc.contains(&e.name.to_lowercase()))
                });
            match found {
                Some(e) => e.clone(),
                None => {
                    eprintln!(
                        "Warning: leader chose unknown employee '{}', falling back to first",
                        name_lc
                    );
                    self.config.employees.first().cloned().expect("employees is non-empty")
                }
            }
        };

        // Parse REVIEWERS line — warn on unrecognised names (#6)
        let reviewers = response
            .lines()
            .find(|l| l.trim_start().to_uppercase().starts_with("REVIEWERS:"))
            .and_then(|line| line.split_once(':'))
            .map(|(_, names)| {
                let names = names.trim();
                if names.to_uppercase() == "NONE" || names.is_empty() {
                    return vec![];
                }
                names
                    .split(',')
                    .filter_map(|n| {
                        let n = n.trim().to_lowercase();
                        let found = self
                            .config
                            .board
                            .iter()
                            .find(|b| b.name.to_lowercase() == n)
                            .cloned();
                        if found.is_none() && !n.is_empty() {
                            eprintln!("Warning: leader named unknown reviewer '{}' — skipping", n);
                        }
                        found
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(RouteDecision { employee, reviewers })
    }

    /// Hire an employee: update in-memory config, seed agent files, persist team.yaml.
    pub fn do_hire(&mut self, name: &str, description: &str) -> Result<EmployeeConfig, String> {
        crate::config::validate_name(name, "employee")?;
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
        reviewers: &[BoardConfig],
        agents_dir: &std::path::Path,
    ) -> Vec<(String, String)> {
        reviewers
            .iter()
            .map(|member| {
                let message = format!(
                    "Task: {}\n\
                     Employee result ({}):\n{}\n\n\
                     If the result is excellent and needs no changes, reply with just: APPROVED\n\
                     Otherwise review in exactly this format:\n\
                     CHALLENGES: <specific problems or risks>\n\
                     IMPROVEMENTS: <concrete changes that would fix them>\n\
                     MEMORY: <one-line note worth saving to employee memory> (omit if nothing notable)\n\
                     PLAYBOOK: <title> | <step-by-step workflow to reuse> (omit if no repeatable process)\n\
                     NEW_SKILL: <capability to automate> (omit if none)",
                    task, employee_name, result
                );
                let member_id = name_to_id(&member.name);
                let feedback = invoke_agent(&member_id, &message, agents_dir).unwrap_or_else(|e| {
                    eprintln!("Warning: board member {} unavailable: {}", member.name, e);
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
    ) -> Result<String, String> {
        // Look up each reviewer's specialty by name — don't zip with self.config.board
        // because board_feedbacks only contains the *selected* reviewers, not all of them (#bug)
        let feedback_section = board_feedbacks
            .iter()
            .map(|(name, feedback)| {
                let specialty = self
                    .config
                    .board
                    .iter()
                    .find(|b| b.name == *name)
                    .map(|b| b.specialty.as_str())
                    .unwrap_or("general review");
                format!("### {} ({})\n{}", name, specialty, feedback)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let agents_dir = get_project_agents_dir(&self.project_dir);
        let leader_id = name_to_id(&self.config.leader.name);
        let message = format!(
            "Original task: {}\n\
             Employee result ({}):\n{}\n\n\
             Board feedback:\n{}\n\n\
             Produce the best possible final result incorporating the board's improvements. \
             Be concise — the user wants results, not a transcript of the review.",
            task, employee_name, employee_result, feedback_section,
        );

        match invoke_agent(&leader_id, &message, &agents_dir) {
            Ok(r) => Ok(r),
            Err(_) => Ok(format!(
                "{}\n\n---\nBoard feedback:\n{}",
                employee_result, feedback_section
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Message builder
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
    playbook: Option<(String, String)>,
    skill:    Option<String>,
}

/// Parse structured fields from board feedback. Uses split_once to avoid unsafe indexing (#1).
fn parse_board_feedback(feedback: &str) -> BoardExtract {
    let mut memory   = None;
    let mut playbook = None;
    let mut skill    = None;

    for line in feedback.lines() {
        let upper = line.trim_start().to_uppercase();
        if upper.starts_with("MEMORY:") {
            if let Some((_, v)) = line.split_once(':') {
                let v = v.trim().to_string();
                if !v.is_empty() {
                    memory = Some(v);
                }
            }
        } else if upper.starts_with("PLAYBOOK:") {
            if let Some((_, rest)) = line.split_once(':') {
                if let Some((title, content)) = rest.split_once('|') {
                    let t = title.trim().to_string();
                    let c = content.trim().to_string();
                    if !t.is_empty() {
                        playbook = Some((t, c));
                    }
                }
            }
        } else if upper.starts_with("NEW_SKILL:") {
            if let Some((_, v)) = line.split_once(':') {
                let v = v.trim().to_string();
                if !v.is_empty() {
                    skill = Some(v);
                }
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

/// Truncate to at most `max` Unicode characters — never splits a multi-byte char (#2).
fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((byte_pos, _)) => &s[..byte_pos],
        None => s,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_to_id_basic() {
        assert_eq!(name_to_id("Technical Reviewer"), "technical-reviewer");
        assert_eq!(name_to_id("Leader"), "leader");
    }

    #[test]
    fn truncate_ascii() {
        assert_eq!(truncate("hello world", 5), "hello");
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn truncate_multibyte() {
        // "日本語" is 3 chars, each 3 bytes
        let s = "日本語テスト";
        assert_eq!(truncate(s, 3), "日本語");
        // must not panic
        let _ = truncate(s, 1);
    }

    #[test]
    fn parse_board_feedback_approved() {
        let ex = parse_board_feedback("APPROVED");
        assert!(ex.memory.is_none());
        assert!(ex.playbook.is_none());
        assert!(ex.skill.is_none());
    }

    #[test]
    fn parse_board_feedback_fields() {
        let fb = "CHALLENGES: too brief\nIMPROVEMENTS: add detail\nMEMORY: always cite sources\nNEW_SKILL: auto-cite";
        let ex = parse_board_feedback(fb);
        assert_eq!(ex.memory.as_deref(), Some("always cite sources"));
        assert_eq!(ex.skill.as_deref(), Some("auto-cite"));
        assert!(ex.playbook.is_none());
    }

    #[test]
    fn parse_board_feedback_playbook() {
        let fb = "CHALLENGES: x\nIMPROVEMENTS: y\nPLAYBOOK: Research Flow | step1, step2";
        let ex = parse_board_feedback(fb);
        let (title, content) = ex.playbook.unwrap();
        assert_eq!(title, "Research Flow");
        assert_eq!(content, "step1, step2");
    }

    #[test]
    fn parse_board_feedback_no_panic_on_missing_colon() {
        // Line with no colon in value position — must not panic
        let fb = "MEMORY";
        let ex = parse_board_feedback(fb);
        assert!(ex.memory.is_none());
    }
}
