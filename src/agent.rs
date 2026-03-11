use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const AGENT_TIMEOUT_SECS: u64 = 120;

/// Error type for agent invocation — distinguishes "not found" from real failures.
enum AgentError {
    /// Binary not on PATH — try the next fallback silently.
    NotFound,
    /// Process found but failed or timed out — log and try next.
    Failed(String),
}

pub fn invoke_agent(agent_id: &str, message: &str, agents_dir: &Path) -> Result<String, String> {
    let prompt = build_prompt(agent_id, message, agents_dir);

    let attempts: &[(&str, &[&str])] = &[
        ("openclaw", &["agent", "--agent", "main", "-m", &prompt, "--deliver"]),
        ("ocl",      &["agent", "--agent", "main", "-m", &prompt, "--deliver"]),
    ];

    for (cmd, args) in attempts {
        match run_with_timeout(cmd, args) {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                eprintln!("Warning: {} (agent {}) exited non-zero: {}", cmd, agent_id, stderr);
            }
            Err(AgentError::NotFound) => {
                // binary not installed, try next silently
            }
            Err(AgentError::Failed(e)) => {
                eprintln!("Warning: {} (agent {}) failed: {}", cmd, agent_id, e);
            }
        }
    }

    Err(format!("Could not invoke agent '{}' via openclaw/ocl", agent_id))
}

/// Spawn a subprocess with a hard timeout. Returns the full output or an AgentError.
/// On timeout the child is left running — it will be orphaned and exit on its own.
fn run_with_timeout(cmd: &str, args: &[&str]) -> Result<std::process::Output, AgentError> {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AgentError::NotFound
            } else {
                AgentError::Failed(e.to_string())
            }
        })?;

    let (tx, rx) = mpsc::channel();
    // Move child into a thread; wait_with_output drains stdout/stderr preventing pipe deadlock.
    thread::spawn(move || {
        tx.send(child.wait_with_output()).ok();
    });

    rx.recv_timeout(Duration::from_secs(AGENT_TIMEOUT_SECS))
        .map_err(|_| AgentError::Failed(format!("timed out after {}s", AGENT_TIMEOUT_SECS)))?
        .map_err(|e| AgentError::Failed(e.to_string()))
}

/// Build the prompt: soul → message.
/// Skills are intentionally excluded — the agent invokes them via openclaw at runtime.
fn build_prompt(agent_id: &str, message: &str, agents_dir: &Path) -> String {
    let agent_dir = agents_dir.join(agent_id);
    let mut prompt = String::new();

    let soul_path = agent_dir.join("soul.md");
    if let Ok(soul) = fs::read_to_string(&soul_path) {
        prompt.push_str(soul.trim());
        prompt.push_str("\n---\n");
    }

    prompt.push_str(message);
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_prompt_no_soul() {
        let dir = PathBuf::from("/nonexistent");
        let prompt = build_prompt("nobody", "hello task", &dir);
        assert_eq!(prompt, "hello task");
    }
}
