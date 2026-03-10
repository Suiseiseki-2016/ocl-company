use std::path::Path;
use std::process::Command;

/// Invoke a named openclaw agent with a task message.
///
/// openclaw loads the agent's soul.md and skills/ from `agents_dir/{id}/`.
/// We only pass the task-specific message — not the assembled soul+skills+memory.
///
/// Equivalent to:
///   openclaw agent --agent researcher \
///                  --agents-dir ./agents \
///                  --message "..." \
///                  --deliver
pub fn invoke_agent(agent_id: &str, message: &str, agents_dir: &Path) -> Result<String, String> {
    let dir = agents_dir.to_str().unwrap_or(".");

    let attempts: &[(&str, &[&str])] = &[
        ("openclaw", &["agent", "--agent", agent_id, "--agents-dir", dir, "--message", message, "--deliver"]),
        ("ocl",      &["agent", "--agent", agent_id, "--agents-dir", dir, "--message", message, "--deliver"]),
    ];

    for (cmd, args) in attempts {
        match Command::new(cmd).args(*args).output() {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                eprintln!("Warning: {} --agent {} exited non-zero: {}", cmd, agent_id, stderr);
            }
            Err(_) => {
                // command not found, try next
            }
        }
    }

    Err(format!("Could not invoke agent '{}' via openclaw/ocl", agent_id))
}
