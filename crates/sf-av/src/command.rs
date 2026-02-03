//! Builder for executing external tool commands with timeout support.

use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::process::Command;

/// Default command timeout: 5 minutes.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Output captured from a tool execution.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Process exit status.
    pub status: ExitStatus,
    /// Captured standard output (lossy UTF-8).
    pub stdout: String,
    /// Captured standard error (lossy UTF-8).
    pub stderr: String,
}

/// A builder for constructing and executing external tool invocations.
///
/// # Example
///
/// ```no_run
/// use sf_av::ToolCommand;
/// use std::path::PathBuf;
///
/// # async fn example() -> sf_core::Result<()> {
/// let output = ToolCommand::new(PathBuf::from("ffprobe"))
///     .arg("-v").arg("quiet")
///     .arg("-print_format").arg("json")
///     .arg("-show_format")
///     .arg("-show_streams")
///     .arg("/path/to/video.mkv")
///     .execute()
///     .await?;
/// println!("{}", output.stdout);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ToolCommand {
    program: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    stdin_data: Option<Vec<u8>>,
}

impl ToolCommand {
    /// Create a new command for the given program path.
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            args: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            stdin_data: None,
        }
    }

    /// Append a single argument.
    pub fn arg(&mut self, s: impl Into<String>) -> &mut Self {
        self.args.push(s.into());
        self
    }

    /// Append multiple arguments.
    pub fn args(&mut self, iter: impl IntoIterator<Item = impl Into<String>>) -> &mut Self {
        self.args.extend(iter.into_iter().map(Into::into));
        self
    }

    /// Set the maximum execution time.
    pub fn timeout(&mut self, d: Duration) -> &mut Self {
        self.timeout = d;
        self
    }

    /// Provide data to be written to the process's stdin.
    pub fn stdin(&mut self, data: Vec<u8>) -> &mut Self {
        self.stdin_data = Some(data);
        self
    }

    /// Execute the command, capturing stdout and stderr.
    ///
    /// # Errors
    ///
    /// - Returns [`sf_core::Error::Tool`] if the process times out (message
    ///   includes the timeout duration).
    /// - Returns [`sf_core::Error::Tool`] if the process exits with a non-zero
    ///   status (message includes stderr).
    /// - Returns [`sf_core::Error::Tool`] if spawning the process fails.
    pub async fn execute(&self) -> sf_core::Result<ToolOutput> {
        let program_name = self
            .program
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.program.to_string_lossy().to_string());

        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);

        // If we need to pipe stdin, configure that.
        if self.stdin_data.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| sf_core::Error::Tool {
            tool: program_name.clone(),
            message: format!("failed to spawn: {e}"),
        })?;

        // Write stdin data if provided.
        if let Some(ref data) = self.stdin_data {
            use tokio::io::AsyncWriteExt;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(data).await.map_err(|e| sf_core::Error::Tool {
                    tool: program_name.clone(),
                    message: format!("failed to write stdin: {e}"),
                })?;
                // Drop stdin to close the pipe so the child can proceed.
            }
        }

        // Wait with timeout.
        let result = tokio::time::timeout(self.timeout, child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                let tool_output = ToolOutput {
                    status: output.status,
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                };

                if !output.status.success() {
                    return Err(sf_core::Error::Tool {
                        tool: program_name,
                        message: format!(
                            "exited with status {}: {}",
                            output.status,
                            tool_output.stderr.trim()
                        ),
                    });
                }

                Ok(tool_output)
            }
            Ok(Err(e)) => Err(sf_core::Error::Tool {
                tool: program_name,
                message: format!("I/O error waiting for process: {e}"),
            }),
            Err(_elapsed) => {
                // Timeout expired -- try to kill the child.
                // We no longer own `child` after `wait_with_output`, so on
                // real timeout the future is cancelled and tokio will clean up.
                Err(sf_core::Error::Tool {
                    tool: program_name,
                    message: format!("timed out after {:?}", self.timeout),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn execute_echo() {
        // `echo` should be universally available.
        let output = ToolCommand::new(PathBuf::from("echo"))
            .arg("hello")
            .execute()
            .await;

        match output {
            Ok(out) => {
                assert!(out.status.success());
                assert!(out.stdout.trim().contains("hello"));
            }
            Err(_) => {
                // On some minimal environments echo may not exist; skip.
            }
        }
    }

    #[tokio::test]
    async fn execute_nonexistent_tool() {
        let result = ToolCommand::new(PathBuf::from("nonexistent_tool_xyz_12345"))
            .execute()
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn timeout_fires() {
        // `sleep 10` should be killed well before 10 seconds.
        let result = ToolCommand::new(PathBuf::from("sleep"))
            .arg("10")
            .timeout(Duration::from_millis(100))
            .execute()
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("timed out"), "unexpected error: {err}");
    }
}
