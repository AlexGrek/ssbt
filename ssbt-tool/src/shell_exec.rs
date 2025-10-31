use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead};
use anyhow::{Result, Context, anyhow};

/// Executes a shell command, prints its output in real-time,
/// and returns an error if the command exits with a non-zero status code.
///
/// Arguments:
/// * `command`: The command to execute (e.g., "sh -c 'echo hello; sleep 1; echo world'").
///
/// Returns:
/// * `Ok(())` on successful execution (exit code 0).
/// * `Err(anyhow::Error)` if the command fails to start, read output, or exits with a non-zero code.
pub fn execute_and_stream_command(command: &str) -> Result<()> {
    // Split the command string into the program and its arguments
    // NOTE: For simple commands, this works. For complex shell commands (like the example below),
    // it's safer to use 'sh -c "your command"'
    let parts: Vec<&str> = command.split_whitespace().collect();
    let program = parts.first().context("Command string is empty")?;
    let args = &parts[1..];

    // --- 1. Spawn the command and pipe stdout ---
    let mut child = Command::new(program)
        .args(args)
        // Crucial: Pipe the output so we can read it directly
        .stdout(Stdio::piped())
        .spawn()
        .context(format!("Failed to spawn command: '{}'", command))?;

    // --- 2. Get the stdout handle and set up a buffered reader ---
    let stdout = child.stdout.take()
        .context("Child process did not have a stdout handle")?;

    let reader = BufReader::new(stdout);
    
    // --- 3. Read and print output line-by-line in real-time ---
    for line in reader.lines() {
        match line {
            Ok(l) => println!("{}", l),
            Err(e) => {
                // Return an error if reading the pipe itself fails
                return Err(e).context("Error reading output from child process");
            }
        }
    }

    // --- 4. Wait for the command to finish and check the exit status ---
    let status = child.wait()
        .context("Failed to wait on child process")?;

    if status.success() {
        Ok(())
    } else {
        // Return an error with the non-zero exit code
        let code = status.code().unwrap_or(-1);
        eprintln!("\n🚨 Command failed with exit code: {}", code);
        
        // Use anyhow! to create a simple, clean error
        Err(anyhow!("Command '{}' failed with exit code: {}", command, code))
    }
}
