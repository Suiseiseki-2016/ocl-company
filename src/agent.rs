use std::fs;
use std::path::Path;
use std::process::Command;

/// Invoke an agent via openclaw's main agent, with soul + skills injected into the message.
///
/// openclaw only requires --agent main. We load the named agent's soul.md and
/// skills/ from the local agents/ directory and prepend them to the message so
/// the main agent adopts the right identity and capabilities for this invocation.
///
///   openclaw agent --agent main -m "<soul + skills + message>" --deliver
pub fn invoke_agent(agent_id: &str, message: &str, agents_dir: &Path) -> Result<String, String> {
    let prompt = build_prompt(agent_id, message, agents_dir);

    let attempts: &[(&str, &[&str])] = &[
        ("openclaw", &["agent", "--agent", "main", "-m", &prompt, "--deliver"]),
        ("ocl",      &["agent", "--agent", "main", "-m", &prompt, "--deliver"]),
    ];

    for (cmd, args) in attempts {
        match Command::new(cmd).args(*args).output() {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                eprintln!("Warning: {} (agent {}) exited non-zero: {}", cmd, agent_id, stderr);
            }
            Err(_) => {
                // command not found, try next
            }
        }
    }

    Err(format!("Could not invoke agent '{}' via openclaw/ocl", agent_id))
}

/// Build the full prompt: soul → skills → message.
fn build_prompt(agent_id: &str, message: &str, agents_dir: &Path) -> String {
    let agent_dir = agents_dir.join(agent_id);
    let mut prompt = String::new();

    // Soul: identity and personality
    let soul_path = agent_dir.join("soul.md");
    if let Ok(soul) = fs::read_to_string(&soul_path) {
        prompt.push_str(soul.trim());
        prompt.push_str("\n---\n");
    }

    // Skills: all *.md files in skills/, sorted alphabetically
    let skills_dir = agent_dir.join("skills");
    if skills_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&skills_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in &entries {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                prompt.push_str(content.trim());
                prompt.push_str("\n\n");
            }
        }
        if !entries.is_empty() {
            prompt.push_str("---\n");
        }
    }

    // Task-specific message content
    prompt.push_str(message);
    prompt
}
