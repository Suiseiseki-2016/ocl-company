use std::path::Path;
use std::process::Command;

/// Invoke a named openclaw agent with a task message.
///
/// openclaw loads the agent's context by name.
/// We only pass the task-specific message.
///
/// Equivalent to:
///   openclaw agent --agent researcher -m "..." --deliver
pub fn invoke_agent(agent_id: &str, message: &str, _agents_dir: &Path) -> Result<String, String> {
    let attempts: &[(&str, &[&str])] = &[
        ("openclaw", &["agent", "--agent", agent_id, "-m", message, "--deliver"]),
        ("ocl",      &["agent", "--agent", agent_id, "-m", message, "--deliver"]),
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
