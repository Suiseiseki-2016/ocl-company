use std::fs;
use std::path::Path;
use std::process::Command;

/// Invoke an agent via openclaw's main agent, with only soul injected into the message.
///
/// openclaw only requires --agent main. We load the named agent's soul.md
/// to establish identity, then pass the message. Skills are NOT injected as
/// text — the agent calls them itself via openclaw's skill-invocation system.
///
///   openclaw agent --agent main -m "<soul + message>" --deliver
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

/// Build the prompt: soul → message.
/// Skills are intentionally excluded — the agent invokes them via openclaw at runtime.
fn build_prompt(agent_id: &str, message: &str, agents_dir: &Path) -> String {
    let agent_dir = agents_dir.join(agent_id);
    let mut prompt = String::new();

    // Soul: identity and personality only
    let soul_path = agent_dir.join("soul.md");
    if let Ok(soul) = fs::read_to_string(&soul_path) {
        prompt.push_str(soul.trim());
        prompt.push_str("\n---\n");
    }

    prompt.push_str(message);
    prompt
}
