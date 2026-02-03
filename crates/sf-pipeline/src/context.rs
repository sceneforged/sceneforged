//! Execution context shared by all actions in a pipeline run.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

/// Sender for reporting progress from within actions.
///
/// Wraps a callback that receives a progress percentage (0.0 -- 100.0) and a
/// human-readable step description.
pub struct ProgressSender {
    callback: Box<dyn Fn(f32, &str) + Send + Sync>,
}

impl ProgressSender {
    /// Create a new sender from the given callback.
    pub fn new(callback: impl Fn(f32, &str) + Send + Sync + 'static) -> Self {
        Self {
            callback: Box::new(callback),
        }
    }

    /// Create a no-op sender that discards all progress reports.
    pub fn noop() -> Self {
        Self {
            callback: Box::new(|_, _| {}),
        }
    }

    /// Report progress.
    pub fn send(&self, progress: f32, step: &str) {
        (self.callback)(progress, step);
    }
}

impl std::fmt::Debug for ProgressSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressSender").finish_non_exhaustive()
    }
}

/// Context passed to every action during validation and execution.
pub struct ActionContext {
    /// Workspace managing temporary and output paths.
    pub workspace: Arc<sf_av::Workspace>,
    /// Probed media information for the input file.
    pub media_info: Arc<sf_probe::MediaInfo>,
    /// Tool registry for looking up external tool paths.
    pub tools: Arc<sf_av::ToolRegistry>,
    /// When `true`, actions should log what they would do but skip actual work.
    pub dry_run: bool,
    /// Token checked between stages; when cancelled the executor aborts early.
    pub cancellation: CancellationToken,
    /// Channel for reporting progress to the caller.
    pub progress: Arc<ProgressSender>,
}

impl ActionContext {
    /// Create a new context with the minimum required fields.
    pub fn new(
        workspace: Arc<sf_av::Workspace>,
        media_info: Arc<sf_probe::MediaInfo>,
        tools: Arc<sf_av::ToolRegistry>,
    ) -> Self {
        Self {
            workspace,
            media_info,
            tools,
            dry_run: false,
            cancellation: CancellationToken::new(),
            progress: Arc::new(ProgressSender::noop()),
        }
    }

    /// Builder: set dry-run mode.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Builder: attach a cancellation token.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = token;
        self
    }

    /// Builder: attach a progress sender.
    pub fn with_progress(mut self, progress: ProgressSender) -> Self {
        self.progress = Arc::new(progress);
        self
    }
}
