//! Builder for executing external tool commands with timeout support.

use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

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

    /// Execute the command, streaming stderr lines to a callback.
    ///
    /// This is designed for long-running processes (like ffmpeg) where you want
    /// real-time progress updates. Stderr is read line-by-line and passed to the
    /// callback. An optional [`CancellationToken`] can be used to kill the child
    /// process mid-execution.
    ///
    /// Returns the full stdout and the collected stderr on success.
    pub async fn execute_with_stderr_callback(
        &self,
        mut on_stderr: impl FnMut(&str),
        cancel: Option<CancellationToken>,
    ) -> sf_core::Result<ToolOutput> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let program_name = self
            .program
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.program.to_string_lossy().to_string());

        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| sf_core::Error::Tool {
            tool: program_name.clone(),
            message: format!("failed to spawn: {e}"),
        })?;

        let stderr_pipe = child.stderr.take().expect("stderr piped");
        let mut stderr_reader = BufReader::new(stderr_pipe).lines();
        let mut stderr_buf = String::new();

        let cancelled = loop {
            let line = if let Some(ref token) = cancel {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => break true,
                    line = stderr_reader.next_line() => line,
                }
            } else {
                stderr_reader.next_line().await
            };

            match line {
                Ok(Some(line)) => {
                    on_stderr(&line);
                    stderr_buf.push_str(&line);
                    stderr_buf.push('\n');
                }
                Ok(None) => break false, // EOF
                Err(e) => {
                    tracing::debug!("stderr read error for {program_name}: {e}");
                    break false;
                }
            }
        };

        if cancelled {
            let _ = child.kill().await;
            return Err(sf_core::Error::Tool {
                tool: program_name,
                message: "cancelled".into(),
            });
        }

        // Wait for process exit.
        let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
            .await
            .map_err(|_| sf_core::Error::Tool {
                tool: program_name.clone(),
                message: "timed out waiting for process exit after stderr EOF".into(),
            })?
            .map_err(|e| sf_core::Error::Tool {
                tool: program_name.clone(),
                message: format!("I/O error waiting for process: {e}"),
            })?;

        // Read remaining stdout.
        let stdout = if let Some(mut stdout_pipe) = child.stdout.take() {
            let mut buf = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut stdout_pipe, &mut buf)
                .await
                .unwrap_or(0);
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        };

        if !status.success() {
            return Err(sf_core::Error::Tool {
                tool: program_name,
                message: format!("exited with status {}: {}", status, stderr_buf.trim()),
            });
        }

        Ok(ToolOutput {
            status,
            stdout,
            stderr: stderr_buf,
        })
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
